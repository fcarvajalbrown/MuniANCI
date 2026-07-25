//! Native host discovery: ARP for L2 liveness and MAC, ICMP echo, TCP last.
//!
//! Hasta 0.4.0 el descubrimiento probaba TCP 80, 445 y 22 y nada mas, asi que
//! perdia impresoras, camaras IP y equipos de red: justo la clase de activo que
//! nadie parchea. El orden de sondeo de este modulo va de la evidencia mas
//! fuerte a la mas debil:
//!
//! 1. **ARP** — capa 2. No lo filtra el firewall del host y es lo unico que
//!    devuelve la MAC. Solo alcanza al propio segmento.
//! 2. **ICMP** — prueba que la pila IP esta viva. En redes municipales
//!    administradas por GPO suele estar bloqueado en el perfil de dominio.
//! 3. **TCP** — solo prueba que un puerto concreto acepta conexion. Es lo que
//!    hacia el escaner completo antes de este modulo.
//!
//! Toda la logica de decision vive aca y **no lleva `cfg`**: asi se prueba sin
//! red y en cualquier plataforma. Los modulos de plataforma solo aportan las
//! llamadas al sistema y devuelven los enums definidos aca.

#[cfg(not(windows))]
mod fallback;
#[cfg(windows)]
mod windows;

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Tipos publicos
// ---------------------------------------------------------------------------

/// How a host proved that it is alive.
///
/// No es un dato de curiosidad tecnica: es evidencia de calidad distinta. `Arp`
/// prueba presencia fisica en el segmento, `Icmp` prueba pila IP viva, y `Tcp`
/// solo prueba que un puerto acepta conexion. Un inventario de activos que
/// sustenta el deber del Art. 7 deberia poder decir de que calidad es la
/// evidencia de cada fila.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryMethod {
    Arp,
    Icmp,
    Tcp,
}

impl DiscoveryMethod {
    /// Ranks the method by strength of evidence, higher is stronger.
    ///
    /// Lo usa el normalizador para decidir que metodo gana cuando dos sondas
    /// reportan el mismo host.
    pub fn strength(self) -> u8 {
        match self {
            DiscoveryMethod::Arp => 3,
            DiscoveryMethod::Icmp => 2,
            DiscoveryMethod::Tcp => 1,
        }
    }

    /// Returns the label used in the report, in Spanish.
    pub fn etiqueta(self) -> &'static str {
        match self {
            DiscoveryMethod::Arp => "ARP (capa 2)",
            DiscoveryMethod::Icmp => "ICMP",
            DiscoveryMethod::Tcp => "TCP",
        }
    }
}

/// Evidence that a host answered a probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEvidence {
    pub method: DiscoveryMethod,
    /// MAC only ever comes from ARP; ICMP and TCP always leave this `None`.
    pub mac: Option<String>,
    pub rtt_ms: Option<u32>,
}

/// Per-sweep tuning, sourced from `munianci.config.json`.
///
/// `SendARP` no acepta timeout: lo fija la resolucion de vecinos de Windows
/// (tres sondas espaciadas alrededor de un segundo). Por eso aca no hay campo
/// para el, y por eso el limite de ritmo importa mas que los timeouts.
#[derive(Debug, Clone)]
pub struct Ajustes {
    pub arp: bool,
    pub icmp: bool,
    pub tcp: bool,
    /// Sondas ARP por segundo. 0 es sin limite.
    pub arp_pps: u32,
    pub icmp_timeout: Duration,
    pub tcp_timeout: Duration,
}

impl Default for Ajustes {
    /// Los tres metodos activos, con el ARP limitado por seguridad de red.
    fn default() -> Self {
        Self {
            arp: true,
            icmp: true,
            tcp: true,
            arp_pps: 10,
            icmp_timeout: Duration::from_millis(700),
            tcp_timeout: Duration::from_millis(120),
        }
    }
}

/// Tracks methods that turned out to be unavailable on this machine.
///
/// Si `SendARP` devuelve `ERROR_NOT_SUPPORTED` en la primera IP —pasa con
/// adaptadores VPN, PPP y tuneles— reintentarlo en las otras 252 cuesta minutos
/// para nada. Un metodo apagado no es evidencia de que los hosts esten caidos:
/// el barrido sigue con los que queden.
#[derive(Debug, Default)]
pub struct MethodState {
    arp_off: AtomicBool,
    icmp_off: AtomicBool,
}

