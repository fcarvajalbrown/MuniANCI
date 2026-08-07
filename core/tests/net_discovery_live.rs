#![cfg(windows)]
//! Live verification of the native Win32 discovery path.
//!
//! **Marcados `#[ignore]` a proposito**: requieren una LAN, y el principio
//! offline-first del proyecto exige que `cargo test` corra sin conexion.
//!
//! ```text
//! cargo test -p munigpt-core --test net_discovery_live -- --ignored --nocapture
//! ```
//!
//! No hay asercion de tiempo: un barrido de un /24 depende de cuantos equipos
//! haya encendidos y de como conteste el switch, asi que un umbral de reloj
//! seria una prueba intermitente, no una regresion.

use munigpt_core::probes::host_discovery;
use munigpt_core::probes::net_discovery::{Ajustes, DiscoveryMethod, MethodState, Pacer};
use munigpt_core::types::{FindingPayload, Scope};
use std::net::Ipv4Addr;

#[test]
#[ignore = "requiere red"]
fn el_barrido_resuelve_macs_unicas_en_capa_dos() {
    let findings = host_discovery::run_con(Scope::Lan, &Ajustes::default())
        .expect("el barrido no debe fallar");

    let hosts: Vec<_> = findings
        .iter()
        .filter_map(|f| match &f.payload {
            FindingPayload::Host(h) if !h.is_local => Some(h),
            _ => None,
        })
        .collect();

    println!("{} host(s) remoto(s):", hosts.len());
    for h in &hosts {
        println!("  {:<16} {:?} via {:?}", h.ip, h.mac, h.discovered_by);
    }
    if hosts.is_empty() {
        println!("sin vecinos encendidos; nada que verificar");
        return;
    }

    let macs: Vec<&str> = hosts.iter().filter_map(|h| h.mac.as_deref()).collect();
    assert!(!macs.is_empty(), "ningun host entrego MAC: ARP no funciono");

    // Una MAC repetida es la firma del bug de siguiente salto, que el barrido
    // tiene que haber limpiado antes de devolver los hallazgos.
    let unicas: std::collections::HashSet<_> = macs.iter().collect();
    assert_eq!(unicas.len(), macs.len(), "MAC repetida en dos IP: {macs:?}");

    // Formato AA:BB:CC:DD:EE:FF, que es lo que documenta `Host::mac`.
    for m in &macs {
        assert_eq!(m.len(), 17, "{m}");
        assert!(
            m.split(':').count() == 6
                && m.chars().all(|c| c.is_ascii_hexdigit() || c == ':')
                && m.chars().all(|c| !c.is_ascii_lowercase()),
            "formato de MAC invalido: {m}"
        );
    }

    // Toda MAC vino de ARP: ICMP y TCP nunca la entregan.
    for h in hosts.iter().filter(|h| h.mac.is_some()) {
        assert_eq!(h.discovered_by, Some(DiscoveryMethod::Arp), "{}", h.ip);
    }
}

#[test]
#[ignore = "requiere red"]
fn una_direccion_de_documentacion_no_se_reporta_viva() {
    // 192.0.2.1 es TEST-NET-1 (RFC 5737) y no existe en ninguna red real. Si
    // aparece viva es porque un router intermedio contesto un ICMP unreachable
    // y lo contamos como respuesta: el falso positivo que classify_icmp evita.
    let ev = munigpt_core::probes::net_discovery::probe_host(
        Ipv4Addr::new(192, 0, 2, 1),
        &Ajustes::default(),
        &MethodState::default(),
        &Pacer::new(0),
    );
    assert!(ev.is_none(), "host inventado: {ev:?}");
}
