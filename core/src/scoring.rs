//! Aggregate compliance score.
//!
//! Mecánica tomada del *NIST SP 800-171 DoD Assessment Methodology* (SPRS): se
//! parte de una base fija y se descuentan puntos ponderados por cada control no
//! implementado, admitiendo puntaje negativo cuando el incumplimiento es amplio.
//!
//! La diferencia deliberada con SPRS está en **de dónde salen los pesos**. SPRS
//! los fija por criterio propio del DoD; aquí, cuando la ley clasifica la
//! infracción, el peso sale de esa clasificación (Art. 38° y 39° de la Ley
//! 21.663): gravísima 5, grave 3, leve 1. Así la ponderación es defendible ante
//! una autoridad, porque no es una opinión del producto sino la escala que fija
//! la propia ley.
//!
//! Para los controles técnicos sin correlato en la ley (TLS obsoleto, certificado
//! vencido, cifrado de disco) se usa una tabla propia, documentada aquí como
//! **criterio técnico** y presentada como tal en el informe. No se presenta como
//! exigencia legal.
//!
//! Dos reglas que se siguen del modelo dual:
//!
//! 1. **Solo descuentan las brechas exigibles.** Una brecha de madurez voluntaria
//!    no puede bajar el puntaje de cumplimiento legal: no hay obligación que
//!    incumplir. Se cuenta aparte, en el puntaje de madurez.
//! 2. El puntaje de cumplimiento y el de madurez se informan por separado, nunca
//!    fundidos en un solo número.

use crate::types::{Exigibilidad, Gap, InfractionClass, Severity};
use serde::{Deserialize, Serialize};

/// Base fija del puntaje de cumplimiento, en la mecánica SPRS.
pub const BASE_SCORE: i32 = 100;

/// Peso de un control técnico sin clasificación legal — criterio propio.
fn technical_weight(severity: &Severity) -> u32 {
    match severity {
        Severity::Critical => 3,
        Severity::High     => 2,
        Severity::Medium   => 1,
    }
}

/// Desglose del puntaje agregado de una evaluación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceScore {
    /// Base de la que se descuenta (100 = sin brechas exigibles).
    pub base: i32,
    /// Puntos descontados por brechas legalmente exigibles.
    pub deductions: u32,
    /// Puntaje final. Puede ser negativo, igual que en SPRS.
    pub score: i32,
    /// Puntos descontados que provienen de la escala legal (Art. 38°/39°).
    pub legal_deductions: u32,
    /// Puntos descontados por criterio técnico propio, sin base legal directa.
    pub technical_deductions: u32,
    /// Brechas exigibles, por clasificación legal.
    pub gravisimas: usize,
    pub graves: usize,
    pub leves: usize,
    /// Brechas exigibles sin clasificación legal (criterio técnico).
    pub tecnicas: usize,
    /// Brechas informadas como madurez voluntaria — no descuentan.
    pub madurez: usize,
}

impl ComplianceScore {
    /// Computes the aggregate score from the evaluated gaps.
    pub fn from_gaps(gaps: &[Gap]) -> Self {
        let mut s = ComplianceScore {
            base: BASE_SCORE,
            deductions: 0,
            score: BASE_SCORE,
            legal_deductions: 0,
            technical_deductions: 0,
            gravisimas: 0,
            graves: 0,
            leves: 0,
            tecnicas: 0,
            madurez: 0,
        };

        for gap in gaps {
            if gap.exigibilidad == Exigibilidad::MadurezVoluntaria {
                s.madurez += 1;
                continue;
            }
            match gap.infraction_class {
                Some(class) => {
                    s.legal_deductions += class.score_weight();
                    match class {
                        InfractionClass::Gravisima => s.gravisimas += 1,
                        InfractionClass::Grave     => s.graves += 1,
                        InfractionClass::Leve      => s.leves += 1,
                    }
                }
                None => {
                    s.technical_deductions += technical_weight(&gap.severity);
                    s.tecnicas += 1;
                }
            }
        }

        s.deductions = s.legal_deductions + s.technical_deductions;
        s.score = s.base - s.deductions as i32;
        s
    }

    /// Total de brechas exigibles (las que descuentan).
    pub fn exigibles(&self) -> usize {
        self.gravisimas + self.graves + self.leves + self.tecnicas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AppliesTo, Exigibilidad, Gap, InfractionClass, Severity};

    fn gap(
        severity: Severity,
        exigibilidad: Exigibilidad,
        infraction_class: Option<InfractionClass>,
    ) -> Gap {
        Gap {
            control: "control de prueba".into(),
            finding: "hallazgo".into(),
            severity,
            legal_anchor: "Art. 7°".into(),
            applies_to: AppliesTo::All,
            exigibilidad,
            infraction_class,
            evidence: vec![],
            requires_csirt_report: false,
        }
    }

    #[test]
    fn perfect_score_when_no_gaps() {
        let s = ComplianceScore::from_gaps(&[]);
        assert_eq!(s.score, 100);
        assert_eq!(s.deductions, 0);
    }

    #[test]
    fn legal_weights_follow_articulo_39() {
        let gaps = vec![
            gap(Severity::Critical, Exigibilidad::Exigible, Some(InfractionClass::Gravisima)),
            gap(Severity::High,     Exigibilidad::Exigible, Some(InfractionClass::Grave)),
            gap(Severity::Medium,   Exigibilidad::Exigible, Some(InfractionClass::Leve)),
        ];
        let s = ComplianceScore::from_gaps(&gaps);
        assert_eq!(s.legal_deductions, 5 + 3 + 1);
        assert_eq!(s.technical_deductions, 0);
        assert_eq!(s.score, 100 - 9);
    }

    #[test]
    fn voluntary_maturity_never_deducts() {
        let gaps = vec![
            gap(Severity::Critical, Exigibilidad::MadurezVoluntaria, None),
            gap(Severity::Critical, Exigibilidad::MadurezVoluntaria, Some(InfractionClass::Gravisima)),
        ];
        let s = ComplianceScore::from_gaps(&gaps);
        assert_eq!(s.score, 100, "una brecha no exigible no puede bajar el cumplimiento");
        assert_eq!(s.madurez, 2);
        assert_eq!(s.exigibles(), 0);
    }

    #[test]
    fn technical_weights_are_our_own_criterion() {
        let gaps = vec![
            gap(Severity::Critical, Exigibilidad::Exigible, None),
            gap(Severity::High,     Exigibilidad::Exigible, None),
            gap(Severity::Medium,   Exigibilidad::Exigible, None),
        ];
        let s = ComplianceScore::from_gaps(&gaps);
        assert_eq!(s.technical_deductions, 3 + 2 + 1);
        assert_eq!(s.legal_deductions, 0);
        assert_eq!(s.tecnicas, 3);
    }

    #[test]
    fn score_can_go_negative_like_sprs() {
        let gaps: Vec<Gap> = (0..40)
            .map(|_| gap(Severity::Critical, Exigibilidad::Exigible, Some(InfractionClass::Gravisima)))
            .collect();
        let s = ComplianceScore::from_gaps(&gaps);
        assert!(s.score < 0, "SPRS admite puntaje negativo; el nuestro también");
    }
}
