//! Maturity level 0-3 per compliance domain.
//!
//! ## Qué aporta sobre el pasa/no pasa que ya existe
//!
//! El puntaje agregado ([`crate::scoring`]) dice cuán lejos está la institución del
//! cumplimiento. No dice **dónde**. Un 82/100 puede ser cinco dominios sanos y uno
//! roto, o seis dominios mediocres, y el plan de remediación que corresponde a cada
//! caso no se parece en nada.
//!
//! ## De dónde salen los dominios
//!
//! De los deberes que la ley ya distingue, no de un marco importado. Cada dominio se
//! ancla a un artículo que el producto cita y verificó; ninguno obliga a inventar una
//! categoría. Los dos del Art. 8° son justamente los que una municipalidad **no está
//! obligada** a cumplir pero sí puede medirse voluntariamente: el nivel de madurez
//! expresa eso sin afirmar un incumplimiento legal que no existe.
//!
//! ## De dónde sale la forma
//!
//! La estructura de cuatro niveles por dominio está tomada del **Essential Eight
//! Maturity Model** del Australian Signals Directorate, publicado bajo CC BY 4.0.
//! Se adapta la forma, no los controles: los ocho del ASD no son los deberes de la
//! Ley 21.663. La atribución es condición de la licencia y se emite en el informe
//! (ver [`ESSENTIAL_EIGHT_ATTRIBUTION`]).
//!
//! ## Por qué manda la peor brecha y no el promedio
//!
//! El nivel de un dominio lo fija su peor brecha abierta. Un share SMB accesible sin
//! credenciales no se compensa con diez controles cumplidos: quien entra por ahí no
//! se detiene a promediar. Un criterio proporcional habría dejado ese dominio en
//! nivel 2.
//!
//! ## "No medido" no es nivel 0
//!
//! Un dominio del que no se recogió ningún dato queda fuera del promedio y se informa
//! como no medido. Es la misma regla que gobierna la cobertura CVE: poner 0 sería
//! afirmar un incumplimiento que nadie verificó.

use crate::types::{Exigibilidad, Gap, Severity};
use serde::{Deserialize, Serialize};

/// Atribución exigida por la licencia CC BY 4.0 del modelo del ASD.
pub const ESSENTIAL_EIGHT_ATTRIBUTION: &str =
    "La escala de madurez 0-3 por dominio adapta la forma del Essential Eight Maturity Model \
     del Australian Signals Directorate, usado bajo licencia CC BY 4.0. Los dominios y \
     controles son propios y se anclan en la Ley 21.663; el ASD no avala este producto.";

/// The compliance domains, each anchored to a duty in the law.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    /// Inscripción y datos de contacto ante la ANCI.
    RegistroAnci,
    /// Deber de reportar incidentes al CSIRT Nacional.
    ReporteIncidentes,
    /// Medidas permanentes de prevención — donde caen los hallazgos del escáner.
    MedidasPermanentes,
    /// Continuidad operacional y respaldo.
    Continuidad,
    /// Gobernanza, SGSI, capacitación y delegado.
    GobernanzaSgsi,
}

impl Domain {
    /// Every domain, in report order.
    pub fn all() -> [Domain; 5] {
        [
            Domain::RegistroAnci,
            Domain::ReporteIncidentes,
            Domain::MedidasPermanentes,
            Domain::Continuidad,
            Domain::GobernanzaSgsi,
        ]
    }

    /// Human-readable name for the report.
    pub fn title(self) -> &'static str {
        match self {
            Domain::RegistroAnci => "Registro y contacto con la ANCI",
            Domain::ReporteIncidentes => "Reporte de incidentes",
            Domain::MedidasPermanentes => "Medidas permanentes e higiene técnica",
            Domain::Continuidad => "Continuidad operacional y respaldo",
            Domain::GobernanzaSgsi => "Gobernanza y SGSI",
        }
    }

    /// The duty this domain measures. Se cita en el informe junto al nivel.
    pub fn legal_anchor(self) -> &'static str {
        match self {
            Domain::RegistroAnci => "Art. 9° Ley 21.663 e IG N°1 ANCI",
            Domain::ReporteIncidentes => "Art. 9° Ley 21.663 y DS N°295 de 2024",
            Domain::MedidasPermanentes => "Art. 7° Ley 21.663",
            Domain::Continuidad => "Art. 8° lit. c) y Art. 28° Ley 21.663",
            Domain::GobernanzaSgsi => "Art. 8° lit. a), b), h) e i) Ley 21.663",
        }
    }
}

