//! Win32 host discovery through IP Helper: `SendARP` and `IcmpSendEcho2`.
//!
//! Ninguna de las dos exige privilegios de administrador, que es la razon por
//! la que se eligieron: el escaner corre con la cuenta del funcionario. Tampoco
//! exigen Npcap, descartado en el ROADMAP por no ser redistribuible.
//!
//! Este archivo contiene **solo** las llamadas al sistema. La clasificacion de
//! sus resultados vive en el modulo padre, sin `cfg`, para poder probarse sin
//! red y en cualquier plataforma.
//!
//! Convencion de pruebas que estrena este modulo: lo que necesita tipos Win32
//! pero **no** red va inline aca bajo `#[cfg(all(windows, test))]`, y corre en
//! `cargo test` normal. Lo que necesita red va a `core/tests/` con `#[ignore]`,
//! como `tls_probe_live.rs`.

use super::{
    classify_arp, classify_icmp, ipv4_to_net_u32, Ajustes, ArpOutcome, HostEvidence, IcmpOutcome,
    IcmpReplyView, MethodState, Pacer,
};
use std::cell::OnceCell;
use std::net::Ipv4Addr;
use std::time::Duration;
use windows::Win32::Foundation::{GetLastError, HANDLE};
use windows::Win32::NetworkManagement::IpHelper::{
    IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho2, SendARP, ICMP_ECHO_REPLY,
};

/// Payload del echo, visible para el IDS de la red municipal.
///
/// Se identifica a proposito en vez de imitar a `ping.exe`: un barrido de un
/// /24 genera alerta igual, y un escaner que se declara ante el SOC es mas
/// facil de autorizar que uno que se disfraza.
const PAYLOAD: &[u8] = b"MuniANCI escaner Ley 21.663 ANCI";

/// Tamano minimo que exige `IcmpSendEcho2` para la respuesta.
const REPLY_LEN: usize = size_of::<ICMP_ECHO_REPLY>() + PAYLOAD.len() + 8;

const _: () = assert!(PAYLOAD.len() <= u16::MAX as usize);
const _: () = assert!(REPLY_LEN <= u32::MAX as usize);

/// Reply buffer with the alignment `ICMP_ECHO_REPLY` needs.
///
/// El struct lleva un puntero adentro, o sea alineacion 8 en x64, y un
/// `[u8; N]` en el stack no la garantiza por si solo.
#[repr(C, align(8))]
struct ReplyBuf([u8; REPLY_LEN]);

// ---------------------------------------------------------------------------
// Handle ICMP por hilo
// ---------------------------------------------------------------------------

/// Owns an IP Helper ICMP handle and closes it on drop.
struct IcmpHandle(HANDLE);

impl Drop for IcmpHandle {
    fn drop(&mut self) {
        // SAFETY: el handle salio de `IcmpCreateFile`, es exclusivo de este
        // hilo y no se cerro antes: `IcmpHandle` no es Clone ni Copy.
        let _ = unsafe { IcmpCloseHandle(self.0) };
    }
}

thread_local! {
    /// Un handle por hilo, no uno por direccion.
    ///
    /// Abrir 253 handles del kernel para un barrido es desperdicio, y ademas
    /// `HANDLE` no es `Send` ni `Sync`, asi que compartir uno entre los hilos
    /// de rayon no seria correcto. El `Drop` corre al terminar cada worker.
    static ICMP: OnceCell<Option<IcmpHandle>> = const { OnceCell::new() };
}

/// Runs `f` with this thread's ICMP handle, or `None` if it could not be opened.
fn con_handle<R>(f: impl FnOnce(Option<HANDLE>) -> R) -> R {
    ICMP.with(|celda| {
        let h = celda.get_or_init(|| {
            // SAFETY: sin parametros y sin punteros; devuelve `Result`, asi que
            // no hay que comparar contra INVALID_HANDLE_VALUE a mano.
            unsafe { IcmpCreateFile() }.ok().map(IcmpHandle)
        });
        f(h.as_ref().map(|x| x.0))
    })
}

// ---------------------------------------------------------------------------
// Sondas
// ---------------------------------------------------------------------------

/// Resolves the neighbour's physical address for `ip` over ARP.
fn arp_probe(ip: Ipv4Addr) -> ArpOutcome {
    // La API pide un arreglo de ULONG, no de bytes.
    let mut raw = [0u32; 2];
    let mut len: u32 = 8;
    // SAFETY: `raw` vive todo el llamado y `len` describe su tamano real en
    // bytes; la API los escribe, nunca los libera. `srcip = 0` deja que el
    // stack elija la interfaz de salida, que es lo documentado.
    let rc = unsafe { SendARP(ipv4_to_net_u32(ip), 0, raw.as_mut_ptr().cast(), &mut len) };

    let mut bytes = [0u8; 8];
    bytes[..4].copy_from_slice(&raw[0].to_ne_bytes());
    bytes[4..].copy_from_slice(&raw[1].to_ne_bytes());
    classify_arp(rc, len, &bytes)
}

