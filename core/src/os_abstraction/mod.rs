//! OS abstraction layer for MuniANCI.
//!
//! Defines a single `OsApi` trait that every probe calls. The two platform
//! implementations (`windows.rs`, `unix.rs`) live behind `#[cfg]` guards so
//! only the correct one compiles. Probes never import platform modules
//! directly — they call `os_api()` and get a boxed trait object back.

#[cfg(windows)]
mod windows;
#[cfg(unix)]
mod unix;

use crate::types::{Drive, OsInfo, SoftwareEntry};
use anyhow::Result;
use std::net::IpAddr;

// ---------------------------------------------------------------------------
// Public factory
// ---------------------------------------------------------------------------

/// Returns the platform implementation of [`OsApi`] for the current OS.
///
/// Probes call this once and keep the result for the duration of the scan.
pub fn os_api() -> Box<dyn OsApi> {
    #[cfg(windows)]
    return Box::new(windows::WindowsApi);

    #[cfg(unix)]
    return Box::new(unix::UnixApi);

    #[cfg(not(any(windows, unix)))]
    compile_error!("MuniANCI requires Windows or a Unix-like OS.");
}

// ---------------------------------------------------------------------------
// Trait definition
// ---------------------------------------------------------------------------

/// Abstracts every OS-specific operation that probes need.
///
/// Each method returns `Result<T>` so callers can handle permission errors
/// gracefully (e.g. running without admin rights on Windows) without crashing
/// the entire scan.
pub trait OsApi: Send + Sync {
    /// Returns all storage volumes visible to the current process.
    ///
    /// Includes fixed disks, removable media, and mounted network shares.
    /// Does not include cloud-sync folders — those are detected separately
    /// via [`running_processes`].
    fn local_drives(&self) -> Result<Vec<Drive>>;

    /// Returns all SMB shares reachable from this machine on the given host.
    ///
    /// Pass the local machine's own IP to enumerate local admin shares
    /// (C$, ADMIN$, IPC$). Pass a remote IP for LAN share enumeration.
    /// Returns an empty vec (not an error) if the host has no open shares.
    fn smb_shares(&self, host: IpAddr) -> Result<Vec<Drive>>;

    /// Returns installed software packages with version strings.
    ///
    /// On Windows: queries Win32_Product via WMI.
    /// On Linux: reads dpkg status file or rpm database.
    fn installed_software(&self, host_ip: IpAddr) -> Result<Vec<SoftwareEntry>>;

    /// Returns basic OS information for the local machine.
    ///
    /// Includes OS family, version string, EOL status, and firewall state.
    fn local_os_info(&self) -> Result<OsInfo>;

    /// Returns true if the given drive path has encryption active.
    ///
    /// On Windows: checks BitLocker status via WMI `Win32_EncryptableVolume`.
    /// On Linux: checks if the backing device is a LUKS container via `dmsetup`.
    /// Returns `None` if the status cannot be determined (e.g. no admin rights).
    fn drive_encrypted(&self, drive_path: &str) -> Result<Option<bool>>;

    /// Returns true if the host firewall is active on the local machine.
    ///
    /// On Windows: queries `NetFwMgr` / `HNetCfg.FwMgr` via WMI.
    /// On Linux: checks ufw status or iptables default policy.
    fn firewall_active(&self) -> Result<bool>;

    /// Returns names of running processes relevant to cloud sync detection.
    ///
    /// Looks for: OneDrive.exe, dropbox, googledrivesync, gdrive, Dropbox.
    /// Used by `drive_enum` to flag cloud-sync activity.
    fn cloud_sync_processes(&self) -> Result<Vec<String>>;

    /// Returns names of known backup agent processes running on this machine.
    ///
    /// Looks for: veeam, acronis, rsync, bacula, wbadmin, duplicati.
    /// Used by the compliance engine to check Art. 8° lit. b) continuity.
    fn backup_agent_running(&self) -> Result<bool>;
}