impl std::fmt::Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `pad` y no `write_str`: así `{:<38}` alinea la tabla de la CLI.
        f.pad(self.title())
    }
}

/// The maturity level of a single domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Sin datos: el dominio no entra al promedio. **No es lo mismo que nivel 0.**
    NoMedido,
    /// Alguna brecha exigible crítica o alta.
    Nivel0,
    /// Alguna brecha exigible media.
    Nivel1,
    /// Sin brechas exigibles, pero con brechas de madurez voluntaria.
    Nivel2,
    /// Sin brechas de ningún tipo.
    Nivel3,
}

impl Level {
    /// The numeric level, or `None` when the domain was not measured.
    pub fn value(self) -> Option<u8> {
        match self {
            Level::NoMedido => None,
            Level::Nivel0 => Some(0),
            Level::Nivel1 => Some(1),
            Level::Nivel2 => Some(2),
            Level::Nivel3 => Some(3),
        }
    }

    /// What the institution has to fix to move up one level.
    pub fn meaning(self) -> &'static str {
        match self {
            Level::NoMedido => "sin datos suficientes para evaluar este dominio",
            Level::Nivel0 => "hay al menos una brecha exigible crítica o alta abierta",
            Level::Nivel1 => "hay al menos una brecha exigible de severidad media",
            Level::Nivel2 => "sin brechas exigibles; quedan brechas de madurez voluntaria",
            Level::Nivel3 => "sin brechas exigibles ni de madurez",
        }
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.value() {
            Some(n) => write!(f, "Nivel {n}"),
            None => f.write_str("No medido"),
        }
    }
}

/// The maturity assessment of one domain, with the evidence behind it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainMaturity {
    pub domain: Domain,
    pub level: Level,
    /// Brechas exigibles abiertas en el dominio.
    pub exigibles: usize,
    /// Brechas de madurez voluntaria abiertas en el dominio.
    pub madurez: usize,
    /// Por qué quedó en ese nivel, en una línea, para el informe.
    pub rationale: String,
}

/// Maturity across every domain, plus the aggregate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaturityProfile {
    pub domains: Vec<DomainMaturity>,
}

impl MaturityProfile {
    /// Scores every domain from the gap list.
    ///
    /// `measured` manda, y no la presencia de brechas. La distinción importa: hoy
    /// una pregunta del cuestionario que nadie respondió genera brecha, porque para
    /// el plan de remediación no se puede dar por cumplido lo que no se demostró.
    /// Pero derivar de ahí un **nivel de madurez** sería otra cosa: si el
    /// cuestionario se omitió por completo, decir "Nivel 0 en Registro ANCI" afirma
    /// un incumplimiento que nadie verificó. Ese dominio queda no medido.
    pub fn from_gaps(gaps: &[Gap], measured: &[Domain]) -> Self {
        let domains = Domain::all()
            .into_iter()
            .map(|d| assess(d, gaps, measured.contains(&d)))
            .collect();
        Self { domains }
    }

    /// The level of one domain.
    pub fn level_of(&self, domain: Domain) -> Level {
        self.domains
            .iter()
            .find(|d| d.domain == domain)
            .map(|d| d.level)
            .unwrap_or(Level::NoMedido)
    }

    /// Average level across the measured domains only.
    ///
    /// Los dominios no medidos quedan fuera: incluirlos como 0 castigaría a la
    /// institución por un límite de la herramienta.
    pub fn average(&self) -> Option<f32> {
        let values: Vec<u8> = self.domains.iter().filter_map(|d| d.level.value()).collect();
        if values.is_empty() {
            return None;
        }
        Some(values.iter().map(|&v| v as f32).sum::<f32>() / values.len() as f32)
    }

    /// Domains left out of the average, so the report can say so.
    pub fn unmeasured(&self) -> Vec<Domain> {
        self.domains
            .iter()
            .filter(|d| d.level == Level::NoMedido)
            .map(|d| d.domain)
            .collect()
    }

    /// The domains in worst shape first — the order the remediation plan follows.
    pub fn weakest_first(&self) -> Vec<&DomainMaturity> {
        let mut out: Vec<&DomainMaturity> = self
            .domains
            .iter()
            .filter(|d| d.level != Level::NoMedido)
            .collect();
        out.sort_by_key(|d| (d.level.value().unwrap_or(u8::MAX), std::cmp::Reverse(d.exigibles)));
        out
    }
}

