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
    // El metodo de descubrimiento no se pisa con el ultimo que llegue: gana el
    // de evidencia mas fuerte (ARP > ICMP > TCP), porque el campo describe la
    // mejor prueba que hay de que el host existe, no la ultima sonda que corrio.
    let fuerza = |m: Option<crate::probes::net_discovery::DiscoveryMethod>| {
        m.map_or(0, |m| m.strength())
    };
    if fuerza(b.discovered_by) > fuerza(a.discovered_by) {
        a.discovered_by = b.discovered_by;
    }
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
                discovered_by: None,
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
    fn el_merge_conserva_la_mac_del_hallazgo_arp() {
        // El descubrimiento remoto trae la MAC y el local no. Si el merge la
        // pisara con None, la MAC que costo un barrido ARP se perderia aca.
        let ip: IpAddr = "192.168.1.7".parse().unwrap();
        let mut con_mac = host_finding(ip, false);
        if let FindingPayload::Host(h) = &mut con_mac.payload {
            h.mac = Some("00:1A:2B:3C:4D:5E".into());
        }
        for orden in [
            vec![con_mac.clone(), host_finding(ip, true)],
            vec![host_finding(ip, true), con_mac.clone()],
        ] {
            let graph = normalize(orden);
            assert_eq!(graph.hosts.len(), 1);
            assert_eq!(graph.hosts[0].mac.as_deref(), Some("00:1A:2B:3C:4D:5E"));
        }
    }

    #[test]
    fn gana_el_metodo_de_evidencia_mas_fuerte_no_el_ultimo() {
        use crate::probes::net_discovery::DiscoveryMethod;
        // El campo describe la mejor prueba de que el host existe, no la ultima
        // sonda que corrio: si se pisara con la ultima, un host confirmado en
        // capa 2 podria terminar figurando como "visto solo por TCP".
        let ip: IpAddr = "192.168.1.9".parse().unwrap();
        let con = |m: DiscoveryMethod| {
            let mut f = host_finding(ip, false);
            if let FindingPayload::Host(h) = &mut f.payload {
                h.discovered_by = Some(m);
            }
            f
        };
        for orden in [
            vec![con(DiscoveryMethod::Arp), con(DiscoveryMethod::Tcp)],
            vec![con(DiscoveryMethod::Tcp), con(DiscoveryMethod::Arp)],
            vec![con(DiscoveryMethod::Icmp), con(DiscoveryMethod::Arp)],
        ] {
            let graph = normalize(orden);
            assert_eq!(graph.hosts[0].discovered_by, Some(DiscoveryMethod::Arp));
        }
    }

    #[test]
    fn un_host_sin_metodo_no_borra_el_que_ya_habia() {
        use crate::probes::net_discovery::DiscoveryMethod;
        let ip: IpAddr = "192.168.1.11".parse().unwrap();
        let mut con_metodo = host_finding(ip, false);
        if let FindingPayload::Host(h) = &mut con_metodo.payload {
            h.discovered_by = Some(DiscoveryMethod::Icmp);
        }
        let graph = normalize(vec![con_metodo, host_finding(ip, true)]);
        assert_eq!(graph.hosts[0].discovered_by, Some(DiscoveryMethod::Icmp));
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