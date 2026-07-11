//! MuniGPT backend lifecycle — runs the RAG backend as a sidecar of the Tauri host.
//!
//! This ports `assistant/electron/main.js` (startBackend / waitForBackend /
//! stopBackend) into the Rust host: on app start we spawn the Python backend,
//! poll `GET /status` in a background thread until it reports `ready`, and reap
//! the whole process tree (uvicorn spawns llama-server children) on exit.
//!
//! Dev-mode only for now: the backend is launched with `python -m uvicorn`
//! against the project's virtualenv. How Python ships in the packaged installer
//! (PyInstaller sidecar vs embedded interpreter) is a later decision — it changes
//! only `resolve_python()` / `backend_dir()`, not the lifecycle here.

use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

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
        let port = std::env::var("MUNIGPT_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(DEFAULT_PORT);
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
    }
}

/// Spawn the backend and start polling it for readiness. Non-blocking: the UI and
/// the scanner remain usable even if the backend never comes up.
pub fn start(app: &AppHandle) {
    let (host, port) = {
        let s = app.state::<AssistantState>();
        (s.host.clone(), s.port)
    };

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

/// Build and spawn the `uvicorn main:app` process from the backend directory.
fn spawn_backend(host: &str, port: u16) -> std::io::Result<Child> {
    let python = resolve_python();
    let dir = backend_dir();
    let mut cmd = Command::new(python);
    cmd.args([
        "-m",
        "uvicorn",
        "main:app",
        "--host",
        host,
        "--port",
        &port.to_string(),
    ])
    .current_dir(&dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn()
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
    use super::check_status_once;
    use std::io::{Read, Write};
    use std::net::TcpListener;

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
