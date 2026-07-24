//! Runtime configuration the municipality's IT staff can edit.
//!
//! ## Por qué existe
//!
//! Hasta acá todo lo ajustable del producto estaba compilado (la institución, el
//! tier) o escondido en variables de entorno del sidecar. Los plazos del plan de
//! remediación son lo primero que TI municipal necesita mover: 15 días para una
//! brecha crítica puede ser razonable en una municipalidad con equipo propio y
//! absurdo en una que depende de un proveedor externo.
//!
//! ## Dónde vive
//!
//! Se resuelve en dos pasos, y el primero que exista gana:
//!
//! 1. La ruta de `MUNIANI_CONFIG`.
//! 2. `munianci.config.json` junto al ejecutable.
//!
//! Si no hay ninguno, rigen los valores por defecto. Es el mismo patrón del
//! catálogo KEV: editable con el Bloc de notas, sin rebuild ni instalador, y a la
//! vista de quien audite el equipo.
//!
//! ## Cómo se agregan bloques nuevos
//!
//! Cada área del producto aporta su propia sección al struct [`Config`], con
//! `#[serde(default)]` para que un archivo viejo siga cargando cuando aparezcan
//! campos nuevos. `munianci --escribir-config` regenera el ejemplo comentado.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Variable de entorno que fuerza una ruta de configuración.
pub const CONFIG_ENV: &str = "MUNIANI_CONFIG";

/// Nombre que se busca junto al ejecutable.
pub const CONFIG_FILE_NAME: &str = "munianci.config.json";

/// Everything IT can tune without rebuilding.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Comentario de cabecera del archivo. Se emite al escribir el ejemplo y se
    /// ignora al leer: JSON no admite comentarios y este es el sustituto.
    #[serde(rename = "_ayuda", skip_serializing_if = "Vec::is_empty")]
    pub ayuda: Vec<String>,
    pub poam: PoamConfig,
}

/// Remediation plan settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PoamConfig {
    /// Plazo sugerido, en días corridos, para una brecha crítica.
    pub plazo_dias_critica: u32,
    /// Plazo sugerido, en días corridos, para una brecha alta.
    pub plazo_dias_alta: u32,
    /// Plazo sugerido, en días corridos, para una brecha media.
    pub plazo_dias_media: u32,
}

impl Default for PoamConfig {
    /// Valores de partida, no plazos legales.
    ///
    /// La única fecha que el régimen chileno sí fija es la del reporte del Art. 9°
    /// (3 horas), que el informe trata aparte. Estos tres números son un criterio
    /// operativo propio, pensado para que TI los ajuste a su realidad.
    fn default() -> Self {
        Self {
            plazo_dias_critica: 15,
            plazo_dias_alta: 30,
            plazo_dias_media: 90,
        }
    }
}

impl PoamConfig {
    /// The suggested deadline in days for a severity.
    pub fn plazo_dias(&self, severity: crate::types::Severity) -> u32 {
        use crate::types::Severity::*;
        match severity {
            Critical => self.plazo_dias_critica,
            High => self.plazo_dias_alta,
            Medium => self.plazo_dias_media,
        }
    }
}

/// Where the effective configuration came from. Va al informe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    /// No había archivo: rigen los valores por defecto del producto.
    PorDefecto,
    /// Se leyó de disco.
    Archivo(PathBuf),
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::PorDefecto => write!(f, "valores por defecto del producto"),
            ConfigSource::Archivo(p) => write!(f, "{}", p.display()),
        }
    }
}

impl Config {
    /// Loads the configuration, falling back to the defaults.
    ///
    /// Un archivo ilegible **no** degrada en silencio: se avisa por stderr y se
    /// sigue con los valores por defecto, para que TI note que su edición no tomó
    /// efecto en vez de descubrirlo en un informe con plazos equivocados.
    pub fn load() -> (Self, ConfigSource) {
        for path in candidate_paths() {
            if !path.exists() {
                continue;
            }
            match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| serde_json::from_str::<Config>(sin_bom(&t)).map_err(|e| e.to_string()))
            {
                Ok(cfg) => return (cfg, ConfigSource::Archivo(path)),
                Err(why) => eprintln!(
                    "[config] {} no se pudo leer: {why} — rigen los valores por defecto",
                    path.display()
                ),
            }
        }
        (Config::default(), ConfigSource::PorDefecto)
    }

    /// The annotated example written by `--escribir-config`.
    ///
    /// Nadie configura lo que no sabe que existe: el archivo lleva sus propios
    /// valores por defecto y una descripción de cada campo.
    pub fn ejemplo() -> Self {
        Config {
            ayuda: vec![
                "Configuración de MuniANCI. Editable por el área de TI municipal.".into(),
                "Se busca en MUNIANI_CONFIG y, si no, junto al ejecutable como munianci.config.json.".into(),
                "Si este archivo no existe, rigen los valores por defecto del producto.".into(),
                "".into(),
                "poam.plazo_dias_*: plazo sugerido de corrección, en días corridos, por severidad.".into(),
                "  NO son plazos legales. El régimen de la Ley 21.663 fija un solo plazo".into(),
                "  perentorio, el del reporte al CSIRT del Art. 9° (3 horas), que el informe".into(),
                "  trata aparte. Estos son un criterio operativo que cada municipalidad ajusta.".into(),
            ],
            poam: PoamConfig::default(),
        }
    }

    /// Writes the annotated example to `path`.
    pub fn escribir_ejemplo(path: &std::path::Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(&Config::ejemplo())
            .expect("la configuración de ejemplo siempre serializa");
        std::fs::write(path, json + "\n")
    }
}

