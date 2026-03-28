//! Declarative compliance questionnaire for Art. 8° controls that cannot be scanned.
use crate::types::{AppliesTo, Gap, Severity, Tier};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Question catalogue
// ---------------------------------------------------------------------------

/// A yes/no compliance question tied to a specific legal obligation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: QuestionId,
    pub text: String,
    pub legal_anchor: String,
    pub severity_if_no: Severity,
    pub applies_to: AppliesTo,
}

/// Stable identifiers for each question — used to key answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionId {
    DelegadoCiberseguridad,
    PlanContinuidad,
    PlanCertificado,
    CapacitacionContinua,
    SgsiImplementado,
    RegistroAcciones,
    InscritoAnci,
}

/// Returns the full catalogue of declarative questions.
pub fn catalogue() -> Vec<Question> {
    vec![
        Question {
            id: QuestionId::InscritoAnci,
            text: "¿La institución se encuentra inscrita en la plataforma de reporte de incidentes de la ANCI?".into(),
            legal_anchor: "IG N°1 ANCI (jun 2025) — inscripción obligatoria PSE".into(),
            severity_if_no: Severity::Critical,
            applies_to: AppliesTo::OivAndPse,
        },
        Question {
            id: QuestionId::DelegadoCiberseguridad,
            text: "¿Se ha designado un Delegado de Ciberseguridad con contraparte formal ante la ANCI?".into(),
            legal_anchor: "Art. 8° lit. i) Ley 21.663; IG N°3 ANCI (dic 2025)".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::Oiv,
        },
        Question {
            id: QuestionId::SgsiImplementado,
            text: "¿Existe un Sistema de Gestión de Seguridad de la Información (SGSI) continuo implementado?".into(),
            legal_anchor: "Art. 8° lit. a) Ley 21.663".into(),
            severity_if_no: Severity::Critical,
            applies_to: AppliesTo::Oiv,
        },
        Question {
            id: QuestionId::RegistroAcciones,
            text: "¿Se mantiene un registro formal de las acciones ejecutadas dentro del SGSI?".into(),
            legal_anchor: "Art. 8° lit. b) Ley 21.663".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::Oiv,
        },
        Question {
            id: QuestionId::PlanContinuidad,
            text: "¿Existe un plan de continuidad operacional y ciberseguridad elaborado e implementado?".into(),
            legal_anchor: "Art. 8° lit. c) Ley 21.663".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::Oiv,
        },
        Question {
            id: QuestionId::PlanCertificado,
            text: "¿El plan de continuidad ha sido certificado en los últimos 2 años por un centro autorizado por la ANCI?".into(),
            legal_anchor: "Art. 8° lit. c) y Art. 28° Ley 21.663".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::Oiv,
        },
        Question {
            id: QuestionId::CapacitacionContinua,
            text: "¿Existen programas de capacitación y ciberhigiene continua para trabajadores y colaboradores?".into(),
            legal_anchor: "Art. 8° lit. h) Ley 21.663".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::Oiv,
        },
    ]
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Single answer to a questionnaire question.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub question_id: QuestionId,
    pub compliant: bool,
    /// Optional free-text evidence provided by the operator.
    pub notes: Option<String>,
}

/// Complete set of answers from the operator before or during the scan.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuestionnaireResponse {
    pub answers: Vec<Answer>,
}

impl QuestionnaireResponse {
    /// Returns the answer for a given question ID, if present.
    pub fn get(&self, id: QuestionId) -> Option<&Answer> {
        self.answers.iter().find(|a| a.question_id == id)
    }
}

// ---------------------------------------------------------------------------
// Gap conversion
// ---------------------------------------------------------------------------

/// Converts non-compliant questionnaire answers into Gap values.
pub fn to_gaps(response: &QuestionnaireResponse, tier: Tier) -> Vec<Gap> {
    let mut gaps = Vec::new();

    for question in catalogue() {
        if !question.applies_to.is_mandatory_for(tier) {
            continue;
        }
        let answer = response.get(question.id);
        let non_compliant = answer.map(|a| !a.compliant).unwrap_or(true); // unanswered = gap

        if non_compliant {
            let evidence = answer
                .and_then(|a| a.notes.clone())
                .map(|n| vec![n])
                .unwrap_or_else(|| vec!["No respondido o declarado no cumplido".into()]);

            gaps.push(Gap {
                control:              question.text.clone(),
                finding:              format!("Control declarativo no cumplido: {}", question.text),
                severity:             question.severity_if_no.clone(),
                legal_anchor:         question.legal_anchor.clone(),
                applies_to:           question.applies_to.clone(),
                evidence,
                requires_csirt_report: false, // set later by significance filter
            });
        }
    }
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unanswered_oiv_questions_produce_gaps() {
        let response = QuestionnaireResponse::default();
        let gaps = to_gaps(&response, Tier::Oiv);
        assert!(!gaps.is_empty());
    }

    #[test]
    fn compliant_answer_suppresses_gap() {
        let mut response = QuestionnaireResponse::default();
        response.answers.push(Answer {
            question_id: QuestionId::DelegadoCiberseguridad,
            compliant: true,
            notes: None,
        });
        let gaps = to_gaps(&response, Tier::Oiv);
        assert!(!gaps.iter().any(|g| g.control.contains("Delegado")));
    }

    #[test]
    fn pse_skips_oiv_only_questions() {
        let response = QuestionnaireResponse::default();
        let gaps = to_gaps(&response, Tier::Pse);
        // DelegadoCiberseguridad is Oiv-only — must not appear for PSE
        assert!(!gaps.iter().any(|g| g.control.contains("Delegado")));
    }

    #[test]
    fn catalogue_is_non_empty() {
        assert!(!catalogue().is_empty());
    }
}