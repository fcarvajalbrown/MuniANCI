//! Discovers live hosts on the local subnet.
//!
//! Este modulo orquesta: arma los `RawFinding`, hace el reverse-DNS y decide
//! que direcciones barrer. Como se sondea cada una vive en
//! [`crate::probes::net_discovery`].
use crate::probes::net_discovery::{self, Ajustes, HostEvidence, MethodState, Pacer};
use crate::types::{FindingPayload, Host, ProbeKind, RawFinding};
use anyhow::Result;
use chrono::Utc;
use std::net::{IpAddr, Ipv4Addr};

/// Entry point — call this from the rayon thread pool.
pub fn run(scope: crate::types::Scope) -> Result<Vec<RawFinding>> {
    run_con(scope, &Ajustes::default())
}

/// Same as [`run`], with the network settings TI can adjust.
pub fn run_con(scope: crate::types::Scope, ajustes: &Ajustes) -> Result<Vec<RawFinding>> {
    let mut findings = Vec::new();

    // Always include the local machine.
    findings.push(local_host_finding()?);

    if scope == crate::types::Scope::Lan {
        let subnet = local_subnet()?;
        let state = MethodState::default();
        let pacer = Pacer::new(ajustes.arp_pps);

        use rayon::prelude::*;
        let mut vivos: Vec<(Ipv4Addr, HostEvidence)> = subnet
            .into_par_iter()
            .filter_map(|ip| net_discovery::probe_host(ip, ajustes, &state, &pacer).map(|e| (ip, e)))
            .collect();

        let borradas = net_discovery::descartar_macs_de_next_hop(&mut vivos);
        if borradas > 0 {
            eprintln!(
                "aviso: {borradas} direcciones compartian la misma MAC y se descarto en todas. \
                 Suele significar que el segmento no es un /24 y ARP devolvio la MAC del router."
            );
        }

        // Orden estable para que el informe no cambie entre corridas iguales.
        vivos.sort_by_key(|(ip, _)| *ip);
        for (ip, ev) in vivos {
            findings.push(make_finding(ip, false, Some(&ev)));
        }
    }
    Ok(findings)
}

/// Builds a RawFinding for the local machine using os_api for hostname.
fn local_host_finding() -> Result<RawFinding> {
    let ip = local_ip()?;
    let hostname = dns_lookup::lookup_addr(&ip).ok();
    Ok(RawFinding {
        probe: ProbeKind::HostDiscovery,
        timestamp: Utc::now(),
        payload: FindingPayload::Host(Host {
            ip,
            hostname,
            // SendARP contra la propia IP devuelve longitud 0. Sacar la MAC
            // local necesita GetAdaptersAddresses, que es un item aparte.
            mac: None,
            os_banner: None, // filled later by service_probe
            is_local: true,
        }),
    })
}

/// Returns all host IPs in the /24 subnet of the local machine.
fn local_subnet() -> Result<Vec<Ipv4Addr>> {
    let ip = match local_ip()? {
        IpAddr::V4(v4) => v4,
        _ => anyhow::bail!("IPv6-only host not supported for LAN sweep"),
    };
    Ok(net_discovery::subnet_de(ip))
}

/// Builds a RawFinding for a remote host with reverse-DNS lookup.
fn make_finding(ip: Ipv4Addr, is_local: bool, ev: Option<&HostEvidence>) -> RawFinding {
    let ip = IpAddr::V4(ip);
    let hostname = dns_lookup::lookup_addr(&ip).ok();
    RawFinding {
        probe: ProbeKind::HostDiscovery,
        timestamp: Utc::now(),
        payload: FindingPayload::Host(Host {
            ip,
            hostname,
            mac: ev.and_then(|e| e.mac.clone()),
            os_banner: None,
            is_local,
        }),
    }
}

/// Returns the primary outbound IP of this machine.
fn local_ip() -> Result<IpAddr> {
    use std::net::UdpSocket;
    let s = UdpSocket::bind("0.0.0.0:0")?;
    s.connect("8.8.8.8:80")?;
    Ok(s.local_addr()?.ip())
}
