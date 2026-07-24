//! Version-by-version TLS probing.
//!
//! ## Por qué a mano y no con una librería TLS
//!
//! La pregunta de cumplimiento no es "¿qué versión negocia este servidor?" sino
//! "¿sigue **aceptando** TLS 1.0/1.1?". Un servidor que soporta 1.2 y 1.0 a la vez
//! negocia 1.2 con cualquier cliente moderno, así que un handshake normal jamás
//! revelaría la exposición.
//!
//! Y no se puede delegar en `rustls`: no soporta —ni soportará— SSL3, TLS 1.0 ni
//! TLS 1.1, que es justamente lo que hay que detectar. Por eso se construye un
//! `ClientHello` mínimo por versión y se lee el `ServerHello`: si el servidor
//! responde con la misma versión que se le ofreció, esa versión está habilitada;
//! si responde un `alert`, no lo está.
//!
//! Esto reemplaza el comportamiento anterior, que devolvía `"TLSv1.2"` fijo en
//! todo handshake exitoso y por lo tanto hacía que el control "TLS 1.0/1.1/SSLv3
//! activo" no pudiera dispararse nunca.

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::Duration;

/// A TLS/SSL protocol version, as reported to the compliance engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TlsVersion {
    Ssl3,
    Tls10,
    Tls11,
    Tls12,
    Tls13,
}

impl TlsVersion {
    /// The wire value used in `ClientHello.client_version`.
    fn wire(&self) -> u16 {
        match self {
            TlsVersion::Ssl3  => 0x0300,
            TlsVersion::Tls10 => 0x0301,
            TlsVersion::Tls11 => 0x0302,
            TlsVersion::Tls12 => 0x0303,
            // TLS 1.3 keeps legacy_version at 0x0303 and signals 0x0304 in the
            // supported_versions extension.
            TlsVersion::Tls13 => 0x0303,
        }
    }

    /// Label used in reports and matched by the compliance engine.
    pub fn label(&self) -> &'static str {
        match self {
            TlsVersion::Ssl3  => "SSLv3",
            TlsVersion::Tls10 => "TLSv1.0",
            TlsVersion::Tls11 => "TLSv1.1",
            TlsVersion::Tls12 => "TLSv1.2",
            TlsVersion::Tls13 => "TLSv1.3",
        }
    }

    /// Versions considered obsolete for compliance purposes.
    pub fn is_obsolete(&self) -> bool {
        matches!(self, TlsVersion::Ssl3 | TlsVersion::Tls10 | TlsVersion::Tls11)
    }
}

impl std::fmt::Display for TlsVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Every version we probe for, oldest first.
const PROBED: &[TlsVersion] = &[
    TlsVersion::Ssl3,
    TlsVersion::Tls10,
    TlsVersion::Tls11,
    TlsVersion::Tls12,
    TlsVersion::Tls13,
];

/// Probes each protocol version independently and returns those the server accepts.
///
/// Returns an empty vec when the port does not speak TLS at all.
pub fn probe_versions(host: IpAddr, port: u16, timeout: Duration) -> Vec<TlsVersion> {
    PROBED
        .iter()
        .copied()
        .filter(|v| accepts_version(host, port, *v, timeout).unwrap_or(false))
        .collect()
}

/// Opens a connection, offers exactly `version`, and reports whether the server
/// agreed to it.
fn accepts_version(
    host: IpAddr,
    port: u16,
    version: TlsVersion,
    timeout: Duration,
) -> std::io::Result<bool> {
    let addr = SocketAddr::new(host, port);
    let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    stream.write_all(&client_hello(version))?;
    stream.flush()?;

    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf)?;
    Ok(server_agreed(&buf[..n], version))
}

// ---------------------------------------------------------------------------
// ClientHello construction
// ---------------------------------------------------------------------------

/// Cipher suites offered. Deliberately broad and old-friendly: a server that
/// only speaks legacy ciphers must still answer, or we would read its refusal as
/// "version not supported" and under-report the exposure.
const CIPHER_SUITES: &[u16] = &[
    0xC02F, // ECDHE_RSA_WITH_AES_128_GCM_SHA256
    0xC02B, // ECDHE_ECDSA_WITH_AES_128_GCM_SHA256
    0xC030, // ECDHE_RSA_WITH_AES_256_GCM_SHA384
    0xC013, // ECDHE_RSA_WITH_AES_128_CBC_SHA
    0xC014, // ECDHE_RSA_WITH_AES_256_CBC_SHA
    0xC009, // ECDHE_ECDSA_WITH_AES_128_CBC_SHA
    0x009C, // RSA_WITH_AES_128_GCM_SHA256
    0x002F, // RSA_WITH_AES_128_CBC_SHA
    0x0035, // RSA_WITH_AES_256_CBC_SHA
    0x000A, // RSA_WITH_3DES_EDE_CBC_SHA
    0x0005, // RSA_WITH_RC4_128_SHA
    0x1301, // TLS_AES_128_GCM_SHA256        (TLS 1.3)
    0x1302, // TLS_AES_256_GCM_SHA384        (TLS 1.3)
    0x1303, // TLS_CHACHA20_POLY1305_SHA256  (TLS 1.3)
];

