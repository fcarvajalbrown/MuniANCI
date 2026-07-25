//! Chile's official cybersecurity incident taxonomy (Res. Ex. N°7/2025).
//!
//! ## Qué es esto
//!
//! La Resolución Exenta N°7 de 2025 de la ANCI fija la taxonomía con que hay que
//! clasificar un incidente al reportarlo al CSIRT Nacional: cuatro áreas de impacto,
//! once efectos observables y cuarenta categorías. Este módulo la transcribe tal como
//! está publicada en el Diario Oficial, sin reformular nada.
//!
//! Obliga a "las instituciones públicas y privadas que presten servicios calificados
//! como esenciales" (Art. primero), lo que incluye a una municipalidad por el Art. 4°
//! inciso 2 de la Ley 21.663.
//!
//! ## Por qué el escáner NO clasifica solo
//!
//! El Art. segundo manda clasificar por "los efectos observables del **hecho
//! acaecido**". Un incidente que ocurrió. Lo que este producto detecta son brechas y
//! vulnerabilidades: condiciones que **todavía no** produjeron ningún hecho.
//!
//! Mapear una brecha a una categoría automáticamente afirmaría un incidente que no
//! ocurrió, en un documento dirigido al CSIRT Nacional. Por eso el catálogo se publica
//! como referencia y [`Clasificacion`] queda en `None` hasta que una persona —que sí
//! puede observar el hecho— la complete.
//!
//! ## Cómo se mantiene
//!
//! El texto es ley: no se corrige, no se resume y no se traduce. Si la ANCI publica una
//! resolución que reemplace a la N°7, se transcribe la nueva y se cambia [`FUENTE`].
//! Las pruebas fijan los conteos exactos (4 / 11 / 40) justamente para que una edición
//! descuidada no borre una categoría en silencio.

use serde::{Deserialize, Serialize};

/// Where the taxonomy comes from. Va al informe y al JSON.
///
/// Mismo criterio que `kev_provenance`: un documento que dice estar alineado a una
/// norma sin decir a qué edición de esa norma no es auditable.
pub const FUENTE: &str = "Res. Ex. N 7/2025 ANCI, D.O. N 44.088 del 2025-03-01, CVE 2617388";

/// One of the four areas of impact (Art. tercero).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    /// Letra con que la resolución la enumera: `a` a `d`.
    pub letra: char,
    pub nombre: &'static str,
    pub efectos: &'static [Efecto],
}

/// One of the eleven observable effects.
///
/// El `ordinal` es el romano de la enumeración plana del Art. cuarto (`i` a `xi`), no
/// el de la enumeración por área del Art. tercero, porque es el que identifica al
/// efecto sin ambigüedad. La `definicion` sí viene del Art. tercero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Efecto {
    pub ordinal: &'static str,
    pub nombre: &'static str,
    pub definicion: &'static str,
    pub categorias: &'static [&'static str],
}

