// Registro de riesgos: leer y anotar el estado de cada hallazgo desde la GUI.
//
// El seguimiento hasta el cierre vive en `core::historico`, pero sin estos dos comandos
// solo se podría operar desde la línea de comandos, que no es lo que abre el área de TI
// de una municipalidad. Mismo patrón que `monitoreo.rs`: la GUI lee estado y lo escribe.
use muniani_core::historico::{EstadoRiesgo, Historico, Riesgo, nombre_archivo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, thiserror::Error)]
pub enum RiesgoError {
    #[error("No se pudo abrir el registro de riesgos: {0}")]
    Historico(String),
}

/// Una fila del registro, tal como la ve la interfaz.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiesgoUi {
    pub id: String,
    pub control: String,
    /// `abierto` | `investigando` | `cerrado` | `falso_positivo` | `aceptado`.
    pub estado: String,
    pub responsable: Option<String>,
    pub plazo: Option<String>,
    pub nota: Option<String>,
    pub cerrado_el: Option<String>,
    pub actualizado: String,
}

impl From<Riesgo> for RiesgoUi {
    fn from(r: Riesgo) -> Self {
        Self {
            id: r.id,
            control: r.control,
            estado: r.estado.texto().to_string(),
            responsable: r.responsable,
            plazo: r.plazo,
            nota: r.nota,
            cerrado_el: r.cerrado_el,
            actualizado: r.actualizado,
        }
    }
}

fn ruta(institucion: &str) -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_default()
        .join(nombre_archivo(institucion))
}

/// Tauri command: todo lo anotado hasta ahora.
#[tauri::command]
pub async fn listar_riesgos() -> Result<Vec<RiesgoUi>, RiesgoError> {
    let p = ruta(&super::branding::institution());
    // Que el histórico no exista todavía no es un error: es la primera vez.
    if !p.exists() {
        return Ok(Vec::new());
    }
    let h = Historico::abrir(&p).map_err(|e| RiesgoError::Historico(e.to_string()))?;
    let filas = h.riesgos().map_err(|e| RiesgoError::Historico(e.to_string()))?;
    Ok(filas.into_iter().map(RiesgoUi::from).collect())
}

/// Tauri command: anota o actualiza el estado de un hallazgo.
///
/// `estado` viaja como texto y no como enum para no obligar al frontend a espejar el
/// enum de Rust; un valor desconocido se lee como `abierto`, que es el error seguro
/// porque deja el hallazgo a la vista.
#[tauri::command]
pub async fn anotar_riesgo(
    control: String,
    estado: String,
    responsable: Option<String>,
    plazo: Option<String>,
    nota: Option<String>,
) -> Result<RiesgoUi, RiesgoError> {
    // El identificador se deriva aquí y no llega desde la interfaz. Es el mismo UUID v5
    // que el POA&M emite en `risk/uuid`, así que guardar el estado bajo cualquier otra
    // clave lo desconectaría del documento que entrega la municipalidad, sin aviso.
    let id = muniani_core::poam::id_de_riesgo_de_control(&control).to_string();
    let p = ruta(&super::branding::institution());
    let mut h = Historico::abrir(&p).map_err(|e| RiesgoError::Historico(e.to_string()))?;

    // La fecha de cierre la administra `core`, así que se le pasa la que ya había: sin
    // esto, reabrir y volver a cerrar borraría cuándo se cerró la primera vez.
    let previo = h.riesgo(&id).map_err(|e| RiesgoError::Historico(e.to_string()))?;

    let r = Riesgo {
        id: id.clone(),
        control,
        estado: EstadoRiesgo::desde_texto(&estado),
        responsable,
        plazo,
        nota,
        cerrado_el: previo.and_then(|p| p.cerrado_el),
        actualizado: String::new(),
    };
    h.anotar_riesgo(&r).map_err(|e| RiesgoError::Historico(e.to_string()))?;

    let guardado = h
        .riesgo(&id)
        .map_err(|e| RiesgoError::Historico(e.to_string()))?
        .ok_or_else(|| RiesgoError::Historico("no se pudo releer lo anotado".into()))?;
    Ok(guardado.into())
}
