//! CISA Known Exploited Vulnerabilities (KEV).
//!
//! Es la única señal del producto basada en **explotación observada** y no en
//! criticidad teórica: convierte "este equipo tiene 300 CVE" en "este equipo tiene
//! 4 CVE que se están explotando hoy". Por eso una CVE en KEV eleva la brecha a
//! `Critical` (ver `compliance_engine::check_known_cves`) y encabeza el plan de
//! remediación.
//!
//! ## De dónde sale el catálogo
//!
//! Se resuelve en tres pasos, y el primero que exista gana:
//!
//! 1. La ruta de `MUNIANI_KEV_FILE`.
//! 2. `known_exploited_vulnerabilities.json` junto al ejecutable.
//! 3. El snapshot embebido en el binario (`data/kev.json.gz`).
//!
//! Los dos primeros aceptan **el JSON tal cual lo publica CISA**, sin conversión:
//! el municipio descarga el archivo y lo deja en la carpeta. Esto importa porque
//! KEV se actualiza cada pocos días y un binario de hace seis meses, sin esta
//! salida, informaría un catálogo viejo sin decirlo. El informe declara siempre la
//! versión y el origen del catálogo efectivamente usado.
//!
//! ## Qué se conserva y qué no
//!
//! Se guardan identificador, producto, nombre, fecha de ingreso y uso en campañas
//! de ransomware. Se dejan fuera a propósito `requiredAction` y `dueDate`: esos
//! plazos obligan a agencias federales de EE.UU. por la BOD 26-04 y **no** a una
//! municipalidad chilena. Reproducirlos en un informe ANCI sugeriría un plazo legal
//! que no existe.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Variable de entorno que fuerza una ruta de catálogo.
pub const KEV_FILE_ENV: &str = "MUNIANI_KEV_FILE";

/// Nombre que CISA le da al archivo, y el que se busca junto al ejecutable.
pub const KEV_FILE_NAME: &str = "known_exploited_vulnerabilities.json";

/// Snapshot embebido, generado con `cargo run --release -p nvd-index -- kev`.
static EMBEDDED: &[u8] = include_bytes!("../data/kev.json.gz");

/// Una vulnerabilidad del catálogo, reducida a lo que el informe justifica.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KevEntry {
    pub cve_id: String,
    pub vendor_project: String,
    pub product: String,
    /// Nombre que le da CISA a la vulnerabilidad.
    pub name: String,
    /// Fecha de ingreso al catálogo, `YYYY-MM-DD`.
    pub date_added: String,
    /// CISA la ha visto usada en campañas de ransomware.
    pub ransomware: bool,
}

/// El catálogo completo, con la metadata que permite fecharlo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KevCatalogue {
    pub catalog_version: String,
    pub date_released: String,
    pub entries: Vec<KevEntry>,
}

/// De dónde se leyó el catálogo. Va al informe: un catálogo sustituido a mano
/// tiene que ser visible para quien audite el resultado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KevSource {
    /// El snapshot compilado dentro del binario.
    Embedded,
    /// Un archivo de CISA encontrado en disco.
    File(PathBuf),
    /// No se pudo leer ninguno: el enriquecimiento KEV queda declarado como ausente.
    Unavailable(String),
}

impl std::fmt::Display for KevSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KevSource::Embedded => write!(f, "snapshot embebido"),
            KevSource::File(p) => write!(f, "archivo {}", p.display()),
            KevSource::Unavailable(why) => write!(f, "no disponible ({why})"),
        }
    }
}

/// El catálogo cargado, con su índice de búsqueda.
#[derive(Debug)]
pub struct Kev {
    catalogue: KevCatalogue,
    index: HashMap<String, usize>,
    source: KevSource,
}

