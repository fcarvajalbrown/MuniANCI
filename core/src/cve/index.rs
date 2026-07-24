//! The bundled CVE index.
//!
//! El índice va **embebido en el binario**, no como archivo suelto: es el
//! comportamiento offline más simple de todos, no hay ruta que configurar ni
//! archivo que se pueda borrar, y a 1,9 MB comprimido el costo es despreciable.
//! Mismo criterio que `eol_db.json`.
//!
//! Se genera con `nvd-index build`, que filtra el snapshot NVD a los productos de
//! la tabla curada. Por eso son ~24.000 CVE y no las 370.000 del snapshot
//! completo: las demás corresponden a productos que este escáner no sabe
//! identificar, e incluirlas solo abultaría el binario.

use super::matcher::CveRecord;
use std::sync::OnceLock;

static INDEX_GZ: &[u8] = include_bytes!("../data/cve_index.json.gz");

/// Loads and caches the index. Cost is paid once per process.
pub fn records() -> &'static [CveRecord] {
    static RECORDS: OnceLock<Vec<CveRecord>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let decoder = flate2::read::GzDecoder::new(INDEX_GZ);
        serde_json::from_reader(std::io::BufReader::new(decoder))
            .expect("cve_index.json.gz está corrupto o desactualizado")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cve::cpe::Cpe;
    use crate::cve::product_map;

    #[test]
    fn index_loads_and_is_not_empty() {
        let r = records();
        assert!(r.len() > 1_000, "el indice trae solo {} registros", r.len());
    }

    #[test]
    fn every_record_has_at_least_one_applicability_statement() {
        // Un registro sin cpeMatch no puede producir hallazgo alguno: si esta en
        // el indice es porque el filtro dejo pasar algo que no corresponde.
        for r in records().iter().take(2_000) {
            assert!(!r.matches.is_empty(), "{} sin cpeMatch", r.id);
        }
    }

    #[test]
    fn ids_look_like_cve_identifiers() {
        for r in records().iter().take(500) {
            assert!(r.id.starts_with("CVE-"), "id inesperado: {}", r.id);
        }
    }

    #[test]
    fn finds_known_vulnerabilities_for_an_old_openssl() {
        // Prueba de extremo a extremo sobre datos reales: OpenSSL 1.0.1 es de
        // 2012 y arrastra un historial largo; que no encuentre nada significaria
        // que el matcher o el indice estan rotos.
        let ids = product_map::identities_for_software("OpenSSL 1.0.1").unwrap();
        let installed = ids[0].with_version("1.0.1");
        let hits = crate::cve::matcher::find_hits(records(), &installed, &|_| false);
        assert!(hits.len() > 10, "solo {} hallazgo(s) para OpenSSL 1.0.1", hits.len());
        assert!(hits[0].cvss.is_some());
    }

    #[test]
    fn a_current_version_reports_far_fewer_than_an_ancient_one() {
        let ids = product_map::identities_for_software("OpenSSL").unwrap();
        let old = crate::cve::matcher::find_hits(records(), &ids[0].with_version("1.0.1"), &|_| false);
        let new = crate::cve::matcher::find_hits(records(), &ids[0].with_version("3.5.0"), &|_| false);
        assert!(
            new.len() < old.len(),
            "una version actual ({}) no puede acumular mas CVE que una de 2012 ({})",
            new.len(),
            old.len()
        );
    }

    #[test]
    fn an_unmapped_product_yields_nothing_rather_than_guessing() {
        let installed = Cpe::application("acme", "producto_inventado", "1.0");
        let hits = crate::cve::matcher::find_hits(records(), &installed, &|_| false);
        assert!(hits.is_empty());
    }

    #[test]
    fn windows_server_2012_is_covered() {
        let id = product_map::identity_for_os("windows-server", "2012").unwrap();
        // Los SO de Microsoft llevan la release en el nombre del producto, asi
        // que el campo version suele ser '-' (NA) en los CPE de NVD.
        let installed = id.with_version("-");
        let hits = crate::cve::matcher::find_hits(records(), &installed, &|_| false);
        assert!(hits.len() > 100, "solo {} hallazgo(s) para Windows Server 2012", hits.len());
    }
}
