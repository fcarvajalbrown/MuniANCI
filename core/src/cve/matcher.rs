//! CPE -> CVE matching over the bundled NVD snapshot.
//!
//! Reimplementa en Rust el criterio de `cpe2cve` de nvdtools en vez de empaquetar
//! esa herramienta: es comparación de rangos de versión, y traer un runtime Go
//! adicional contradiría el principio de un solo binario del proyecto. La
//! herramienta original se conserva como oráculo de pruebas en desarrollo.
//!
//! Un `cpeMatch` de NVD acota la versión de dos maneras que hay que respetar a la
//! vez: el campo `version` del propio CPE (exacto, o `*`/`-` para "cualquiera") y
//! los cuatro límites opcionales `versionStart*`/`versionEnd*`.

use super::cpe::{compare_versions, Cpe, ANY};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Optional version bounds attached to a `cpeMatch`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRange {
    pub start_including: Option<String>,
    pub start_excluding: Option<String>,
    pub end_including: Option<String>,
    pub end_excluding: Option<String>,
}

impl VersionRange {
    /// Whether any bound is set at all.
    pub fn is_unbounded(&self) -> bool {
        self.start_including.is_none()
            && self.start_excluding.is_none()
            && self.end_including.is_none()
            && self.end_excluding.is_none()
    }

    /// Whether `version` falls inside the range.
    pub fn contains(&self, version: &str) -> bool {
        if let Some(v) = &self.start_including {
            if compare_versions(version, v) == Ordering::Less {
                return false;
            }
        }
        if let Some(v) = &self.start_excluding {
            if compare_versions(version, v) != Ordering::Greater {
                return false;
            }
        }
        if let Some(v) = &self.end_including {
            if compare_versions(version, v) == Ordering::Greater {
                return false;
            }
        }
        if let Some(v) = &self.end_excluding {
            if compare_versions(version, v) != Ordering::Less {
                return false;
            }
        }
        true
    }
}

/// One applicability statement from a CVE's `configurations` node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpeMatch {
    pub criteria: Cpe,
    #[serde(default)]
    pub range: VersionRange,
    /// NVD marks some CPEs as present-but-not-vulnerable (typically the platform
    /// half of an "A running on B" statement). Those must never produce a finding.
    pub vulnerable: bool,
}

impl CpeMatch {
    /// Whether this statement applies to an installed product.
    pub fn applies_to(&self, installed: &Cpe) -> bool {
        if !self.vulnerable {
            return false;
        }
        if !self.criteria.same_product_as(installed) {
            return false;
        }
        // An installed product with no usable version cannot be judged. Saying
        // "vulnerable" here would be inventing a fact about the machine.
        if installed.version.is_empty() || installed.version == ANY {
            return false;
        }

        // Hay productos cuya release va en el NOMBRE y no en el campo versión: los
        // sistemas operativos de Microsoft son el caso típico
        // (`windows_server_2012`, `windows_10_22h2`). Para esos se consulta con
        // versión `-`, y entonces la identidad del producto ya es la versión.
        let identity_carries_the_release = installed.version == "-";

        match self.criteria.version.as_str() {
            ANY | "-" if identity_carries_the_release => {
                // No hay versión que comparar: el nombre del producto identifica
                // exactamente la release. El enunciado es preciso, no vago.
                true
            }
            // `*` (ANY) o `-` (NA) delegan por completo en los límites de rango.
            //
            // Si tampoco hay límites, el enunciado no contiene NINGUNA información
            // de versión, y aquí se decide deliberadamente no afirmarlo. Tomado al
            // pie de la letra, NVD estaría diciendo "todas las versiones", pero eso
            // proviene de registros antiguos nunca enriquecidos: en una prueba real
            // esta rama le adjudicaba a Firefox 153 una CVE de 2007 con CVSS 10.0, y
            // a Office 2016 otra de 2007. Es el mismo criterio, simétrico, que ya se
            // aplica al producto instalado: sin versión no se juzga. Se prefiere
            // perder algún verdadero positivo antes que llenar de falsos un informe
            // que un municipio podría presentar ante la ANCI.
            ANY | "-" => !self.range.is_unbounded() && self.range.contains(&installed.version),
            exact => {
                // An exact version pinned in the CPE wins; the bounds, if any,
                // still have to agree.
                compare_versions(exact, &installed.version) == Ordering::Equal
                    && (self.range.is_unbounded() || self.range.contains(&installed.version))
            }
        }
    }
}

