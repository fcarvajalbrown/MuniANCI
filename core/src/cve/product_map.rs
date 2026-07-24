//! Curated software-name -> CPE table.
//!
//! Traduce lo que reporta el sistema operativo ("Microsoft SQL Server 2014
//! (64-bit)") al CPE que NVD entiende. Reutiliza el `detect_product_slug()` que
//! ya existe en `eol_enrichment`, de modo que hay **una sola** lista de productos
//! reconocidos y no dos que se desincronicen.
//!
//! Ninguna entrada de la tabla fue escrita de memoria: todas se extrajeron del
//! snapshot NVD con `nvd-index catalog`, y cada una guarda el número de CVE que
//! la mencionaban al momento de extraerla, como evidencia auditable. El comando
//! `nvd-index validate` vuelve a comprobarlo contra los datos y falla si una
//! entrada dejó de existir.
//!
//! La búsqueda de esos nombres no fue trivial y justifica el método: nginx vive
//! bajo el vendor `f5`, no `nginx`; redis y elasticsearch existen bajo dos
//! vendors distintos a la vez; y Visual Studio codifica el año en el nombre del
//! producto, por lo que quedó deliberadamente fuera.

use super::cpe::Cpe;
use serde::Deserialize;
use std::collections::HashMap;

static CPE_MAP_JSON: &str = include_str!("../data/cpe_map.json");

/// One verified CPE identity for a product.
#[derive(Debug, Clone, Deserialize)]
pub struct CpeIdentity {
    pub part: String,
    pub vendor: String,
    pub product: String,
    /// CVE count observed when this entry was extracted — the audit trail that
    /// proves the identifier was read from the data rather than invented.
    #[serde(default)]
    pub cves_at_extraction: u64,
}

impl CpeIdentity {
    /// Builds the installed-product CPE to match against, for a given version.
    pub fn with_version(&self, version: &str) -> Cpe {
        Cpe {
            part: self.part.clone(),
            vendor: self.vendor.clone(),
            product: self.product.clone(),
            version: version.trim().to_ascii_lowercase(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawMap {
    products: HashMap<String, Vec<CpeIdentity>>,
    operating_systems: HashMap<String, HashMap<String, CpeIdentity>>,
}

fn load() -> &'static RawMap {
    use std::sync::OnceLock;
    static MAP: OnceLock<RawMap> = OnceLock::new();
    MAP.get_or_init(|| serde_json::from_str(CPE_MAP_JSON).expect("cpe_map.json está malformado"))
}

/// Returns the CPE identities for an installed application name, if it is one of
/// the curated products.
///
/// `None` significa "no evaluado", nunca "sin vulnerabilidades": la diferencia se
/// informa explícitamente al usuario vía [`super::Coverage`].
pub fn identities_for_software(name: &str) -> Option<&'static [CpeIdentity]> {
    let slug = crate::eol_enrichment::detect_product_slug(name)?;
    load().products.get(slug).map(|v| v.as_slice())
}

/// Returns the CPE identity for an operating system, keyed by the same release
/// cycle that `eol_enrichment` already derives from the build number.
pub fn identity_for_os(family_slug: &str, cycle: &str) -> Option<&'static CpeIdentity> {
    load().operating_systems.get(family_slug)?.get(cycle)
}

/// Every curated identity, for validation tooling.
pub fn all_identities() -> Vec<(String, &'static CpeIdentity)> {
    let map = load();
    let mut out: Vec<(String, &CpeIdentity)> = Vec::new();
    for (slug, ids) in &map.products {
        for id in ids {
            out.push((slug.clone(), id));
        }
    }
    for (family, cycles) in &map.operating_systems {
        for (cycle, id) in cycles {
            out.push((format!("{family}:{cycle}"), id));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_file_parses() {
        assert!(!load().products.is_empty());
        assert!(!load().operating_systems.is_empty());
    }

    #[test]
    fn resolves_a_real_windows_software_name() {
        let ids = identities_for_software("Microsoft SQL Server 2014 (64-bit)")
            .expect("SQL Server debe estar mapeado");
        assert_eq!(ids[0].vendor, "microsoft");
        assert_eq!(ids[0].product, "sql_server");
    }

    #[test]
    fn nginx_is_under_the_f5_vendor_not_nginx() {
        // El caso que justifica extraer los nombres en vez de recordarlos.
        let ids = identities_for_software("nginx 1.24").unwrap();
        assert_eq!(ids[0].vendor, "f5");
    }

    #[test]
    fn products_living_under_two_vendors_keep_both() {
        let redis = identities_for_software("Redis 7").unwrap();
        let vendors: Vec<&str> = redis.iter().map(|i| i.vendor.as_str()).collect();
        assert!(vendors.contains(&"redis") && vendors.contains(&"redislabs"), "{vendors:?}");
    }

    #[test]
    fn unknown_software_is_unmapped_not_clean() {
        // Devolver None significa "no evaluado". Que el informe lo diga es
        // responsabilidad de Coverage, pero aquí no se puede fingir cobertura.
        assert!(identities_for_software("Armoury Crate Service").is_none());
    }

    #[test]
    fn visual_studio_is_deliberately_unmapped() {
        // Su CPE codifica el año en el nombre del producto; mapearlo sin conocer
        // el año cruzaria CVE entre versiones.
        assert!(identities_for_software("Microsoft Visual Studio 2019").is_none());
    }

    #[test]
    fn os_cycles_match_the_eol_cycle_keys() {
        let id = identity_for_os("windows", "10-22H2").expect("cycle de eol_enrichment");
        assert_eq!(id.product, "windows_10_22h2");
        assert_eq!(id.part, "o");

        let srv = identity_for_os("windows-server", "2019").unwrap();
        assert_eq!(srv.product, "windows_server_2019");
    }

    #[test]
    fn every_entry_carries_its_extraction_evidence() {
        for (slug, id) in all_identities() {
            assert!(
                id.cves_at_extraction > 0,
                "{slug} ({}:{}) sin evidencia de extraccion: o se verifica contra el \
                 snapshot o no entra en la tabla",
                id.vendor,
                id.product
            );
        }
    }

    #[test]
    fn identity_builds_a_parseable_cpe() {
        let ids = identities_for_software("Google Chrome").unwrap();
        let cpe = ids[0].with_version("120.0.1");
        assert_eq!(cpe.to_string(), "cpe:2.3:a:google:chrome:120.0.1");
        assert!(Cpe::parse(&format!("{cpe}:*:*:*:*:*:*:*")).is_some());
    }
}