/// Sends one ICMP echo request to `ip` and waits synchronously for the reply.
fn icmp_probe(ip: Ipv4Addr, timeout: Duration) -> IcmpOutcome {
    let dest = ipv4_to_net_u32(ip);
    let ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);

    con_handle(|handle| {
        let Some(h) = handle else {
            return IcmpOutcome::Unavailable;
        };
        let mut buf = ReplyBuf([0u8; REPLY_LEN]);

        // SAFETY: `PAYLOAD` es estatico; `buf` vive hasta el final de este
        // bloque y su tamano se declara con `REPLY_LEN`. Con `event` y
        // `apcroutine` en `None` la llamada es sincrona, que es lo que conviene
        // porque el barrido ya se paraleliza con rayon.
        let replies = unsafe {
            IcmpSendEcho2(
                h,
                None,
                None,
                None,
                dest,
                PAYLOAD.as_ptr().cast(),
                PAYLOAD.len() as u16,
                None,
                buf.0.as_mut_ptr().cast(),
                REPLY_LEN as u32,
                ms,
            )
        };

        if replies == 0 {
            // SAFETY: no toma punteros ni tiene efectos; solo lee el ultimo
            // error de este hilo, y nada corrio entremedio.
            let e = unsafe { GetLastError() }.0;
            return classify_icmp(0, e, None, dest);
        }

        // SAFETY: con `replies > 0` la API garantiza al menos un
        // `ICMP_ECHO_REPLY` al inicio del buffer. Se lee sin alinear por si
        // acaso, y nunca se desreferencia `Data`: solo se copian escalares.
        let r: ICMP_ECHO_REPLY = unsafe { std::ptr::read_unaligned(buf.0.as_ptr().cast()) };
        let vista = IcmpReplyView {
            status: r.Status,
            address: r.Address,
            rtt_ms: r.RoundTripTime,
        };
        classify_icmp(replies, 0, Some(vista), dest)
    })
}

// ---------------------------------------------------------------------------
// Entrada del modulo
// ---------------------------------------------------------------------------

/// Probes one address with the native ARP -> ICMP -> TCP escalation.
pub(super) fn probe(
    ip: Ipv4Addr,
    ajustes: &Ajustes,
    state: &MethodState,
    pacer: &Pacer,
) -> Option<HostEvidence> {
    super::decide(
        || {
            if !ajustes.arp || !state.arp_disponible() {
                return ArpOutcome::NoAnswer;
            }
            pacer.esperar();
            let out = arp_probe(ip);
            if out == ArpOutcome::Unsupported && state.apagar_arp() {
                eprintln!(
                    "aviso: SendARP no funciona en este adaptador (suele pasar con VPN, PPP y \
                     tuneles). El barrido sigue sin ARP, asi que no habra direcciones MAC."
                );
            }
            out
        },
        || {
            if !ajustes.icmp || !state.icmp_disponible() {
                return IcmpOutcome::NoAnswer;
            }
            let out = icmp_probe(ip, ajustes.icmp_timeout);
            if out == IcmpOutcome::Unavailable && state.apagar_icmp() {
                eprintln!("aviso: no se pudo abrir el handle ICMP. El barrido sigue sin ping.");
            }
            out
        },
        || ajustes.tcp && super::tcp_ladder(ip, ajustes.tcp_timeout),
    )
}

#[cfg(all(windows, test))]
mod tests {
    use super::*;

    #[test]
    fn el_buffer_de_respuesta_cabe_el_reply_mas_el_payload() {
        // Es una const, pero el test la ancla contra un cambio futuro de payload.
        assert!(REPLY_LEN >= size_of::<ICMP_ECHO_REPLY>() + PAYLOAD.len() + 8);
    }

    #[test]
    fn el_buffer_de_respuesta_esta_alineado_a_ocho() {
        let buf = ReplyBuf([0u8; REPLY_LEN]);
        assert_eq!(buf.0.as_ptr() as usize % 8, 0);
    }

    #[test]
    fn el_payload_se_identifica_como_muniani() {
        // Decision explicita: no imitamos ping.exe. Si alguien lo cambia por un
        // payload anonimo, que sea a proposito y no de pasada.
        let texto = String::from_utf8_lossy(PAYLOAD);
        assert!(texto.contains("MuniANCI"), "payload: {texto}");
    }

    #[test]
    fn arp_contra_loopback_no_reporta_direccion_fisica() {
        // Determinista y sin red: SendARP contra loopback devuelve longitud 0.
        // De paso verifica que la feature del crate esta activa y que el enlace
        // a iphlpapi.dll funciona.
        let out = arp_probe(Ipv4Addr::LOCALHOST);
        assert!(
            !matches!(out, ArpOutcome::Resolved { mac: Some(_) }),
            "loopback no puede entregar una MAC de vecino: {out:?}"
        );
    }

    #[test]
    fn icmp_responde_en_loopback() {
        // Unico test que ejercita el camino ICMP completo, y funciona sin
        // adaptador de red. La asercion es contra `Unavailable` y no contra
        // `Reply`: un host endurecido no puede poner la suite en rojo, pero un
        // error nuestro si tiene que salir.
        let out = icmp_probe(Ipv4Addr::LOCALHOST, Duration::from_millis(500));
        eprintln!("icmp loopback: {out:?}");
        assert!(
            !matches!(out, IcmpOutcome::Unavailable),
            "el handle ICMP o los parametros estan mal: {out:?}"
        );
    }

    #[test]
    fn el_handle_icmp_se_crea_y_se_cierra_sin_fugas() {
        // Corre en un hilo aparte para no consumir el handle thread-local de la
        // prueba de arriba.
        std::thread::spawn(|| {
            for _ in 0..100 {
                // SAFETY: mismo contrato que `con_handle`; se cierra en `Drop`.
                let h = unsafe { IcmpCreateFile() }.ok().map(IcmpHandle);
                assert!(h.is_some(), "IcmpCreateFile fallo");
            }
        })
        .join()
        .expect("el hilo de handles no debe entrar en panico");
    }
}
