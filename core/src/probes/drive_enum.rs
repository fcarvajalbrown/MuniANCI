//! Enumerates all storage: local disks, network shares, cloud-sync folders.
use crate::os_abstraction::os_api;
use crate::types::{Drive, DriveKind, FindingPayload, ProbeKind, RawFinding, Scope};
use anyhow::Result;
use chrono::Utc;
use std::net::IpAddr;

/// Entry point — discovers drives on local machine and optionally LAN hosts.
pub fn run(scope: Scope, lan_hosts: &[IpAddr]) -> Result<Vec<RawFinding>> {
    let api = os_api();
    let mut findings = Vec::new();

    // Local drives first.
    for drive in api.local_drives()? {
        findings.push(to_finding(drive));
    }

    // Cloud-sync processes count as a DriveKind::CloudSync finding.
    for name in api.cloud_sync_processes()? {
        findings.push(cloud_sync_finding(name));
    }

    // SMB shares on each discovered LAN host (skipped in Local scope).
    if scope == Scope::Lan {
        for &host in lan_hosts {
            for share in api.smb_shares(host)? {
                findings.push(to_finding(share));
            }
        }
    }

    Ok(findings)
}

/// Wraps a Drive in a RawFinding.
fn to_finding(drive: Drive) -> RawFinding {
    RawFinding {
        probe:     ProbeKind::DriveEnum,
        timestamp: Utc::now(),
        payload:   FindingPayload::Drive(drive),
    }
}

/// Creates a synthetic Drive finding for a detected cloud-sync process.
fn cloud_sync_finding(process_name: String) -> RawFinding {
    RawFinding {
        probe:     ProbeKind::DriveEnum,
        timestamp: Utc::now(),
        payload:   FindingPayload::Drive(Drive {
            path:        format!("process:{process_name}"),
            kind:        DriveKind::CloudSync,
            total_bytes: None,
            free_bytes:  None,
            encrypted:   None,
            host_ip:     None,
        }),
    }
}