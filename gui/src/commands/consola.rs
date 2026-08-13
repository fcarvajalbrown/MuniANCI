use munigpt_core::types::ScanResult;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use tauri::ipc::Channel;

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
    #[error("La consola no está abierta.")]
    Cerrada,
}

#[derive(Default)]
pub struct ConsolaEstado {
    sesion: Mutex<Option<Sesion>>,
}

struct Sesion {
    maestro: Box<dyn MasterPty + Send>,
    escritor: Box<dyn Write + Send>,
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

#[tauri::command]
pub fn consola_iniciar(
    estado: tauri::State<'_, ConsolaEstado>,
    salida: Channel<String>,
    result: Option<ScanResult>,
    filas: u16,
    columnas: u16,
) -> Result<(), ConsolaError> {
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

    let sistema = native_pty_system();
    let par = sistema
        .openpty(PtySize {
            rows: filas.max(1),
            cols: columnas.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| ConsolaError::Apertura(e.to_string()))?;

    let mut orden = CommandBuilder::new(consola_del_sistema());
    for arg in argumentos_consola() {
        orden.arg(arg);
    }
    orden.cwd(&carpeta);

    let mut hijo = par
        .slave
        .spawn_command(orden)
        .map_err(|e| ConsolaError::Apertura(e.to_string()))?;

    let mut lector = par
        .master
        .try_clone_reader()
        .map_err(|e| ConsolaError::Apertura(e.to_string()))?;
    let escritor = par
        .master
        .take_writer()
        .map_err(|e| ConsolaError::Apertura(e.to_string()))?;

    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        let mut pendiente: Vec<u8> = Vec::new();
        loop {
            match lector.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    pendiente.extend_from_slice(&buffer[..n]);
                    let hasta = match std::str::from_utf8(&pendiente) {
                        Ok(_) => pendiente.len(),
                        Err(e) => e.valid_up_to(),
                    };
                    if hasta > 0 {
                        let texto = String::from_utf8_lossy(&pendiente[..hasta]).to_string();
                        if salida.send(texto).is_err() {
                            break;
                        }
                        pendiente.drain(..hasta);
                    }
                }
            }
        }
        let _ = hijo.wait();
    });

    let mut sesion = estado.sesion.lock().map_err(|_| ConsolaError::Cerrada)?;
    *sesion = Some(Sesion {
        maestro: par.master,
        escritor,
    });
    Ok(())
}

#[tauri::command]
pub fn consola_escribir(
    estado: tauri::State<'_, ConsolaEstado>,
    datos: String,
) -> Result<(), ConsolaError> {
    let mut guardia = estado.sesion.lock().map_err(|_| ConsolaError::Cerrada)?;
    let sesion = guardia.as_mut().ok_or(ConsolaError::Cerrada)?;
    sesion
        .escritor
        .write_all(datos.as_bytes())
        .map_err(|e| ConsolaError::Io(e.to_string()))?;
    sesion
        .escritor
        .flush()
        .map_err(|e| ConsolaError::Io(e.to_string()))
}

#[tauri::command]
pub fn consola_redimensionar(
    estado: tauri::State<'_, ConsolaEstado>,
    filas: u16,
    columnas: u16,
) -> Result<(), ConsolaError> {
    let guardia = estado.sesion.lock().map_err(|_| ConsolaError::Cerrada)?;
    let sesion = guardia.as_ref().ok_or(ConsolaError::Cerrada)?;
    sesion
        .maestro
        .resize(PtySize {
            rows: filas.max(1),
            cols: columnas.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| ConsolaError::Apertura(e.to_string()))
}

#[tauri::command]
pub fn consola_cerrar(estado: tauri::State<'_, ConsolaEstado>) -> Result<(), ConsolaError> {
    let mut guardia = estado.sesion.lock().map_err(|_| ConsolaError::Cerrada)?;
    *guardia = None;
    Ok(())
}

fn consola_del_sistema() -> &'static str {
    if cfg!(windows) {
        "powershell.exe"
    } else {
        "bash"
    }
}

fn argumentos_consola() -> Vec<&'static str> {
    if cfg!(windows) {
        vec!["-NoLogo", "-NoExit", "-Command", "claude"]
    } else {
        vec!["-lc", "claude"]
    }
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
