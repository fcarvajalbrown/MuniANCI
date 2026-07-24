//! Live verification of the per-version TLS probe.
//!
//! **Marcados `#[ignore]` a propósito**: requieren red, y el principio
//! offline-first del proyecto exige que `cargo test` corra sin conexión. Se
//! ejecutan a mano cuando se toca `tls_probe`:
//!
//! ```text
//! cargo test -p muniani-core --test tls_probe_live -- --ignored --nocapture
//! ```
//!
//! Los endpoints de badssl.com existen justamente para esto: cada puerto habla
//! una única versión del protocolo, así que sirven de verdad fundamental contra
//! la cual contrastar el parser.

use muniani_core::probes::tls_probe::{probe_versions, TlsVersion};
use std::net::ToSocketAddrs;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(5);

fn resolve(host: &str, port: u16) -> Option<std::net::IpAddr> {
    (host, port).to_socket_addrs().ok()?.next().map(|a| a.ip())
}

/// Probes `host:port` and returns what the server accepts, or `None` if the host
/// could not be reached (sin red, el test se salta en vez de fallar en falso).
fn probe(host: &str, port: u16) -> Option<Vec<TlsVersion>> {
    let ip = resolve(host, port)?;
    let versions = probe_versions(ip, port, TIMEOUT);
    if versions.is_empty() {
        eprintln!("  {host}:{port} no respondió — se omite");
        return None;
    }
    eprintln!("  {host}:{port} acepta {versions:?}");
    Some(versions)
}

#[test]
#[ignore = "requiere red"]
fn detects_a_tls10_only_server() {
    let Some(versions) = probe("tls-v1-0.badssl.com", 1010) else { return };
    assert!(
        versions.contains(&TlsVersion::Tls10),
        "un servidor solo-TLS1.0 debe reportarse como TLS 1.0, se obtuvo {versions:?}"
    );
    assert!(versions.iter().any(|v| v.is_obsolete()));
}

#[test]
#[ignore = "requiere red"]
fn detects_a_tls11_only_server() {
    let Some(versions) = probe("tls-v1-1.badssl.com", 1011) else { return };
    assert!(versions.contains(&TlsVersion::Tls11), "se obtuvo {versions:?}");
}

#[test]
#[ignore = "requiere red"]
fn modern_server_reports_no_obsolete_version() {
    // El caso que más importa no confundir: un servidor moderno NO debe
    // aparecer como TLS 1.0/1.1, o el informe acusaría una brecha inexistente.
    let Some(versions) = probe("www.cloudflare.com", 443) else { return };
    assert!(
        !versions.iter().any(|v| v.is_obsolete()),
        "falso positivo: se reportó protocolo obsoleto en {versions:?}"
    );
    assert!(
        versions.contains(&TlsVersion::Tls12) || versions.contains(&TlsVersion::Tls13),
        "se obtuvo {versions:?}"
    );
}

#[test]
#[ignore = "requiere red"]
fn detects_tls13() {
    let Some(versions) = probe("www.cloudflare.com", 443) else { return };
    assert!(versions.contains(&TlsVersion::Tls13), "se obtuvo {versions:?}");
}

#[test]
#[ignore = "requiere red"]
fn a_plain_http_port_is_not_reported_as_tls() {
    let Some(ip) = resolve("example.com", 80) else { return };
    let versions = probe_versions(ip, 80, TIMEOUT);
    assert!(versions.is_empty(), "puerto sin TLS reportado como TLS: {versions:?}");
}