impl Kev {
    /// Builds the lookup index over a catalogue.
    pub fn new(catalogue: KevCatalogue, source: KevSource) -> Self {
        let index = catalogue
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.cve_id.to_ascii_uppercase(), i))
            .collect();
        Self { catalogue, index, source }
    }

    /// An empty catalogue, used when nothing could be read.
    pub fn unavailable(why: impl Into<String>) -> Self {
        Self::new(KevCatalogue::default(), KevSource::Unavailable(why.into()))
    }

    /// Whether a CVE id is in the catalogue.
    pub fn contains(&self, cve_id: &str) -> bool {
        self.index.contains_key(&cve_id.to_ascii_uppercase())
    }

    /// The full entry for a CVE id, when present.
    pub fn get(&self, cve_id: &str) -> Option<&KevEntry> {
        self.index
            .get(&cve_id.to_ascii_uppercase())
            .map(|&i| &self.catalogue.entries[i])
    }

    pub fn len(&self) -> usize {
        self.catalogue.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.catalogue.entries.is_empty()
    }

    pub fn catalog_version(&self) -> &str {
        &self.catalogue.catalog_version
    }

    pub fn date_released(&self) -> &str {
        &self.catalogue.date_released
    }

    pub fn source(&self) -> &KevSource {
        &self.source
    }

    /// One line describing the catalogue in use, for the report header.
    ///
    /// Se emite siempre, incluso cuando no hay catálogo: un informe que no dice
    /// con qué datos se evaluó la explotación activa no es auditable.
    pub fn provenance(&self) -> String {
        match &self.source {
            KevSource::Unavailable(why) => {
                format!("CISA KEV: sin catálogo ({why}) — no se evaluó explotación activa")
            }
            src => format!(
                "CISA KEV {} ({} vulnerabilidades, publicado {}) — {src}",
                self.catalogue.catalog_version,
                self.catalogue.entries.len(),
                self.catalogue.date_released,
            ),
        }
    }
}

/// The process-wide catalogue, resolved once.
pub fn catalogue() -> &'static Kev {
    static CACHE: OnceLock<Kev> = OnceLock::new();
    CACHE.get_or_init(load)
}

/// Resolves the catalogue: env override, then next to the executable, then embedded.
fn load() -> Kev {
    for path in override_paths() {
        if !path.exists() {
            continue;
        }
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| from_cisa_json(&t)) {
            Ok(cat) => return Kev::new(cat, KevSource::File(path)),
            // Un archivo puesto a mano que no se puede leer no debe degradar en
            // silencio al embebido: se avisa y se sigue con el embebido.
            Err(why) => eprintln!("[kev] {} ilegible: {why} — se usa el snapshot embebido", path.display()),
        }
    }
    match embedded() {
        Ok(cat) => Kev::new(cat, KevSource::Embedded),
        Err(why) => Kev::unavailable(why),
    }
}

/// Candidate override paths, in priority order.
fn override_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(p) = std::env::var(KEV_FILE_ENV) {
        if !p.trim().is_empty() {
            out.push(PathBuf::from(p));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join(KEV_FILE_NAME));
        }
    }
    out
}

/// Decompresses and parses the embedded snapshot.
fn embedded() -> Result<KevCatalogue, String> {
    use std::io::Read;
    let mut buf = String::new();
    flate2::read::GzDecoder::new(EMBEDDED)
        .read_to_string(&mut buf)
        .map_err(|e| format!("no se pudo descomprimir el snapshot embebido: {e}"))?;
    serde_json::from_str(&buf).map_err(|e| format!("snapshot embebido corrupto: {e}"))
}

// ---------------------------------------------------------------------------
// Formato original de CISA
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawCatalogue {
    #[serde(rename = "catalogVersion")]
    catalog_version: String,
    #[serde(rename = "dateReleased")]
    date_released: String,
    vulnerabilities: Vec<RawEntry>,
}

#[derive(Deserialize)]
struct RawEntry {
    #[serde(rename = "cveID")]
    cve_id: String,
    #[serde(default, rename = "vendorProject")]
    vendor_project: String,
    #[serde(default)]
    product: String,
    #[serde(default, rename = "vulnerabilityName")]
    vulnerability_name: String,
    #[serde(default, rename = "dateAdded")]
    date_added: String,
    #[serde(default, rename = "knownRansomwareCampaignUse")]
    known_ransomware_campaign_use: String,
}

/// Parses the JSON exactly as CISA publishes it.
///
/// Es la misma función que usa el conversor de build time, para que el archivo
/// externo y el snapshot embebido no puedan divergir en su interpretación.
pub fn from_cisa_json(text: &str) -> Result<KevCatalogue, String> {
    let raw: RawCatalogue = serde_json::from_str(text)
        .map_err(|e| format!("no tiene la forma del catálogo KEV de CISA: {e}"))?;

    let entries = raw
        .vulnerabilities
        .into_iter()
        .map(|v| KevEntry {
            cve_id: v.cve_id.to_ascii_uppercase(),
            vendor_project: v.vendor_project,
            product: v.product,
            name: v.vulnerability_name,
            // CISA usa "Known" / "Unknown" / "" — solo "Known" afirma el uso.
            ransomware: v.known_ransomware_campaign_use.eq_ignore_ascii_case("known"),
            date_added: v.date_added,
        })
        .collect();

    Ok(KevCatalogue {
        catalog_version: raw.catalog_version,
        date_released: raw.date_released,
        entries,
    })
}

