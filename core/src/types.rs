//! Core domain types for MuniANCI.
//!
//! Every module in muniani-core speaks these types. Nothing here does I/O —
//! it is pure data. The compliance engine, normalizer, and report builder all
//! consume and produce values defined in this file.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

// ---------------------------------------------------------------------------
// Institution tier
// ---------------------------------------------------------------------------

/// Classification of the scanned institution under Ley 21.663.
///
/// Determines which controls are legally required vs. measured as voluntary
/// maturity. The operator passes this at scan time via `--tier`.
///
/// Para una **municipalidad el tier correcto es `Pse`**, no `Oiv` ni
/// `Unclassified`: sus servicios son esenciales por definición legal (Art. 4°
/// inciso 2, vía Art. 1°, que incluye a las Municipalidades en la Administración
/// del Estado), pero la Res. Ex. N°87 de ANCI las excluyó expresamente del primer
/// proceso de calificación de OIV, y la nómina preliminar de la segunda etapa
/// tampoco las incluye. Ver `docs/research/0.5.0-escaner-y-cumplimiento-anci.md` §1.2.
///
/// El tier es un dato con fecha, no una constante: el Art. 6° obliga a la Agencia
/// a revisar la calificación al menos cada tres años.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Operador de Importancia Vital — deberes específicos del Art. 8° exigibles.
    Oiv,
    /// Prestador de servicio esencial — Arts. 7° y 9° e IG N°1 exigibles; el
    /// Art. 8° se mide como madurez voluntaria. Es el tier de una municipalidad.
    Pse,
    /// Institución cuyo estatus no se ha determinado — escaneo informativo.
    Unclassified,
}

impl std::fmt::Display for Tier {
    /// Formats the tier for display in reports.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tier::Oiv => write!(f, "Operador de Importancia Vital (OIV)"),
            Tier::Pse => write!(f, "Prestador de Servicio Esencial (PSE)"),
            Tier::Unclassified => write!(f, "No clasificado"),
        }
    }
}

// ---------------------------------------------------------------------------
// Scan scope
// ---------------------------------------------------------------------------

/// Whether the scan targets only the local machine or the full LAN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Local,
    Lan,
}

// ---------------------------------------------------------------------------
// Asset types
// ---------------------------------------------------------------------------

/// A discovered network host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Host {
    /// IP address of the host.
    pub ip: IpAddr,
    /// Reverse-DNS hostname, if resolvable.
    pub hostname: Option<String>,
    /// MAC address as a colon-separated hex string, if discoverable (LAN only).
    pub mac: Option<String>,
    /// Operating system banner, if fingerprinted.
    pub os_banner: Option<String>,
    /// Whether this host is the machine running the scan.
    pub is_local: bool,
}

/// A storage volume — local disk, network share, or removable media.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drive {
    /// Display name or UNC path (e.g. `C:\`, `\\server\share`, `/dev/sda1`).
    pub path: String,
    /// Drive kind.
    pub kind: DriveKind,
    /// Total capacity in bytes, if readable.
    pub total_bytes: Option<u64>,
    /// Free space in bytes, if readable.
    pub free_bytes: Option<u64>,
    /// Whether BitLocker (Windows) or LUKS (Linux) encryption is active.
    pub encrypted: Option<bool>,
    /// IP of the host owning this drive (None = local machine).
    pub host_ip: Option<IpAddr>,
}

/// Distinguishes how a drive is attached / accessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveKind {
    /// Fixed local disk.
    Fixed,
    /// Removable USB or external drive.
    Removable,
    /// SMB/CIFS network share.
    Smb,
    /// NFS mount.
    Nfs,
    /// WebDAV mount.
    WebDav,
    /// Cloud sync folder (OneDrive, Dropbox, Google Drive process detected).
    CloudSync,
    /// Unknown / other.
    Unknown,
}

