//! Windows implementation of [`OsApi`].
//!
//! All Windows-specific system calls live here. Uses the `windows` crate for
//! WMI queries and Win32 API calls. Nothing outside `os_abstraction/` imports
//! this module directly — callers use [`super::os_api()`].
//!
//! Privilege notes:
//! - Most WMI queries work as a standard user.
//! - `Win32_EncryptableVolume` (BitLocker) requires local admin.
//! - `Win32_Product` (installed software) works as standard user but is slow
//!   (~30 s on large installs) because it triggers an MSI reconfiguration pass.
//!   We fall back to the registry key `HKLM\SOFTWARE\Microsoft\Windows\
//!   CurrentVersion\Uninstall` which is fast and requires no elevation.

use super::OsApi;
use crate::types::{Drive, DriveKind, OsInfo, SoftwareEntry};
use anyhow::{Context, Result};
use std::net::IpAddr;

/// Unit struct — no state needed; all calls are stateless Win32/WMI queries.
pub(super) struct WindowsApi;

impl OsApi for WindowsApi {
    /// Enumerates fixed and removable drives via `GetLogicalDriveStringsW`
    /// and queries free/total space with `GetDiskFreeSpaceExW`.
    fn local_drives(&self) -> Result<Vec<Drive>> {
        use windows::Win32::Storage::FileSystem::{
            GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDriveStringsW,
        };
        
        use windows::core::PWSTR;

        let mut buf = [0u16; 256];
        let len = unsafe { GetLogicalDriveStringsW(Some(&mut buf)) };
        if len == 0 {
            anyhow::bail!("GetLogicalDriveStringsW failed");
        }

        let mut drives = Vec::new();
        let raw: Vec<u16> = buf[..len as usize].to_vec();

        // Buffer is a sequence of null-terminated strings, double-null terminated.
        for drive_str in raw.split(|&c| c == 0).filter(|s| !s.is_empty()) {
            let path = String::from_utf16_lossy(drive_str);
            let path_w: Vec<u16> = drive_str.iter().copied().chain([0]).collect();

            let drive_type = unsafe { GetDriveTypeW(PWSTR(path_w.as_ptr() as *mut u16)) };
            let kind = match drive_type {
                3 => DriveKind::Fixed,
                2 => DriveKind::Removable,
                _                           => DriveKind::Unknown,
            };

            let (mut total, mut free) = (0u64, 0u64);
            let _ = unsafe {
                GetDiskFreeSpaceExW(
                    PWSTR(path_w.as_ptr() as *mut u16),
                    None,
                    Some(&mut total),
                    Some(&mut free),
                )
            };

            let encrypted = self.drive_encrypted(&path).ok().flatten();

            drives.push(Drive {
                path,
                kind,
                total_bytes: if total > 0 { Some(total) } else { None },
                free_bytes:  if free  > 0 { Some(free)  } else { None },
                encrypted,
                host_ip: None,
            });
        }
        Ok(drives)
    }

