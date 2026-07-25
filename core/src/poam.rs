//! Prioritised remediation plan, exported as an OSCAL Plan of Action and Milestones.
//!
//! ## Por qué OSCAL y no SCAP/XCCDF/OVAL
//!
//! Porque el modelo POA&M **es** el entregable: un plan de acción con hallazgos,
//! riesgos y plazos. OVAL habría exigido escribir una definición formal de test por
//! control, y no se identificó ningún consumidor de SCAP en la cadena ANCI.
//!
//! ## Qué se emite
//!
//! El POA&M. Assessment Results queda pendiente: ese modelo describe una evaluación
//! formal conducida contra un System Security Plan que este producto todavía no
//! genera, así que emitirlo hoy obligaría a apuntar a documentos inexistentes.
//!
//! ## Cómo se ordena
//!
//! De mayor a menor peso: **(1)** CVE presente en el catálogo KEV de CISA, **(2)**
//! calificación legal de la infracción según el Art. 39°, **(3)** severidad técnica
//! del hallazgo, **(4)** CVSS. Poner KEV primero es defendible: es la única señal
//! basada en explotación observada y no en criticidad teórica. Ordenar por la multa
//! primero ordenaría por miedo a la sanción, no por riesgo.
//!
//! ## Los plazos no son plazos legales
//!
//! El único plazo perentorio del régimen es el del reporte al CSIRT del Art. 9°.
//! Los de aquí son un criterio operativo, configurable por TI municipal
//! ([`crate::config::PoamConfig`]), y cada ítem lo dice.
//!
//! ## Estructura
//!
//! Verificada contra la referencia JSON de NIST y el ejemplo oficial del repo
//! `usnistgov/oscal-content` (OSCAL 1.2.2), no reconstruida de memoria. Un dato que
//! importa: `deadline` vive en el **risk**, no en el `poam-item`.

use crate::config::PoamConfig;
use crate::types::{Exigibilidad, Gap, InfractionClass, ScanResult, Severity};
use serde::Serialize;
use uuid::Uuid;

/// Versión de OSCAL contra la que se emite. Debe coincidir con `oscal-version`.
pub const OSCAL_VERSION: &str = "1.2.2";

/// Espacio de nombres de MuniANCI para UUID v5.
///
/// Arbitrario y propio de la aplicación, como prevé el RFC 9562 para v5. Lo que
/// importa no es el valor sino que sea **fijo**: así el mismo hallazgo conserva su
/// identificador entre escaneos y el histórico por comuna puede seguirlo.
const NAMESPACE: Uuid = Uuid::from_u128(0xdd2a9eac_e3d5_4759_8b74_41c50d4cfe17);

/// Aviso que acompaña a cada plazo sugerido.
const AVISO_PLAZO: &str =
    "Plazo sugerido por criterio operativo, configurable por el área de TI. No es un plazo \
     legal: el único plazo perentorio de la Ley 21.663 es el reporte al CSIRT del Art. 9°.";

// ---------------------------------------------------------------------------
// Priorización
// ---------------------------------------------------------------------------

/// One line of the remediation plan, before serialisation.
#[derive(Debug, Clone)]
pub struct Item<'a> {
    pub gap: &'a Gap,
    /// Posición en el plan, empezando en 1.
    pub orden: usize,
    /// Días sugeridos para corregir.
    pub plazo_dias: u32,
    /// Por qué quedó en esa posición, en lenguaje llano.
    pub motivo: String,
}

/// Sort key: lower is more urgent.
///
/// Se devuelve una tupla en vez de comparar a mano para que el orden sea total y
/// obviamente estable: cualquier empate lo rompe el nombre del control.
fn prioridad(gap: &Gap) -> (u8, u8, u8, u8, i32, String) {
    let kev = if tiene_kev(gap) { 0 } else { 1 };

    // Art. 39°: gravísima pesa más que grave, y grave más que leve. Una brecha de
    // madurez voluntaria no acarrea consecuencia legal, así que no compite acá.
    let legal = match (gap.exigibilidad, gap.infraction_class) {
        (Exigibilidad::Exigible, Some(InfractionClass::Gravisima)) => 0,
        (Exigibilidad::Exigible, Some(InfractionClass::Grave)) => 1,
        (Exigibilidad::Exigible, Some(InfractionClass::Leve)) => 2,
        (Exigibilidad::Exigible, None) => 3,
        (Exigibilidad::MadurezVoluntaria, _) => 4,
    };

    // Desempate dentro de la misma clase legal: lo que se observó va antes que lo
    // que solo quedó sin responder. Ambas cosas son brechas, pero una pide
    // corregir y la otra pide averiguar.
    let verificado = if gap.evaluated { 0 } else { 1 };

    let severidad = match gap.severity {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
    };

    // CVSS descendente: se niega para que "mayor CVSS" sea "menor clave".
    let cvss = -(peor_cvss(gap).unwrap_or(0.0) * 10.0) as i32;

    (kev, legal, verificado, severidad, cvss, gap.control.clone())
}

