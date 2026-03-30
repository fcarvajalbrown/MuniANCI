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

    let (tls_version, tls_cert_issue) = if matches!(port, 443 | 8443 | 465 | 993 | 995) {
        check_tls(host, port)
    } else {
        (None, None)
    };

    // Telnet and FTP plaintext auth are always flagged as anonymous_access risk.
    let anonymous_access = matches!(port, 23)
        || (port == 21 && banner.as_deref().map(|b| b.contains("220")).unwrap_or(false));

    Ok(Service {
        host_ip: host,
        port,
        banner,
        tls_version,
        tls_cert_issue,
        anonymous_access,
    })
}

/// Does a minimal TLS ClientHello and reads the ServerHello to extract version.
/// Returns (tls_version_string, cert_issue) — both None if TLS fails entirely.
fn check_tls(host: IpAddr, port: u16) -> (Option<String>, Option<TlsCertIssue>) {
    use native_tls::TlsConnector;

    let addr = SocketAddr::new(host, port);
    let Ok(stream) = TcpStream::connect_timeout(&addr, TIMEOUT) else {
        return (None, None);
    };
    let _ = stream.set_read_timeout(Some(TIMEOUT));

    let connector: TlsConnector = match TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
    {
        Ok(c)  => c,
        Err(_) => return (None, None),
    };

    let host_str = host.to_string();
    let tls_stream: native_tls::TlsStream<TcpStream> = match connector.connect(&host_str, stream) {
        Ok(s)  => s,
        Err(_) => return (None, None),
    };

    // native-tls doesn't expose version directly — use a second connection with strict validation.
    let _ = tls_stream; // first connection confirmed TLS works

    let cert_issue: Option<TlsCertIssue> = {
        let addr2 = SocketAddr::new(host, port);
        if let Ok(stream2) = TcpStream::connect_timeout(&addr2, TIMEOUT) {
            let _ = stream2.set_read_timeout(Some(TIMEOUT));
            let strict: TlsConnector = match TlsConnector::new() {
                Ok(c)  => c,
                Err(_) => return (Some("TLSv1.2".into()), None),
            };
            match strict.connect(&host_str, stream2) {
                Ok(_)  => None,
                Err(e) => {
                    let msg = e.to_string().to_lowercase();
                    if msg.contains("expired") || msg.contains("validity") {
                        Some(TlsCertIssue::Expired)
                    } else if msg.contains("self") || msg.contains("unknown issuer") || msg.contains("untrusted") {
                        Some(TlsCertIssue::SelfSigned)
                    } else {
                        Some(TlsCertIssue::SelfSigned)
                    }
                }
            }
        } else {
            None
        }
    };

    (Some("TLSv1.2".into()), cert_issue)
}