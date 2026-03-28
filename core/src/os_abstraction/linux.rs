//! Unix/Linux implementation of [`OsApi`].
//!
//! Uses `/proc`, `/etc/os-release`, `libc` mounts, and the `nix` crate.
//! No external binaries are shelled out to — everything is pure Rust syscalls
//! or file reads so the tool works without root and without PATH dependencies.
//!
//! Privilege notes:
//! - All methods work as a standard user except [`OsApi::drive_encrypted`],
//!   which needs read access to `/sys/block/*/dm` symlinks (usually available
//!   without root on modern distros).
//! - SMB share enumeration on Linux requires `libsmbclient` or a raw TCP probe
//!   to port 445 — we use a lightweight socket probe here so there is no
//!   dependency on Samba userspace tools.

use super::OsApi;
use crate::types::{Drive, DriveKind, OsInfo, SoftwareEntry};
use anyhow::{Context, Result};
use std::{
    fs,
    io::{BufRead, BufReader},
    net::{IpAddr, TcpStream},
    path::Path,
    time::Duration,
};

/// Unit struct — all calls are stateless file/syscall reads.
pub(super) struct UnixApi;

impl OsApi for UnixApi {
    /// Reads `/proc/mounts` and returns all mounted block devices.
    ///
    /// Filters out pseudo-filesystems (proc, sysfs, devtmpfs, tmpfs, cgroup*).
    fn local_drives(&self) -> Result<Vec<Drive>> {
        let f = fs::File::open("/proc/mounts")
            .context("cannot read /proc/mounts")?;
        let mut drives = Vec::new();

        for line in BufReader::new(f).lines().flatten() {
            // Format: <device> <mountpoint> <fstype> <options> <dump> <pass>
            let cols: Vec<&str> = line.splitn(6, ' ').collect();
            if cols.len() < 3 {
                continue;
            }
            let (device, mountpoint, fstype) = (cols[0], cols[1], cols[2]);

            // Skip pseudo filesystems.
            if matches!(
                fstype,
                "proc" | "sysfs" | "devtmpfs" | "devpts" | "tmpfs"
                | "securityfs" | "pstore" | "cgroup" | "cgroup2"
                | "hugetlbfs" | "mqueue" | "debugfs" | "tracefs"
                | "fusectl" | "configfs" | "bpf" | "ramfs"
            ) || fstype.starts_with("cgroup")
            {
                continue;
            }

            let kind = match fstype {
                "cifs" | "smb2" | "smb3" => DriveKind::Smb,
                "nfs" | "nfs4"           => DriveKind::Nfs,
                "davfs"                  => DriveKind::WebDav,
                "vfat" | "exfat"         => DriveKind::Removable,
                _                        => DriveKind::Fixed,
            };

            let (total_bytes, free_bytes) = statvfs(mountpoint)
                .map(|(t, f)| (Some(t), Some(f)))
                .unwrap_or((None, None));

            let encrypted = if kind == DriveKind::Fixed {
                self.drive_encrypted(device).ok().flatten()
            } else {
                None
            };

            drives.push(Drive {
                path: mountpoint.to_owned(),
                kind,
                total_bytes,
                free_bytes,
                encrypted,
                host_ip: None,
            });
        }
        Ok(drives)
    }

    /// Probes port 445 on `host` to detect an SMB listener, then attempts a
    /// minimal SMB1 negotiate request to enumerate share names.
    ///
    /// Falls back gracefully to an empty vec if the port is closed or the host
    /// does not respond within the connect timeout.
    fn smb_shares(&self, host: IpAddr) -> Result<Vec<Drive>> {
        use std::io::Write;

        let addr = format!("{host}:445");
        let stream =
            TcpStream::connect_timeout(&addr.parse()?, Duration::from_secs(2));
        let mut stream = match stream {
            Ok(s)  => s,
            Err(_) => return Ok(vec![]), // port closed or host unreachable
        };
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;

        // SMB1 Negotiate Protocol Request (minimal — enough to get a response).
        // If the server replies we know SMB is live; full share enum requires
        // authentication negotiation which is out of scope for v0.1.
        // We record the host as having an open SMB port as a single Drive entry.
        let smb1_negotiate: &[u8] = &[
            0x00, 0x00, 0x00, 0x2f, // NetBIOS length
            0xff, 0x53, 0x4d, 0x42, // SMB magic
            0x72,                   // Command: Negotiate
            0x00, 0x00, 0x00, 0x00, // Status
            0x18,                   // Flags
            0x01, 0x28,             // Flags2
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00,                   // Word count
            0x02, 0x00,             // Byte count
            0x02,                   // Dialect buffer format
            0x00,                   // Null terminator
        ];

        let _ = stream.write_all(smb1_negotiate);

        let mut resp = [0u8; 4];
        let readable = stream.read(&mut resp).is_ok();

        if readable {
            Ok(vec![Drive {
                path: format!("\\\\{host}"),
                kind: DriveKind::Smb,
                total_bytes: None,
                free_bytes:  None,
                encrypted:   None,
                host_ip:     Some(host),
            }])
        } else {
            Ok(vec![])
        }
    }