/// Whether the gap's evidence mentions an actively exploited vulnerability.
fn tiene_kev(gap: &Gap) -> bool {
    gap.evidence.iter().any(|e| e.contains("explotada(s) activamente"))
}

/// Highest CVSS quoted in the gap's evidence, when there is one.
fn peor_cvss(gap: &Gap) -> Option<f32> {
    gap.evidence
        .iter()
        .filter_map(|e| {
            let i = e.find("peor CVSS ")? + "peor CVSS ".len();
            e[i..]
                .split(|c: char| !c.is_ascii_digit() && c != '.')
                .next()?
                .parse::<f32>()
                .ok()
        })
        .fold(None, |acc, c| Some(acc.map_or(c, |a: f32| a.max(c))))
}

/// Builds the ordered plan.
pub fn plan<'a>(gaps: &'a [Gap], config: &PoamConfig) -> Vec<Item<'a>> {
    let mut ordenados: Vec<&Gap> = gaps.iter().collect();
    ordenados.sort_by_key(|g| prioridad(g));

    ordenados
        .into_iter()
        .enumerate()
        .map(|(i, gap)| Item {
            orden: i + 1,
            plazo_dias: config.plazo_dias(gap.severity),
            motivo: motivo(gap),
            gap,
        })
        .collect()
}

/// Plain-language reason for the item's position.
///
/// Un ítem que viene de una pregunta sin responder lo dice. Sin esa marca, el plan
/// se lee como si el escáner hubiera **encontrado** algo, cuando en realidad nadie
/// contestó: lo primero que hay que hacer ahí es responder, no remediar.
fn motivo(gap: &Gap) -> String {
    if !gap.evaluated {
        return "no se respondió esta pregunta del cuestionario: primero hay que verificarla".into();
    }
    if tiene_kev(gap) {
        return "hay vulnerabilidades explotándose en la práctica sobre estos activos".into();
    }
    match (gap.exigibilidad, gap.infraction_class) {
        (Exigibilidad::MadurezVoluntaria, _) => {
            "mejora de madurez; no es exigible a esta institución".into()
        }
        (_, Some(c)) => format!(
            "incumplimiento exigible, clasificado como infracción {} (Art. 39°)",
            c.to_string().to_lowercase()
        ),
        (_, None) => format!(
            "hallazgo técnico de severidad {}",
            gap.severity.to_string().to_lowercase()
        ),
    }
}

// ---------------------------------------------------------------------------
// Serialización OSCAL
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Documento {
    #[serde(rename = "plan-of-action-and-milestones")]
    poam: Poam,
}

#[derive(Serialize)]
struct Poam {
    uuid: Uuid,
    metadata: Metadata,
    #[serde(rename = "system-id")]
    system_id: SystemId,
    observations: Vec<Observation>,
    risks: Vec<Risk>,
    #[serde(rename = "poam-items")]
    poam_items: Vec<PoamItem>,
}

#[derive(Serialize)]
struct Metadata {
    title: String,
    published: String,
    #[serde(rename = "last-modified")]
    last_modified: String,
    version: String,
    #[serde(rename = "oscal-version")]
    oscal_version: String,
    remarks: String,
}

#[derive(Serialize)]
struct SystemId {
    #[serde(rename = "identifier-type")]
    identifier_type: String,
    id: String,
}

#[derive(Serialize)]
struct Observation {
    uuid: Uuid,
    title: String,
    description: String,
    /// EXAMINE / INTERVIEW / TEST, según cómo se obtuvo el dato.
    methods: Vec<String>,
    collected: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    remarks: Option<String>,
}