/// A listening network service on a host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    /// Host where this service was found.
    pub host_ip: IpAddr,
    /// TCP/UDP port.
    pub port: u16,
    /// Protocol string from banner grab (e.g. "SSH-2.0-OpenSSH_8.9").
    pub banner: Option<String>,
    /// TLS version if the service speaks TLS (e.g. "TLSv1.2", "TLSv1.3").
    pub tls_version: Option<String>,
    /// Whether the TLS certificate is expired or self-signed.
    pub tls_cert_issue: Option<TlsCertIssue>,
    /// True if the service accepted a connection without credentials.
    pub anonymous_access: bool,
}

/// Certificate validation problem found during TLS handshake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsCertIssue {
    Expired,
    SelfSigned,
    ExpiredAndSelfSigned,
}

/// An installed software package (from WMI, dpkg, or rpm).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareEntry {
    /// Package or application name.
    pub name: String,
    /// Version string as reported by the OS.
    pub version: String,
    /// Host where this software is installed.
    pub host_ip: IpAddr,
    /// True if this version is known to be end-of-life.
    pub is_eol: bool,
    /// Highest CVSS score among known CVEs for this version, if any.
    pub max_cvss: Option<f32>,
}

// ---------------------------------------------------------------------------
// Raw finding — probe output before normalization
// ---------------------------------------------------------------------------

/// Unstructured output from a single probe run.
///
/// Probes produce `RawFinding` values; the normalizer converts them into the
/// typed asset graph and eventually into `Gap` values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFinding {
    /// Which probe produced this finding.
    pub probe: ProbeKind,
    /// When the finding was recorded.
    pub timestamp: DateTime<Utc>,
    /// Structured payload — probe-specific.
    pub payload: FindingPayload,
}

/// Which probe produced a raw finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeKind {
    HostDiscovery,
    DriveEnum,
    ServiceProbe,
    SwInventory,
    OsCheck,
}

/// Typed payload variants — one per probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FindingPayload {
    Host(Host),
    Drive(Drive),
    Service(Service),
    Software(SoftwareEntry),
    OsInfo(OsInfo),
}

/// Basic OS information collected by the os_check probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    /// Host this info belongs to.
    pub host_ip: IpAddr,
    /// OS family: "windows" | "linux" | "macos".
    pub family: String,
    /// OS version string (e.g. "Windows Server 2008 R2", "Ubuntu 22.04").
    pub version: String,
    /// True if this OS version is end-of-life.
    pub is_eol: bool,
    /// True if Windows Firewall / ufw / iptables is active.
    pub firewall_active: bool,
    /// True if a backup agent is running.
    pub backup_agent_running: Option<bool>,
}

// ---------------------------------------------------------------------------
// Asset graph — normalizer output
// ---------------------------------------------------------------------------

/// The normalized view of everything discovered during a scan.
///
/// Built by `normalizer::normalize()` from a `Vec<RawFinding>`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AssetGraph {
    pub hosts: Vec<Host>,
    pub drives: Vec<Drive>,
    pub services: Vec<Service>,
    pub software: Vec<SoftwareEntry>,
    pub os_info: Vec<OsInfo>,
}

// ---------------------------------------------------------------------------
// Compliance gap — compliance_engine output
// ---------------------------------------------------------------------------

/// Legal classification of a breach under Ley 21.663.
///
/// Not a judgement of technical risk — it is the classification the law itself
/// assigns. Art. 39° classifies breaches of each literal of Art. 8° (OIV only);
/// Art. 38° classifies the general ones. Used to weight the aggregate score
/// without inventing a weighting of our own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InfractionClass {
    Leve,
    Grave,
    Gravisima,
}

impl InfractionClass {
    /// Score deduction — SPRS mechanics (fixed base minus weighted deductions),
    /// but with the weights anchored in the law's own severity scale.
    pub fn score_weight(&self) -> u32 {
        match self {
            InfractionClass::Leve      => 1,
            InfractionClass::Grave     => 3,
            InfractionClass::Gravisima => 5,
        }
    }

    /// Maximum fine in UTM for this class, per Art. 40°.
    pub fn max_utm(&self, tier: Tier) -> u32 {
        let oiv = tier == Tier::Oiv;
        match (self, oiv) {
            (InfractionClass::Leve,      false) =>  5_000,
            (InfractionClass::Leve,      true)  => 10_000,
            (InfractionClass::Grave,     false) => 10_000,
            (InfractionClass::Grave,     true)  => 20_000,
            (InfractionClass::Gravisima, false) => 20_000,
            (InfractionClass::Gravisima, true)  => 40_000,
        }
    }
}