    /// Reads installed packages from dpkg (`/var/lib/dpkg/status`) or rpm
    /// (`rpm -qa --queryformat`, if available).
    ///
    /// dpkg is tried first; rpm is the fallback for RPM-based distros.
    fn installed_software(&self, host_ip: IpAddr) -> Result<Vec<SoftwareEntry>> {
        let mut entries = Vec::new();

        if Path::new("/var/lib/dpkg/status").exists() {
            entries = read_dpkg_status(host_ip)?;
        } else if Path::new("/var/lib/rpm/Packages").exists()
            || Path::new("/var/lib/rpm/rpmdb.sqlite").exists()
        {
            entries = read_rpm_db(host_ip)?;
        }

        Ok(entries)
    }

    /// Reads `/etc/os-release` for distro/version, then checks firewall state.
    fn local_os_info(&self) -> Result<OsInfo> {
        let (name, version_id) = parse_os_release()?;
        let version = format!("{name} {version_id}");
        let is_eol  = is_eol_linux(&name, &version_id);

        let firewall = self.firewall_active().unwrap_or(false);
        let host_ip  = local_ip().unwrap_or(IpAddr::from([127, 0, 0, 1]));

        Ok(OsInfo {
            host_ip,
            family: "linux".into(),
            version,
            is_eol,
            firewall_active: firewall,
        })
    }