/// A vulnerability record, reduced to what the report needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CveRecord {
    pub id: String,
    /// CVSS base score, when NVD has one. Since April 2026 many records carry
    /// none, so this is genuinely optional and must not be defaulted to 0.
    pub cvss: Option<f32>,
    pub severity: Option<String>,
    pub description: Option<String>,
    /// Applicability statements, flattened from every `configurations` node.
    pub matches: Vec<CpeMatch>,
}

impl CveRecord {
    /// Whether this CVE affects the installed product.
    pub fn affects(&self, installed: &Cpe) -> bool {
        self.matches.iter().any(|m| m.applies_to(installed))
    }
}

/// A CVE found to affect a specific installed product.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CveHit {
    pub cve_id: String,
    pub cvss: Option<f32>,
    pub severity: Option<String>,
    /// True when the CVE is in CISA's Known Exploited Vulnerabilities catalogue —
    /// the only signal here based on observed exploitation rather than theory.
    pub known_exploited: bool,
    /// The CPE that produced the match, so a reviewer can audit the finding.
    pub matched_cpe: String,
}

/// Finds every CVE in `records` that affects `installed`.
///
/// Results are ordered by exploitation first and CVSS second: a vulnerability
/// that is actually being exploited outranks a theoretically worse one that is not.
pub fn find_hits(records: &[CveRecord], installed: &Cpe, kev: &dyn Fn(&str) -> bool) -> Vec<CveHit> {
    let mut hits: Vec<CveHit> = records
        .iter()
        .filter(|r| r.affects(installed))
        .map(|r| CveHit {
            cve_id: r.id.clone(),
            cvss: r.cvss,
            severity: r.severity.clone(),
            known_exploited: kev(&r.id),
            matched_cpe: installed.to_string(),
        })
        .collect();

    hits.sort_by(|a, b| {
        b.known_exploited
            .cmp(&a.known_exploited)
            .then_with(|| {
                b.cvss
                    .unwrap_or(-1.0)
                    .partial_cmp(&a.cvss.unwrap_or(-1.0))
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| a.cve_id.cmp(&b.cve_id))
    });
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(
        start_inc: Option<&str>,
        start_exc: Option<&str>,
        end_inc: Option<&str>,
        end_exc: Option<&str>,
    ) -> VersionRange {
        VersionRange {
            start_including: start_inc.map(String::from),
            start_excluding: start_exc.map(String::from),
            end_including: end_inc.map(String::from),
            end_excluding: end_exc.map(String::from),
        }
    }

    fn m(criteria: &str, r: VersionRange, vulnerable: bool) -> CpeMatch {
        CpeMatch {
            criteria: Cpe::parse(criteria).expect("CPE de prueba invalido"),
            range: r,
            vulnerable,
        }
    }

    fn installed(vendor: &str, product: &str, version: &str) -> Cpe {
        Cpe::application(vendor, product, version)
    }

    #[test]
    fn end_excluding_is_exclusive() {
        let cm = m(
            "cpe:2.3:a:apache:tomcat:*:*:*:*:*:*:*:*",
            range(Some("9.0.0"), None, None, Some("9.0.50")),
            true,
        );
        assert!(cm.applies_to(&installed("apache", "tomcat", "9.0.49")));
        assert!(!cm.applies_to(&installed("apache", "tomcat", "9.0.50")), "el limite excluyente no entra");
        assert!(!cm.applies_to(&installed("apache", "tomcat", "8.5.0")), "bajo el limite inferior");
    }

    #[test]
    fn end_including_is_inclusive() {
        let cm = m(
            "cpe:2.3:a:apache:tomcat:*:*:*:*:*:*:*:*",
            range(None, None, Some("9.0.50"), None),
            true,
        );
        assert!(cm.applies_to(&installed("apache", "tomcat", "9.0.50")));
        assert!(!cm.applies_to(&installed("apache", "tomcat", "9.0.51")));
    }

    #[test]
    fn start_excluding_is_exclusive() {
        let cm = m(
            "cpe:2.3:a:apache:tomcat:*:*:*:*:*:*:*:*",
            range(None, Some("9.0.0"), None, None),
            true,
        );
        assert!(!cm.applies_to(&installed("apache", "tomcat", "9.0.0")));
        assert!(cm.applies_to(&installed("apache", "tomcat", "9.0.1")));
    }

    #[test]
    fn non_vulnerable_statements_never_produce_a_finding() {
        let cm = m(
            "cpe:2.3:a:apache:tomcat:*:*:*:*:*:*:*:*",
            VersionRange::default(),
            false,
        );
        assert!(!cm.applies_to(&installed("apache", "tomcat", "9.0.1")));
    }

    #[test]
    fn exact_version_in_the_cpe_must_match_exactly() {
        let cm = m(
            "cpe:2.3:a:microsoft:sql_server:2014:*:*:*:*:*:*:*",
            VersionRange::default(),
            true,
        );
        assert!(cm.applies_to(&installed("microsoft", "sql_server", "2014")));
        assert!(!cm.applies_to(&installed("microsoft", "sql_server", "2016")));
    }

    #[test]
    fn unbounded_wildcard_is_never_asserted_against_a_specific_version() {
        // El enunciado no dice nada sobre versiones, asi que no puede sostener
        // que la instalada sea vulnerable. Es el caso que producia CVE de 2007
        // sobre Firefox 153 en una prueba real.
        let cm = m(
            "cpe:2.3:a:openssl:openssl:*:*:*:*:*:*:*:*",
            VersionRange::default(),
            true,
        );
        assert!(!cm.applies_to(&installed("openssl", "openssl", "1.0.2")));
        assert!(!cm.applies_to(&installed("openssl", "openssl", "3.5.0")));
    }

    #[test]
    fn a_wildcard_with_bounds_still_matches() {
        // Lo anterior no debe romper el caso normal y bien enriquecido.
        let cm = m(
            "cpe:2.3:a:openssl:openssl:*:*:*:*:*:*:*:*",
            range(None, None, None, Some("3.0.0")),
            true,
        );
        assert!(cm.applies_to(&installed("openssl", "openssl", "1.0.2")));
        assert!(!cm.applies_to(&installed("openssl", "openssl", "3.5.0")));
    }

    #[test]
    fn a_product_without_a_version_is_never_judged() {
        // No saber la version instalada no es lo mismo que ser vulnerable.
        let cm = m(
            "cpe:2.3:a:openssl:openssl:*:*:*:*:*:*:*:*",
            VersionRange::default(),
            true,
        );
        assert!(!cm.applies_to(&installed("openssl", "openssl", "")));
        assert!(!cm.applies_to(&installed("openssl", "openssl", "*")));
    }

    #[test]
    fn a_different_product_never_matches() {
        let cm = m(
            "cpe:2.3:a:apache:tomcat:*:*:*:*:*:*:*:*",
            VersionRange::default(),
            true,
        );
        assert!(!cm.applies_to(&installed("apache", "http_server", "2.4.1")));
    }

    fn record(id: &str, cvss: Option<f32>, matches: Vec<CpeMatch>) -> CveRecord {
        CveRecord {
            id: id.into(),
            cvss,
            severity: None,
            description: None,
            matches,
        }
    }

    #[test]
    fn known_exploited_outranks_a_higher_cvss() {
        let wildcard = || {
            m(
                "cpe:2.3:a:openssl:openssl:*:*:*:*:*:*:*:*",
                range(None, None, None, Some("3.0.0")),
                true,
            )
        };
        let records = vec![
            record("CVE-2000-0001", Some(9.8), vec![wildcard()]),
            record("CVE-2000-0002", Some(5.3), vec![wildcard()]),
        ];
        let kev = |id: &str| id == "CVE-2000-0002";
        let hits = find_hits(&records, &installed("openssl", "openssl", "1.1.1"), &kev);
        assert_eq!(hits[0].cve_id, "CVE-2000-0002", "lo explotado va primero");
        assert!(hits[0].known_exploited);
        assert_eq!(hits[1].cve_id, "CVE-2000-0001");
    }

    #[test]
    fn a_record_without_cvss_sorts_last_but_is_not_dropped() {
        // Desde el cambio de politica de NVD de 2026 muchas CVE no traen CVSS.
        // No tener puntaje no es lo mismo que no existir.
        let wildcard = || {
            m(
                "cpe:2.3:a:openssl:openssl:*:*:*:*:*:*:*:*",
                range(None, None, None, Some("3.0.0")),
                true,
            )
        };
        let records = vec![
            record("CVE-2000-0003", None, vec![wildcard()]),
            record("CVE-2000-0004", Some(4.0), vec![wildcard()]),
        ];
        let hits = find_hits(&records, &installed("openssl", "openssl", "1.1.1"), &|_| false);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].cve_id, "CVE-2000-0004");
        assert_eq!(hits[1].cve_id, "CVE-2000-0003");
    }

    #[test]
    fn no_hits_when_nothing_applies() {
        let records = vec![record(
            "CVE-2000-0005",
            Some(7.0),
            vec![m(
                "cpe:2.3:a:apache:tomcat:*:*:*:*:*:*:*:*",
                range(None, None, None, Some("10.0.0")),
                true,
            )],
        )];
        let hits = find_hits(&records, &installed("openssl", "openssl", "1.1.1"), &|_| false);
        assert!(hits.is_empty());
    }
}
