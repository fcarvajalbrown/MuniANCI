// Exports the last scan result to PDF or JSON using a native save dialog.
use muniani_core::{report_builder, types::ScanResult};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_shell::ShellExt;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Pdf,
    Json,
    /// El informe ejecutivo de una página. Va aparte del técnico y no como su
    /// primera página porque tienen destinatarios distintos: el ejecutivo se le manda
    /// al alcalde, y el técnico lleva IP y rutas de recursos compartidos, así que
    /// conviene tratarlo como reservado (ver `report_builder::write_executive_pdf`).
    /// Por eso la Vista Municipal exporta este y no el otro.
    Ejecutivo,
}

#[derive(Debug, Serialize, thiserror::Error)]
pub enum ExportError {
    #[error("Exportación cancelada por el usuario.")]
    Cancelled,
    #[error("Error al escribir el archivo: {0}")]
    Io(String),
    #[error("Error al generar el PDF: {0}")]
    Pdf(String),
}

/// Tauri command — opens a native save dialog then writes PDF or JSON to the chosen path.
#[tauri::command]
pub async fn export_report(
    app: AppHandle,
    result: ScanResult,
    format: ExportFormat,
) -> Result<String, ExportError> {
    let (default_name, filter_name, filter_ext) = match format {
        ExportFormat::Pdf       => ("informe_muniani.pdf", "Documento PDF", vec!["pdf"]),
        ExportFormat::Json      => ("csirt_report.json",   "Archivo JSON",  vec!["json"]),
        ExportFormat::Ejecutivo => ("informe_ejecutivo.pdf", "Documento PDF", vec!["pdf"]),
    };

    // Show native save dialog — blocks until user picks a path or cancels.
    let path = app
        .dialog()
        .file()
        .set_file_name(default_name)
        .add_filter(filter_name, &filter_ext)
        .blocking_save_file()
        .ok_or(ExportError::Cancelled)?;

    let path = match path {
        FilePath::Path(p) => p,
        _ => return Err(ExportError::Io("Ruta de archivo inválida.".into())),
    };

    // La configuración de TI manda también acá. Antes la exportación de la GUI usaba
    // los valores por defecto, así que una municipalidad que fijaba oficio en
    // `munianci.config.json` seguía recibiendo carta desde la interfaz y oficio desde
    // la CLI, para el mismo escaneo.
    let (config, _) = muniani_core::config::Config::load();

    match format {
        ExportFormat::Pdf => {
            report_builder::write_pdf_completo(
                &result,
                &config.informe,
                config.informe.tamano_papel_tecnico,
                &path.to_string_lossy(),
            )
            .map_err(|e| ExportError::Pdf(e.to_string()))?;
        }
        ExportFormat::Ejecutivo => {
            report_builder::write_executive_pdf_con(
                &result,
                &config.poam,
                &config.informe,
                config.informe.tamano_papel_ejecutivo,
                &path.to_string_lossy(),
            )
            .map_err(|e| ExportError::Pdf(e.to_string()))?;
        }
        ExportFormat::Json => {
            let json = serde_json::to_string_pretty(&result)
                .map_err(|e| ExportError::Io(e.to_string()))?;
            std::fs::write(&path, json)
                .map_err(|e| ExportError::Io(e.to_string()))?;
        }
    }

    // Open the containing folder in Explorer so the user can find the file immediately.
    let folder = path
        .parent()
        .unwrap_or(&path)
        .to_string_lossy()
        .to_string();
    let _ = app.shell().command("explorer").arg(&folder).spawn();

    Ok(path.to_string_lossy().to_string())
}