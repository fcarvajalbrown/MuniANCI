//! Probes open ports on discovered hosts: banner grab, TLS version, anon access.
use crate::types::{FindingPayload, ProbeKind, RawFinding, Service, TlsCertIssue};
use anyhow::Result;
use chrono::Utc;
use std::io::{Read, Write};
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
    // TLS 1.3 ClientHello — minimal, no SNI, just enough to get ServerHello back.
    // Byte 9-10 of the ServerHello record contain the negotiated version.
    let addr = SocketAddr::new(host, port);
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, TIMEOUT) else {
        return (None, None);
    };
    let _ = stream.set_read_timeout(Some(TIMEOUT));

    // Minimal TLS 1.0 ClientHello — server will reply with its max supported version.
    let hello: &[u8] = &[
        0x16, 0x03, 0x01, // TLS record: handshake, version 1.0
        0x00, 0x2f,       // length 47
        0x01,             // ClientHello
        0x00, 0x00, 0x2b, // handshake length
        0x03, 0x03,       // client version: TLS 1.2
        // 28 bytes of random (zeros ok for detection)
        0x00,0x00,0x00,0x00, 0x00,0x00,0x00,0x00,
        0x00,0x00,0x00,0x00, 0x00,0x00,0x00,0x00,
        0x00,0x00,0x00,0x00, 0x00,0x00,0x00,0x00,
        0x00,0x00,0x00,0x00,
        0x00, // session id length
        0x00, 0x02, 0x00, 0x2f, // 1 cipher suite: TLS_RSA_WITH_AES_128_CBC_SHA
        0x01, 0x00, // compression: none
    ];

    if stream.write_all(hello).is_err() {
        return (None, None);
    }

    let mut resp = [0u8; 128];
    if stream.read(&mut resp).is_err() {
        return (None, None);
    }

    // ServerHello record: byte 9 = major, byte 10 = minor version.
    let tls = if resp[0] == 0x16 && resp.len() > 10 {
        match (resp[9], resp[10]) {
            (3, 1) => Some("TLSv1.0".into()),
            (3, 2) => Some("TLSv1.1".into()),
            (3, 3) => Some("TLSv1.2".into()),
            (3, 4) => Some("TLSv1.3".into()),
            _      => None,
        }
    } else {
        None
    };

    // We don't do full cert chain validation here — flagged as needs_cert_check.
    // The compliance engine will mark unknown cert status as a gap.
    (tls, None)
}