/// Reads and parses a CISA catalogue from disk.
pub fn from_cisa_file(path: &Path) -> Result<KevCatalogue, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
    from_cisa_json(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "title": "CISA Catalog of Known Exploited Vulnerabilities",
      "catalogVersion": "2026.07.24",
      "dateReleased": "2026-07-24T17:40:56.0086Z",
      "count": 2,
      "vulnerabilities": [
        {
          "cveID": "CVE-2021-44228",
          "vendorProject": "Apache",
          "product": "Log4j2",
          "vulnerabilityName": "Apache Log4j2 Remote Code Execution Vulnerability",
          "dateAdded": "2021-12-10",
          "requiredAction": "Apply updates per vendor instructions.",
          "dueDate": "2021-12-24",
          "knownRansomwareCampaignUse": "Known"
        },
        {
          "cveID": "cve-2014-0160",
          "vendorProject": "OpenSSL",
          "product": "OpenSSL",
          "vulnerabilityName": "OpenSSL Information Disclosure Vulnerability",
          "dateAdded": "2022-05-04",
          "knownRansomwareCampaignUse": "Unknown"
        }
      ]
    }"#;

    fn sample() -> Kev {
        Kev::new(from_cisa_json(SAMPLE).unwrap(), KevSource::Embedded)
    }

    #[test]
    fn parses_the_catalogue_cisa_actually_publishes() {
        let cat = from_cisa_json(SAMPLE).unwrap();
        assert_eq!(cat.catalog_version, "2026.07.24");
        assert_eq!(cat.entries.len(), 2);
        assert_eq!(cat.entries[0].cve_id, "CVE-2021-44228");
        assert_eq!(cat.entries[0].product, "Log4j2");
    }

    #[test]
    fn ransomware_flag_only_when_cisa_says_known() {
        let cat = from_cisa_json(SAMPLE).unwrap();
        assert!(cat.entries[0].ransomware, "Log4Shell viene marcada Known");
        assert!(!cat.entries[1].ransomware, "Unknown no es una afirmacion");
    }

    #[test]
    fn lookup_is_case_insensitive_in_both_directions() {
        let kev = sample();
        assert!(kev.contains("CVE-2021-44228"));
        assert!(kev.contains("cve-2021-44228"));
        // La segunda entrada viene en minusculas en el origen.
        assert!(kev.contains("CVE-2014-0160"));
        assert_eq!(kev.get("cve-2014-0160").unwrap().vendor_project, "OpenSSL");
    }

    #[test]
    fn a_cve_outside_the_catalogue_is_not_claimed_as_exploited() {
        assert!(!sample().contains("CVE-1999-0001"));
    }

    #[test]
    fn provenance_names_the_version_and_the_origin() {
        let p = sample().provenance();
        assert!(p.contains("2026.07.24"), "{p}");
        assert!(p.contains("embebido"), "{p}");
    }

    #[test]
    fn an_unavailable_catalogue_says_so_instead_of_reporting_zero_exploited() {
        let kev = Kev::unavailable("prueba");
        assert!(kev.is_empty());
        assert!(kev.provenance().contains("no se evaluó explotación activa"), "{}", kev.provenance());
    }

    #[test]
    fn garbage_is_rejected_rather_than_parsed_into_an_empty_catalogue() {
        assert!(from_cisa_json("{\"nope\": 1}").is_err());
    }

    // El catalogo real que viaja en el binario. Si el snapshot embebido se rompe
    // o queda vacio, el producto dejaria de marcar explotacion activa en silencio.
    #[test]
    fn the_embedded_snapshot_is_real() {
        let kev = catalogue();
        assert!(kev.len() > 1_000, "solo {} entradas", kev.len());
        assert!(!kev.catalog_version().is_empty());
        assert!(
            kev.contains("CVE-2021-44228"),
            "Log4Shell tiene que estar en cualquier KEV valido"
        );
    }
}
