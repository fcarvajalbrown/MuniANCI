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
    // --- Red de Conectividad Segura del Estado (DS N°293 de 2024) ---
    // Obligan a todo organo de la Administracion del Estado; su Art. 4° nombra
    // expresamente a las municipalidades entre los integrantes obligados.
    RcseIntegrada,
    RcseContratosSemestral,
    RcseMonitoreoTrafico,
    RcseDominioGobCl,
    RcseFqdnInformados,
    // --- Decreto 7 de 2023, Norma Tecnica de Seguridad de la Informacion ---
    // Obligan a todo organo de la Administracion del Estado, pero se miden como
    // madurez y nunca como incumplimiento: ver `to_gaps`.
    D7DiagnosticoInicial,
    D7PoliticaAprobada,
    D7PoliticaAlcance,
    D7ResponsableSeguridad,
    D7ResponsableActivos,
    D7RolesNoExternalizados,
    D7FuncionProteccion,
    D7CodigoMalicioso,
    D7FuncionRespuesta,
    D7FuncionRecuperacion,
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

            // Los deberes de la RCSE se reparten segun de que hablan: informar
            // contratos y nombres de dominio es gobernanza; estar integrado a la Red
            // y no estorbar su monitoreo es higiene tecnica permanente.
            QuestionId::RcseContratosSemestral
            | QuestionId::RcseFqdnInformados => D::GobernanzaSgsi,

            QuestionId::RcseIntegrada
            | QuestionId::RcseMonitoreoTrafico
            | QuestionId::RcseDominioGobCl => D::MedidasPermanentes,

            // Decreto 7: sus cinco funciones son las del Titulo Tercero, y de ahi
            // salen los dominios. El diagnostico, la Politica, su alcance y los dos
            // responsables caen todos en identificacion, porque el Art. 7 pone ahi
            // las categorias de contexto, gobernanza y gestion de activos.
            QuestionId::D7DiagnosticoInicial
            | QuestionId::D7PoliticaAprobada
            | QuestionId::D7PoliticaAlcance
            | QuestionId::D7ResponsableSeguridad
            | QuestionId::D7ResponsableActivos
            | QuestionId::D7RolesNoExternalizados => D::D7Identificacion,

            QuestionId::D7FuncionProteccion => D::D7Proteccion,
            QuestionId::D7CodigoMalicioso => D::D7Deteccion,
            QuestionId::D7FuncionRespuesta => D::D7Respuesta,
            QuestionId::D7FuncionRecuperacion => D::D7Recuperacion,
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
            // un PSE privado el DS N°293 no lo alcanza. `All` sobreextiende en ese caso
            // hipotético; se prefiere igual, porque el producto se compila para órganos
            // del Estado y `OivAndPse` dejaba fuera al municipio aún sin clasificar,
            // que también integra la RCSE.
            applies_to: AppliesTo::All,
            infraction_class: Some(InfractionClass::Leve),
            evidence_example: "Acto de designación del delegado, con independencia funcional del área de TI según la IG N°3.".into(),
        },

        // -------------------------------------------------------------------
        // Red de Conectividad Segura del Estado — DS N°293 de 2024
        //
        // Su Art. 4° obliga a integrar la RCSE, entre otros, a "las
        // Municipalidades", y su Art. 1° alcanza a los organos de la
        // Administracion del Estado que se conecten. No es el Art. 8° de la ley:
        // es un reglamento propio, con sus propios deberes.
        //
        // Ninguna de estas preguntas lleva `infraction_class`. El decreto no fija
        // una escala de infracciones propia, y este producto no inventa una: se
        // afirma el deber y su articulo, no su sancion.
        // -------------------------------------------------------------------
        Question {
            id: QuestionId::RcseIntegrada,
            text: "¿La institución está integrada a la Red de Conectividad Segura del Estado (RCSE)?".into(),
            legal_anchor: "Art. 4° del DS N°293 de 2024 (D.O. 11-04-2025) — nombra expresamente a las municipalidades entre quienes deberán integrar la RCSE. El Director de la ANCI puede exceptuar por resolución fundada, por razones técnicas o de recursos".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Constancia de incorporación a la RCSE, o la resolución fundada de la ANCI que exceptúa a la institución.".into(),
        },
        Question {
            id: QuestionId::RcseContratosSemestral,
            text: "¿Se informan a la ANCI, cada seis meses, todos los contratos vigentes de telecomunicaciones, transmisión de datos, acceso a internet, infraestructura digital, servicios digitales, TI y almacenamiento de datos?".into(),
            legal_anchor: "Art. 6° del DS N°293 de 2024 — informe semestral. Las modificaciones contractuales se informan dentro de 15 días corridos desde la total tramitación del acto que las aprueba".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Último informe semestral de contratos remitido a la ANCI, con su fecha de envío.".into(),
        },
        Question {
            id: QuestionId::RcseMonitoreoTrafico,
            text: "¿Se permite a la ANCI el monitoreo del tráfico de red, sin medidas que lo impidan?".into(),
            legal_anchor: "Art. 7° del DS N°293 de 2024 — los integrantes deberán permitir el monitoreo, inhibiendo las medidas que impidan su materialización".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Configuración vigente que habilita el monitoreo acordado con la Agencia.".into(),
        },
        Question {
            id: QuestionId::RcseDominioGobCl,
            text: "¿El sitio web institucional usa un subdominio .gob.cl registrado ante la Agencia, y el dominio .cl equivalente redirige a él?".into(),
            legal_anchor: "Art. 8° del DS N°293 de 2024; su disposición transitoria cuarta dio un año desde la entrada en vigor (11-04-2025) para comenzar a usarlo, plazo vencido el 11-04-2026".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Subdominio .gob.cl registrado en nic.gob.cl, con el .cl redirigiendo y las tablas reversas publicadas.".into(),
        },
        Question {
            id: QuestionId::RcseFqdnInformados,
            text: "¿Se informó a la ANCI todo nombre de dominio completamente calificado (FQDN) fuera de gob.cl asociado a activos, servicios, sitios o sistemas web expuestos a internet?".into(),
            legal_anchor: "Art. 8°, inciso final, del DS N°293 de 2024".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Inventario de FQDN fuera de gob.cl remitido a la Agencia, con su fecha de envío.".into(),
        },

        // -------------------------------------------------------------------
        // Decreto 7 de 2023 (MINSEGPRES) — Norma Técnica de Seguridad de la
        // Información y Ciberseguridad de la Ley 21.180.
        //
        // Ninguna de estas preguntas afirma una infracción, y no es un olvido:
        // el decreto no fija escala sancionatoria propia, y su guía técnica dice
        // de sí misma que "no crea obligaciones adicionales". Además su §3.6
        // admite que la Política se desarrolle gradualmente, así que estas se
        // miden como madurez. `to_gaps` lo impone por marco, no pregunta a
        // pregunta, y hay pruebas que lo fijan.
        // -------------------------------------------------------------------
        Question {
            id: QuestionId::D7DiagnosticoInicial,
            text: "¿La institución realizó el diagnóstico inicial del estado de ciberseguridad de sus plataformas electrónicas, cubriendo personas, procesos y tecnología?".into(),
            legal_anchor: "Art. 4° del Decreto 7 de 2023 (MINSEGPRES)".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Informe de diagnóstico inicial con su síntesis de madurez institucional, y su registro en el Catálogo de Plataformas.".into(),
        },
        Question {
            id: QuestionId::D7PoliticaAprobada,
            text: "¿Existe una Política de Seguridad de la Información y Ciberseguridad aprobada por acto administrativo del Jefe Superior de Servicio?".into(),
            legal_anchor: "Art. 5° del Decreto 7 de 2023 (MINSEGPRES)".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Decreto o resolución que aprueba la Política, con su número y fecha.".into(),
        },
        Question {
            id: QuestionId::D7PoliticaAlcance,
            text: "¿La Política define su alcance subjetivo (a quiénes aplica) y objetivo (qué activos y plataformas cubre), junto con la legislación aplicable?".into(),
            legal_anchor: "Art. 5° del Decreto 7 de 2023, numerales 1 a 3".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Apartado de alcance de la Política, distinguiendo funcionarios y terceros de los activos y plataformas cubiertos.".into(),
        },
        Question {
            id: QuestionId::D7ResponsableSeguridad,
            text: "¿Se designó un responsable institucional de seguridad de la información y ciberseguridad?".into(),
            legal_anchor: "Art. 5° del Decreto 7 de 2023, numeral 4".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Acto administrativo de designación. Un encargado nombrado bajo el Instructivo Presidencial N°8 de 2018 se entiende cumplido.".into(),
        },
        Question {
            id: QuestionId::D7ResponsableActivos,
            text: "¿Se designó un responsable de los activos de información, encargado de identificarlos, clasificarlos y gestionar su riesgo?".into(),
            legal_anchor: "Art. 5° del Decreto 7 de 2023, numeral 4".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Acto de designación. El decreto permite que este rol y el de seguridad recaigan en una misma persona.".into(),
        },
        Question {
            id: QuestionId::D7RolesNoExternalizados,
            text: "¿Ambos roles los ejercen funcionarios de la institución, sin externalizar su desempeño bajo ninguna forma?".into(),
            legal_anchor: "Art. 5° del Decreto 7 de 2023, inciso final del numeral 4".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Calidad jurídica de quienes ejercen los roles. El decreto lo prohíbe expresamente: no se admite proveedor externo.".into(),
        },
        Question {
            id: QuestionId::D7FuncionProteccion,
            text: "¿La institución desarrolló la función de protección: gestión de servidores y redes, autenticación y control de acceso, seguridad de los datos y registro de eventos?".into(),
            legal_anchor: "Art. 8° del Decreto 7 de 2023 (MINSEGPRES)".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Procedimientos de control de acceso y de registro de eventos sobre las plataformas que sustentan procedimientos administrativos.".into(),
        },
        Question {
            id: QuestionId::D7CodigoMalicioso,
            text: "¿Los servidores y las plataformas electrónicas cuentan con medidas adecuadas de protección contra código malicioso, con monitoreo continuo?".into(),
            legal_anchor: "Art. 9° del Decreto 7 de 2023 (MINSEGPRES)".into(),
            severity_if_no: Severity::High,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Solución antimalware desplegada en servidores, con su consola de administración y la fecha de la última actualización de firmas.".into(),
        },
        Question {
            id: QuestionId::D7FuncionRespuesta,
            text: "¿Existe planificación de respuesta ante incidentes, con comunicación, análisis y mitigación definidos?".into(),
            legal_anchor: "Art. 10° del Decreto 7 de 2023 (MINSEGPRES)".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Plan o procedimiento de respuesta a incidentes, con roles y vías de comunicación interna y externa.".into(),
        },
        Question {
            id: QuestionId::D7FuncionRecuperacion,
            text: "¿Existen planes de recuperación para restablecer las plataformas, servidores y servicios afectados por un incidente?".into(),
            legal_anchor: "Art. 11° del Decreto 7 de 2023 (MINSEGPRES)".into(),
            severity_if_no: Severity::Medium,
            applies_to: AppliesTo::All,
            infraction_class: None,
            evidence_example: "Plan de recuperación con sus tiempos objetivo y la última prueba de restauración realizada.".into(),
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
        // La exigibilidad depende del marco antes que del tier. El de la Ley 21.663 la
        // resuelve el tier, como siempre. El del Decreto 7 se mide **siempre** como
        // madurez, por dos razones que se sostienen solas: su guía técnica dice de sí
        // misma que "no crea obligaciones adicionales" y su §3.6 admite desarrollar la
        // Política gradualmente; y traducir su Art. 13°, que remite a la gradualidad del
        // DFL N°1, a un deber de seguridad exigible es interpretación jurídica que este
        // producto no hace. Ver `docs/research/0.7.0-*.md` §3.2.
        let exigibilidad = match question.id.domain().marco() {
            crate::maturity::Marco::Ley21663 => question.applies_to.exigibilidad_for(tier),
            crate::maturity::Marco::Decreto7 => Exigibilidad::MadurezVoluntaria,
        };
        let answer = response.get(question.id);
        let respondida = answer.is_some();
        let non_compliant = answer.map(|a| !a.compliant).unwrap_or(true);

        if !non_compliant {
            continue;
        }

        let estado = if respondida { "no cumplido" } else { "no respondido" };

        let evidence = answer
            .and_then(|a| a.notes.clone())
            .map(|n| vec![n])
            .unwrap_or_else(|| {
                vec![if respondida { "Declarado no cumplido" } else { "No respondido" }.into()]
            });

        let finding = match exigibilidad {
            Exigibilidad::Exigible => {
                format!("Control declarativo {estado}: {}", question.text)
            }
            Exigibilidad::MadurezVoluntaria => format!(
                "Brecha de madurez (no exigible a esta institución), control {estado}: {}",
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
    fn una_pregunta_sin_responder_no_se_afirma_como_incumplida() {
        let gaps = to_gaps(&QuestionnaireResponse::default(), Tier::Oiv);
        let sin_responder = gaps
            .iter()
            .find(|g| g.control.contains("Delegado"))
            .expect("sigue siendo brecha");
        assert!(!sin_responder.evaluated);
        assert!(
            !sin_responder.finding.contains("no cumplido"),
            "nadie la respondio: {}",
            sin_responder.finding
        );
        assert!(
            sin_responder.finding.contains("no respondido"),
            "{}",
            sin_responder.finding
        );
        assert_eq!(sin_responder.evidence, vec!["No respondido".to_string()]);
    }

    #[test]
    fn una_declarada_no_cumplida_si_se_afirma_como_incumplida() {
        let mut response = QuestionnaireResponse::default();
        response.answers.push(Answer {
            question_id: QuestionId::DelegadoCiberseguridad,
            compliant: false,
            notes: None,
        });
        let gaps = to_gaps(&response, Tier::Oiv);
        let declarada = gaps
            .iter()
            .find(|g| g.control.contains("Delegado"))
            .expect("sigue siendo brecha");
        assert!(declarada.evaluated);
        assert!(declarada.finding.contains("no cumplido"), "{}", declarada.finding);
        assert_eq!(declarada.evidence, vec!["Declarado no cumplido".to_string()]);
    }

    #[test]
    fn la_nota_del_operador_gana_sobre_el_texto_por_defecto() {
        let mut response = QuestionnaireResponse::default();
        response.answers.push(Answer {
            question_id: QuestionId::DelegadoCiberseguridad,
            compliant: false,
            notes: Some("Decreto alcaldicio en tramite".into()),
        });
        let gaps = to_gaps(&response, Tier::Oiv);
        let declarada = gaps.iter().find(|g| g.control.contains("Delegado")).unwrap();
        assert_eq!(declarada.evidence, vec!["Decreto alcaldicio en tramite".to_string()]);
    }

    #[test]
    fn sin_responder_una_de_madurez_tampoco_se_afirma_incumplida() {
        let gaps = to_gaps(&QuestionnaireResponse::default(), Tier::Pse);
        let sgsi = gaps.iter().find(|g| g.control.contains("SGSI")).unwrap();
        assert_eq!(sgsi.exigibilidad, Exigibilidad::MadurezVoluntaria);
        assert!(!sgsi.evaluated);
        assert!(sgsi.finding.contains("no respondido"), "{}", sgsi.finding);
    }

    #[test]
    fn no_responder_nada_no_reduce_el_numero_de_brechas() {
        let sin_responder = to_gaps(&QuestionnaireResponse::default(), Tier::Oiv);
        let mut todas_no = QuestionnaireResponse::default();
        for q in catalogue() {
            todas_no.answers.push(Answer { question_id: q.id, compliant: false, notes: None });
        }
        assert_eq!(sin_responder.len(), to_gaps(&todas_no, Tier::Oiv).len());
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

    /// Los cinco deberes propios del DS N°293.
    ///
    /// Se filtran por id y no por el texto del anclaje: el delegado cita el decreto
    /// tambien, pero su deber nace ademas del Art. 8° lit. i) de la ley, que si
    /// clasifica la infraccion.
    const IDS_RCSE: [QuestionId; 5] = [
        QuestionId::RcseIntegrada,
        QuestionId::RcseContratosSemestral,
        QuestionId::RcseMonitoreoTrafico,
        QuestionId::RcseDominioGobCl,
        QuestionId::RcseFqdnInformados,
    ];

    fn preguntas_rcse() -> Vec<Question> {
        catalogue().into_iter().filter(|q| IDS_RCSE.contains(&q.id)).collect()
    }

    // Le son exigibles a una municipalidad por el Art. 4° del decreto, que la nombra,
    // y no por el Art. 8° de la ley.
    #[test]
    fn the_rcse_duties_are_binding_on_a_municipality() {
        let textos: Vec<String> = preguntas_rcse().into_iter().map(|q| q.text).collect();
        assert_eq!(textos.len(), 5, "los cinco deberes del reglamento");

        let gaps = to_gaps(&QuestionnaireResponse::default(), Tier::Pse);
        for texto in textos {
            let g = gaps.iter().find(|g| g.control == texto).expect(&texto);
            assert_eq!(g.exigibilidad, Exigibilidad::Exigible, "{}", g.control);
        }
    }

    // El plazo que el decreto fija de verdad, y que el informe tiene que decir.
    #[test]
    fn the_six_month_contract_window_is_stated() {
        let q = catalogue()
            .into_iter()
            .find(|q| q.id == QuestionId::RcseContratosSemestral)
            .unwrap();
        assert!(q.text.contains("cada seis meses"), "{}", q.text);
        assert!(q.legal_anchor.contains("Art. 6°"), "{}", q.legal_anchor);
        assert!(q.legal_anchor.contains("semestral"), "{}", q.legal_anchor);
        // Y el plazo corto de las modificaciones, que es el que se olvida.
        assert!(q.legal_anchor.contains("15 días corridos"), "{}", q.legal_anchor);
    }

    // El plazo del subdominio .gob.cl vencio el 11-04-2026 y el informe lo dice.
    #[test]
    fn the_gob_cl_deadline_is_stated_with_its_date() {
        let q = catalogue()
            .into_iter()
            .find(|q| q.id == QuestionId::RcseDominioGobCl)
            .unwrap();
        assert!(q.legal_anchor.contains("11-04-2026"), "{}", q.legal_anchor);
        assert!(q.legal_anchor.contains("transitoria cuarta"), "{}", q.legal_anchor);
    }

    // El decreto no fija escala de infracciones propia. Afirmar una seria inventarla.
    #[test]
    fn the_rcse_duties_claim_no_infraction_class() {
        for q in preguntas_rcse() {
            assert!(
                q.infraction_class.is_none(),
                "{} no puede afirmar una sancion que el decreto no fija",
                q.text
            );
        }
    }

    // Cada anclaje de la RCSE tiene que nombrar su articulo: un deber sin articulo a
    // la vista no es auditable.
    #[test]
    fn every_rcse_anchor_names_its_article() {
        for q in preguntas_rcse() {
            assert!(
                q.legal_anchor.contains("Art. "),
                "{} sin articulo: {}",
                q.text,
                q.legal_anchor
            );
        }
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
    fn oiv_gets_every_ley_21663_duty_as_binding() {
        use crate::maturity::Marco;
        let gaps = to_gaps(&QuestionnaireResponse::default(), Tier::Oiv);
        assert_eq!(gaps.len(), catalogue().len());
        // Acotado a su marco: un OIV está obligado por todo lo de la Ley 21.663, pero el
        // Decreto 7 no se mide así para nadie. Antes de que existiera un segundo marco
        // esta prueba decía "todo", y decirlo hoy sería falso.
        let ley = gaps.iter().filter(|g| g.domain.marco() == Marco::Ley21663);
        assert!(ley.clone().count() > 0);
        assert!(ley.map(|g| g.exigibilidad).all(|e| e == Exigibilidad::Exigible));
    }

    #[test]
    fn el_decreto_7_se_mide_como_madurez_en_todos_los_tiers() {
        use crate::maturity::Marco;
        // Ni siquiera a un OIV se le afirma incumplimiento del Decreto 7: su guía técnica
        // dice que no crea obligaciones adicionales, y su §3.6 admite desarrollar la
        // Política gradualmente.
        for tier in [Tier::Oiv, Tier::Pse, Tier::Unclassified] {
            let gaps = to_gaps(&QuestionnaireResponse::default(), tier);
            let d7: Vec<_> = gaps.iter().filter(|g| g.domain.marco() == Marco::Decreto7).collect();
            assert!(!d7.is_empty(), "faltan las preguntas del Decreto 7 en {tier:?}");
            for g in d7 {
                assert_eq!(g.exigibilidad, Exigibilidad::MadurezVoluntaria, "{}", g.control);
                assert!(g.infraction_class.is_none(), "{} no puede clasificar infracción", g.control);
                assert!(!g.requires_csirt_report, "{} no dispara reporte al CSIRT", g.control);
            }
        }
    }

    #[test]
    fn las_cinco_funciones_del_decreto_7_tienen_pregunta() {
        use crate::maturity::{Domain, Marco};
        // Si una función quedara sin pregunta, su dominio saldría siempre "no medido" y
        // el informe mostraría una sección hueca.
        let gaps = to_gaps(&QuestionnaireResponse::default(), Tier::Pse);
        for d in Domain::de(Marco::Decreto7) {
            assert!(gaps.iter().any(|g| g.domain == d), "{d} sin ninguna pregunta");
        }
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