/// Builds a minimal but well-formed ClientHello record offering `version`.
fn client_hello(version: TlsVersion) -> Vec<u8> {
    let mut body = Vec::new();

    // client_version
    body.extend_from_slice(&version.wire().to_be_bytes());

    // random (32 bytes) — a fixed, obviously-synthetic value. This is a
    // reachability probe, never a real session, so no entropy is needed and a
    // constant keeps the probe reproducible.
    body.extend_from_slice(&[0x4D; 32]);

    // session_id: empty
    body.push(0x00);

    // cipher_suites
    let suites_len = (CIPHER_SUITES.len() * 2) as u16;
    body.extend_from_slice(&suites_len.to_be_bytes());
    for suite in CIPHER_SUITES {
        body.extend_from_slice(&suite.to_be_bytes());
    }

    // compression_methods: just "null"
    body.push(0x01);
    body.push(0x00);

    // extensions
    let ext = extensions(version);
    body.extend_from_slice(&(ext.len() as u16).to_be_bytes());
    body.extend_from_slice(&ext);

    // handshake header: type + u24 length
    let mut handshake = vec![0x01];
    let len = body.len();
    handshake.push((len >> 16) as u8);
    handshake.push((len >> 8) as u8);
    handshake.push(len as u8);
    handshake.extend_from_slice(&body);

    // record header. The record-layer version stays at 0x0301 for compatibility
    // with middleboxes; the negotiated version is the one inside the handshake.
    let mut record = vec![0x16, 0x03, 0x01];
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend_from_slice(&handshake);
    record
}

/// Extensions block. SSL3 predates extensions, so it gets none.
fn extensions(version: TlsVersion) -> Vec<u8> {
    let mut ext = Vec::new();
    if version == TlsVersion::Ssl3 {
        return ext;
    }

    // supported_groups (0x000a): secp256r1, secp384r1, x25519
    ext.extend_from_slice(&[0x00, 0x0a, 0x00, 0x08, 0x00, 0x06, 0x00, 0x17, 0x00, 0x18, 0x00, 0x1d]);
    // ec_point_formats (0x000b): uncompressed
    ext.extend_from_slice(&[0x00, 0x0b, 0x00, 0x02, 0x01, 0x00]);

    // signature_algorithms (0x000d) — required from TLS 1.2 onwards.
    if matches!(version, TlsVersion::Tls12 | TlsVersion::Tls13) {
        ext.extend_from_slice(&[
            0x00, 0x0d, 0x00, 0x0e, 0x00, 0x0c,
            0x04, 0x03, // ecdsa_secp256r1_sha256
            0x08, 0x04, // rsa_pss_rsae_sha256
            0x04, 0x01, // rsa_pkcs1_sha256
            0x05, 0x01, // rsa_pkcs1_sha384
            0x06, 0x01, // rsa_pkcs1_sha512
            0x02, 0x01, // rsa_pkcs1_sha1
        ]);
    }

    // supported_versions (0x002b) — the only way to ask for TLS 1.3.
    if version == TlsVersion::Tls13 {
        ext.extend_from_slice(&[0x00, 0x2b, 0x00, 0x03, 0x02, 0x03, 0x04]);
        // key_share (0x0033) with an empty client_shares list: the server answers
        // with HelloRetryRequest, which is enough to prove it speaks TLS 1.3.
        ext.extend_from_slice(&[0x00, 0x33, 0x00, 0x02, 0x00, 0x00]);
    }

    ext
}

// ---------------------------------------------------------------------------
// ServerHello parsing
// ---------------------------------------------------------------------------

/// Returns true when the response is a ServerHello agreeing to `asked`.
///
/// An `alert` record (content type 0x15) means refusal — typically
/// `protocol_version(70)` — and any short or malformed answer is treated as
/// "not supported" rather than guessed at.
fn server_agreed(resp: &[u8], asked: TlsVersion) -> bool {
    // Record header: type(1) version(2) length(2)
    if resp.len() < 5 || resp[0] != 0x16 {
        return false;
    }
    // Handshake header: msg_type(1) length(3); msg_type 0x02 = ServerHello
    if resp.len() < 9 || resp[5] != 0x02 {
        return false;
    }
    let server_version = u16::from_be_bytes([resp[9], resp[10]]);

    if asked == TlsVersion::Tls13 {
        // TLS 1.3 pins legacy_version to 0x0303 and signals the real one in the
        // supported_versions extension of the ServerHello.
        return server_version == 0x0303 && has_supported_version_13(resp);
    }

    server_version == asked.wire() && !has_supported_version_13(resp)
}

