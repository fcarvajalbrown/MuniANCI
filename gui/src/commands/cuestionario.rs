use munigpt_core::config::{Config, CuestionarioConfig, RespuestaConfig};
use munigpt_core::questionnaire::{catalogue, exigibilidad_de};
use munigpt_core::types::Exigibilidad;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreguntaCuestionario {
    pub clave: String,
    pub texto: String,
    pub anclaje_legal: String,
    pub ejemplo_evidencia: String,
    pub dominio: String,
    pub severidad: String,
    pub exigible: bool,
    pub respondida: bool,
    pub cumple: bool,
    pub nota: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RespuestaEntrante {
    pub clave: String,
    pub cumple: bool,
    pub nota: Option<String>,
}

#[tauri::command]
pub fn cuestionario_leer() -> Vec<PreguntaCuestionario> {
    let (config, _) = Config::load();
    let tier = super::start_scan::tier_resuelto();

    catalogue()
        .into_iter()
        .map(|pregunta| {
            let clave = pregunta.id.clave().unwrap_or_default();
            let guardada = config.cuestionario.respuestas.get(&clave);

            PreguntaCuestionario {
                exigible: exigibilidad_de(&pregunta, tier) == Exigibilidad::Exigible,
                dominio: pregunta.id.domain().title().to_string(),
                severidad: pregunta.severity_if_no.to_string(),
                texto: pregunta.text,
                anclaje_legal: pregunta.legal_anchor,
                ejemplo_evidencia: pregunta.evidence_example,
                respondida: guardada.is_some(),
                cumple: guardada.map(|r| r.cumple).unwrap_or(false),
                nota: guardada.and_then(|r| r.nota.clone()),
                clave,
            }
        })
        .collect()
}

#[tauri::command]
pub fn cuestionario_guardar(respuestas: Vec<RespuestaEntrante>) -> Result<String, String> {
    let claves_validas: std::collections::BTreeSet<String> = catalogue()
        .iter()
        .filter_map(|p| p.id.clave())
        .collect();

    let mut guardadas: BTreeMap<String, RespuestaConfig> = BTreeMap::new();
    for r in respuestas {
        if !claves_validas.contains(&r.clave) {
            return Err(format!("La pregunta '{}' no existe en el catalogo.", r.clave));
        }
        let nota = r.nota.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
        guardadas.insert(r.clave, RespuestaConfig { cumple: r.cumple, nota });
    }

    let (mut config, _) = Config::load();
    config.cuestionario = CuestionarioConfig { respuestas: guardadas };

    let ruta = munigpt_core::config::ruta_escritura()
        .ok_or("No se pudo determinar donde guardar la configuracion.")?;
    config.guardar(&ruta).map_err(|e| e.to_string())?;

    Ok(ruta.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_catalogo_se_expone_entero_y_con_claves_estables() {
        let preguntas = cuestionario_leer();

        assert_eq!(preguntas.len(), catalogue().len());
        assert!(preguntas.iter().all(|p| !p.clave.is_empty()));
        assert!(preguntas.iter().all(|p| !p.texto.is_empty()));
    }

    #[test]
    fn una_clave_inventada_se_rechaza_en_vez_de_guardarse() {
        let error = cuestionario_guardar(vec![RespuestaEntrante {
            clave: "control_que_no_existe".into(),
            cumple: true,
            nota: None,
        }])
        .unwrap_err();

        assert!(error.contains("no existe"), "{error}");
    }
}
