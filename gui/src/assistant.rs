//! MuniGPT backend lifecycle — runs the RAG backend as a sidecar of the Tauri host.
//!
//! This replaces the former Electron shell's lifecycle logic (startBackend /
//! waitForBackend / stopBackend, now removed) with the Rust host: on app
//! start we spawn the Python backend,
//! poll `GET /status` in a background thread until it reports `ready`, and reap
//! the whole process tree (uvicorn spawns llama-server children) on exit.
//!
//! Two launch modes, chosen automatically (see `build_launch_command`):
//! - **Packaged (release):** a PyInstaller `--onedir` sidecar binary
//!   (`munigpt-backend[.exe]`) bundled next to the host executable is run directly.
//! - **Dev:** falls back to `python -m uvicorn` against the project virtualenv when
//!   no packaged binary is present.
//!
//! On spawn we pass `MUNIGPT_PARENT_PID` so the backend's parent-alive watchdog
//! (assistant/backend/watchdog.py) self-terminates if the host dies abnormally,
//! complementing the `taskkill /T /F` reap in `shutdown()`.

use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8000;
const READY_TIMEOUT: Duration = Duration::from_secs(180);
const POLL_INTERVAL: Duration = Duration::from_millis(1500);

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Managed state: the backend child process and its readiness flag.
pub struct AssistantState {
    child: Mutex<Option<Child>>,
    ready: AtomicBool,
    host: String,
    port: u16,
}

