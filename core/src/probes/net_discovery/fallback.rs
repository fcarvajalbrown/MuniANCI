//! Portable discovery fallback: the TCP connect ladder, unchanged.
//!
//! Es literalmente lo que hacia `host_discovery::is_alive` antes del
//! descubrimiento nativo. Se conserva sin cambios a proposito: mientras la rama
//! Linux no tenga camino propio (`pnet`, diferido en el ROADMAP porque en
//! Windows exigiria Npcap, que no es redistribuible), el crate tiene que seguir
//! compilando y el escaneo en Linux no debe cambiar de resultado.

use super::{Ajustes, DiscoveryMethod, HostEvidence, MethodState, Pacer};
use std::net::Ipv4Addr;

/// Probes one address using only the portable TCP ladder.
pub(super) fn probe(
    ip: Ipv4Addr,
    ajustes: &Ajustes,
    _state: &MethodState,
    _pacer: &Pacer,
) -> Option<HostEvidence> {
    // Sin ARP ni ICMP nativos no hay metodo que apagar ni ritmo que limitar,
    // por eso `_state` y `_pacer` no se usan aca.
    if !ajustes.tcp {
        return None;
    }
    super::tcp_ladder(ip, ajustes.tcp_timeout).then(|| HostEvidence {
        method: DiscoveryMethod::Tcp,
        mac: None,
        rtt_ms: None,
    })
}
