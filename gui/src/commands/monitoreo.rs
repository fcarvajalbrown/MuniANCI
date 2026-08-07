// Estado del monitoreo continuo y exportación del paquete de evidencia.
//
// Son los dos botones que le faltaban al hito 0.6.0: la aplicación ya sabía calcular la
// deriva y armar el paquete, pero solo desde la línea de comandos, que no es lo que un
// funcionario municipal abre.
use munigpt_core::{
    config::Config,
    evidencia,
    historico::{Historico, nombre_archivo},
    programacion,
    types::ScanResult,
};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_shell::ShellExt;

/// What the UI needs to decide whether to nag about a stale measurement.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoMonitoreo {
    /// Fecha del último escaneo registrado, en RFC 3339.
    pub ultimo_escaneo: Option<String>,
    /// Días transcurridos desde entonces.
    pub dias: Option<i64>,
    /// Si esos días superan el umbral configurado.
    pub vencido: bool,
    /// Umbral configurado, para poder decirlo en el aviso.
    pub umbral_dias: u32,
    /// Cuántas mediciones lleva esta institución.
    pub mediciones: usize,
    /// Si existe la tarea en el Programador de tareas de Windows.
    pub tarea_programada: bool,
    /// Lo que hay que advertir antes de crear la tarea.
    pub advertencia: String,
}

#[derive(Debug, Serialize, thiserror::Error)]
pub enum MonitoreoError {
    #[error("Exportación cancelada por el usuario.")]
    Cancelada,
    #[error("No se pudo leer el histórico: {0}")]
    Historico(String),
    #[error("No se pudo generar el paquete de evidencia: {0}")]
    Evidencia(String),
}

/// Where the per-comuna history lives: next to the executable, like the config.
fn ruta_historico(institucion: &str) -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(nombre_archivo(institucion))
}

/// Tauri command — how long since the last scan, and whether that is too long.
///
/// Es la red de seguridad del reescaneo programado: si una política de grupo impidió
/// crear la tarea, esto es lo único que le va a recordar a la municipalidad que su
/// medición envejeció.
#[tauri::command]
pub async fn estado_monitoreo() -> Result<EstadoMonitoreo, MonitoreoError> {
    let (config, _) = Config::load();
    let institucion = super::branding::institution();
    let ruta = ruta_historico(&institucion);

    // Un histórico que todavía no existe no es un error: es la primera vez.
    let (ultimo, mediciones) = if ruta.exists() {
        let h = Historico::abrir(&ruta).map_err(|e| MonitoreoError::Historico(e.to_string()))?;
        let u = h.ultimo().map_err(|e| MonitoreoError::Historico(e.to_string()))?;
        let n = h.cuantos().map_err(|e| MonitoreoError::Historico(e.to_string()))?;
        (u, n)
    } else {
        (None, 0)
    };

    let dias = ultimo.as_ref().and_then(|r| {
        chrono::DateTime::parse_from_rfc3339(&r.fecha)
            .ok()
            .map(|f| (chrono::Utc::now() - f.with_timezone(&chrono::Utc)).num_days())
    });

    Ok(EstadoMonitoreo {
        vencido: dias.is_some_and(|d| d >= i64::from(config.monitoreo.aviso_vencido_dias)),
        umbral_dias: config.monitoreo.aviso_vencido_dias,
        ultimo_escaneo: ultimo.map(|r| r.fecha),
        dias,
        mediciones,
        // Un error consultando la tarea no puede tumbar el aviso: se informa que no
        // hay tarea, que es lo conservador.
        tarea_programada: programacion::programada().unwrap_or(false),
        advertencia: programacion::ADVERTENCIA.to_string(),
    })
}

/// What the export produced, so the UI can say it plainly.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenciaExportada {
    pub ruta: String,
    pub archivos: usize,
    pub bytes: u64,
    pub oxum: String,
    pub manifiesto: String,
    pub instrucciones: String,
}

/// Tauri command — picks a folder and writes the dated, hash-sealed evidence package.
///
/// Elige carpeta y no archivo porque el paquete son varios archivos que solo tienen
/// sentido juntos: el manifiesto no vale nada separado de lo que sella.
#[tauri::command]
pub async fn exportar_evidencia(
    app: AppHandle,
    result: ScanResult,
) -> Result<EvidenciaExportada, MonitoreoError> {
    let destino = app
        .dialog()
        .file()
        .blocking_pick_folder()
        .ok_or(MonitoreoError::Cancelada)?;

    let destino = match destino {
        FilePath::Path(p) => p,
        _ => return Err(MonitoreoError::Evidencia("ruta invalida".into())),
    };

    let (config, _) = Config::load();
    let paquete = evidencia::escribir(&result, &config, &destino)
        .map_err(|e| MonitoreoError::Evidencia(format!("{e:#}")))?;

    // Se abre la carpeta en el Explorador, igual que hace la exportación de informes:
    // un archivo que el funcionario no encuentra es un archivo que no existe.
    let _ = app.shell().command("explorer").arg(paquete.ruta.to_string_lossy().to_string()).spawn();

    Ok(EvidenciaExportada {
        ruta: paquete.ruta.to_string_lossy().to_string(),
        archivos: paquete.archivos.len(),
        bytes: paquete.bytes(),
        oxum: paquete.oxum,
        manifiesto: evidencia::MANIFIESTO.to_string(),
        instrucciones: evidencia::INSTRUCCIONES.to_string(),
    })
}