impl AssistantState {
    pub fn new() -> Self {
        let host = std::env::var("MUNIGPT_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
        let preferido = std::env::var("MUNIGPT_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        let port = puerto_utilizable(&host, preferido);
        Self {
            child: Mutex::new(None),
            ready: AtomicBool::new(false),
            host,
            port,
        }
    }

    fn api_base(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

impl Default for AssistantState {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of the backend state, surfaced to the frontend.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssistantStatus {
    pub running: bool,
    pub ready: bool,
    pub api_base: String,
    /// Whether this installation carries the Asistente at all. False on a
    /// scanner-only install, and the UI says so instead of waiting for a backend
    /// that was never shipped.
    pub installed: bool,
}

/// Tauri command: current backend status (is the process alive, is it ready).
#[tauri::command]
pub fn assistant_status(state: tauri::State<'_, AssistantState>) -> AssistantStatus {
    let running = state
        .child
        .lock()
        .unwrap()
        .as_mut()
        .map(|c| matches!(c.try_wait(), Ok(None)))
        .unwrap_or(false);
    AssistantStatus {
        running,
        ready: state.ready.load(Ordering::Relaxed),
        api_base: state.api_base(),
        installed: installed(),
    }
}

/// Tauri command: pick the offline model pack folder with a native dialog, and hand
/// the path back so the frontend can POST it to `/models/pack`.
///
/// The dialog runs here and not in the webview on purpose: the main window's
/// capability grants only `core:default`, so the JS dialog API is out of reach by
/// design (least-privilege, same reasoning as `export_report`). Returns `None` when
/// the user cancels.
#[tauri::command]
pub async fn assistant_pick_pack_dir(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .set_title("Elija la carpeta con el paquete de modelos")
        .blocking_pick_folder()
        .map(|p| p.to_string())
}

/// Is the Asistente present in this installation? True with a packaged sidecar
/// binary, or with the backend source tree in a dev checkout. False on a
/// scanner-only installer, which is a supported build and not a broken one.
pub fn installed() -> bool {
    packaged_sidecar_bin().is_some() || backend_dir().exists()
}

/// Spawn the backend and start polling it for readiness. Non-blocking: the UI and
/// the scanner remain usable even if the backend never comes up.
pub fn start(app: &AppHandle) {
    let (host, port) = {
        let s = app.state::<AssistantState>();
        (s.host.clone(), s.port)
    };

    // Scanner-only install: nothing to spawn, and no readiness poll either. Without
    // this the UI would sit through the whole timeout before reporting a failure the
    // host already knew about at startup.
    if !installed() {
        eprintln!("[asistente] esta instalacion no incluye el Asistente; no se inicia");
        return;
    }

    match spawn_backend(&host, port) {
        Ok(child) => {
            let s = app.state::<AssistantState>();
            *s.child.lock().unwrap() = Some(child);
        }
        Err(e) => {
            eprintln!("[asistente] no se pudo iniciar el backend: {e}");
            return;
        }
    }

    let app = app.clone();
    std::thread::spawn(move || {
        let deadline = Instant::now() + READY_TIMEOUT;
        while Instant::now() < deadline {
            if check_status_once(&host, port) {
                let s = app.state::<AssistantState>();
                s.ready.store(true, Ordering::Relaxed);
                let _ = app.emit("assistant-ready", s.api_base());
                return;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        eprintln!("[asistente] el backend no respondio a /status dentro del tiempo limite");
        let _ = app.emit("assistant-timeout", ());
    });
}

/// Kill the backend process tree. Called on app exit.
pub fn shutdown(app: &AppHandle) {
    let s = app.state::<AssistantState>();
    let child = s.child.lock().unwrap().take();
    if let Some(mut child) = child {
        let pid = child.id();
        #[cfg(windows)]
        {
            // uvicorn spawns llama-server children — kill the whole tree.
            use std::os::windows::process::CommandExt;
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
        #[cfg(not(windows))]
        {
            let _ = pid; // silence unused on non-Windows
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Build and spawn the backend process (packaged onedir binary, or `python -m
/// uvicorn` in dev). Passes the branding and the parent PID for the watchdog.
fn spawn_backend(host: &str, port: u16) -> std::io::Result<Child> {
    let mut cmd = build_launch_command(host, port);

    // Parent-alive watchdog: tell the sidecar which process to outlive. If the host
    // dies abnormally (crash/kill) and the taskkill reap in shutdown() never runs,
    // watchdog.py sees this PID vanish and self-terminates (reaping llama-server).
    cmd.env("MUNIGPT_PARENT_PID", std::process::id().to_string());

    if std::env::var_os("MUNIGPT_MUNICIPIO").is_none() {
        if let Some(institution) = crate::commands::branding::institucion_forzada() {
            cmd.env("MUNIGPT_MUNICIPIO", institution);
        }
    }

    // Where a downloaded chat model may be written. Outside the install directory on
    // purpose: gigabytes of GGUF should not depend on the installer's file bookkeeping
    // across upgrades, and a per-machine install would put the app directory out of
    // reach for a non-admin user. The backend keeps reading the models that ship next
    // to the executable too (fetch_models.models_search_path), so the bundled
    // embedding model stays visible.
    if std::env::var_os("MUNIGPT_MODELS_DIR").is_none() {
        if let Some(dir) = user_models_dir() {
            // Created eagerly so IT can find the folder to paste a model into by hand.
            let _ = std::fs::create_dir_all(&dir);
            cmd.env("MUNIGPT_MODELS_DIR", &dir);
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn()
}

/// Choose how to launch the backend: a packaged PyInstaller `--onedir` binary if one
/// is present (release), otherwise the dev fallback `python -m uvicorn main:app`.
/// Branding and creation flags are applied by the caller (`spawn_backend`).
fn build_launch_command(host: &str, port: u16) -> Command {
    let port_s = port.to_string();
    if let Some(bin) = packaged_sidecar_bin() {
        // The onedir binary parses --host/--port itself (see backend/run_server.py).
        // Run it from its own folder so bundled data (config, db/) resolves.
        let mut cmd = Command::new(&bin);
        cmd.args(["--host", host, "--port", &port_s]);
        if let Some(parent) = bin.parent() {
            cmd.current_dir(parent);
        }
        return cmd;
    }
    let mut cmd = Command::new(resolve_python());
    cmd.args(["-m", "uvicorn", "main:app", "--host", host, "--port", &port_s])
        .current_dir(backend_dir());
    cmd
}

/// Locate the packaged sidecar executable, if this is a release/onedir layout.
/// Overridable via `MUNIGPT_SIDECAR_BIN`. Returns `None` in a dev tree (no packaged
/// binary), so `build_launch_command` falls back to `python -m uvicorn`.
fn packaged_sidecar_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("MUNIGPT_SIDECAR_BIN") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    #[cfg(windows)]
    let exe_name = "munigpt-backend.exe";
    #[cfg(not(windows))]
    let exe_name = "munigpt-backend";

    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    // Packaged layouts: the onedir folder shipped as a Tauri resource next to the
    // host executable. Refined further in the packaging phase (installer layout).
    let candidates = [
        exe_dir.join("backend").join(exe_name),
        exe_dir.join("assistant").join("backend").join(exe_name),
    ];
    first_existing(&candidates)
}

/// Pick a port the sidecar can actually bind: the preferred one if it is free, else
/// one the OS hands out.
///
/// 8000 is a popular port and this is a desktop app, not a server: on a PC with any
/// other development tool or local service running, the sidecar used to fail to bind
/// and die, and the tab reported that the Asistente "could not start" — blaming the
/// installation for someone else's port. Nothing downstream cares about the number:
/// the frontend reads the base URL from `assistant_status`, and the CSP allows the
/// whole loopback range. An explicit `MUNIGPT_PORT` is still honoured first; it just
/// stops being a single point of failure.
///
/// There is a window between releasing the probe socket and uvicorn binding it. It is
/// the same approach the backend already uses to pick llama-server ports, and losing
/// the race is far less likely than colliding with a long-lived listener on 8000.
fn puerto_utilizable(host: &str, preferido: u16) -> u16 {
    use std::net::TcpListener;
    if TcpListener::bind((host, preferido)).is_ok() {
        return preferido;
    }
    match TcpListener::bind((host, 0)).and_then(|l| l.local_addr()) {
        Ok(addr) => {
            eprintln!(
                "[asistente] el puerto {preferido} esta ocupado; se usa el {}",
                addr.port()
            );
            addr.port()
        }
        Err(_) => preferido,
    }
}

/// Writable directory for downloaded GGUF models: `%LOCALAPPDATA%\MuniANCI\models`.
/// A plain, typeable path rather than the bundle identifier, because municipal IT may
/// well paste a model file in there by hand. `None` off Windows, where the backend's
/// own default applies.
fn user_models_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(|base| PathBuf::from(base).join("MuniANCI").join("models"))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// First path in `candidates` that exists on disk, if any.
fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.exists()).cloned()
}

/// Locate the backend directory (`assistant/backend`). Overridable via
/// `MUNIGPT_BACKEND_DIR`; otherwise resolved relative to the running executable
/// (`target/{debug,release}/muniani-gui.exe` -> repo root -> assistant/backend).
fn backend_dir() -> PathBuf {
    if let Ok(p) = std::env::var("MUNIGPT_BACKEND_DIR") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // Dev/release tree: target/<profile>/ -> ../../assistant/backend
            let dev = exe_dir.join("..").join("..").join("assistant").join("backend");
            if dev.exists() {
                return dev;
            }
            // Packaged fallback (resources next to the exe) — refined in the
            // packaging phase.
            let packaged = exe_dir.join("assistant").join("backend");
            if packaged.exists() {
                return packaged;
            }
        }
    }
    PathBuf::from("assistant").join("backend")
}

/// Pick the Python interpreter. Prefers a project virtualenv under `assistant/`,
/// falls back to `MUNIGPT_PYTHON`, then bare `python` on PATH.
fn resolve_python() -> PathBuf {
    if let Ok(p) = std::env::var("MUNIGPT_PYTHON") {
        return PathBuf::from(p);
    }
    // The backend dir is assistant/backend; the venv sits at assistant/.venv or
    // assistant/venv (gitignored, created by the developer).
    if let Some(assistant) = backend_dir().parent().map(PathBuf::from) {
        #[cfg(windows)]
        let rel = ["Scripts", "python.exe"];
        #[cfg(not(windows))]
        let rel = ["bin", "python"];
        for venv in [".venv", "venv"] {
            let cand = assistant.join(venv).join(rel[0]).join(rel[1]);
            if cand.exists() {
                return cand;
            }
        }
    }
    PathBuf::from("python")
}

/// One raw-HTTP `GET /status`. Returns true when the backend answers 200 with a
/// body that reports `status: ok` and `ready: true` (same contract Electron used).
/// Deliberately dependency-free (no HTTP crate) — a plain TCP request is enough.
fn check_status_once(host: &str, port: u16) -> bool {
    use std::io::{Read, Write};

    let Ok(addr) = format!("{host}:{port}").parse::<SocketAddr>() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(3)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));

    let req = format!(
        "GET /status HTTP/1.1\r\nHost: {host}:{port}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }

    let mut buf = Vec::new();
    // Cap the read so a misbehaving peer can't stream forever.
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() > 64 * 1024 {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let resp = String::from_utf8_lossy(&buf);
    let Some((headers, body)) = resp.split_once("\r\n\r\n") else {
        return false;
    };
    if !headers.starts_with("HTTP/1.1 200") && !headers.starts_with("HTTP/1.0 200") {
        return false;
    }
    let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    compact.contains("\"status\":\"ok\"") && compact.contains("\"ready\":true")
}

#[cfg(test)]
mod tests {
    use super::{check_status_once, first_existing, user_models_dir};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;

    /// Con el puerto preferido ocupado, se elige otro en vez de morir: 8000 es un
    /// puerto popular y el sidecar no tiene por que perder contra quien llego antes.
    #[test]
    fn un_puerto_ocupado_no_mata_al_sidecar() {
        use std::net::TcpListener;
        let ocupado = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let puerto = ocupado.local_addr().unwrap().port();

        let elegido = super::puerto_utilizable("127.0.0.1", puerto);

        assert_ne!(elegido, puerto, "deberia haber elegido otro puerto");
        assert_ne!(elegido, 0);
        // Y el elegido tiene que servir de verdad.
        assert!(TcpListener::bind(("127.0.0.1", elegido)).is_ok());
    }

    #[test]
    fn un_puerto_libre_se_respeta() {
        let libre = {
            use std::net::TcpListener;
            let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            l.local_addr().unwrap().port() // se libera al salir del bloque
        };
        assert_eq!(super::puerto_utilizable("127.0.0.1", libre), libre);
    }

    #[test]
    fn first_existing_returns_none_when_all_absent() {
        let candidates = [
            PathBuf::from("Z:/muniani-nonexistent/a"),
            PathBuf::from("Z:/muniani-nonexistent/b"),
        ];
        assert!(first_existing(&candidates).is_none());
    }

    /// The models directory must sit OUTSIDE the install directory: an upgrade
    /// reinstalls the app folder, and a 2,5 GB download must not ride on that.
    #[cfg(windows)]
    #[test]
    fn user_models_dir_is_under_local_appdata() {
        let dir = user_models_dir().expect("LOCALAPPDATA siempre existe en Windows");
        let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap());
        assert!(dir.starts_with(&local), "{dir:?} deberia estar bajo {local:?}");
        assert!(dir.ends_with(PathBuf::from("MuniANCI").join("models")));
        // Not next to the running executable, which is what the installer replaces.
        let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        assert!(!dir.starts_with(exe_dir));
    }

    #[test]
    fn first_existing_picks_the_first_present() {
        // current_exe() is guaranteed to exist while the test binary runs.
        let real = std::env::current_exe().unwrap();
        let candidates = [PathBuf::from("Z:/muniani-nonexistent/a"), real.clone()];
        assert_eq!(first_existing(&candidates), Some(real));
    }

    /// Stand up a one-shot HTTP server on 127.0.0.1 that replies with `response`,
    /// then run check_status_once against it and return the verdict.
    fn probe_against(response: &'static str) -> bool {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf); // consume the request line/headers
                let _ = stream.write_all(response.as_bytes());
                // Dropping the stream closes it -> EOF for the client (Connection: close).
            }
        });
        let verdict = check_status_once("127.0.0.1", port);
        let _ = handle.join();
        verdict
    }

    #[test]
    fn ready_true_is_detected() {
        let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
                    {\"status\":\"ok\",\"ready\":true,\"missingModels\":[]}";
        assert!(probe_against(resp));
    }

    #[test]
    fn ready_true_with_whitespace_is_detected() {
        // Real FastAPI/JSON often has spaces after colons — normalization must handle it.
        let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
                    { \"status\": \"ok\", \"ready\": true }";
        assert!(probe_against(resp));
    }

    #[test]
    fn ready_false_is_rejected() {
        let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
                    {\"status\":\"ok\",\"ready\":false,\"missingModels\":[\"chat\"]}";
        assert!(!probe_against(resp));
    }

    #[test]
    fn non_200_is_rejected() {
        let resp = "HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\n\r\n\
                    {\"status\":\"ok\",\"ready\":true}";
        assert!(!probe_against(resp));
    }
}
