//! Converts raw probe output into a deduplicated, typed AssetGraph.
use crate::types::{
    AssetGraph, FindingPayload, Host, RawFinding,
};
use std::collections::HashMap;
use std::net::IpAddr;

/// Consumes all raw findings and produces a single AssetGraph.
pub fn normalize(findings: Vec<RawFinding>) -> AssetGraph {
    let mut graph = AssetGraph::default();

    // Use host IP as dedup key so multiple probes don't create duplicate hosts.
    let mut host_map: HashMap<IpAddr, Host> = HashMap::new();

    for f in findings {
        match f.payload {
            FindingPayload::Host(h) => {
                // Keep the entry with the most data — prefer is_local=true.
                host_map
                    .entry(h.ip)
                    .and_modify(|existing| merge_host(existing, &h))
                    .or_insert(h);
            }
            FindingPayload::Drive(d)    => graph.drives.push(d),
            FindingPayload::Service(s)  => graph.services.push(s),
            FindingPayload::Software(sw) => graph.software.push(sw),
            FindingPayload::OsInfo(o)   => graph.os_info.push(o),
        }
    }

    graph.hosts = host_map.into_values().collect();

    // Stable ordering makes report diffs readable.
    graph.hosts.sort_by_key(|h| h.ip);
    graph.services.sort_by_key(|s| (s.host_ip, s.port));
    graph.drives.sort_by(|a, b| a.path.cmp(&b.path));

    graph
}

/// Merges b into a, preferring non-None fields from b.
fn merge_host(a: &mut Host, b: &Host) {
    if b.hostname.is_some()  { a.hostname  = b.hostname.clone(); }
    if b.mac.is_some()       { a.mac       = b.mac.clone(); }
    if b.os_banner.is_some() { a.os_banner = b.os_banner.clone(); }
    if b.is_local            { a.is_local  = true; }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ProbeKind};
    use chrono::Utc;
    use std::net::IpAddr;

    fn host_finding(ip: IpAddr, is_local: bool) -> RawFinding {
        RawFinding {
            probe: ProbeKind::HostDiscovery,
            timestamp: Utc::now(),
            payload: FindingPayload::Host(Host {
                ip,
                hostname: None,
                mac: None,
                os_banner: None,
                is_local,
            }),
        }
    }

    #[test]
    fn deduplicates_same_ip() {
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let findings = vec![host_finding(ip, false), host_finding(ip, true)];
        let graph = normalize(findings);
        assert_eq!(graph.hosts.len(), 1);
        assert!(graph.hosts[0].is_local);
    }

    #[test]
    fn hosts_sorted_by_ip() {
        let ips: Vec<IpAddr> = vec![
            "192.168.1.3".parse().unwrap(),
            "192.168.1.1".parse().unwrap(),
        ];
        let findings = ips.iter().map(|&ip| host_finding(ip, false)).collect();
        let graph = normalize(findings);
        assert!(graph.hosts[0].ip < graph.hosts[1].ip);
    }
}