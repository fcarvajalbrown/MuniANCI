//! Discovers live hosts on the local subnet via ARP sweep + ICMP ping.
use crate::types::{FindingPayload, Host, ProbeKind, RawFinding};
use anyhow::Result;
use chrono::Utc;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

/// Entry point — call this from the rayon thread pool.
pub fn run(scope: crate::types::Scope) -> Result<Vec<RawFinding>> {
    let mut findings = Vec::new();

    // Always include the local machine.
    findings.push(local_host_finding()?);

    if scope == crate::types::Scope::Lan {
        let subnet = local_subnet()?;
        for ip in subnet {
            if is_alive(ip) {
                findings.push(make_finding(ip, false));
            }
        }
    }
    Ok(findings)
}

/// Builds a RawFinding for the local machine using os_api for hostname.
fn local_host_finding() -> Result<RawFinding> {
    let ip = local_ip()?;
    let hostname = dns_lookup::lookup_addr(&ip).ok();
    Ok(RawFinding {
        probe:     ProbeKind::HostDiscovery,
        timestamp: Utc::now(),
        payload:   FindingPayload::Host(Host {
            ip,
            hostname,
            mac:       None, // mac needs raw sockets; skipped for non-root
            os_banner: None, // filled later by service_probe
            is_local:  true,
        }),
    })
}

/// Sends an ICMP echo request; returns true if we get any reply within 300ms.
fn is_alive(ip: Ipv4Addr) -> bool {
    // Raw ICMP needs elevated privs — fall back to TCP port 7 (echo) or 80.
    let addr = SocketAddr::new(IpAddr::V4(ip), 80);
    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
        || tcp_probe(ip, 445)
        || tcp_probe(ip, 22)
}

/// Quick TCP connect probe to check if a port is open.
fn tcp_probe(ip: Ipv4Addr, port: u16) -> bool {
    let addr = SocketAddr::new(IpAddr::V4(ip), port);
    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

/// Returns all host IPs in the /24 subnet of the local machine.
fn local_subnet() -> Result<Vec<Ipv4Addr>> {
    let ip = match local_ip()? {
        IpAddr::V4(v4) => v4,
        _ => anyhow::bail!("IPv6-only host not supported for LAN sweep"),
    };
    let octets = ip.octets();
    // Sweep .1–.254, skip the local machine itself.
    Ok((1u8..=254)
        .filter(|&last| last != octets[3])
        .map(|last| Ipv4Addr::new(octets[0], octets[1], octets[2], last))
        .collect())
}

/// Builds a RawFinding for a remote host with reverse-DNS lookup.
fn make_finding(ip: Ipv4Addr, is_local: bool) -> RawFinding {
    let ip = IpAddr::V4(ip);
    let hostname = dns_lookup::lookup_addr(&ip).ok();
    RawFinding {
        probe:     ProbeKind::HostDiscovery,
        timestamp: Utc::now(),
        payload:   FindingPayload::Host(Host {
            ip,
            hostname,
            mac:       None,
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