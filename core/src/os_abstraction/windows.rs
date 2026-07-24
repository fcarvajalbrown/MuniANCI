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
use anyhow::Result;
use std::net::IpAddr;

/// Unit struct — no state needed; all calls are stateless Win32/WMI queries.
pub(super) struct WindowsApi;

impl OsApi for WindowsApi {
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

        for drive_str in raw.split(|&c| c == 0).filter(|s| !s.is_empty()) {
            let path = String::from_utf16_lossy(drive_str);
            let path_w: Vec<u16> = drive_str.iter().copied().chain([0]).collect();

            let drive_type = unsafe { GetDriveTypeW(PWSTR(path_w.as_ptr() as *mut u16)) };
            let kind = match drive_type {
                3 => DriveKind::Fixed,
                2 => DriveKind::Removable,
                _ => DriveKind::Unknown,
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

        unsafe { let _ = WNetCloseEnum(h_enum); };
        Ok(shares)
    }

    fn installed_software(&self, host_ip: IpAddr) -> Result<Vec<SoftwareEntry>> {
        use windows::Win32::System::Registry::{
            RegEnumKeyExW, RegOpenKeyExW,
            HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, KEY_WOW64_32KEY,
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
                    Some(0),
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
                        Some(windows::core::PWSTR(name_buf.as_mut_ptr())),
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
                        Some(0), flags, &mut hsubkey,
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
                        is_eol:   false,
                        max_cvss: None,
                        cves:     vec![],
                    });
                }
            }
        }

        Ok(entries)
    }

    fn local_os_info(&self) -> Result<OsInfo> {
        use windows::Win32::System::SystemInformation::OSVERSIONINFOW;

        #[link(name = "ntdll")]
        unsafe extern "system" {
            unsafe fn RtlGetVersion(lpVersionInformation: *mut OSVERSIONINFOW) -> i32;
        }

        let mut info = OSVERSIONINFOW {
            dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        let status = unsafe { RtlGetVersion(&mut info) };
        if status != 0 {
            anyhow::bail!("RtlGetVersion failed with status: {}", status);
        }

        let version = format!(
            "Windows {}.{} build {}",
            info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber
        );

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
            backup_agent_running: None,
            cves: vec![],
        })
    }

    fn drive_encrypted(&self, drive_path: &str) -> Result<Option<bool>> {
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
            Err(_)     => Ok(None),
        }
    }

    fn firewall_active(&self) -> Result<bool> {
        use windows::Win32::System::Registry::{
            RegOpenKeyExW, RegQueryValueExW,
            HKEY_LOCAL_MACHINE, KEY_READ, REG_DWORD,
        };

        let profiles = [
            "SYSTEM\\CurrentControlSet\\Services\\SharedAccess\\Parameters\\FirewallPolicy\\DomainProfile",
            "SYSTEM\\CurrentControlSet\\Services\\SharedAccess\\Parameters\\FirewallPolicy\\StandardProfile",
            "SYSTEM\\CurrentControlSet\\Services\\SharedAccess\\Parameters\\FirewallPolicy\\PublicProfile",
        ];

        for profile in profiles {
            let path_w = profile.encode_utf16().chain(std::iter::once(0_u16)).collect::<Vec<u16>>();
            let mut hkey = windows::Win32::System::Registry::HKEY::default();

            if unsafe {
                RegOpenKeyExW(
                    HKEY_LOCAL_MACHINE,
                    windows::core::PCWSTR(path_w.as_ptr()),
                    Some(0),
                    KEY_READ,
                    &mut hkey,
                )
            }.is_err() {
                continue;
            }

            let value_w = "EnableFirewall\0".encode_utf16().collect::<Vec<u16>>();
            let mut data = 0u32;
            let mut data_size = std::mem::size_of::<u32>() as u32;
            let mut kind = windows::Win32::System::Registry::REG_VALUE_TYPE::default();

            let rc = unsafe {
                RegQueryValueExW(
                    hkey,
                    windows::core::PCWSTR(value_w.as_ptr()),
                    None,
                    Some(&mut kind),
                    Some(&mut data as *mut u32 as *mut u8),
                    Some(&mut data_size),
                )
            };

            if rc.is_ok() && kind == REG_DWORD && data == 1 {
                return Ok(true);
            }
        }
        Ok(false)
    }

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
// WMI COM helpers
// ---------------------------------------------------------------------------