/// Scans a ServerHello for a supported_versions extension carrying TLS 1.3.
fn has_supported_version_13(resp: &[u8]) -> bool {
    // Walk to the extensions block:
    // 5 record + 4 handshake + 2 version + 32 random = 43, then session_id.
    let mut i = 43usize;
    let session_id_len = match resp.get(i) {
        Some(&l) => l as usize,
        None => return false,
    };
    i += 1 + session_id_len;
    i += 2; // cipher_suite
    i += 1; // compression_method

    // extensions_length
    if i + 2 > resp.len() {
        return false;
    }
    let ext_total = u16::from_be_bytes([resp[i], resp[i + 1]]) as usize;
    i += 2;
    let end = (i + ext_total).min(resp.len());

    while i + 4 <= end {
        let ext_type = u16::from_be_bytes([resp[i], resp[i + 1]]);
        let ext_len = u16::from_be_bytes([resp[i + 2], resp[i + 3]]) as usize;
        i += 4;
        if ext_type == 0x002b && ext_len >= 2 && i + 2 <= resp.len() {
            return u16::from_be_bytes([resp[i], resp[i + 1]]) == 0x0304;
        }
        i += ext_len;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a synthetic ServerHello for parser tests.
    fn server_hello(legacy_version: u16, supported_versions_13: bool) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&legacy_version.to_be_bytes());
        body.extend_from_slice(&[0x00; 32]); // random
        body.push(0x00); // session_id length
        body.extend_from_slice(&[0xC0, 0x2F]); // cipher_suite
        body.push(0x00); // compression

        let mut ext = Vec::new();
        if supported_versions_13 {
            ext.extend_from_slice(&[0x00, 0x2b, 0x00, 0x02, 0x03, 0x04]);
        }
        body.extend_from_slice(&(ext.len() as u16).to_be_bytes());
        body.extend_from_slice(&ext);

        let mut handshake = vec![0x02];
        let len = body.len();
        handshake.push((len >> 16) as u8);
        handshake.push((len >> 8) as u8);
        handshake.push(len as u8);
        handshake.extend_from_slice(&body);

        let mut record = vec![0x16, 0x03, 0x03];
        record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
        record.extend_from_slice(&handshake);
        record
    }

    #[test]
    fn detects_tls10_when_server_agrees() {
        let resp = server_hello(0x0301, false);
        assert!(server_agreed(&resp, TlsVersion::Tls10));
    }

    #[test]
    fn rejects_when_server_picks_another_version() {
        // Pedimos 1.0 y el servidor responde 1.2: 1.0 NO está habilitado.
        let resp = server_hello(0x0303, false);
        assert!(!server_agreed(&resp, TlsVersion::Tls10));
    }

    #[test]
    fn detects_tls13_via_supported_versions() {
        let resp = server_hello(0x0303, true);
        assert!(server_agreed(&resp, TlsVersion::Tls13));
    }

    #[test]
    fn tls12_probe_is_not_fooled_by_a_tls13_answer() {
        // legacy_version 0x0303 pero con supported_versions=1.3: es 1.3, no 1.2.
        let resp = server_hello(0x0303, true);
        assert!(!server_agreed(&resp, TlsVersion::Tls12));
    }

    #[test]
    fn alert_record_means_version_not_supported() {
        // content_type 0x15 = alert, con protocol_version(70).
        let alert = vec![0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x46];
        assert!(!server_agreed(&alert, TlsVersion::Tls10));
    }

    #[test]
    fn empty_or_truncated_answer_is_not_supported() {
        assert!(!server_agreed(&[], TlsVersion::Tls12));
        assert!(!server_agreed(&[0x16, 0x03], TlsVersion::Tls12));
        assert!(!server_agreed(&[0x16, 0x03, 0x03, 0x00, 0x30, 0x02], TlsVersion::Tls12));
    }

    #[test]
    fn plain_http_answer_is_not_tls() {
        assert!(!server_agreed(b"HTTP/1.1 400 Bad Request\r\n", TlsVersion::Tls12));
    }

    #[test]
    fn obsolete_set_matches_the_compliance_control() {
        assert!(TlsVersion::Ssl3.is_obsolete());
        assert!(TlsVersion::Tls10.is_obsolete());
        assert!(TlsVersion::Tls11.is_obsolete());
        assert!(!TlsVersion::Tls12.is_obsolete());
        assert!(!TlsVersion::Tls13.is_obsolete());
    }

    #[test]
    fn client_hello_is_well_formed() {
        let hello = client_hello(TlsVersion::Tls12);
        assert_eq!(hello[0], 0x16, "record type debe ser handshake");
        let record_len = u16::from_be_bytes([hello[3], hello[4]]) as usize;
        assert_eq!(record_len, hello.len() - 5, "largo de record inconsistente");
        assert_eq!(hello[5], 0x01, "handshake type debe ser client_hello");
        let hs_len = ((hello[6] as usize) << 16) | ((hello[7] as usize) << 8) | hello[8] as usize;
        assert_eq!(hs_len, hello.len() - 9, "largo de handshake inconsistente");
        assert_eq!(u16::from_be_bytes([hello[9], hello[10]]), 0x0303);
    }

    #[test]
    fn ssl3_hello_carries_no_extensions() {
        assert!(extensions(TlsVersion::Ssl3).is_empty());
    }
}