#[derive(Serialize)]
struct Risk {
    uuid: Uuid,
    title: String,
    description: String,
    statement: String,
    status: String,
    deadline: String,
    #[serde(rename = "related-observations")]
    related_observations: Vec<RelatedObservation>,
    remarks: String,
}

#[derive(Serialize)]
struct RelatedObservation {
    #[serde(rename = "observation-uuid")]
    observation_uuid: Uuid,
}

#[derive(Serialize)]
struct RelatedRisk {
    #[serde(rename = "risk-uuid")]
    risk_uuid: Uuid,
}

#[derive(Serialize)]
struct PoamItem {
    uuid: Uuid,
    title: String,
    description: String,
    #[serde(rename = "related-observations")]
    related_observations: Vec<RelatedObservation>,
    #[serde(rename = "related-risks")]
    related_risks: Vec<RelatedRisk>,
}

/// Stable identifier for a gap, so it survives across scans.
fn uuid_de(prefijo: &str, gap: &Gap) -> Uuid {
    Uuid::new_v5(&NAMESPACE, format!("{prefijo}:{}", gap.control).as_bytes())
}

/// Serialises the plan as an OSCAL POA&M document.
pub fn to_oscal(result: &ScanResult, config: &PoamConfig) -> serde_json::Value {
    let items = plan(&result.gaps, config);
    let momento = result.scanned_at.to_rfc3339();

    let observations: Vec<Observation> = items
        .iter()
        .map(|i| Observation {
            uuid: uuid_de("obs", i.gap),
            title: i.gap.control.clone(),
            description: i.gap.finding.clone(),
            methods: vec![metodo(i.gap).into()],
            collected: momento.clone(),
            remarks: if i.gap.evidence.is_empty() {
                None
            } else {
                Some(i.gap.evidence.join("; "))
            },
        })
        .collect();

    let risks: Vec<Risk> = items
        .iter()
        .map(|i| Risk {
            uuid: uuid_de("risk", i.gap),
            title: i.gap.control.clone(),
            description: i.gap.finding.clone(),
            statement: i.gap.legal_anchor.clone(),
            // "open" es el estado inicial: nada se ha remediado todavia.
            status: "open".into(),
            deadline: (result.scanned_at + chrono::Duration::days(i.plazo_dias as i64))
                .to_rfc3339(),
            related_observations: vec![RelatedObservation {
                observation_uuid: uuid_de("obs", i.gap),
            }],
            remarks: AVISO_PLAZO.into(),
        })
        .collect();

    let poam_items: Vec<PoamItem> = items
        .iter()
        .map(|i| PoamItem {
            uuid: uuid_de("item", i.gap),
            title: format!("{}. {}", i.orden, i.gap.control),
            description: format!(
                "{} Prioridad {} de {}: {}. Plazo sugerido: {} dias.",
                i.gap.finding,
                i.orden,
                items.len(),
                i.motivo,
                i.plazo_dias
            ),
            related_observations: vec![RelatedObservation {
                observation_uuid: uuid_de("obs", i.gap),
            }],
            related_risks: vec![RelatedRisk {
                risk_uuid: uuid_de("risk", i.gap),
            }],
        })
        .collect();

    let doc = Documento {
        poam: Poam {
            uuid: Uuid::new_v5(
                &NAMESPACE,
                format!("poam:{}:{}", result.meta.institution_name, momento).as_bytes(),
            ),
            metadata: Metadata {
                title: format!(
                    "Plan de remediación — {} (Ley 21.663)",
                    result.meta.institution_name
                ),
                published: momento.clone(),
                last_modified: momento.clone(),
                version: env!("CARGO_PKG_VERSION").into(),
                oscal_version: OSCAL_VERSION.into(),
                remarks: format!(
                    "Generado por MuniANCI v{}. Orden de prioridad: explotación observada (CISA KEV), \
                     luego calificación de la infracción (Art. 39° Ley 21.663), luego severidad \
                     técnica y por último CVSS. {AVISO_PLAZO}",
                    env!("CARGO_PKG_VERSION")
                ),
            },
            system_id: SystemId {
                identifier_type: "https://ietf.org/rfc/rfc4122".into(),
                id: Uuid::new_v5(&NAMESPACE, result.meta.institution_name.as_bytes()).to_string(),
            },
            observations,
            risks,
            poam_items,
        },
    };

    serde_json::to_value(doc).expect("el documento OSCAL siempre serializa")
}