/// The taxonomy, verbatim.
pub static CATALOGO: &[Area] = &[
    Area {
        letra: 'a',
        nombre: "Impacto en el uso legítimo de recursos",
        efectos: &[
            Efecto {
                ordinal: "i",
                nombre: "Uso no autorizado de redes y sistemas informáticos",
                definicion: "Uso no autorizado de sistemas de la institución afectada, ya sea a través de explotación de vulnerabilidades, uso no autorizado de credenciales, acceso a almacenamiento en nube, u otros.",
                categorias: &[
                    "Acceso no autorizado a almacenamiento",
                    "Ataque de fuerza bruta exitoso",
                    "Explotación de vulnerabilidades de autenticación",
                    "Uso de credenciales comprometidas",
                ],
            },
            Efecto {
                ordinal: "ii",
                nombre: "Actividades de phishing o fraude en infraestructura propia",
                definicion: "Envío de phishing a través de servidores de la institución afectada, o almacenamiento de sitios fraudulentos en redes y sistemas informáticos de la institución afectada.",
                categorias: &[
                    "Envío de correo no deseado o phishing desde infraestructura propia",
                    "Inclusión de sitio fraudulento en infraestructura propia",
                ],
            },
            Efecto {
                ordinal: "iii",
                nombre: "Actividades de phishing o fraude relacionadas con la institución",
                definicion: "Envío de phishing relacionado con la institución afectada por parte de terceros, o almacenamiento de sitios fraudulentos por parte de terceros.",
                categorias: &[
                    "Envío de correo no deseado o phishing sobre una organización",
                    "Envío de correo no deseado o phishing usando remitentes de la institución",
                ],
            },
            Efecto {
                ordinal: "iv",
                nombre: "Ejecución no autorizada de código",
                definicion: "Inclusión y ejecución no autorizada de código en sistemas de la institución afectada.",
                categorias: &[
                    "Ejecución remota de código a través de parámetros de aplicación",
                    "Inyección de requerimientos (prompts) en modelos grandes de lenguaje (LLM)",
                    "Inyección de consultas NoSQL",
                    "Inyección de consultas SQL",
                ],
            },
        ],
    },
    Area {
        letra: 'b',
        nombre: "Impacto en la confidencialidad de la información",
        efectos: &[
            Efecto {
                ordinal: "v",
                nombre: "Exfiltración y/o exposición de datos",
                definicion: "Pérdida de información confidencial con o sin divulgación pública, y/o información confidencial expuesta accidental o intencionalmente.",
                categorias: &[
                    "Adversario en el medio (MitM)",
                    "Apropiación de credenciales mediante phishing",
                    "Base de datos sin protección (S3 buckets, Elasticsearch, MongoDB expuestos)",
                    "Documentos públicos con datos sensibles",
                    "Filtración de datos personales",
                    "Keylogger en uso",
                    "Divulgación de enumeraciones de usuarios y/o credenciales de usuarios en foros",
                ],
            },
            Efecto {
                ordinal: "vi",
                nombre: "Exfiltración y/o exposición de configuraciones",
                definicion: "Pérdida o exposición accidental de configuraciones y parámetros confidenciales de un sistema o aplicación de la institución afectada.",
                categorias: &[
                    "Filtración de configuraciones en rutas de aplicación",
                    "Filtración de secretos en rutas de aplicación",
                ],
            },
            Efecto {
                ordinal: "vii",
                nombre: "Exfiltración y/o exposición de código fuente",
                definicion: "Pérdida o exposición excesiva del código fuente de un sistema de la institución afectada.",
                categorias: &[
                    "Archivo(s) de control de versión expuestos en aplicación",
                    "Sistema de control de versión expuesto",
                ],
            },
        ],
    },
    Area {
        letra: 'c',
        nombre: "Impacto en la disponibilidad de un servicio esencial",
        efectos: &[
            Efecto {
                ordinal: "viii",
                nombre: "Indisponibilidad y/o denegación de servicio",
                definicion: "Pérdida total del funcionamiento de un servicio, sistema o servidor, o saturación de red impidiendo su operación normal.",
                categorias: &[
                    "Agotamiento de conexiones TCP",
                    "Apagado no autorizado de sistemas informáticos",
                    "Ataque de amplificación DNS/NTP",
                    "Ataque físico contra infraestructura TI",
                    "Denegación de servicio a través de la explotación de vulnerabilidades",
                    "Eliminación de configuraciones críticas",
                    "Tráfico de red excesivo (volumétrico)",
                ],
            },
            Efecto {
                ordinal: "ix",
                nombre: "Degradación de servicio",
                definicion: "Pérdida parcial del rendimiento o funcionalidad de un servicio, sistema o servidor.",
                categorias: &[
                    "Secuestro de recursos (cryptojacking)",
                    "Sobrecarga de bases de datos",
                    "Uso excesivo de ancho de banda",
                ],
            },
        ],
    },
    Area {
        letra: 'd',
        nombre: "Impacto en la integridad de la información",
        efectos: &[
            Efecto {
                ordinal: "x",
                nombre: "Modificación no autorizada de datos",
                definicion: "Alteración no autorizada de información contenida en sistemas, servicios o servidores.",
                categorias: &[
                    "Alteración de bases de datos",
                    "Alteración de sitio web (defacement)",
                    "Manipulación de datos no autentificados",
                    "Modificación de logs de auditoría",
                ],
            },
            Efecto {
                ordinal: "xi",
                nombre: "Manipulación no autorizada de configuración",
                definicion: "Cambio no autorizado de configuraciones en sistemas, servicios o servidores.",
                categorias: &[
                    "Alteración de reglas de firewall",
                    "Desactivación de registros de seguridad",
                    "Modificación de políticas de acceso",
                ],
            },
        ],
    },
];