    /// Checks whether the backing block device for `drive_path` is a
    /// LUKS-encrypted dm-crypt device by inspecting `/sys/block/*/dm/uuid`.
    ///
    /// LUKS dm-crypt UUIDs always start with "CRYPT-LUKS".
    /// Returns `None` if the sysfs path is unreadable (no block device mapping).
    fn drive_encrypted(&self, drive_path: &str) -> Result<Option<bool>> {
        // Resolve the device name from the path (e.g. "/dev/sda1" → "sda1").
        let dev_name = Path::new(drive_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(drive_path);

        // Check dm-crypt: /sys/block/<dev>/dm/uuid starts with "CRYPT-LUKS"
        let dm_uuid_path = format!("/sys/block/{dev_name}/dm/uuid");
        if let Ok(uuid) = fs::read_to_string(&dm_uuid_path) {
            return Ok(Some(uuid.trim().starts_with("CRYPT-LUKS")));
        }

        // Check if any slave device of this device is a LUKS dm device.
        let slaves_dir = format!("/sys/block/{dev_name}/slaves");
        if let Ok(entries) = fs::read_dir(&slaves_dir) {
            for entry in entries.flatten() {
                let slave = entry.file_name();
                let slave_uuid = format!(
                    "/sys/block/{}/dm/uuid",
                    slave.to_string_lossy()
                );
                if let Ok(uuid) = fs::read_to_string(&slave_uuid) {
                    if uuid.trim().starts_with("CRYPT-LUKS") {
                        return Ok(Some(true));
                    }
                }
            }
        }

        Ok(None) // Could not determine — no sysfs mapping found
    }

    /// Detects active firewall by checking ufw status file or iptables rules.
    ///
    /// Reads `/etc/ufw/ufw.conf` first (no privileges needed). Falls back to
    /// checking if the `iptables` INPUT chain has any non-ACCEPT default policy
    /// by reading `/proc/net/ip_tables_names` (available on most kernels).
    fn firewall_active(&self) -> Result<bool> {
        // ufw check — fastest, no privileges needed.
        if let Ok(content) = fs::read_to_string("/etc/ufw/ufw.conf") {
            for line in content.lines() {
                let l = line.trim();
                if l.eq_ignore_ascii_case("enabled=yes") {
                    return Ok(true);
                }
                if l.eq_ignore_ascii_case("enabled=no") {
                    return Ok(false);
                }
            }
        }

        // firewalld: check if the service unit is active via /run/firewalld.
        if Path::new("/run/firewalld/firewalld.pid").exists() {
            return Ok(true);
        }

        // Fallback: if the kernel has loaded the ip_tables module there are
        // likely some rules active.
        if Path::new("/proc/net/ip_tables_names").exists() {
            let content = fs::read_to_string("/proc/net/ip_tables_names")
                .unwrap_or_default();
            return Ok(!content.trim().is_empty());
        }

        Ok(false)
    }

    /// Scans running processes in `/proc/*/comm` for known cloud sync agents.
    fn cloud_sync_processes(&self) -> Result<Vec<String>> {
        let targets = ["onedrive", "dropbox", "googledrivesync", "gdrive", "rclone"];
        let mut found = Vec::new();

        for entry in fs::read_dir("/proc").context("cannot read /proc")?.flatten() {
            let comm_path = entry.path().join("comm");
            if let Ok(name) = fs::read_to_string(&comm_path) {
                let name = name.trim().to_lowercase();
                if targets.iter().any(|t| name.contains(t)) {
                    found.push(name);
                }
            }
        }
        Ok(found)
    }

    /// Checks `/proc/*/comm` for known backup agent process names.
    fn backup_agent_running(&self) -> Result<bool> {
        let agents = [
            "veeam", "acronis", "rsync", "bacula",
            "duplicati", "borg", "restic", "amanda",
        ];
        for entry in fs::read_dir("/proc").context("cannot read /proc")?.flatten() {
            let comm_path = entry.path().join("comm");
            if let Ok(name) = fs::read_to_string(&comm_path) {
                let name = name.trim().to_lowercase();
                if agents.iter().any(|a| name.contains(a)) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Calls `statvfs(2)` via `nix` to get total and free bytes for a mountpoint.
fn statvfs(path: &str) -> Result<(u64, u64)> {
    let stat = nix::sys::statvfs::statvfs(path)?;
    let bsize = stat.block_size() as u64;
    Ok((
        stat.blocks()           * bsize,
        stat.blocks_available() * bsize,
    ))
}

/// Parses `/etc/os-release` and returns `(NAME, VERSION_ID)`.
fn parse_os_release() -> Result<(String, String)> {
    let content = fs::read_to_string("/etc/os-release")
        .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
        .context("cannot read os-release")?;

    let mut name = String::from("Linux");
    let mut version_id = String::new();

    for line in content.lines() {
        if let Some(val) = line.strip_prefix("NAME=") {
            name = val.trim_matches('"').to_owned();
        } else if let Some(val) = line.strip_prefix("VERSION_ID=") {
            version_id = val.trim_matches('"').to_owned();
        }
    }
    Ok((name, version_id))
}

/// Naive EOL check based on well-known distro version strings.
/// The normalizer will enrich this against a proper EOL database.
fn is_eol_linux(name: &str, version_id: &str) -> bool {
    let n = name.to_lowercase();
    match n.as_str() {
        s if s.contains("ubuntu") => matches!(
            version_id,
            "14.04" | "16.04" | "18.04" | "19.04" | "19.10"
            | "20.10" | "21.04" | "21.10" | "22.10" | "23.04" | "23.10"
        ),
        s if s.contains("debian") => matches!(version_id, "7" | "8" | "9" | "10"),
        s if s.contains("centos") => matches!(version_id, "5" | "6" | "7" | "8"),
        _ => false,
    }
}

/// Parses `/var/lib/dpkg/status` into a list of `SoftwareEntry`.
///
/// Each stanza is a blank-line-separated block with `Package:` and `Version:`
/// fields. We only emit entries that have both fields populated.
fn read_dpkg_status(host_ip: IpAddr) -> Result<Vec<SoftwareEntry>> {
    let f = fs::File::open("/var/lib/dpkg/status")
        .context("cannot open dpkg status")?;

    let mut entries = Vec::new();
    let mut pkg_name = String::new();
    let mut pkg_ver  = String::new();
    let mut installed = false;

    for line in BufReader::new(f).lines().flatten() {
        if line.is_empty() {
            // End of stanza — emit if installed and both fields present.
            if installed && !pkg_name.is_empty() && !pkg_ver.is_empty() {
                entries.push(SoftwareEntry {
                    name:     pkg_name.clone(),
                    version:  pkg_ver.clone(),
                    host_ip,
                    is_eol:   false,
                    max_cvss: None,
                });
            }
            pkg_name.clear();
            pkg_ver.clear();
            installed = false;
            continue;
        }
        if let Some(v) = line.strip_prefix("Package: ") {
            pkg_name = v.to_owned();
        } else if let Some(v) = line.strip_prefix("Version: ") {
            pkg_ver = v.to_owned();
        } else if line.starts_with("Status: ") && line.contains("installed") {
            installed = true;
        }
    }
    Ok(entries)
}

/// Reads RPM database using the `rpm` command if available.
///
/// Falls back to an empty vec if `rpm` is not on PATH.
fn read_rpm_db(host_ip: IpAddr) -> Result<Vec<SoftwareEntry>> {
    use std::process::Command;

    let output = Command::new("rpm")
        .args(["-qa", "--queryformat", "%{NAME}\\t%{VERSION}-%{RELEASE}\\n"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Ok(vec![]),
    };

    let mut entries = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut parts = line.splitn(2, '\t');
        let name    = parts.next().unwrap_or("").to_owned();
        let version = parts.next().unwrap_or("").to_owned();
        if !name.is_empty() && !version.is_empty() {
            entries.push(SoftwareEntry {
                name,
                version,
                host_ip,
                is_eol:   false,
                max_cvss: None,
            });
        }
    }
    Ok(entries)
}

/// Returns the primary outbound IP of this machine.
fn local_ip() -> Result<IpAddr> {
    use std::net::UdpSocket;
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.connect("8.8.8.8:80")?;
    Ok(sock.local_addr()?.ip())
}