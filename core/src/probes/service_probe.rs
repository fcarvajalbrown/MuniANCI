//! Probes open ports on discovered hosts: banner grab, TLS version, anon access.
use crate::types::{FindingPayload, ProbeKind, RawFinding, Service, TlsCertIssue};
use anyhow::Result;
use chrono::Utc;
use std::io::Read;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::Duration;

// Ports we care about for compliance — cleartext auth protocols get flagged.
const PROBE_PORTS: &[u16] = &[
    21,   // FTP  — cleartext, anon login check
    22,   // SSH
    23,   // Telnet — always critical
    25,   // SMTP
    80,   // HTTP
    443,  // HTTPS — TLS version check
    445,  // SMB
    3389, // RDP
    8080, // Alt HTTP
    8443, // Alt HTTPS
];

const TIMEOUT: Duration = Duration::from_millis(500);

/// Per-version TLS probes get a longer budget: there are five of them and a
/// handshake costs more than a bare connect.
const TLS_TIMEOUT: Duration = Duration::from_secs(3);

/// Entry point — probes all target ports on every host in the list.
pub fn run(hosts: &[IpAddr]) -> Result<Vec<RawFinding>> {
    let mut findings = Vec::new();
    for &host in hosts {
        for &port in PROBE_PORTS {
            if let Ok(svc) = probe_port(host, port) {
                findings.push(RawFinding {
                    probe:     ProbeKind::ServiceProbe,
                    timestamp: Utc::now(),
                    payload:   FindingPayload::Service(svc),
                });
            }
        }
    }
    Ok(findings)
}

/// Connects to host:port, grabs banner, checks TLS if applicable.
fn probe_port(host: IpAddr, port: u16) -> Result<Service> {
    let addr = SocketAddr::new(host, port);
    let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;

    // Read up to 256 bytes of banner — enough for SSH/FTP/SMTP greetings.
    let mut buf = [0u8; 256];
    let _ = stream.read(&mut buf);
    let banner = std::str::from_utf8(&buf)
        .ok()
        .map(|s| s.trim_matches('\0').trim().to_owned())
        .filter(|s| !s.is_empty());

    let (tls_versions, tls_cert_issue) = if matches!(port, 443 | 8443 | 465 | 993 | 995) {
        let versions = crate::probes::tls_probe::probe_versions(host, port, TLS_TIMEOUT);
        // Only worth validating the certificate if the port actually speaks TLS.
        let cert_issue = if versions.is_empty() { None } else { check_cert_issue(host, port) };
        (versions, cert_issue)
    } else {
        (Vec::new(), None)
    };

    // `tls_version` keeps its meaning of "what you would negotiate": the highest
    // one on offer. The obsolete-protocol control reads `tls_versions` instead.
    let tls_version = tls_versions.iter().max().map(|v| v.label().to_owned());
    let tls_versions: Vec<String> = tls_versions.iter().map(|v| v.label().to_owned()).collect();

    // Telnet and FTP plaintext auth are always flagged as anonymous_access risk.
    let anonymous_access = matches!(port, 23)
        || (port == 21 && banner.as_deref().map(|b| b.contains("220")).unwrap_or(false));

    Ok(Service {
        host_ip: host,
        port,
        banner,
        tls_version,
        tls_versions,
        tls_cert_issue,
        anonymous_access,
    })
}

/// Validates the server certificate and classifies the problem, if any.
///
/// Version detection lives in `tls_probe`: this function used to also report the
/// version, but returned a hardcoded `"TLSv1.2"` on every successful handshake,
/// which made the obsolete-protocol control impossible to trigger.
fn check_cert_issue(host: IpAddr, port: u16) -> Option<TlsCertIssue> {
    use native_tls::TlsConnector;

    // Use a dummy hostname for SNI — passing an IP string causes SChannel on
    // Windows to reject the handshake even with danger_accept_invalid_hostnames.
    let sni = "probe.internal";
    let tls_timeout = Duration::from_secs(3);

    // First: try strict validation — this tells us if the cert is trusted.
    let strict_result = {
        let addr = SocketAddr::new(host, port);
        TcpStream::connect_timeout(&addr, tls_timeout).ok().and_then(|stream| {
            TlsConnector::new().ok().and_then(|c| {
                c.connect(sni, stream).err().map(|e| e.to_string().to_lowercase())
            })
        })
    };

    // Second: connect accepting any cert to confirm TLS is actually running.
    let addr = SocketAddr::new(host, port);
    let Ok(stream) = TcpStream::connect_timeout(&addr, tls_timeout) else {
        return None;
    };

    let connector: TlsConnector = match TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
    {
        Ok(c)  => c,
        Err(_) => return None,
    };

    match connector.connect(sni, stream) {
        Err(_) => None, // not TLS at all
        Ok(_)  => {
            // TLS confirmed. Now classify the cert issue from the strict result.
            match strict_result {
                None => None, // strict also succeeded — cert is valid
                Some(msg) => {
                    if msg.contains("expired") || msg.contains("validity") || msg.contains("date") {
                        Some(TlsCertIssue::Expired)
                    } else if msg.contains("self") || msg.contains("unknown issuer")
                        || msg.contains("untrusted") || msg.contains("root")
                        || msg.contains("certificate verify failed")
                    {
                        Some(TlsCertIssue::SelfSigned)
                    } else {
                        Some(TlsCertIssue::SelfSigned) // conservative
                    }
                }
            }
        }
    }
}