impl MethodState {
    pub fn arp_disponible(&self) -> bool {
        !self.arp_off.load(Ordering::Relaxed)
    }
    pub fn icmp_disponible(&self) -> bool {
        !self.icmp_off.load(Ordering::Relaxed)
    }
    /// Disables ARP; returns true only for the caller that flipped it.
    ///
    /// El booleano es para que el aviso por stderr salga una sola vez y no una
    /// por cada uno de los hilos que estaba sondeando en paralelo.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn apagar_arp(&self) -> bool {
        !self.arp_off.swap(true, Ordering::Relaxed)
    }
    /// Disables ICMP; returns true only for the caller that flipped it.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn apagar_icmp(&self) -> bool {
        !self.icmp_off.swap(true, Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// Resultados por metodo — los producen los modulos de plataforma
// ---------------------------------------------------------------------------

/// Outcome of one `SendARP` call, platform-independent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArpOutcome {
    /// El vecino contesto. `mac` puede ser `None` si la interfaz no entrega una
    /// direccion de 48 bits: vivo, pero sin MAC utilizable como identificador.
    Resolved { mac: Option<String> },
    /// No contesto: host apagado o fuera del segmento.
    NoAnswer,
    /// La API no sirve en esta maquina o adaptador. No es evidencia de nada.
    Unsupported,
}

/// Outcome of one ICMP echo, platform-independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IcmpOutcome {
    Reply { rtt_ms: u32 },
    NoAnswer,
    /// Handle ICMP no obtenible, o error de programacion nuestro.
    Unavailable,
}

/// The three scalar fields we read out of an `ICMP_ECHO_REPLY`.
///
/// Struct plano a proposito: no lleva tipos Win32, asi que la clasificacion se
/// prueba sin red y sin Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IcmpReplyView {
    pub status: u32,
    pub address: u32,
    pub rtt_ms: u32,
}

// Codigos de `IcmpSendEcho2`, replicados aca para poder clasificar sin Windows.
pub(crate) const IP_SUCCESS: u32 = 0;
pub(crate) const IP_BUF_TOO_SMALL: u32 = 11001;

// Codigos de `SendARP`.
pub(crate) const NO_ERROR: u32 = 0;
pub(crate) const ERROR_NOT_SUPPORTED: u32 = 50;
pub(crate) const ERROR_INVALID_PARAMETER: u32 = 87;

// ---------------------------------------------------------------------------
// Entrada publica
// ---------------------------------------------------------------------------

/// Probes one IPv4 address and returns evidence if it answered.
///
/// Nunca devuelve `Result`: un host caido no es un error, y un fallo real de la
/// API tampoco puede abortar el barrido de las otras 252 direcciones.
pub fn probe_host(
    ip: Ipv4Addr,
    ajustes: &Ajustes,
    state: &MethodState,
    pacer: &Pacer,
) -> Option<HostEvidence> {
    #[cfg(windows)]
    {
        windows::probe(ip, ajustes, state, pacer)
    }
    #[cfg(not(windows))]
    {
        fallback::probe(ip, ajustes, state, pacer)
    }
}

/// Runs the ARP -> ICMP -> TCP escalation with injected probe functions.
///
/// Separado de [`probe_host`] justamente para poder probar la decision sin red:
/// las tres sondas entran como closures.
pub(crate) fn decide(
    arp: impl FnOnce() -> ArpOutcome,
    icmp: impl FnOnce() -> IcmpOutcome,
    tcp: impl FnOnce() -> bool,
) -> Option<HostEvidence> {
    match arp() {
        ArpOutcome::Resolved { mac } => {
            return Some(HostEvidence {
                method: DiscoveryMethod::Arp,
                mac,
                rtt_ms: None,
            })
        }
        ArpOutcome::NoAnswer | ArpOutcome::Unsupported => {}
    }
    match icmp() {
        IcmpOutcome::Reply { rtt_ms } => {
            return Some(HostEvidence {
                method: DiscoveryMethod::Icmp,
                mac: None,
                rtt_ms: Some(rtt_ms),
            })
        }
        IcmpOutcome::NoAnswer | IcmpOutcome::Unavailable => {}
    }
    tcp().then_some(HostEvidence {
        method: DiscoveryMethod::Tcp,
        mac: None,
        rtt_ms: None,
    })
}