impl std::fmt::Display for InfractionClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InfractionClass::Leve      => write!(f, "Leve"),
            InfractionClass::Grave     => write!(f, "Grave"),
            InfractionClass::Gravisima => write!(f, "Gravísima"),
        }
    }
}

/// Whether a control is legally binding on the scanned institution, or is being
/// measured as voluntary maturity.
///
/// This is the core of the dual compliance model: an institution that is not an
/// OIV is not bound by Art. 8°, but can still measure itself against it. Saying
/// "no cumple" about a duty that does not bind it would be asserting a breach
/// that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Exigibilidad {
    /// Obligación vigente para el tier de esta institución.
    Exigible,
    /// No exigible hoy; se informa como madurez voluntaria.
    MadurezVoluntaria,
}

impl std::fmt::Display for Exigibilidad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Exigibilidad::Exigible          => write!(f, "Exigible"),
            Exigibilidad::MadurezVoluntaria => write!(f, "Madurez voluntaria (no exigible)"),
        }
    }
}

/// A single compliance gap: one control failed, with evidence and legal anchor.
///
/// The compliance engine produces a `Vec<Gap>` from an `AssetGraph`.
/// The report builder renders this vec into PDF and JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gap {
    /// Human-readable control name (matches the quick-ref table).
    pub control: String,
    /// One-line description of what was found.
    pub finding: String,
    /// Gap severity.
    pub severity: Severity,
    /// Legal anchor: article of Ley 21.663, Instrucción General, or ISO reference.
    pub legal_anchor: String,
    /// Minimum tier for which this gap is mandatory (not just informational).
    pub applies_to: AppliesTo,
    /// Whether this gap is legally binding on the scanned tier, or informational
    /// maturity. Normalised by `compliance_engine::apply_exigibilidad`.
    pub exigibilidad: Exigibilidad,
    /// Legal classification of the breach (Art. 38°/39°), when the law assigns
    /// one. `None` for technical controls with no direct statutory counterpart —
    /// those are our own technical criterion, not a legal requirement.
    pub infraction_class: Option<InfractionClass>,
    /// Raw evidence: host IPs, drive paths, service ports, etc.
    pub evidence: Vec<String>,
    /// Whether this gap triggers the Art. 9° mandatory reporting obligation.
    pub requires_csirt_report: bool,
}

/// Gap severity levels, aligned with the compliance table.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Severity {
    /// Formats severity in Spanish for reports.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Medium   => write!(f, "Medio"),
            Severity::High     => write!(f, "Alto"),
            Severity::Critical => write!(f, "Crítico"),
        }
    }
}

/// Which institution tiers a gap is mandatory for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppliesTo {
    /// Mandatory for all tiers.
    All,
    /// Mandatory only for OIV and PSE.
    OivAndPse,
    /// Mandatory only for OIV.
    Oiv,
}

impl AppliesTo {
    /// Returns true if this gap is mandatory for the given tier.
    pub fn is_mandatory_for(&self, tier: Tier) -> bool {
        match (self, tier) {
            (AppliesTo::All, _)                              => true,
            (AppliesTo::OivAndPse, Tier::Oiv | Tier::Pse)   => true,
            (AppliesTo::Oiv, Tier::Oiv)                      => true,
            _                                                => false,
        }
    }

    /// Whether a control with this scope is legally binding on `tier`, or is
    /// only measured as voluntary maturity. See `Exigibilidad`.
    pub fn exigibilidad_for(&self, tier: Tier) -> Exigibilidad {
        if self.is_mandatory_for(tier) {
            Exigibilidad::Exigible
        } else {
            Exigibilidad::MadurezVoluntaria
        }
    }
}

// ---------------------------------------------------------------------------
// Scan config — top-level input to core::scan()
// ---------------------------------------------------------------------------

/// Serialisable scan metadata — stored in ScanResult and JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanMeta {
    pub institution_name: String,
    pub tier:             Tier,
    pub scope:            Scope,
}