    /// Enumerates SMB shares on `host` using `WNetOpenEnumW` / `WNetEnumResourceW`.
    ///
    /// For the local machine this will include admin shares (C$, ADMIN$, IPC$)
    /// if the current user has sufficient privileges.
    fn smb_shares(&self, host: IpAddr) -> Result<Vec<Drive>> {
        use windows::Win32::NetworkManagement::WNet::{
            WNetCloseEnum, WNetEnumResourceW, WNetOpenEnumW,
            RESOURCE_GLOBALNET, RESOURCETYPE_DISK, RESOURCEUSAGE_CONTAINER,
            NETRESOURCEW,
        };
        use windows::core::PWSTR;

        let server = format!("\\\\{host}\0");
        let server_w: Vec<u16> = server.encode_utf16().collect();

        let mut net_res = NETRESOURCEW {
            dwScope:       RESOURCE_GLOBALNET,
            dwType:        RESOURCETYPE_DISK,
            dwDisplayType: 0,
            dwUsage:       RESOURCEUSAGE_CONTAINER.0,
            lpRemoteName:  PWSTR(server_w.as_ptr() as *mut u16),
            ..Default::default()
        };

        let mut h_enum = windows::Win32::Foundation::HANDLE::default();
        let result = unsafe {
            WNetOpenEnumW(
                RESOURCE_GLOBALNET,
                RESOURCETYPE_DISK,
                RESOURCEUSAGE_CONTAINER,
                Some(&mut net_res),
                &mut h_enum,
            )
        };
        if result.is_err() {
            // Host unreachable or no shares — not a hard error.
            return Ok(vec![]);
        }

        let mut shares = Vec::new();
        let mut count = u32::MAX;
        let mut buf = vec![0u8; 16_384];
        let mut buf_size = buf.len() as u32;

        loop {
            let rc = unsafe {
                WNetEnumResourceW(
                    h_enum,
                    &mut count,
                    buf.as_mut_ptr() as *mut _,
                    &mut buf_size,
                )
            };
            if rc.is_err() {
                break;
            }
            // Parse NETRESOURCEW entries from the raw buffer.
            let entry_size = std::mem::size_of::<NETRESOURCEW>();
            for i in 0..(count as usize) {
                let ptr = buf.as_ptr().wrapping_add(i * entry_size) as *const NETRESOURCEW;
                let entry = unsafe { &*ptr };
                if !entry.lpRemoteName.is_null() {
                    let remote = unsafe { entry.lpRemoteName.to_string() }
                        .unwrap_or_default();
                    shares.push(Drive {
                        path: remote,
                        kind: DriveKind::Smb,
                        total_bytes: None,
                        free_bytes:  None,
                        encrypted:   None,
                        host_ip:     Some(host),
                    });
                }
            }
        }

        unsafe { WNetCloseEnum(h_enum) };
        Ok(shares)
    }

    /// Reads installed software from the Windows registry uninstall keys.
    ///
    /// Checks both 64-bit and 32-bit hives:
    /// - `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`
    /// - `HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`
    ///
    /// This is faster and more reliable than `Win32_Product` (no MSI side effects).
    fn installed_software(&self, host_ip: IpAddr) -> Result<Vec<SoftwareEntry>> {
        use windows::Win32::System::Registry::{
            RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
            HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, KEY_WOW64_64KEY, KEY_WOW64_32KEY,
        };

        const UNINSTALL_PATH: &str =
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
        const UNINSTALL_PATH_32: &str =
            "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

        let mut entries = Vec::new();

        for (path, flags) in [
            (UNINSTALL_PATH,    KEY_READ | KEY_WOW64_64KEY),
            (UNINSTALL_PATH_32, KEY_READ | KEY_WOW64_32KEY),
        ] {
            let path_w = path.encode_utf16().chain(std::iter::once(0_u16)).collect::<Vec<u16>>();
            let mut hkey = windows::Win32::System::Registry::HKEY::default();

            let rc = unsafe {
                RegOpenKeyExW(
                    HKEY_LOCAL_MACHINE,
                    windows::core::PCWSTR(path_w.as_ptr()),
                    0,
                    flags,
                    &mut hkey,
                )
            };
            if rc.is_err() {
                continue;
            }

            let mut idx = 0u32;
            loop {
                let mut name_buf = [0u16; 256];
                let mut name_len = name_buf.len() as u32;
                let rc = unsafe {
                    RegEnumKeyExW(
                        hkey,
                        idx,
                        windows::core::PWSTR(name_buf.as_mut_ptr()),
                        &mut name_len,
                        None, None, None, None,
                    )
                };
                if rc.is_err() {
                    break;
                }
                idx += 1;

                let subkey_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                let full_path = format!("{path}\\{subkey_name}");
                let full_w: Vec<u16> = full_path.encode_utf16().chain([0]).collect();
                let mut hsubkey = windows::Win32::System::Registry::HKEY::default();

                if unsafe {
                    RegOpenKeyExW(
                        HKEY_LOCAL_MACHINE,
                        windows::core::PCWSTR(full_w.as_ptr()),
                        0, flags, &mut hsubkey,
                    )
                }.is_err() {
                    continue;
                }

                let name    = reg_read_string(hsubkey, "DisplayName").unwrap_or_default();
                let version = reg_read_string(hsubkey, "DisplayVersion").unwrap_or_default();

                if !name.is_empty() && !version.is_empty() {
                    entries.push(SoftwareEntry {
                        name,
                        version,
                        host_ip,
                        is_eol:   false, // enriched later by normalizer
                        max_cvss: None,  // enriched later by normalizer
                    });
                }
            }
        }

        Ok(entries)
    }