/// Scores a single domain.
fn assess(domain: Domain, gaps: &[Gap], measured: bool) -> DomainMaturity {
    if !measured {
        return DomainMaturity {
            domain,
            level: Level::NoMedido,
            exigibles: 0,
            madurez: 0,
            rationale: "no se evaluó ningún control de este dominio".into(),
        };
    }

    // Solo las brechas que descansan en algo observado fijan el nivel. Las que
    // vienen de una pregunta sin responder siguen en la lista y en el plan, pero
    // no pueden hacer caer un dominio: nadie las verificó.
    let mine: Vec<&Gap> = gaps
        .iter()
        .filter(|g| g.domain == domain && g.evaluated)
        .collect();
    let exigibles: Vec<&&Gap> = mine
        .iter()
        .filter(|g| g.exigibilidad == Exigibilidad::Exigible)
        .collect();
    let madurez = mine.len() - exigibles.len();

    let peor = exigibles.iter().map(|g| g.severity).max();

    let (level, rationale) = match peor {
        Some(Severity::Critical) | Some(Severity::High) => {
            let g = exigibles
                .iter()
                .find(|g| g.severity == peor.unwrap())
                .unwrap();
            (Level::Nivel0, format!("brecha exigible {}: {}", severity_es(g.severity), g.control))
        }
        Some(Severity::Medium) => {
            let g = exigibles.iter().find(|g| g.severity == Severity::Medium).unwrap();
            (Level::Nivel1, format!("brecha exigible media: {}", g.control))
        }
        // Sin brechas exigibles. Queda ver si hay de madurez voluntaria.
        _ if madurez > 0 => (
            Level::Nivel2,
            format!("sin brechas exigibles; {madurez} de madurez voluntaria pendiente(s)"),
        ),
        _ => (Level::Nivel3, "sin brechas exigibles ni de madurez".into()),
    };

    DomainMaturity {
        domain,
        level,
        exigibles: exigibles.len(),
        madurez,
        rationale,
    }
}