/// Full scan config including the progress callback (not serialisable).
pub struct ScanConfig {
    pub institution_name: String,
    pub tier:             Tier,
    pub scope:            Scope,
    pub progress_cb:      Option<Box<dyn Fn(u8) + Send + Sync>>,
    pub log_cb:           Option<Box<dyn Fn(&str) + Send + Sync>>,
}

impl ScanConfig {
    pub fn report_progress(&self, pct: u8) {
        if let Some(cb) = &self.progress_cb { cb(pct); }
    }
    pub fn log(&self, msg: &str) {
        if let Some(cb) = &self.log_cb { cb(msg); }
    }
    pub fn meta(&self) -> ScanMeta {
        ScanMeta {
            institution_name: self.institution_name.clone(),
            tier:  self.tier,
            scope: self.scope,
        }
    }
}

// ---------------------------------------------------------------------------
// Scan result — top-level output of core::scan()
// ---------------------------------------------------------------------------

/// The complete output of a scan run.
#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResult {
    pub meta:        ScanMeta,
    pub asset_graph: AssetGraph,
    pub gaps:        Vec<Gap>,
    pub scanned_at:  DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_to_oiv_is_not_mandatory_for_unclassified() {
        assert!(!AppliesTo::Oiv.is_mandatory_for(Tier::Unclassified));
    }

    #[test]
    fn applies_to_all_is_mandatory_for_every_tier() {
        for tier in [Tier::Oiv, Tier::Pse, Tier::Unclassified] {
            assert!(AppliesTo::All.is_mandatory_for(tier));
        }
    }

    #[test]
    fn art8_is_voluntary_maturity_for_a_municipality() {
        // Una municipalidad es Pse: la Res. Ex. N°87 la excluyó del primer
        // proceso de calificación de OIV, así que el Art. 8° no la obliga.
        assert_eq!(
            AppliesTo::Oiv.exigibilidad_for(Tier::Pse),
            Exigibilidad::MadurezVoluntaria
        );
    }

    #[test]
    fn art7_and_art9_are_binding_on_a_municipality() {
        assert_eq!(AppliesTo::All.exigibilidad_for(Tier::Pse), Exigibilidad::Exigible);
        assert_eq!(AppliesTo::OivAndPse.exigibilidad_for(Tier::Pse), Exigibilidad::Exigible);
    }

    #[test]
    fn art8_is_binding_on_an_oiv() {
        assert_eq!(AppliesTo::Oiv.exigibilidad_for(Tier::Oiv), Exigibilidad::Exigible);
    }

    #[test]
    fn infraction_weights_follow_the_legal_scale() {
        assert_eq!(InfractionClass::Gravisima.score_weight(), 5);
        assert_eq!(InfractionClass::Grave.score_weight(), 3);
        assert_eq!(InfractionClass::Leve.score_weight(), 1);
    }

    #[test]
    fn max_utm_matches_articulo_40() {
        // Art. 40°: leves 5.000 / 10.000 OIV; graves 10.000 / 20.000 OIV;
        // gravísimas 20.000 / 40.000 OIV.
        assert_eq!(InfractionClass::Leve.max_utm(Tier::Pse),       5_000);
        assert_eq!(InfractionClass::Leve.max_utm(Tier::Oiv),      10_000);
        assert_eq!(InfractionClass::Grave.max_utm(Tier::Pse),     10_000);
        assert_eq!(InfractionClass::Grave.max_utm(Tier::Oiv),     20_000);
        assert_eq!(InfractionClass::Gravisima.max_utm(Tier::Pse), 20_000);
        assert_eq!(InfractionClass::Gravisima.max_utm(Tier::Oiv), 40_000);
    }

    #[test]
    fn severity_ordering_is_correct() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
    }

    #[test]
    fn scan_config_progress_noop_when_no_cb() {
        let cfg = ScanConfig {
            institution_name: "Municipalidad de Santiago".into(),
            tier:        Tier::Pse,
            scope:       Scope::Local,
            progress_cb: None,
            log_cb: None,
        };
        cfg.report_progress(50);
    }
}