    /// Queries OS version via `RtlGetVersion` (accurate, unlike `GetVersionEx`
    /// which lies to non-manifested processes).
    fn local_os_info(&self) -> Result<OsInfo> {
        use windows::Win32::System::SystemInformation::{
            OSVERSIONINFOW, RtlGetVersion,
        };

        let mut info = OSVERSIONINFOW {
            dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        unsafe { RtlGetVersion(&mut info) }
            .context("RtlGetVersion failed")?;

        let version = format!(
            "Windows {}.{} build {}",
            info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber
        );

        // EOL: Windows 7 (6.1), 8 (6.2), Server 2008 (6.0/6.1).
        let is_eol = info.dwMajorVersion < 10
            || (info.dwMajorVersion == 10 && info.dwBuildNumber < 17763);

        let firewall = self.firewall_active().unwrap_or(false);

        let local_ip = local_ip().unwrap_or(IpAddr::from([127, 0, 0, 1]));

        Ok(OsInfo {
            host_ip: local_ip,
            family: "windows".into(),
            version,
            is_eol,
            firewall_active: firewall,
        })
    }

    /// Checks BitLocker status for `drive_path` via WMI `Win32_EncryptableVolume`.
    ///
    /// Returns `None` if the query fails (likely insufficient privileges).
    /// Requires local admin — standard users cannot query `Win32_EncryptableVolume`.
    fn drive_encrypted(&self, drive_path: &str) -> Result<Option<bool>> {
        // WMI query: SELECT ProtectionStatus FROM Win32_EncryptableVolume
        //            WHERE DriveLetter = '<drive_path stripped of trailing \>'
        // ProtectionStatus: 0 = unprotected, 1 = protected, 2 = unknown.
        //
        // Full WMI COM initialisation is verbose — we use a lightweight helper.
        // Returns None rather than Err on access denied so the scan continues.
        let letter = drive_path.trim_end_matches('\\');
        match wmi_scalar_u32(
            "root\\cimv2\\Security\\MicrosoftVolumeEncryption",
            &format!(
                "SELECT ProtectionStatus FROM Win32_EncryptableVolume \
                 WHERE DriveLetter = '{letter}'"
            ),
            "ProtectionStatus",
        ) {
            Ok(status) => Ok(Some(status == 1)),
            Err(_)     => Ok(None), // no admin rights or BitLocker not provisioned
        }
    }

    /// Queries Windows Firewall state via WMI `Win32_FirewallProduct`
    /// and the legacy `HNetCfg.FwMgr` COM object.
    fn firewall_active(&self) -> Result<bool> {
        match wmi_scalar_u32(
            "root\\SecurityCenter2",
            "SELECT productState FROM AntiVirusProduct",
            "productState",
        ) {
            // productState bitmask: bits 12-15 == 0x1 means enabled.
            Ok(state) => Ok((state >> 12) & 0xF == 1),
            // Fall back to the simpler legacy profile check.
            Err(_) => {
                let enabled = wmi_scalar_u32(
                    "root\\cimv2",
                    "SELECT EnabledState FROM Win32_Service \
                     WHERE Name='MpsSvc'",
                    "EnabledState",
                )
                .map(|s| s == 2) // 2 = running
                .unwrap_or(false);
                Ok(enabled)
            }
        }
    }

    /// Lists running process names and filters for known cloud sync agents.
    fn cloud_sync_processes(&self) -> Result<Vec<String>> {
        let all = wmi_string_list(
            "root\\cimv2",
            "SELECT Name FROM Win32_Process",
            "Name",
        )?;
        let targets = ["onedrive", "dropbox", "googledrivesync", "gdrive", "backup"];
        Ok(all
            .into_iter()
            .filter(|n| {
                let l = n.to_lowercase();
                targets.iter().any(|t| l.contains(t))
            })
            .collect())
    }

    /// Checks for known backup agent processes via Win32_Process.
    fn backup_agent_running(&self) -> Result<bool> {
        let procs = wmi_string_list(
            "root\\cimv2",
            "SELECT Name FROM Win32_Process",
            "Name",
        )?;
        let agents = [
            "veeam", "acronis", "rsync", "bacula",
            "wbadmin", "duplicati", "robocopy",
        ];
        Ok(procs.iter().any(|p| {
            let l = p.to_lowercase();
            agents.iter().any(|a| l.contains(a))
        }))
    }
}

// ---------------------------------------------------------------------------
// Private WMI helpers
// ---------------------------------------------------------------------------

/// Runs a WMI scalar query and returns the named `u32` property from the
/// first result row. Returns `Err` if the namespace is inaccessible or the
/// property is missing.
///
/// This is a minimal synchronous COM/WMI call — no third-party WMI crate
/// required. Callers should treat `Err` as "unknown" rather than fatal.
fn wmi_scalar_u32(namespace: &str, query: &str, property: &str) -> Result<u32> {
    // Full WMI COM setup is ~80 lines; extracted here to keep impl blocks readable.
    // Placeholder: real implementation initialises COM, creates IWbemLocator,
    // connects to namespace, executes query, iterates IEnumWbemClassObject,
    // reads the named property via IWbemClassObject::Get.
    //
    // For now returns Err so callers fall back gracefully during development.
    let _ = (namespace, query, property);
    anyhow::bail!("wmi_scalar_u32: not yet implemented")
}

/// Runs a WMI query and returns all values of the named string property
/// across all result rows.
fn wmi_string_list(namespace: &str, query: &str, property: &str) -> Result<Vec<String>> {
    let _ = (namespace, query, property);
    anyhow::bail!("wmi_string_list: not yet implemented")
}

/// Reads a REG_SZ value from an open registry key.
fn reg_read_string(
    hkey: windows::Win32::System::Registry::HKEY,
    value: &str,
) -> Option<String> {
    use windows::Win32::System::Registry::{RegQueryValueExW, REG_SZ};

    let value_w: Vec<u16> = value.encode_utf16().chain([0]).collect();
    let mut buf = vec![0u16; 512];
    let mut buf_bytes = (buf.len() * 2) as u32;
    let mut kind = windows::Win32::System::Registry::REG_VALUE_TYPE::default();

    let rc = unsafe {
        RegQueryValueExW(
            hkey,
            windows::core::PCWSTR(value_w.as_ptr()),
            None,
            Some(&mut kind),
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut buf_bytes),
        )
    };
    if rc.is_err() || kind != REG_SZ {
        return None;
    }
    let chars = (buf_bytes / 2).saturating_sub(1) as usize;
    Some(String::from_utf16_lossy(&buf[..chars]))
}

/// Returns the primary local IP address of this machine.
fn local_ip() -> Result<IpAddr> {
    use std::net::UdpSocket;
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.connect("8.8.8.8:80")?;
    Ok(sock.local_addr()?.ip())
}