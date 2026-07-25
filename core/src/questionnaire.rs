//! Declarative compliance questionnaire for controls that cannot be scanned.
//!
//! Cubre dos bloques distintos, y la distinción importa legalmente:
//!
//! - **Deberes generales** (Art. 7°, Art. 9° e IG N°1): obligan a toda institución
//!   que presta servicios esenciales, incluidas las municipalidades.
//! - **Deberes específicos del Art. 8°**: obligan **solo a los OIV**. Para una
//!   institución que no es OIV se miden como madurez voluntaria, nunca como
//!   incumplimiento legal.
//!
//! Con una excepción verificada el 2026-07-25: el **delegado de ciberseguridad**. El
//! Art. 8° lit. i) lo exige a los OIV, pero el **Art. 5° inciso 2 del DS N°293 de
//! 2024** se lo exige además a todo órgano de la Administración del Estado que integre
//! la RCSE, y su Art. 4° nombra a las municipalidades entre los integrantes obligados.
//! O sea que el mismo deber llega por dos instrumentos distintos, y solo uno se agota
//! en los OIV.
//!
//! Cada anclaje legal de este catálogo fue verificado contra el texto oficial;
//! ver `docs/research/0.5.0-escaner-y-cumplimiento-anci.md` §1 y §3.5.
use crate::types::{AppliesTo, Exigibilidad, Gap, InfractionClass, Severity, Tier};
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
    /// Operational risk severity, derived from `infraction_class` where the law
    /// assigns one (gravísima -> Critical, grave -> High, leve -> Medium).
    pub severity_if_no: Severity,
    pub applies_to: AppliesTo,
    /// How the law classifies this breach (Art. 38°/39°), when it classifies it.
    pub infraction_class: Option<InfractionClass>,
    /// Concrete example of what would evidence compliance, shown to the operator.
    pub evidence_example: String,
}

/// Stable identifiers for each question — used to key answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionId {
    // --- Deberes generales: obligan también a una municipalidad ---
    MedidasPermanentes,
    InscritoAnci,
    EncargadoReporte,
    CasillaInstitucional,
    SegundoFactorEncargado,
    NombramientoAcreditado,
    ProcedimientoReporte,
    // --- Deberes del Art. 8°: solo OIV (madurez voluntaria en los demás) ---
    DelegadoCiberseguridad,
    PlanContinuidad,
    PlanCertificado,
    CapacitacionContinua,
    SgsiImplementado,
    RegistroAcciones,
}

impl QuestionId {
    /// The maturity domain this duty belongs to.
    ///
    /// El mapeo vive aquí, en un solo `match` exhaustivo, y no como un campo
    /// repetido en cada pregunta: agregar una pregunta nueva no compila hasta
    /// decidir su dominio, que es exactamente la revisión que se quiere forzar.
    pub fn domain(self) -> crate::maturity::Domain {
        use crate::maturity::Domain as D;
        match self {
            // Inscripción y datos de contacto ante la ANCI (IG N°1).
            QuestionId::InscritoAnci
            | QuestionId::EncargadoReporte
            | QuestionId::CasillaInstitucional
            | QuestionId::SegundoFactorEncargado
            | QuestionId::NombramientoAcreditado => D::RegistroAnci,

            // Deber de reportar del Art. 9°.
            QuestionId::ProcedimientoReporte => D::ReporteIncidentes,

            // Medidas permanentes de prevención del Art. 7°.
            QuestionId::MedidasPermanentes => D::MedidasPermanentes,

            // Plan de continuidad y su certificación (Art. 8° lit. c, Art. 28°).
            QuestionId::PlanContinuidad | QuestionId::PlanCertificado => D::Continuidad,

            // SGSI, su bitácora, capacitación y delegado.
            QuestionId::SgsiImplementado
            | QuestionId::RegistroAcciones
            | QuestionId::CapacitacionContinua
            | QuestionId::DelegadoCiberseguridad => D::GobernanzaSgsi,
        }
    }
}