// ---------------------------------------------------------------------------
// Helpers puros
// ---------------------------------------------------------------------------

/// Converts an IPv4 address to the `u32` Win32 expects, network byte order.
pub(crate) fn ipv4_to_net_u32(ip: Ipv4Addr) -> u32 {
    u32::from_ne_bytes(ip.octets())
}

/// Formats a physical address as a colon-separated uppercase hex string.
///
/// Devuelve `None` en vez de una cadena rara cuando la direccion no sirve como
/// identificador de activo: un inventario con `00:00:00:00:00:00` repetido en
/// diez filas es peor que un campo vacio.
pub(crate) fn format_mac(bytes: &[u8], len: u32) -> Option<String> {
    let n = (len as usize).min(bytes.len());
    if n != 6 {
        return None;
    }
    let m = &bytes[..6];
    if m.iter().all(|&b| b == 0x00) || m.iter().all(|&b| b == 0xFF) {
        return None;
    }
    let mut s = String::with_capacity(17);
    for (i, b) in m.iter().enumerate() {
        if i > 0 {
            s.push(':');
        }
        s.push_str(&format!("{b:02X}"));
    }
    Some(s)
}

/// Maps a `SendARP` return code plus its out-length into an [`ArpOutcome`].
///
/// La distincion que importa es `Unsupported` contra `NoAnswer`: tratar el
/// primero como host muerto haria que en una maquina con adaptador VPN el
/// barrido reporte cero hosts sin explicar por que.
pub(crate) fn classify_arp(rc: u32, len: u32, raw: &[u8]) -> ArpOutcome {
    match rc {
        NO_ERROR => match len {
            // Pasa con loopback y con la propia IP. No es evidencia de vecino.
            0 => ArpOutcome::NoAnswer,
            // Interfaces no-Ethernet devuelven otra longitud. El host contesto,
            // pero eso no es una MAC de 48 bits y meterla en un campo
            // documentado como MAC seria mentir en el inventario.
            6 => ArpOutcome::Resolved {
                mac: format_mac(raw, len),
            },
            _ => ArpOutcome::Resolved { mac: None },
        },
        // El adaptador no hace ARP. Apaga el metodo para el resto del barrido.
        ERROR_NOT_SUPPORTED => ArpOutcome::Unsupported,
        // Bug nuestro, no un host caido.
        ERROR_INVALID_PARAMETER => ArpOutcome::Unsupported,
        // ERROR_GEN_FAILURE (31), ERROR_BAD_NET_NAME (67), ERROR_NOT_FOUND
        // (1168) y cualquier codigo desconocido: no se toma como host vivo.
        _ => ArpOutcome::NoAnswer,
    }
}

/// Maps an `IcmpSendEcho2` result into an [`IcmpOutcome`].
///
/// `IcmpSendEcho2` devuelve el numero de respuestas, no un codigo, y
/// `replies > 0` **no** significa vivo: un router intermedio puede contestar un
/// ICMP unreachable y eso llega como respuesta. Por eso se exige
/// `status == IP_SUCCESS` y que la direccion que contesto sea la que se sondeo.
/// Sin esa doble condicion se inventan hosts que no existen, que es el peor
/// error posible en un inventario que va a un informe de cumplimiento.
pub(crate) fn classify_icmp(
    replies: u32,
    last_error: u32,
    reply: Option<IcmpReplyView>,
    target: u32,
) -> IcmpOutcome {
    if replies == 0 {
        return match last_error {
            IP_BUF_TOO_SMALL | ERROR_INVALID_PARAMETER => IcmpOutcome::Unavailable,
            // IP_REQ_TIMED_OUT, IP_DEST_HOST_UNREACHABLE, IP_DEST_NET_UNREACHABLE
            // y el resto: el host no contesto.
            _ => IcmpOutcome::NoAnswer,
        };
    }
    match reply {
        Some(r) if r.status == IP_SUCCESS && r.address == target => {
            IcmpOutcome::Reply { rtt_ms: r.rtt_ms }
        }
        _ => IcmpOutcome::NoAnswer,
    }
}

