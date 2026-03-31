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
        ExportFormat::Pdf  => ("informe_muniani.pdf",  "Documento PDF",  vec!["pdf"]),
        ExportFormat::Json => ("csirt_report.json",    "Archivo JSON",   vec!["json"]),
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

    match format {
        ExportFormat::Pdf => {
            report_builder::write_pdf(&result, &path)
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