/// Strips a UTF-8 byte order mark, if the editor left one.
///
/// No es un detalle menor en este producto: el Bloc de notas y PowerShell, que es
/// con lo que TI municipal va a editar este archivo en Windows, escriben UTF-8 con
/// BOM por defecto, y `serde_json` lo rechaza con "expected value at line 1
/// column 1". Sin esto, la primera edición de TI se descarta en silencio y el
/// informe sale con los plazos que TI creía haber cambiado.
pub fn sin_bom(texto: &str) -> &str {
    texto.strip_prefix('\u{feff}').unwrap_or(texto)
}

/// Candidate config paths, in priority order.
fn candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(p) = std::env::var(CONFIG_ENV) {
        if !p.trim().is_empty() {
            out.push(PathBuf::from(p));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join(CONFIG_FILE_NAME));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Severity;

    #[test]
    fn defaults_order_the_deadlines_by_urgency() {
        let c = PoamConfig::default();
        assert!(c.plazo_dias_critica < c.plazo_dias_alta);
        assert!(c.plazo_dias_alta < c.plazo_dias_media);
    }

    #[test]
    fn each_severity_maps_to_its_deadline() {
        let c = PoamConfig::default();
        assert_eq!(c.plazo_dias(Severity::Critical), 15);
        assert_eq!(c.plazo_dias(Severity::High), 30);
        assert_eq!(c.plazo_dias(Severity::Medium), 90);
    }

    #[test]
    fn it_can_override_a_single_deadline_without_restating_the_rest() {
        // Un municipio que depende de un proveedor externo necesita mas margen.
        let c: Config = serde_json::from_str(r#"{"poam":{"plazo_dias_critica":45}}"#).unwrap();
        assert_eq!(c.poam.plazo_dias_critica, 45);
        assert_eq!(c.poam.plazo_dias_alta, 30, "los demas conservan el defecto");
    }

    #[test]
    fn an_empty_file_is_the_default_configuration() {
        let c: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(c.poam, PoamConfig::default());
    }

    // Un archivo escrito por una version futura, con campos que esta no conoce,
    // tiene que seguir cargando en vez de dejar al municipio sin configuracion.
    #[test]
    fn unknown_fields_do_not_break_loading() {
        let c: Config =
            serde_json::from_str(r#"{"poam":{"plazo_dias_alta":45},"bloque_futuro":{"x":1}}"#)
                .unwrap();
        assert_eq!(c.poam.plazo_dias_alta, 45);
    }

    #[test]
    fn the_example_explains_that_the_deadlines_are_not_legal() {
        let ayuda = Config::ejemplo().ayuda.join(" ");
        assert!(ayuda.contains("NO son plazos legales"), "{ayuda}");
        assert!(ayuda.contains("Art. 9"), "{ayuda}");
    }

    // El Bloc de notas y PowerShell escriben UTF-8 con BOM por defecto en Windows.
    // Sin esto, la primera edicion de TI se descartaba en silencio: medido, no
    // supuesto, al probar el override de plazos de punta a punta.
    #[test]
    fn a_file_saved_by_notepad_with_a_bom_still_loads() {
        let con_bom = "\u{feff}{\"poam\":{\"plazo_dias_alta\":3}}";
        assert!(serde_json::from_str::<Config>(con_bom).is_err(), "el BOM si rompe a serde");
        let c: Config = serde_json::from_str(sin_bom(con_bom)).unwrap();
        assert_eq!(c.poam.plazo_dias_alta, 3);
    }

    #[test]
    fn stripping_the_bom_leaves_a_clean_file_untouched() {
        assert_eq!(sin_bom("{}"), "{}");
    }

    #[test]
    fn the_example_round_trips_through_the_loader() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        Config::escribir_ejemplo(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let leido: Config = serde_json::from_str(&text).unwrap();
        assert_eq!(leido.poam, PoamConfig::default());
    }
}