/// TCP connect ladder: the pre-0.5.0 discovery, kept as the last resort.
///
/// Compartido por Windows y por el fallback portable, para que haya una sola
/// implementacion y se pruebe una sola vez.
pub(crate) fn tcp_ladder(ip: Ipv4Addr, timeout: Duration) -> bool {
    const PUERTOS: [u16; 3] = [80, 445, 22];
    PUERTOS.iter().any(|&p| {
        let addr = SocketAddr::new(IpAddr::V4(ip), p);
        std::net::TcpStream::connect_timeout(&addr, timeout).is_ok()
    })
}

/// Returns every host address in the /24 around `ip`, minus `ip` itself.
///
/// Se extrajo de `local_subnet()` para poder probarla: la version original
/// abria un `UdpSocket` y por eso no tenia cobertura. Sigue asumiendo /24; la
/// mascara real requiere `GetAdaptersAddresses` y es un item aparte.
pub(crate) fn subnet_de(ip: Ipv4Addr) -> Vec<Ipv4Addr> {
    let o = ip.octets();
    (1u8..=254)
        .filter(|&last| last != o[3])
        .map(|last| Ipv4Addr::new(o[0], o[1], o[2], last))
        .collect()
}

// ---------------------------------------------------------------------------
// Limitador de ritmo
// ---------------------------------------------------------------------------

/// Spaces out ARP probes to at most `pps` per second across all threads.
///
/// No es una optimizacion. Dynamic ARP Inspection en switches Cisco limita el
/// ARP en puertos de acceso no confiables y al superar el umbral deja el puerto
/// en err-disable: el escaner dejaria sin red al equipo desde el que corre, y
/// alguien de TI tendria que ir a rehabilitar el puerto. El valor por defecto
/// queda debajo del umbral tipico.
#[derive(Debug)]
pub struct Pacer {
    min_gap: Option<Duration>,
    proxima: Mutex<Option<Instant>>,
}

impl Pacer {
    /// Builds a pacer for `pps` probes per second; `0` means no limit.
    pub fn new(pps: u32) -> Self {
        Self {
            min_gap: (pps > 0).then(|| Duration::from_secs_f64(1.0 / f64::from(pps))),
            proxima: Mutex::new(None),
        }
    }

    /// Blocks until this thread may send its next probe.
    pub fn esperar(&self) {
        let Some(gap) = self.min_gap else { return };
        let ahora = Instant::now();
        // El lock se suelta antes de dormir: si no, un hilo dormido bloquearia
        // a todos los demas y el limite dejaria de ser por segundo para pasar a
        // ser secuencial.
        let dormir = {
            let Ok(mut proxima) = self.proxima.lock() else {
                return;
            };
            let turno = proxima.map_or(ahora, |t| t.max(ahora));
            *proxima = Some(turno + gap);
            turno.saturating_duration_since(ahora)
        };
        if !dormir.is_zero() {
            std::thread::sleep(dormir);
        }
    }
}

// ---------------------------------------------------------------------------
// Correccion de MAC de siguiente salto
// ---------------------------------------------------------------------------