fn severity_es(s: Severity) -> &'static str {
    match s {
        Severity::Critical => "crítica",
        Severity::High => "alta",
        Severity::Medium => "media",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AppliesTo, InfractionClass};

    fn gap(domain: Domain, severity: Severity, exigibilidad: Exigibilidad) -> Gap {
        Gap {
            control: format!("control de prueba {severity:?}"),
            finding: "hallazgo".into(),
            severity,
            legal_anchor: "prueba".into(),
            applies_to: AppliesTo::All,
            exigibilidad,
            infraction_class: Some(InfractionClass::Leve),
            domain,
            evaluated: true,
            evidence: vec![],
            requires_csirt_report: false,
        }
    }

    fn unevaluated(domain: Domain, severity: Severity) -> Gap {
        Gap { evaluated: false, ..gap(domain, severity, Exigibilidad::Exigible) }
    }

    fn all_measured() -> Vec<Domain> {
        Domain::all().to_vec()
    }

    #[test]
    fn a_clean_domain_reaches_the_top_level() {
        let p = MaturityProfile::from_gaps(&[], &all_measured());
        assert_eq!(p.level_of(Domain::MedidasPermanentes), Level::Nivel3);
        assert_eq!(p.average(), Some(3.0));
    }

    #[test]
    fn a_critical_gap_drops_the_domain_to_zero() {
        let gaps = vec![gap(Domain::MedidasPermanentes, Severity::Critical, Exigibilidad::Exigible)];
        let p = MaturityProfile::from_gaps(&gaps, &all_measured());
        assert_eq!(p.level_of(Domain::MedidasPermanentes), Level::Nivel0);
        // Los demas dominios no se contagian.
        assert_eq!(p.level_of(Domain::RegistroAnci), Level::Nivel3);
    }

    // El punto del criterio elegido: un hallazgo grave no se diluye entre
    // controles que si se cumplen.
    #[test]
    fn one_critical_gap_is_not_offset_by_the_rest_of_the_domain() {
        let mut gaps = vec![gap(Domain::MedidasPermanentes, Severity::Critical, Exigibilidad::Exigible)];
        for _ in 0..10 {
            gaps.push(gap(Domain::MedidasPermanentes, Severity::Medium, Exigibilidad::Exigible));
        }
        let p = MaturityProfile::from_gaps(&gaps, &all_measured());
        assert_eq!(p.level_of(Domain::MedidasPermanentes), Level::Nivel0);
    }

    #[test]
    fn a_medium_gap_lands_on_level_one() {
        let gaps = vec![gap(Domain::Continuidad, Severity::Medium, Exigibilidad::Exigible)];
        let p = MaturityProfile::from_gaps(&gaps, &all_measured());
        assert_eq!(p.level_of(Domain::Continuidad), Level::Nivel1);
    }

    // Una municipalidad no esta obligada por el Art. 8: sus brechas ahi son de
    // madurez y no pueden hacerla caer al nivel de un incumplimiento legal.
    #[test]
    fn voluntary_gaps_cap_the_domain_at_two_without_pushing_it_lower() {
        let gaps = vec![
            gap(Domain::GobernanzaSgsi, Severity::Critical, Exigibilidad::MadurezVoluntaria),
            gap(Domain::GobernanzaSgsi, Severity::High, Exigibilidad::MadurezVoluntaria),
        ];
        let p = MaturityProfile::from_gaps(&gaps, &all_measured());
        assert_eq!(p.level_of(Domain::GobernanzaSgsi), Level::Nivel2);
    }

    #[test]
    fn an_unmeasured_domain_is_not_level_zero() {
        let measured = vec![Domain::MedidasPermanentes];
        let p = MaturityProfile::from_gaps(&[], &measured);
        assert_eq!(p.level_of(Domain::RegistroAnci), Level::NoMedido);
        assert_eq!(p.level_of(Domain::RegistroAnci).value(), None);
        assert_eq!(p.unmeasured().len(), 4);
    }

    #[test]
    fn unmeasured_domains_stay_out_of_the_average() {
        let measured = vec![Domain::MedidasPermanentes];
        let p = MaturityProfile::from_gaps(&[], &measured);
        // Solo el dominio medido promedia: 3.0, no 3/5 = 0.6.
        assert_eq!(p.average(), Some(3.0));
    }

    // El caso de --no-questionnaire: las preguntas sin responder generan brecha
    // para el plan de remediacion, pero no autorizan a fijar un nivel de madurez.
    #[test]
    fn gaps_from_an_unmeasured_domain_do_not_create_a_level() {
        let p = MaturityProfile::from_gaps(
            &[gap(Domain::ReporteIncidentes, Severity::High, Exigibilidad::Exigible)],
            &[Domain::MedidasPermanentes],
        );
        assert_eq!(p.level_of(Domain::ReporteIncidentes), Level::NoMedido);
        assert_eq!(p.level_of(Domain::MedidasPermanentes), Level::Nivel3);
    }

    // El caso mixto: un dominio con evidencia del escaner (respaldo) mas preguntas
    // que nadie respondio. Solo lo observado puede fijar el nivel.
    #[test]
    fn an_unanswered_question_does_not_drag_down_a_measured_domain() {
        let gaps = vec![
            unevaluated(Domain::Continuidad, Severity::Critical),
            unevaluated(Domain::Continuidad, Severity::High),
        ];
        let p = MaturityProfile::from_gaps(&gaps, &all_measured());
        assert_eq!(
            p.level_of(Domain::Continuidad),
            Level::Nivel3,
            "el escaner no encontro nada; las preguntas sin responder no son hallazgos"
        );
        let d = p.domains.iter().find(|d| d.domain == Domain::Continuidad).unwrap();
        assert_eq!(d.exigibles, 0);
    }

    #[test]
    fn nothing_measured_at_all_yields_no_average() {
        let p = MaturityProfile::from_gaps(&[], &[]);
        assert_eq!(p.average(), None);
        assert_eq!(p.unmeasured().len(), 5);
    }

    #[test]
    fn the_weakest_domain_leads_the_order() {
        let gaps = vec![
            gap(Domain::MedidasPermanentes, Severity::Medium, Exigibilidad::Exigible),
            gap(Domain::Continuidad, Severity::Critical, Exigibilidad::Exigible),
        ];
        let p = MaturityProfile::from_gaps(&gaps, &all_measured());
        let orden = p.weakest_first();
        assert_eq!(orden[0].domain, Domain::Continuidad);
        assert_eq!(orden[1].domain, Domain::MedidasPermanentes);
    }

    #[test]
    fn the_rationale_names_the_gap_that_set_the_level() {
        let gaps = vec![gap(Domain::MedidasPermanentes, Severity::Critical, Exigibilidad::Exigible)];
        let p = MaturityProfile::from_gaps(&gaps, &all_measured());
        let d = p.domains.iter().find(|d| d.domain == Domain::MedidasPermanentes).unwrap();
        assert!(d.rationale.contains("crítica"), "{}", d.rationale);
        assert!(d.rationale.contains("control de prueba"), "{}", d.rationale);
    }

    #[test]
    fn every_domain_carries_a_legal_anchor() {
        for d in Domain::all() {
            assert!(d.legal_anchor().contains("Art."), "{d:?} sin anclaje legal");
            assert!(!d.title().is_empty());
        }
    }
}