/// Every observable effect, flattened in the order of Art. cuarto.
pub fn efectos() -> impl Iterator<Item = &'static Efecto> {
    CATALOGO.iter().flat_map(|a| a.efectos.iter())
}

/// How many categories the taxonomy defines.
pub fn total_categorias() -> usize {
    efectos().map(|e| e.categorias.len()).sum()
}

/// Finds the effect an observable category belongs to, with its area.
///
/// Devuelve el área además del efecto porque un reporte al CSIRT nombra los tres
/// niveles, y deducir el área a partir del efecto fuera de aquí invita a que alguien
/// la deduzca mal.
pub fn buscar(categoria: &str) -> Option<(&'static Area, &'static Efecto)> {
    CATALOGO.iter().find_map(|area| {
        area.efectos
            .iter()
            .find(|e| e.categorias.contains(&categoria))
            .map(|efecto| (area, efecto))
    })
}

/// A human's classification of an actual incident.
///
/// No la produce el escáner. Existe para que el JSON tenga dónde recibirla cuando un
/// funcionario clasifique un hecho que sí ocurrió, y para poder validarla contra el
/// catálogo antes de que salga hacia el CSIRT.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clasificacion {
    pub area: String,
    pub efecto: String,
    pub categoria: String,
}

impl Clasificacion {
    /// Builds a classification from a category, filling in its area and effect.
    ///
    /// `None` si la categoría no está en la resolución. Es deliberado que no exista
    /// forma de construir una `Clasificacion` con texto libre: una categoría inventada
    /// en un reporte al CSIRT es exactamente lo que este módulo evita.
    pub fn de_categoria(categoria: &str) -> Option<Self> {
        buscar(categoria).map(|(area, efecto)| Self {
            area: area.nombre.to_string(),
            efecto: efecto.nombre.to_string(),
            categoria: categoria.to_string(),
        })
    }

    /// Whether the three levels are consistent with the official taxonomy.
    pub fn es_valida(&self) -> bool {
        matches!(buscar(&self.categoria),
            Some((area, efecto)) if area.nombre == self.area && efecto.nombre == self.efecto)
    }
}

/// The reference block the CSIRT JSON carries.
///
/// Lleva los conteos y la procedencia, no las cuarenta categorías: repetir el catálogo
/// entero en el JSON de cada escaneo lo llenaría de texto legal idéntico. Quien lo
/// necesite tiene [`CATALOGO`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxonomiaAnci {
    /// `String` y no `&'static str` a propósito: el `ScanResult` completo se
    /// deserializa desde el JSON de un escaneo anterior, y un campo prestado ataría
    /// toda la estructura a `'static`.
    pub fuente: String,
    pub areas_impacto: usize,
    pub efectos_observables: usize,
    pub categorias: usize,
    /// La clasificación del incidente, cuando una persona la haya hecho.
    ///
    /// Siempre `None` en la salida de un escaneo: el escáner detecta brechas, no
    /// hechos acaecidos, y el Art. segundo clasifica hechos acaecidos.
    pub clasificacion_incidente: Option<Clasificacion>,
}