fn wmi_scalar_u32(namespace: &str, query: &str, property: &str) -> Result<u32> {
    let results = wmi_query(namespace, query, &[property])?;
    results
        .into_iter()
        .next()
        .and_then(|mut row| row.remove(property))
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(|| anyhow::anyhow!("property {} not found", property))
}

fn wmi_string_list(namespace: &str, query: &str, property: &str) -> Result<Vec<String>> {
    let results = wmi_query(namespace, query, &[property])?;
    Ok(results
        .into_iter()
        .filter_map(|mut row| row.remove(property))
        .collect())
}

fn wmi_query(
    namespace: &str,
    query: &str,
    properties: &[&str],
) -> Result<Vec<std::collections::HashMap<String, String>>> {
    use windows::core::BSTR;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoSetProxyBlanket, CoUninitialize,
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        RPC_C_AUTHN_LEVEL_PKT_PRIVACY, RPC_C_IMP_LEVEL_IMPERSONATE,
    };
    use windows::Win32::System::Com::EOAC_NONE;
    use windows::Win32::System::Wmi::{
        IWbemLocator, WbemLocator,
        WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_INFINITE,
    };
    use windows::Win32::System::Variant::VARIANT;

    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let com_inited = hr.is_ok();

    let result = (|| -> Result<Vec<std::collections::HashMap<String, String>>> {
        let locator: IWbemLocator = unsafe {
            CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)
        }.map_err(|e| anyhow::anyhow!("CoCreateInstance: {e}"))?;

        let ns_bstr = BSTR::from(namespace);
        let empty = BSTR::default();
        let services = unsafe {
            locator.ConnectServer(&ns_bstr, &empty, &empty, &empty, 0, &empty, None)
        }.map_err(|e| anyhow::anyhow!("ConnectServer {namespace}: {e}"))?;

        unsafe {
            CoSetProxyBlanket(
                &services,
                10, // RPC_C_AUTHN_WINNT
                0,  // RPC_C_AUTHZ_NONE
                None,
                RPC_C_AUTHN_LEVEL_PKT_PRIVACY,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            )
        }.map_err(|e| anyhow::anyhow!("CoSetProxyBlanket: {e}"))?;

        let wql        = BSTR::from("WQL");
        let query_bstr = BSTR::from(query);
        let enumerator = unsafe {
            services.ExecQuery(
                &wql,
                &query_bstr,
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                None,
            )
        }.map_err(|e| anyhow::anyhow!("ExecQuery: {e}"))?;

        let mut rows = Vec::new();
        loop {
            let mut objects = [None; 1];
            let mut returned = 0u32;
            let hr = unsafe {
                enumerator.Next(WBEM_INFINITE, &mut objects, &mut returned)
            };
            if hr.is_err() || returned == 0 {
                break;
            }
            let obj = match &objects[0] {
                Some(o) => o.clone(),
                None    => break,
            };

            let mut row = std::collections::HashMap::new();
            for &prop in properties {
                let prop_bstr = BSTR::from(prop);
                let mut variant = VARIANT::default();
                if unsafe { obj.Get(&prop_bstr, 0, &mut variant, None, None) }.is_ok() {
                    row.insert(prop.to_string(), variant_to_string(&variant));
                }
            }
            rows.push(row);
        }

        Ok(rows)
    })();

    if com_inited {
        unsafe { CoUninitialize() };
    }

    result
}

fn variant_to_string(v: &windows::Win32::System::Variant::VARIANT) -> String {
    use windows::Win32::System::Variant::{
        VT_BOOL, VT_BSTR, VT_I2, VT_I4, VT_I8, VT_NULL, VT_UI2, VT_UI4, VT_UI8,
    };
    unsafe {
        let inner = &v.Anonymous.Anonymous;
        let anon  = &inner.Anonymous;
        match inner.vt {
            VT_BSTR => anon.bstrVal.to_string(),
            VT_I4   => anon.lVal.to_string(),
            VT_UI4  => anon.ulVal.to_string(),
            VT_I2   => anon.iVal.to_string(),
            VT_UI2  => anon.uiVal.to_string(),
            VT_I8   => anon.llVal.to_string(),
            VT_UI8  => anon.ullVal.to_string(),
            VT_BOOL => if anon.boolVal.as_bool() { "1".into() } else { "0".into() },
            VT_NULL => String::new(),
            _       => String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Registry helper
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Network helper
// ---------------------------------------------------------------------------

fn local_ip() -> Result<IpAddr> {
    use std::net::UdpSocket;
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.connect("8.8.8.8:80")?;
    Ok(sock.local_addr()?.ip())
}