/// Returns the full catalogue of declarative questions.
///
/// Orden: primero los deberes generales (exigibles a toda institución obligada),
/// después los del Art. 8° (exigibles solo a OIV).
pub fn catalogue() -> Vec<Question> {
    vec![
        // -------------------------------------------------------------------
        // Deberes generales — obligan también a una municipalidad
        // -------------------------------------------------------------------
        Question {
            id: QuestionId::MedidasPermanentes,
            text: "¿La institución aplica de manera permanente medidas para prevenir, reportar y resolver incidentes de ciberseguridad, conforme a los protocolos y estándares de la ANCI?".into(),
            legal_anchor: "Art. 7° Ley 21.663 — deberes generales; su incumplimiento es infracción grave (Art. 38°, graves N°1)".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::All,
            infraction_class: Some(InfractionClass::Grave),
            evidence_example: "Política de seguridad vigente y aprobada por el jefe de servicio, con fecha de última revisión.".into(),
        },
        Question {
            id: QuestionId::InscritoAnci,
            text: "¿La institución se encuentra inscrita en la plataforma de reporte de incidentes de la ANCI (portal.anci.gob.cl)?".into(),
            legal_anchor: "IG N°1 ANCI, art. primero (D.O. 04-06-2025); su incumplimiento es infracción leve (art. octavo de la IG, por Art. 38° N°2)".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: Some(InfractionClass::Leve),
            evidence_example: "Comprobante de inscripción en portal.anci.gob.cl a nombre de la institución.".into(),
        },
        Question {
            id: QuestionId::EncargadoReporte,
            text: "¿Hay designado un encargado de reportar incidentes que cuente con formación o experiencia técnica o profesional en ciberseguridad?".into(),
            legal_anchor: "IG N°1 ANCI, arts. primero y tercero".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: Some(InfractionClass::Leve),
            evidence_example: "Acto de designación del encargado, más su currículum o certificado de formación en ciberseguridad.".into(),
        },
        Question {
            id: QuestionId::CasillaInstitucional,
            text: "¿Se informó a la ANCI una casilla de correo institucional como canal oficial de comunicación?".into(),
            legal_anchor: "IG N°1 ANCI, art. segundo; Art. 7° del Reglamento de reporte de incidentes (DS N°295 de 2024)".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: Some(InfractionClass::Leve),
            evidence_example: "Casilla institucional registrada en la plataforma (no una casilla personal ni de dominio externo).".into(),
        },
        Question {
            id: QuestionId::SegundoFactorEncargado,
            text: "¿El encargado de reportar tiene activado un segundo factor de autenticación (TOTP o passkeys) en la plataforma?".into(),
            // La IG N°2 (D.O. 26-12-2025) autoriza medios alternativos de autenticación
            // para el encargado que no pueda acceder a Clave Única, acreditando el
            // vínculo con la institución. Citar solo la IG N°1 dejaba a esa
            // municipalidad sin salida aparente.
            legal_anchor: "IG N°1 ANCI, art. cuarto — Clave Única con contraseña robusta y doble factor; IG N°2 ANCI, art. primero (D.O. 26-12-2025) autoriza medios alternativos cuando no se puede acceder a Clave Única".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: Some(InfractionClass::Leve),
            evidence_example: "Captura de la configuración de doble factor activo en la cuenta del encargado.".into(),
        },
        Question {
            id: QuestionId::NombramientoAcreditado,
            text: "¿Se acreditó ante la ANCI el nombramiento del encargado mediante documento firmado con firma electrónica avanzada por el representante legal?".into(),
            legal_anchor: "IG N°1 ANCI, art. quinto — plazo de 5 días hábiles para subsanar".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: Some(InfractionClass::Leve),
            evidence_example: "Documento de designación con firma electrónica avanzada del representante legal, cargado en la plataforma.".into(),
        },
        Question {
            id: QuestionId::ProcedimientoReporte,
            text: "¿Existe un procedimiento interno que permita cumplir los plazos de reporte del Art. 9° (alerta temprana en 3 horas, actualización en 72 horas, informe final en 15 días corridos)?".into(),
            legal_anchor: "Art. 9° Ley 21.663; incumplir el deber de reportar es infracción grave (Art. 38°, graves N°5)".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::All,
            infraction_class: Some(InfractionClass::Grave),
            evidence_example: "Procedimiento escrito con responsables y vías de contacto fuera de horario, y registro del último simulacro o incidente reportado.".into(),
        },
        // -------------------------------------------------------------------
        // Deberes específicos del Art. 8° — solo OIV
        // -------------------------------------------------------------------
        Question {
            id: QuestionId::SgsiImplementado,
            text: "¿Existe un Sistema de Gestión de Seguridad de la Información (SGSI) continuo implementado?".into(),
            legal_anchor: "Art. 8° lit. a) Ley 21.663 — infracción grave (Art. 39°, graves N°1)".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::Oiv,
            infraction_class: Some(InfractionClass::Grave),
            evidence_example: "Documento del SGSI vigente con su matriz de riesgos y la fecha de la última revisión.".into(),
        },
        Question {
            id: QuestionId::PlanContinuidad,
            text: "¿Existe un plan de continuidad operacional y ciberseguridad elaborado e implementado?".into(),
            legal_anchor: "Art. 8° lit. c) Ley 21.663 — infracción grave (Art. 39°, graves N°2)".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::Oiv,
            infraction_class: Some(InfractionClass::Grave),
            evidence_example: "Plan de continuidad aprobado, con su acta de la última prueba o ejercicio.".into(),
        },
        Question {
            id: QuestionId::PlanCertificado,
            text: "¿El plan de continuidad cuenta con certificación conforme al Art. 28° y se somete a revisiones periódicas con una frecuencia mínima de dos años?".into(),
            legal_anchor: "Art. 8° lit. c) y Art. 28° Ley 21.663 — la certificación debe tener al menos un año de vigencia".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::Oiv,
            infraction_class: Some(InfractionClass::Leve),
            evidence_example: "Certificado emitido por una entidad del registro de certificadoras autorizadas de la ANCI, con su fecha de vigencia.".into(),
        },
        Question {
            id: QuestionId::RegistroAcciones,
            text: "¿Se mantiene un registro formal de las acciones ejecutadas dentro del SGSI?".into(),
            legal_anchor: "Art. 8° lit. b) Ley 21.663 — infracción leve (Art. 39°, leves N°1)".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::Oiv,
            infraction_class: Some(InfractionClass::Leve),
            evidence_example: "Bitácora o sistema de tickets con las acciones del SGSI fechadas y con responsable.".into(),
        },
        Question {
            id: QuestionId::CapacitacionContinua,
            text: "¿Existen programas de capacitación, formación y educación continua para trabajadores y colaboradores, incluidas campañas de ciberhigiene?".into(),
            legal_anchor: "Art. 8° lit. h) Ley 21.663 — infracción leve (Art. 39°, leves N°3)".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::Oiv,
            infraction_class: Some(InfractionClass::Leve),
            evidence_example: "Registro de asistencia de la última capacitación y el plan anual de ciberhigiene.".into(),
        },
        Question {
            id: QuestionId::DelegadoCiberseguridad,
            text: "¿Se ha designado un Delegado de Ciberseguridad que actúe como contraparte ante la ANCI?".into(),
            legal_anchor: "Art. 8° lit. i) Ley 21.663 e IG N°3 ANCI (D.O. 26-12-2025) para OIV; para los órganos de la Administración del Estado, Art. 5° inciso 2 del DS N°293 de 2024 (D.O. 11-04-2025), que los obliga como integrantes de la RCSE — infracción leve (Art. 39°, leves N°4)".into(),
            severity_if_no: Severity::Medium,
            // El deber le llega a una municipalidad por dos caminos distintos, y solo
            // uno se agota en los OIV. La IG N°3 se dirige a los calificados como OIV;
            // pero el Art. 5° del DS N°293 obliga a designar delegado a **todo órgano
            // de la Administración del Estado** que integre la RCSE, y su Art. 4°
            // nombra a las municipalidades entre los integrantes obligados.
            //
            // Límite conocido: `Tier` no distingue un PSE estatal de uno privado, y a
            // un PSE privado el DS N°293 no lo alcanza. `OivAndPse` sobreextiende en
            // ese caso hipotético; se prefiere sobre `Oiv`, que subrepresentaba el
            // deber de todos los clientes reales del producto, que son municipales.
            applies_to: AppliesTo::OivAndPse,
            infraction_class: Some(InfractionClass::Leve),
            evidence_example: "Acto de designación del delegado, con independencia funcional del área de TI según la IG N°3.".into(),
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

/// Whether a control name comes from the questionnaire rather than the scanner.
///
/// La deriva necesita distinguirlos y el histórico no guarda el origen de cada
/// brecha. No hace falta que lo guarde: `to_gaps` usa el texto de la pregunta como
/// nombre del control, así que el catálogo mismo es la respuesta.
///
/// La diferencia importa porque las dos familias se comportan al revés cuando una
/// brecha desaparece. Un control declarativo que nadie respondió **sigue** apareciendo
/// como brecha (ver `to_gaps`), así que si desaparece es porque alguien declaró que se
/// cumple. Un control técnico desaparece también cuando el escaneo no llegó a mirarlo.
pub fn es_declarativo(control: &str) -> bool {
    catalogue().iter().any(|q| q.text == control)
}

// ---------------------------------------------------------------------------
// Gap conversion
// ---------------------------------------------------------------------------

/// Converts non-compliant questionnaire answers into Gap values.
///
/// A diferencia de la versión anterior, **no descarta** las preguntas que no
/// obligan al tier escaneado: las emite marcadas como `MadurezVoluntaria`. Así un
/// municipio (que no es OIV) obtiene un diagnóstico completo del Art. 8° sin que
/// el informe afirme un incumplimiento legal que no existe.
pub fn to_gaps(response: &QuestionnaireResponse, tier: Tier) -> Vec<Gap> {
    let mut gaps = Vec::new();

    for question in catalogue() {
        let exigibilidad = question.applies_to.exigibilidad_for(tier);
        let answer = response.get(question.id);
        let non_compliant = answer.map(|a| !a.compliant).unwrap_or(true); // unanswered = gap

        if !non_compliant {
            continue;
        }

        let evidence = answer
            .and_then(|a| a.notes.clone())
            .map(|n| vec![n])
            .unwrap_or_else(|| vec!["No respondido o declarado no cumplido".into()]);

        let finding = match exigibilidad {
            Exigibilidad::Exigible => {
                format!("Control declarativo no cumplido: {}", question.text)
            }
            Exigibilidad::MadurezVoluntaria => format!(
                "Brecha de madurez (no exigible a esta institución): {}",
                question.text
            ),
        };

        // Una brecha no exigible no puede acarrear consecuencia legal: se le
        // suprime la clasificación de infracción.
        let infraction_class = match exigibilidad {
            Exigibilidad::Exigible          => question.infraction_class,
            Exigibilidad::MadurezVoluntaria => None,
        };

        gaps.push(Gap {
            control:              question.text.clone(),
            finding,
            severity:             question.severity_if_no.clone(),
            legal_anchor:         question.legal_anchor.clone(),
            applies_to:           question.applies_to.clone(),
            exigibilidad,
            infraction_class,
            domain:               question.id.domain(),
            // Una pregunta que nadie respondió no es evidencia de nada. Sigue
            // siendo brecha (no se demostró cumplimiento) pero no fija madurez.
            evaluated:            answer.is_some(),
            evidence,
            requires_csirt_report: false, // set later by significance filter
        });
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
    fn pse_gets_oiv_only_questions_as_voluntary_maturity() {
        let response = QuestionnaireResponse::default();
        let gaps = to_gaps(&response, Tier::Pse);
        // El SGSI del Art. 8° lit. b) sigue siendo exigible solo a los OIV. Antes esta
        // prueba usaba el Delegado, que dejo de servir de ejemplo: el DS N°293 lo hace
        // exigible tambien a los organos del Estado.
        let sgsi = gaps
            .iter()
            .find(|g| g.control.contains("SGSI"))
            .expect("el Art. 8° debe informarse como madurez, no desaparecer");
        assert_eq!(sgsi.exigibilidad, Exigibilidad::MadurezVoluntaria);
    }

    // El deber le llega a una municipalidad por el Art. 5° del DS N°293, no por el
    // Art. 8° de la ley. Informarlo como "no exigible" le decia que no le tocaba.
    #[test]
    fn the_cybersecurity_delegate_is_binding_on_a_municipality_too() {
        let gaps = to_gaps(&QuestionnaireResponse::default(), Tier::Pse);
        let delegado = gaps
            .iter()
            .find(|g| g.control.contains("Delegado"))
            .expect("no puede desaparecer del informe");
        assert_eq!(delegado.exigibilidad, Exigibilidad::Exigible);
        assert!(delegado.legal_anchor.contains("293"), "{}", delegado.legal_anchor);
        assert!(delegado.legal_anchor.contains("RCSE"), "{}", delegado.legal_anchor);
        // Y sigue siendo exigible a un OIV, por el camino de la ley.
        let oiv = to_gaps(&QuestionnaireResponse::default(), Tier::Oiv);
        let d = oiv.iter().find(|g| g.control.contains("Delegado")).unwrap();
        assert_eq!(d.exigibilidad, Exigibilidad::Exigible);
    }

    #[test]
    fn voluntary_maturity_gap_carries_no_infraction_class() {
        // No es exigible, luego no puede haber infracción que clasificar.
        let gaps = to_gaps(&QuestionnaireResponse::default(), Tier::Pse);
        for gap in gaps.iter().filter(|g| g.exigibilidad == Exigibilidad::MadurezVoluntaria) {
            assert!(gap.infraction_class.is_none(), "{} no debe tener infracción", gap.control);
        }
    }

    #[test]
    fn general_duties_are_binding_on_a_municipality() {
        let gaps = to_gaps(&QuestionnaireResponse::default(), Tier::Pse);
        for id_text in ["inscrita en la plataforma", "manera permanente", "plazos de reporte"] {
            let gap = gaps
                .iter()
                .find(|g| g.control.contains(id_text))
                .unwrap_or_else(|| panic!("falta la pregunta que contiene {id_text:?}"));
            assert_eq!(gap.exigibilidad, Exigibilidad::Exigible);
            assert!(gap.infraction_class.is_some());
        }
    }

    #[test]
    fn oiv_gets_everything_as_binding() {
        let gaps = to_gaps(&QuestionnaireResponse::default(), Tier::Oiv);
        assert_eq!(gaps.len(), catalogue().len());
        assert!(gaps.iter().all(|g| g.exigibilidad == Exigibilidad::Exigible));
    }

    #[test]
    fn every_question_has_an_anchor_and_an_evidence_example() {
        for q in catalogue() {
            assert!(!q.legal_anchor.trim().is_empty(), "{:?} sin anclaje legal", q.id);
            assert!(!q.evidence_example.trim().is_empty(), "{:?} sin ejemplo de evidencia", q.id);
        }
    }

    #[test]
    fn severity_tracks_the_legal_classification() {
        // grave -> High, leve -> Medium. Es la regla que documenta `severity_if_no`.
        for q in catalogue() {
            match q.infraction_class {
                Some(InfractionClass::Grave)     => assert_eq!(q.severity_if_no, Severity::High, "{:?}", q.id),
                Some(InfractionClass::Leve)      => assert_eq!(q.severity_if_no, Severity::Medium, "{:?}", q.id),
                Some(InfractionClass::Gravisima) => assert_eq!(q.severity_if_no, Severity::Critical, "{:?}", q.id),
                None => {}
            }
        }
    }

    #[test]
    fn catalogue_is_non_empty() {
        assert!(!catalogue().is_empty());
    }
}