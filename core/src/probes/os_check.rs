//! Collects OS version, EOL status, firewall state, and backup agent presence.
use crate::os_abstraction::os_api;
use crate::types::{FindingPayload, ProbeKind, RawFinding};
use anyhow::Result;
use chrono::Utc;

/// Entry point — runs all OS-level checks on the local machine only.
pub fn run() -> Result<Vec<RawFinding>> {
    let api = os_api();
    let mut info = api.local_os_info()?;
    info.backup_agent_running = api.backup_agent_running().ok();

    Ok(vec![RawFinding {
        probe:     ProbeKind::OsCheck,
        timestamp: Utc::now(),
        payload:   FindingPayload::OsInfo(info),
    }])
}