/// The OSCAL assessment method that produced the finding.
///
/// El cuestionario es autorreporte del operador: en OSCAL eso es `INTERVIEW`, no
/// `TEST`. La distinción es exactamente la que el informe ya hace entre evidencia
/// observada y lo declarado.
fn metodo(gap: &Gap) -> &'static str {
    if gap.evidence.iter().any(|e| e.starts_with("No respondido")) || !gap.evaluated {
        "INTERVIEW"
    } else if gap.domain == crate::maturity::Domain::MedidasPermanentes {
        "TEST"
    } else {
        "INTERVIEW"
    }
}

/// Writes the POA&M to disk.
pub fn write(result: &ScanResult, config: &PoamConfig, path: &std::path::Path) -> anyhow::Result<()> {
    let json = to_oscal(result, config);
    std::fs::write(path, serde_json::to_string_pretty(&json)? + "\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maturity::Domain;
    use crate::types::{AppliesTo, AssetGraph, ScanMeta, Scope, Tier};
    use chrono::Utc;

    fn gap(
        control: &str,
        severity: Severity,
        exigibilidad: Exigibilidad,
        infraction_class: Option<InfractionClass>,
        evidence: Vec<&str>,
    ) -> Gap {
        Gap {
            control: control.into(),
            finding: format!("hallazgo de {control}"),
            severity,
            legal_anchor: "Art. 7° Ley 21.663".into(),
            applies_to: AppliesTo::All,
            exigibilidad,
            infraction_class,
            domain: Domain::MedidasPermanentes,
            evaluated: true,
            evidence: evidence.into_iter().map(String::from).collect(),
            requires_csirt_report: false,
        }
    }

    fn result_con(gaps: Vec<Gap>) -> ScanResult {
        ScanResult {
            meta: ScanMeta {
                institution_name: "Municipalidad de Prueba".into(),
                tier: Tier::Pse,
                scope: Scope::Local,
            },
            asset_graph: AssetGraph::default(),
            maturity: crate::maturity::MaturityProfile::from_gaps(&gaps, &[]),
            ley21180: None,
            score: crate::scoring::ComplianceScore::from_gaps(&gaps),
            gaps,
            cve_coverage: crate::cve::Coverage::default(),
            kev_provenance: "prueba".into(),
            taxonomia_anci: crate::taxonomia::TaxonomiaAnci::default(),
            delta: None,
            deriva: None,
            scanned_at: Utc::now(),
        }
    }

    fn orden<'a>(items: &'a [Item<'a>]) -> Vec<&'a str> {
        items.iter().map(|i| i.gap.control.as_str()).collect()
    }

    // El criterio del hito: lo que se esta explotando hoy va primero, aunque otra
    // brecha tenga peor calificacion legal.
    #[test]
    fn an_actively_exploited_finding_leads_the_plan() {
        let gaps = vec![
            gap("Sin SGSI", Severity::Critical, Exigibilidad::Exigible,
                Some(InfractionClass::Gravisima), vec![]),
            gap("Vulnerabilidades conocidas (CVE)", Severity::High, Exigibilidad::Exigible, None,
                vec!["Firefox 1.0 — 3 CVE (peor CVSS 8.1), 2 explotada(s) activamente"]),
        ];
        let items = plan(&gaps, &PoamConfig::default());
        assert_eq!(orden(&items)[0], "Vulnerabilidades conocidas (CVE)");
        assert!(items[0].motivo.contains("explotándose"), "{}", items[0].motivo);
    }

    #[test]
    fn without_kev_the_legal_classification_decides() {
        let gaps = vec![
            gap("Leve", Severity::Critical, Exigibilidad::Exigible, Some(InfractionClass::Leve), vec![]),
            gap("Gravisima", Severity::Medium, Exigibilidad::Exigible, Some(InfractionClass::Gravisima), vec![]),
        ];
        assert_eq!(orden(&plan(&gaps, &PoamConfig::default()))[0], "Gravisima");
    }

    // Una brecha de madurez no acarrea consecuencia legal: no puede desplazar a un
    // incumplimiento exigible, por grave que parezca su severidad tecnica.
    #[test]
    fn voluntary_maturity_gaps_close_the_plan() {
        let gaps = vec![
            gap("Madurez", Severity::Critical, Exigibilidad::MadurezVoluntaria, None, vec![]),
            gap("Exigible", Severity::Medium, Exigibilidad::Exigible, None, vec![]),
        ];
        let items = plan(&gaps, &PoamConfig::default());
        assert_eq!(orden(&items), vec!["Exigible", "Madurez"]);
        assert!(items[1].motivo.contains("no es exigible"), "{}", items[1].motivo);
    }

    #[test]
    fn cvss_breaks_the_tie_between_equivalent_findings() {
        let gaps = vec![
            gap("Bajo", Severity::High, Exigibilidad::Exigible, None,
                vec!["x 1.0 — 1 CVE (peor CVSS 5.2)"]),
            gap("Alto", Severity::High, Exigibilidad::Exigible, None,
                vec!["x 1.0 — 1 CVE (peor CVSS 9.8)"]),
        ];
        assert_eq!(orden(&plan(&gaps, &PoamConfig::default()))[0], "Alto");
    }

    #[test]
    fn the_order_is_stable_whatever_the_input_order() {
        let a = gap("Alfa", Severity::High, Exigibilidad::Exigible, None, vec![]);
        let b = gap("Beta", Severity::High, Exigibilidad::Exigible, None, vec![]);
        let directo = [a.clone(), b.clone()];
        let invertido = [b, a];
        let uno = plan(&directo, &PoamConfig::default());
        let otro = plan(&invertido, &PoamConfig::default());
        assert_eq!(orden(&uno), orden(&otro));
    }

    // Dentro de la misma clase legal, lo observado va antes que lo que nadie
    // respondio: una cosa se corrige, la otra primero hay que averiguarla.
    #[test]
    fn a_verified_finding_outranks_an_unanswered_question_of_the_same_class() {
        let mut sin_responder = gap("Pregunta", Severity::Critical, Exigibilidad::Exigible,
            Some(InfractionClass::Grave), vec![]);
        sin_responder.evaluated = false;
        let observado = gap("Hallazgo", Severity::Medium, Exigibilidad::Exigible,
            Some(InfractionClass::Grave), vec!["10.0.0.1"]);

        let gaps = [sin_responder, observado];
        let items = plan(&gaps, &PoamConfig::default());
        assert_eq!(orden(&items), vec!["Hallazgo", "Pregunta"]);
        assert!(items[1].motivo.contains("no se respondió"), "{}", items[1].motivo);
    }

    // ...pero la explotacion observada sigue mandando sobre todo lo demas.
    #[test]
    fn kev_still_outranks_a_verified_grave_finding() {
        let grave = gap("Grave", Severity::Critical, Exigibilidad::Exigible,
            Some(InfractionClass::Grave), vec!["10.0.0.1"]);
        let kev = gap("CVE", Severity::Medium, Exigibilidad::Exigible, None,
            vec!["x 1.0 — 1 CVE (peor CVSS 5.0), 1 explotada(s) activamente"]);
        let gaps = [grave, kev];
        assert_eq!(orden(&plan(&gaps, &PoamConfig::default()))[0], "CVE");
    }

    #[test]
    fn deadlines_come_from_the_configuration() {
        let cfg = PoamConfig { plazo_dias_critica: 3, plazo_dias_alta: 7, plazo_dias_media: 200 };
        let gaps = vec![gap("C", Severity::Critical, Exigibilidad::Exigible, None, vec![])];
        assert_eq!(plan(&gaps, &cfg)[0].plazo_dias, 3);
    }

    #[test]
    fn cvss_is_read_out_of_the_evidence() {
        let g = gap("x", Severity::High, Exigibilidad::Exigible, None,
            vec!["a 1.0 — 2 CVE (peor CVSS 7.5)", "b 2.0 — 1 CVE (peor CVSS 9.1)"]);
        assert_eq!(peor_cvss(&g), Some(9.1));
        assert_eq!(peor_cvss(&gap("y", Severity::High, Exigibilidad::Exigible, None, vec![])), None);
    }

    // ---------------------------------------------------------------------
    // Estructura OSCAL
    // ---------------------------------------------------------------------

    fn documento() -> serde_json::Value {
        let gaps = vec![
            gap("Vulnerabilidades conocidas (CVE)", Severity::Critical, Exigibilidad::Exigible, None,
                vec!["Firefox 1.0 — 3 CVE (peor CVSS 8.1), 2 explotada(s) activamente"]),
            gap("Sin SGSI", Severity::High, Exigibilidad::Exigible, Some(InfractionClass::Grave), vec![]),
        ];
        to_oscal(&result_con(gaps), &PoamConfig::default())
    }

    #[test]
    fn the_document_has_the_shape_oscal_requires() {
        let d = documento();
        let poam = &d["plan-of-action-and-milestones"];
        assert!(poam.is_object(), "falta la raiz plan-of-action-and-milestones");
        assert!(poam["uuid"].is_string(), "uuid es obligatorio en la raiz");
        let meta = &poam["metadata"];
        for campo in ["title", "last-modified", "version", "oscal-version"] {
            assert!(meta[campo].is_string(), "metadata.{campo} es obligatorio");
        }
        assert_eq!(meta["oscal-version"], OSCAL_VERSION);
        assert_eq!(poam["poam-items"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn every_item_points_at_its_observation_and_its_risk() {
        let d = documento();
        let poam = &d["plan-of-action-and-milestones"];
        let obs: Vec<&str> = poam["observations"].as_array().unwrap().iter()
            .map(|o| o["uuid"].as_str().unwrap()).collect();
        let risks: Vec<&str> = poam["risks"].as_array().unwrap().iter()
            .map(|r| r["uuid"].as_str().unwrap()).collect();

        for item in poam["poam-items"].as_array().unwrap() {
            let o = item["related-observations"][0]["observation-uuid"].as_str().unwrap();
            let r = item["related-risks"][0]["risk-uuid"].as_str().unwrap();
            assert!(obs.contains(&o), "referencia colgada a observacion {o}");
            assert!(risks.contains(&r), "referencia colgada a riesgo {r}");
        }
    }

    // El plazo va en el risk, no en el poam-item: verificado contra el ejemplo
    // oficial de usnistgov/oscal-content.
    #[test]
    fn the_deadline_lives_on_the_risk_and_says_it_is_not_legal() {
        let d = documento();
        let risk = &d["plan-of-action-and-milestones"]["risks"][0];
        assert!(risk["deadline"].is_string());
        assert_eq!(risk["status"], "open");
        assert!(risk["remarks"].as_str().unwrap().contains("No es un plazo legal"));
    }

    #[test]
    fn the_first_item_is_the_one_being_exploited() {
        let d = documento();
        let primero = d["plan-of-action-and-milestones"]["poam-items"][0]["title"]
            .as_str().unwrap().to_string();
        assert!(primero.starts_with("1. Vulnerabilidades conocidas"), "{primero}");
    }

    // El historico por comuna necesita seguir el mismo hallazgo entre escaneos.
    #[test]
    fn the_same_finding_keeps_its_uuid_across_scans() {
        let uno = documento();
        let otro = documento();
        assert_eq!(
            uno["plan-of-action-and-milestones"]["poam-items"][0]["uuid"],
            otro["plan-of-action-and-milestones"]["poam-items"][0]["uuid"],
        );
    }

    #[test]
    fn a_scan_with_no_gaps_still_produces_a_valid_document() {
        let d = to_oscal(&result_con(vec![]), &PoamConfig::default());
        let poam = &d["plan-of-action-and-milestones"];
        assert!(poam["uuid"].is_string());
        assert_eq!(poam["poam-items"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn a_declarative_control_is_recorded_as_an_interview_not_a_test() {
        let mut g = gap("Sin SGSI", Severity::High, Exigibilidad::Exigible, None, vec![]);
        g.domain = Domain::GobernanzaSgsi;
        assert_eq!(metodo(&g), "INTERVIEW");

        let tecnico = gap("Firewall", Severity::High, Exigibilidad::Exigible, None, vec!["10.0.0.1"]);
        assert_eq!(metodo(&tecnico), "TEST");
    }
}