impl Default for TaxonomiaAnci {
    fn default() -> Self {
        Self {
            fuente: FUENTE.to_string(),
            areas_impacto: CATALOGO.len(),
            efectos_observables: efectos().count(),
            categorias: total_categorias(),
            clasificacion_incidente: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Los tres conteos que fija el texto oficial. Si una edición descuidada borra una
    // categoria, esta prueba es la que lo dice.
    #[test]
    fn the_official_counts_are_four_eleven_and_forty() {
        assert_eq!(CATALOGO.len(), 4, "Art. tercero: cuatro areas de impacto");
        assert_eq!(efectos().count(), 11, "Art. tercero: once efectos observables");
        assert_eq!(total_categorias(), 40, "Art. cuarto: cuarenta categorias");
    }

    // El Art. tercero reparte los once efectos como 4-3-2-2 entre las cuatro areas.
    #[test]
    fn each_area_carries_the_effects_the_resolution_gives_it() {
        let reparto: Vec<usize> = CATALOGO.iter().map(|a| a.efectos.len()).collect();
        assert_eq!(reparto, vec![4, 3, 2, 2]);
    }

    // El Art. cuarto reparte las cuarenta categorias asi, efecto por efecto.
    #[test]
    fn each_effect_carries_the_categories_the_resolution_gives_it() {
        let reparto: Vec<usize> = efectos().map(|e| e.categorias.len()).collect();
        assert_eq!(reparto, vec![4, 2, 2, 4, 7, 2, 2, 7, 3, 4, 3]);
    }

    #[test]
    fn the_areas_are_lettered_a_through_d_in_order() {
        let letras: Vec<char> = CATALOGO.iter().map(|a| a.letra).collect();
        assert_eq!(letras, vec!['a', 'b', 'c', 'd']);
    }

    // La enumeracion plana del Art. cuarto, en su orden.
    #[test]
    fn the_effects_are_numbered_i_through_xi_in_order() {
        let ordinales: Vec<&str> = efectos().map(|e| e.ordinal).collect();
        assert_eq!(
            ordinales,
            vec!["i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi"]
        );
    }

    #[test]
    fn nothing_in_the_catalogue_is_empty() {
        for area in CATALOGO {
            assert!(!area.nombre.trim().is_empty());
            assert!(!area.efectos.is_empty(), "{} sin efectos", area.nombre);
            for efecto in area.efectos {
                assert!(!efecto.nombre.trim().is_empty());
                assert!(!efecto.definicion.trim().is_empty(), "{} sin definicion", efecto.nombre);
                assert!(!efecto.categorias.is_empty(), "{} sin categorias", efecto.nombre);
                for cat in efecto.categorias {
                    assert!(!cat.trim().is_empty(), "categoria vacia en {}", efecto.nombre);
                }
            }
        }
    }

    // Una categoria duplicada haria ambigua a `buscar`: devolveria la primera y
    // clasificaria el incidente bajo el area equivocada.
    #[test]
    fn no_category_appears_twice_in_the_whole_taxonomy() {
        let mut vistas = HashSet::new();
        for efecto in efectos() {
            for cat in efecto.categorias {
                assert!(vistas.insert(*cat), "categoria duplicada: {cat}");
            }
        }
        assert_eq!(vistas.len(), 40);
    }

    #[test]
    fn no_effect_name_appears_twice() {
        let nombres: HashSet<&str> = efectos().map(|e| e.nombre).collect();
        assert_eq!(nombres.len(), 11);
    }

    // Los dos primeros efectos comparten casi todo el nombre y se distinguen solo por
    // "en infraestructura propia" contra "relacionadas con la institucion": son
    // efectos distintos en la resolucion y no pueden colapsarse.
    #[test]
    fn the_two_phishing_effects_stay_distinct() {
        let phishing: Vec<&Efecto> = efectos()
            .filter(|e| e.nombre.starts_with("Actividades de phishing"))
            .collect();
        assert_eq!(phishing.len(), 2);
        assert_ne!(phishing[0].nombre, phishing[1].nombre);
        assert_ne!(phishing[0].categorias, phishing[1].categorias);
    }

    // Verbatim contra el Diario Oficial. Tres muestras de sitios distintos del texto:
    // si alguien "corrige" la redaccion oficial, se cae aca.
    #[test]
    fn sample_categories_match_the_official_wording() {
        let (area, efecto) = buscar(
            "Inyección de requerimientos (prompts) en modelos grandes de lenguaje (LLM)",
        )
        .expect("esta categoria esta en el Art. cuarto iv");
        assert_eq!(area.letra, 'a');
        assert_eq!(efecto.ordinal, "iv");

        let (area, efecto) =
            buscar("Base de datos sin protección (S3 buckets, Elasticsearch, MongoDB expuestos)")
                .expect("Art. cuarto v");
        assert_eq!(area.nombre, "Impacto en la confidencialidad de la información");
        assert_eq!(efecto.ordinal, "v");

        let (area, efecto) = buscar("Alteración de sitio web (defacement)").expect("Art. cuarto x");
        assert_eq!(area.letra, 'd');
        assert_eq!(efecto.nombre, "Modificación no autorizada de datos");
    }

    #[test]
    fn an_unknown_category_is_not_found() {
        assert!(buscar("Ransomware").is_none(), "no es una categoria de la resolucion");
        assert!(buscar("").is_none());
        // La busqueda es exacta: la resolucion nombra categorias, no palabras clave.
        assert!(buscar("defacement").is_none());
    }

    #[test]
    fn a_classification_is_built_from_its_category_and_fills_the_other_two_levels() {
        let c = Clasificacion::de_categoria("Uso de credenciales comprometidas").unwrap();
        assert_eq!(c.area, "Impacto en el uso legítimo de recursos");
        assert_eq!(c.efecto, "Uso no autorizado de redes y sistemas informáticos");
        assert!(c.es_valida());
    }

    #[test]
    fn a_classification_cannot_be_built_from_an_invented_category() {
        assert!(Clasificacion::de_categoria("Ataque de ransomware").is_none());
    }

    // Un JSON traido de afuera puede venir con los tres niveles descoordinados.
    #[test]
    fn a_classification_with_mismatched_levels_is_rejected() {
        let mut c = Clasificacion::de_categoria("Alteración de reglas de firewall").unwrap();
        assert!(c.es_valida());

        c.area = "Impacto en la disponibilidad de un servicio esencial".into();
        assert!(!c.es_valida(), "el area no corresponde a esa categoria");

        let mut c = Clasificacion::de_categoria("Alteración de reglas de firewall").unwrap();
        c.efecto = "Degradación de servicio".into();
        assert!(!c.es_valida(), "el efecto no corresponde a esa categoria");

        let mut c = Clasificacion::de_categoria("Alteración de reglas de firewall").unwrap();
        c.categoria = "Categoria inventada".into();
        assert!(!c.es_valida());
    }

    // Toda categoria del catalogo tiene que poder recorrer el viaje completo.
    #[test]
    fn every_category_in_the_catalogue_round_trips_through_a_classification() {
        for efecto in efectos() {
            for cat in efecto.categorias {
                let c = Clasificacion::de_categoria(cat)
                    .unwrap_or_else(|| panic!("no se encontro {cat}"));
                assert!(c.es_valida(), "{cat} no se valida contra su propio catalogo");
                assert_eq!(c.efecto, efecto.nombre);
            }
        }
    }

    #[test]
    fn the_reference_block_reports_the_counts_and_leaves_the_incident_unclassified() {
        let t = TaxonomiaAnci::default();
        assert_eq!(t.areas_impacto, 4);
        assert_eq!(t.efectos_observables, 11);
        assert_eq!(t.categorias, 40);
        assert_eq!(
            t.clasificacion_incidente, None,
            "un escaneo detecta brechas, no hechos acaecidos"
        );
    }

    // La procedencia tiene que identificar la edicion exacta, igual que kev_provenance.
    #[test]
    fn the_provenance_names_the_resolution_and_where_it_was_published() {
        assert!(FUENTE.contains("N 7/2025"), "{FUENTE}");
        assert!(FUENTE.contains("ANCI"), "{FUENTE}");
        assert!(FUENTE.contains("2025-03-01"), "{FUENTE}");
        assert!(FUENTE.contains("2617388"), "el CVE del Diario Oficial: {FUENTE}");
        assert_eq!(TaxonomiaAnci::default().fuente, FUENTE);
    }

    #[test]
    fn the_reference_block_survives_a_json_round_trip() {
        let t = TaxonomiaAnci {
            clasificacion_incidente: Clasificacion::de_categoria("Keylogger en uso"),
            ..Default::default()
        };
        let json = serde_json::to_string(&t).unwrap();
        let leido: TaxonomiaAnci = serde_json::from_str(&json).unwrap();
        assert_eq!(leido, t);
        assert!(leido.clasificacion_incidente.unwrap().es_valida());
    }

    // El JSON va al CSIRT Nacional: los nombres de campo son parte del contrato.
    #[test]
    fn the_json_field_names_are_the_ones_the_report_promises() {
        let json = serde_json::to_value(TaxonomiaAnci::default()).unwrap();
        for campo in [
            "fuente",
            "areas_impacto",
            "efectos_observables",
            "categorias",
            "clasificacion_incidente",
        ] {
            assert!(json.get(campo).is_some(), "falta el campo {campo}");
        }
        assert!(json["clasificacion_incidente"].is_null());
    }
}
