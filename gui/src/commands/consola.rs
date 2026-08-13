use munigpt_core::types::ScanResult;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const ARCHIVO_CONTEXTO: &str = "escaneo-actual.json";

#[derive(Debug, Serialize, thiserror::Error)]
pub enum ConsolaError {
    #[error("La consola de asistencia solo está disponible en Windows.")]
    NoDisponible,
    #[error("No se encontró el asistente de consola en este equipo.")]
    SinAsistente,
    #[error("No se pudo dejar el escaneo para la consola: {0}")]
    Io(String),
    #[error("No se pudo abrir la consola: {0}")]
    Apertura(String),
}

fn carpeta_trabajo() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn hay_asistente_consola() -> bool {
    if !cfg!(windows) {
        return false;
    }
    Command::new("where")
        .arg("claude")
        .output()
        .map(|salida| salida.status.success())
        .unwrap_or(false)
}

#[tauri::command]
pub fn consola_disponible() -> bool {
    hay_asistente_consola()
}

#[tauri::command]
pub fn abrir_consola(result: Option<ScanResult>) -> Result<String, ConsolaError> {
    if !cfg!(windows) {
        return Err(ConsolaError::NoDisponible);
    }
    if !hay_asistente_consola() {
        return Err(ConsolaError::SinAsistente);
    }

    let carpeta = carpeta_trabajo();

    if let Some(escaneo) = result {
        let json = serde_json::to_string_pretty(&escaneo)
            .map_err(|e| ConsolaError::Io(e.to_string()))?;
        std::fs::write(carpeta.join(ARCHIVO_CONTEXTO), json)
            .map_err(|e| ConsolaError::Io(e.to_string()))?;
    }

    lanzar(&carpeta)?;
    Ok(carpeta.join(ARCHIVO_CONTEXTO).to_string_lossy().to_string())
}

#[cfg(windows)]
fn lanzar(carpeta: &Path) -> Result<(), ConsolaError> {
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

    Command::new("powershell.exe")
        .current_dir(carpeta)
        .args(["-NoLogo", "-NoExit", "-Command", "claude"])
        .creation_flags(CREATE_NEW_CONSOLE)
        .spawn()
        .map_err(|e| ConsolaError::Apertura(e.to_string()))?;
    Ok(())
}

#[cfg(not(windows))]
fn lanzar(_carpeta: &Path) -> Result<(), ConsolaError> {
    Err(ConsolaError::NoDisponible)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_carpeta_de_trabajo_es_la_del_ejecutable() {
        let esperada = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        assert_eq!(carpeta_trabajo(), esperada);
    }

    #[test]
    fn fuera_de_windows_la_consola_no_esta_disponible() {
        if !cfg!(windows) {
            assert!(!consola_disponible());
        }
    }

    #[test]
    fn el_archivo_de_contexto_es_json() {
        assert!(ARCHIVO_CONTEXTO.ends_with(".json"));
    }
}