/// Blanks out any MAC that shows up on more than one address.
///
/// `SendARP` hace consulta de ruta: si la IP destino no esta on-link, el stack
/// puede resolver la MAC del **siguiente salto** y devolverla como si fuera del
/// destino. Como [`subnet_de`] asume /24 sin verificarlo, una red con mascara
/// distinta o enrutada dejaria doscientas filas del inventario con la MAC del
/// gateway. Una MAC repetida es siempre un error: se descarta en todas.
///
/// Devuelve cuantas direcciones perdieron la MAC, para poder avisarlo.
pub(crate) fn descartar_macs_de_next_hop(hallazgos: &mut [(Ipv4Addr, HostEvidence)]) -> usize {
    use std::collections::HashMap;
    let mut cuenta: HashMap<&str, usize> = HashMap::new();
    for (_, ev) in hallazgos.iter() {
        if let Some(m) = ev.mac.as_deref() {
            *cuenta.entry(m).or_default() += 1;
        }
    }
    let repetidas: Vec<String> = cuenta
        .into_iter()
        .filter(|&(_, n)| n > 1)
        .map(|(m, _)| m.to_string())
        .collect();
    if repetidas.is_empty() {
        return 0;
    }
    let mut borradas = 0;
    for (_, ev) in hallazgos.iter_mut() {
        if ev.mac.as_deref().is_some_and(|m| repetidas.iter().any(|r| r == m)) {
            ev.mac = None;
            borradas += 1;
        }
    }
    borradas
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- conversion de IP ---------------------------------------------------

    #[test]
    fn ipv4_to_net_u32_preserva_el_orden_de_los_octetos() {
        // La asercion portable es sobre el invariante, no sobre un numero
        // magico: en x86 el u32 se lee al reves, y eso es exactamente lo que
        // Win32 espera.
        let ip = Ipv4Addr::new(192, 168, 1, 10);
        assert_eq!(ipv4_to_net_u32(ip).to_ne_bytes(), ip.octets());
    }

    // -- formateo de MAC ----------------------------------------------------

    #[test]
    fn format_mac_usa_hex_mayuscula_separado_por_dos_puntos() {
        let m = format_mac(&[0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e, 0, 0], 6);
        assert_eq!(m.as_deref(), Some("00:1A:2B:3C:4D:5E"));
    }

    #[test]
    fn format_mac_rechaza_longitud_cero() {
        // Es lo que devuelve SendARP contra loopback y contra la propia IP.
        assert_eq!(format_mac(&[0xAA; 8], 0), None);
    }

    #[test]
    fn format_mac_rechaza_longitudes_distintas_de_seis() {
        for len in [1, 4, 5, 7, 8] {
            assert_eq!(format_mac(&[0xAA; 8], len), None, "len {len}");
        }
    }

    #[test]
    fn format_mac_rechaza_la_direccion_toda_ceros() {
        assert_eq!(format_mac(&[0x00; 8], 6), None);
    }

    #[test]
    fn format_mac_rechaza_la_direccion_de_broadcast() {
        assert_eq!(format_mac(&[0xFF; 8], 6), None);
    }

    #[test]
    fn format_mac_nunca_lee_mas_alla_del_buffer() {
        // Una longitud absurda devuelta por la API no puede provocar un panic.
        assert_eq!(format_mac(&[0xAA; 8], 64), None);
        assert_eq!(format_mac(&[], 6), None);
    }

    // -- clasificacion de ARP -----------------------------------------------

    #[test]
    fn arp_longitud_cero_no_es_evidencia_de_vecino() {
        assert_eq!(classify_arp(NO_ERROR, 0, &[0; 8]), ArpOutcome::NoAnswer);
    }

    #[test]
    fn arp_resuelto_entrega_la_mac() {
        let raw = [0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e, 0, 0];
        assert_eq!(
            classify_arp(NO_ERROR, 6, &raw),
            ArpOutcome::Resolved {
                mac: Some("00:1A:2B:3C:4D:5E".into())
            }
        );
    }

    #[test]
    fn arp_gen_failure_es_host_sin_respuesta() {
        // 31 = ERROR_GEN_FAILURE, el caso normal de "no contesto".
        assert_eq!(classify_arp(31, 0, &[0; 8]), ArpOutcome::NoAnswer);
    }

    #[test]
    fn arp_not_supported_apaga_el_metodo() {
        assert_eq!(
            classify_arp(ERROR_NOT_SUPPORTED, 0, &[0; 8]),
            ArpOutcome::Unsupported
        );
        assert_eq!(
            classify_arp(ERROR_INVALID_PARAMETER, 0, &[0; 8]),
            ArpOutcome::Unsupported
        );
    }

    #[test]
    fn arp_codigo_desconocido_no_se_toma_como_host_vivo() {
        assert_eq!(classify_arp(1234, 6, &[0xAA; 8]), ArpOutcome::NoAnswer);
    }

    #[test]
    fn arp_longitud_no_ethernet_reporta_vivo_sin_mac() {
        // FireWire e InfiniBand contestan con direcciones de otro largo. El
        // host esta vivo, pero no hay MAC de 48 bits que reportar.
        assert_eq!(
            classify_arp(NO_ERROR, 8, &[0xAA; 8]),
            ArpOutcome::Resolved { mac: None }
        );
    }

    // -- clasificacion de ICMP ----------------------------------------------

    fn reply(status: u32, address: u32) -> Option<IcmpReplyView> {
        Some(IcmpReplyView {
            status,
            address,
            rtt_ms: 4,
        })
    }

    #[test]
    fn icmp_respuesta_de_router_unreachable_no_es_host_vivo() {
        // El router contesta un unreachable y eso llega como "una respuesta".
        const IP_DEST_HOST_UNREACHABLE: u32 = 11003;
        let out = classify_icmp(1, 0, reply(IP_DEST_HOST_UNREACHABLE, 99), 99);
        assert_eq!(out, IcmpOutcome::NoAnswer);
    }

    #[test]
    fn icmp_respuesta_de_otra_direccion_no_es_host_vivo() {
        let out = classify_icmp(1, 0, reply(IP_SUCCESS, 77), 99);
        assert_eq!(out, IcmpOutcome::NoAnswer);
    }

    #[test]
    fn icmp_timeout_es_host_sin_respuesta() {
        const IP_REQ_TIMED_OUT: u32 = 11010;
        const IP_DEST_NET_UNREACHABLE: u32 = 11002;
        for e in [IP_REQ_TIMED_OUT, IP_DEST_NET_UNREACHABLE] {
            assert_eq!(classify_icmp(0, e, None, 99), IcmpOutcome::NoAnswer, "{e}");
        }
    }

    #[test]
    fn icmp_buffer_too_small_es_bug_no_host_muerto() {
        assert_eq!(
            classify_icmp(0, IP_BUF_TOO_SMALL, None, 99),
            IcmpOutcome::Unavailable
        );
        assert_eq!(
            classify_icmp(0, ERROR_INVALID_PARAMETER, None, 99),
            IcmpOutcome::Unavailable
        );
    }

    #[test]
    fn icmp_respuesta_valida_reporta_rtt() {
        assert_eq!(
            classify_icmp(1, 0, reply(IP_SUCCESS, 99), 99),
            IcmpOutcome::Reply { rtt_ms: 4 }
        );
    }

    #[test]
    fn icmp_respuesta_contada_pero_sin_cuerpo_no_es_host_vivo() {
        assert_eq!(classify_icmp(1, 0, None, 99), IcmpOutcome::NoAnswer);
    }

    // -- el ladder ----------------------------------------------------------

    #[test]
    fn el_ladder_prefiere_la_evidencia_arp_y_no_sigue_sondeando() {
        let mut se_sondeo_icmp = false;
        let mut se_sondeo_tcp = false;
        let ev = decide(
            || ArpOutcome::Resolved {
                mac: Some("AA:BB:CC:DD:EE:FF".into()),
            },
            || {
                se_sondeo_icmp = true;
                IcmpOutcome::NoAnswer
            },
            || {
                se_sondeo_tcp = true;
                true
            },
        )
        .expect("ARP resuelto tiene que reportar el host");
        assert_eq!(ev.method, DiscoveryMethod::Arp);
        assert_eq!(ev.mac.as_deref(), Some("AA:BB:CC:DD:EE:FF"));
        assert!(!se_sondeo_icmp, "no debe sondear ICMP si ARP resolvio");
        assert!(!se_sondeo_tcp, "no debe sondear TCP si ARP resolvio");
    }

    #[test]
    fn sin_arp_el_ladder_cae_a_icmp() {
        let ev = decide(
            || ArpOutcome::NoAnswer,
            || IcmpOutcome::Reply { rtt_ms: 12 },
            || panic!("no debe llegar a TCP"),
        )
        .expect("ICMP con respuesta tiene que reportar el host");
        assert_eq!(ev.method, DiscoveryMethod::Icmp);
        assert_eq!(ev.mac, None);
        assert_eq!(ev.rtt_ms, Some(12));
    }

    #[test]
    fn sin_arp_ni_icmp_el_ladder_cae_a_tcp() {
        let ev = decide(|| ArpOutcome::NoAnswer, || IcmpOutcome::NoAnswer, || true)
            .expect("TCP abierto tiene que reportar el host");
        assert_eq!(ev.method, DiscoveryMethod::Tcp);
        assert_eq!(ev.mac, None);
    }

    #[test]
    fn si_ningun_metodo_responde_el_host_no_se_reporta() {
        let ev = decide(|| ArpOutcome::NoAnswer, || IcmpOutcome::NoAnswer, || false);
        assert_eq!(ev, None);
    }

    #[test]
    fn arp_no_soportado_no_impide_descubrir_por_icmp() {
        let ev = decide(
            || ArpOutcome::Unsupported,
            || IcmpOutcome::Reply { rtt_ms: 1 },
            || false,
        )
        .expect("un metodo no soportado no puede ocultar el host");
        assert_eq!(ev.method, DiscoveryMethod::Icmp);
    }

    // -- fuerza de la evidencia ---------------------------------------------

    #[test]
    fn arp_es_evidencia_mas_fuerte_que_icmp_y_esta_que_tcp() {
        assert!(DiscoveryMethod::Arp.strength() > DiscoveryMethod::Icmp.strength());
        assert!(DiscoveryMethod::Icmp.strength() > DiscoveryMethod::Tcp.strength());
    }

    #[test]
    fn el_metodo_se_serializa_en_minuscula() {
        let j = serde_json::to_string(&DiscoveryMethod::Arp).unwrap();
        assert_eq!(j, "\"arp\"");
    }

    // -- subred -------------------------------------------------------------

    #[test]
    fn subnet_de_omite_la_propia_ip_y_los_extremos() {
        let ips = subnet_de(Ipv4Addr::new(192, 168, 1, 10));
        assert_eq!(ips.len(), 253);
        assert!(!ips.contains(&Ipv4Addr::new(192, 168, 1, 10)), "la propia IP");
        assert!(!ips.contains(&Ipv4Addr::new(192, 168, 1, 0)), "la red");
        assert!(!ips.contains(&Ipv4Addr::new(192, 168, 1, 255)), "el broadcast");
        assert!(ips.contains(&Ipv4Addr::new(192, 168, 1, 1)));
        assert!(ips.contains(&Ipv4Addr::new(192, 168, 1, 254)));
    }

    // -- macs de siguiente salto --------------------------------------------

    #[test]
    fn macs_repetidas_se_descartan_como_next_hop() {
        let gw = HostEvidence {
            method: DiscoveryMethod::Arp,
            mac: Some("AA:BB:CC:DD:EE:FF".into()),
            rtt_ms: None,
        };
        let propia = HostEvidence {
            method: DiscoveryMethod::Arp,
            mac: Some("11:22:33:44:55:66".into()),
            rtt_ms: None,
        };
        let mut h = vec![
            (Ipv4Addr::new(10, 0, 0, 1), gw.clone()),
            (Ipv4Addr::new(10, 0, 0, 2), gw.clone()),
            (Ipv4Addr::new(10, 0, 0, 3), gw),
            (Ipv4Addr::new(10, 0, 0, 4), propia),
        ];
        assert_eq!(descartar_macs_de_next_hop(&mut h), 3);
        assert!(h[0].1.mac.is_none());
        assert!(h[1].1.mac.is_none());
        assert!(h[2].1.mac.is_none());
        assert_eq!(h[3].1.mac.as_deref(), Some("11:22:33:44:55:66"));
        // Los hosts siguen reportados: perder la MAC no los borra del inventario.
        assert_eq!(h.len(), 4);
    }

    #[test]
    fn macs_unicas_se_conservan() {
        let mut h = vec![
            (
                Ipv4Addr::new(10, 0, 0, 1),
                HostEvidence {
                    method: DiscoveryMethod::Arp,
                    mac: Some("AA:BB:CC:DD:EE:01".into()),
                    rtt_ms: None,
                },
            ),
            (
                Ipv4Addr::new(10, 0, 0, 2),
                HostEvidence {
                    method: DiscoveryMethod::Arp,
                    mac: Some("AA:BB:CC:DD:EE:02".into()),
                    rtt_ms: None,
                },
            ),
        ];
        assert_eq!(descartar_macs_de_next_hop(&mut h), 0);
        assert!(h.iter().all(|(_, e)| e.mac.is_some()));
    }

    // -- limitador de ritmo -------------------------------------------------

    #[test]
    fn el_pacer_espacia_las_llamadas_al_ritmo_configurado() {
        let p = Pacer::new(50); // 20 ms entre sondas
        let t0 = Instant::now();
        for _ in 0..4 {
            p.esperar();
        }
        // Cuatro turnos son tres esperas de 20 ms. Se deja holgura hacia abajo
        // porque la granularidad del sleep en Windows es de ~15 ms.
        assert!(
            t0.elapsed() >= Duration::from_millis(45),
            "espero muy poco: {:?}",
            t0.elapsed()
        );
    }

    #[test]
    fn el_pacer_sin_limite_no_espera() {
        let p = Pacer::new(0);
        let t0 = Instant::now();
        for _ in 0..1000 {
            p.esperar();
        }
        assert!(t0.elapsed() < Duration::from_millis(100));
    }
}
