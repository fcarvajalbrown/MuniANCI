//! Collects installed software from every discovered host via os_api.
use crate::os_abstraction::os_api;
use crate::types::{FindingPayload, ProbeKind, RawFinding};
use anyhow::Result;
use chrono::Utc;
use std::net::IpAddr;

/// Entry point — queries installed packages on all provided hosts.
pub fn run(hosts: &[IpAddr]) -> Result<Vec<RawFinding>> {
    let api = os_api();
    let mut findings = Vec::new();

    for &host in hosts {
        // Non-local hosts: sw_inventory is local-only for v0.1 — skip silently.
        let entries = api.installed_software(host)?;
        for sw in entries {
            findings.push(RawFinding {
                probe:     ProbeKind::SwInventory,
                timestamp: Utc::now(),
                payload:   FindingPayload::Software(sw),
            });
        }
    }
    Ok(findings)
}