# Changelog

All notable changes to MuniANCI will be documented here.
Format: [Semantic Versioning](https://semver.org).

---

## [0.4.0] — 2026-07-12 — empaquetado y fundaciones de confianza y medición

Empaquetado para PCs municipales y fundaciones para que toda mejora posterior sea
auditable y medible (ROADMAP 0.4.0).

### Added
- **CI (GitHub Actions)** — primera CI del repo: build + tests (Windows), y gates
  de auditoría de dependencias que BLOQUEAN (`cargo audit`, `cargo deny` para
  licencias/bans/sources, `pip-audit`), más generación de **SBOM** SPDX + CycloneDX
  (Rust y backend Python) como artefacto descargable.
- **Mitigación de inyección indirecta de prompts** en la ruta RAG (`sanitize.py`):
  saneamiento en tiempo de indexación (quita caracteres ocultos/bidi, neutraliza
  frases de override y marcadores de rol) + *spotlighting* del contexto recuperado
  (delimitadores, marcado como datos) para el modelo. OWASP LLM 2025.
- **Lanzamiento del sidecar empaquetado** (`--onedir` PyInstaller) con fallback a
  `python -m uvicorn` en dev, más **watchdog padre-vivo** (`watchdog.py`): el backend
  se autotermina si el host muere de forma anormal.
- **Distribución de modelos (D2)** — `models.manifest.json` + `fetch_models.py`:
  descarga reanudable con verificación SHA256 y paquete offline copiable para equipos
  air-gapped. Orígenes verificados por coincidencia de SHA256 contra el puntero
  git-LFS de cada repo.
- **Harness de evaluación offline** (`eval/`) — set dorado de 45 preguntas legales
  reales derivadas del corpus (aprobado) + métricas de recuperación deterministas
  (recall@k, MRR, precisión) como gate reproducible. Base: recall@k=0.978. Capa de
  juez LLM (`eval_judge.py`, Ragas con el llama.cpp local como juez, totalmente
  offline) implementada y validada; es una actividad manual pesada, no un gate de CI.
- **Mirror `vendor/`** — estructura, `.gitignore` de artefactos grandes y
  `PROVENANCE.md` (nombre/versión/origen/SHA256/licencia por artefacto).

### Changed
- **CSP estricta** de Tauri v2 (`connect-src` limitado a `127.0.0.1:8000` + IPC;
  sin orígenes externos) y **capability de menor privilegio** (webview reducido a
  `core:default`; diálogo/shell son nativos de Rust, fuera del ACL del webview). CSP de
  desarrollo aparte para no romper el HMR de Vite.
- **WebView2**: instalador offline embebido (`webviewInstallMode = offlineInstaller`)
  para PCs municipales air-gapped.

### Security / dependencias
- Se resolvieron 2 avisos RustSec de severidad alta: `lopdf` 0.34 -> 0.42
  (RUSTSEC-2026-0187, desbordamiento de pila) y `crossbeam-epoch` 0.9.18 -> 0.9.20.
  Dos avisos DoS transitivos de `quick-xml` (fijados por `tauri -> plist`) quedan
  documentados e ignorados con condición de remoción; cualquier otro aviso bloquea.

---

## [0.3.0] — 2026-07-11 — módulo Asistente (fusión MuniGPT)

MuniGPT, antes un producto de escritorio propio (asistente legal RAG offline), se
integró como el módulo **Asistente** de MuniANCI. Un solo producto Tauri, dos
módulos. Plan e historial en `docs/MERGE-PLAN-MuniGPT.md`.

### Added
- Módulo `assistant/` — backend FastAPI + RAG (llama.cpp embebido + LanceDB),
  importado con historia vía `git subtree`. Toda la inferencia corre local; la
  única salida a la red es `/search` (DuckDuckGo), apagada por defecto.
- `gui/src/assistant.rs` — ciclo de vida del backend como *sidecar* del proceso
  Tauri: lo levanta en el `setup` hook, sondea `GET /status` hasta `ready`, y reap
  del árbol de procesos (uvicorn + llama-server) al cerrar. Reemplaza al antiguo
  shell Electron.
- Pestaña **Asistente** en la GUI — el chat RAG portado a `gui/frontend`
  (streaming SSE, citas, chips de desambiguación) apuntando a `127.0.0.1:8000`.
- Bases vectoriales por comuna intercambiables — `rag.db_dir()` resuelve
  `MUNIGPT_DB_DIR` -> `db_<slug-comuna>` -> `db`.
- `app_branding` (comando Tauri) y `gui/src/commands/branding.rs` — exponen la
  institución/tier compilados al frontend para el encabezado.

### Changed
- **Marca unificada por cliente (Fase 4).** `MUNIANI_INSTITUTION` (env de
  compilación) ahora alimenta ambos módulos: el host pasa el valor al backend del
  Asistente como `MUNIGPT_MUNICIPIO`, que gobierna la personalización del prompt y
  la selección de base (`db_<slug>`). El backend resuelve el `municipio` en orden
  `MUNIGPT_MUNICIPIO` -> `config.json`. En builds sin marca, el Asistente conserva
  su `config.json` (no se rompe el demo). El encabezado de la GUI muestra la
  institución en vez del texto fijo "MuniANCI".

### Removed
- Shell Electron (`assistant/electron/`) y frontend standalone
  (`assistant/frontend/`) — superados por el host Tauri y `gui/frontend`.
- `assistant/package.json` / `package-lock.json` (config electron-builder) e
  instalador Inno Setup standalone (`assistant/installer/munigpt.iss`). El
  instalador unificado es una fase posterior (Fase 5, aún no ejecutada).

---

## [0.2.0] — 2026-03-31

### Added
- `muniani-gui` — Tauri 2 desktop GUI with React/TypeScript/Vite frontend
  - Vista Municipal (worker tab) — plain-Spanish gap summary, UTM fine scale, CSIRT notice
  - Vista Técnica (IT tab) — full gap table with evidence, live technical log terminal, asset detail
  - Progress channel streaming from Rust core to both tabs via `ScanProgress { pct, log }`
  - Native PDF and JSON export with OS save dialog (`tauri-plugin-dialog`)
  - Post-export folder reveal via `tauri-plugin-shell`
  - NIST/NSA design system — IBM Plex Sans + IBM Plex Mono, federal color palette
  - Per-client build via compile-time env vars (`MUNIANI_INSTITUTION`, `MUNIANI_TIER`)
- `eol_enrichment` module — post-normalization EOL patching via bundled static database
  - 38 products covered: Windows, Office, SQL Server, .NET, Python, Node.js, PHP, MySQL,
    PostgreSQL, MariaDB, MongoDB, Redis, Elasticsearch, Apache, nginx, Tomcat, OpenSSL,
    VMware, Veeam, LibreOffice, Firefox, Chrome, and more
  - Source: endoflife.date (March 2026 snapshot), embedded as `core/src/data/eol_db.json`
  - Fixes Office 2016 incorrectly reported as `is_eol: false` in v0.1
- Full WMI COM implementation — `wmi_query`, `wmi_scalar_u32`, `wmi_string_list`
- Real firewall detection via Windows registry (no elevation required)
- TLS certificate chain validation — classifies `Expired`, `SelfSigned`, `ExpiredAndSelfSigned`
- `backup_agent_running: Option<bool>` in `OsInfo` — `None` = WMI failed, `Some(false)` = no agent
- `log_cb` field in `ScanConfig` — separate technical log callback for GUI terminal
- BitLocker gap suppressed for PSE tier (OIV-only control per Art. 8° lit. a)
- PDF encoding fix — `to_pdf_safe()` sanitizes UTF-8 to WinAnsiEncoding (printpdf 0.9.1)
- 28 unit tests passing across all core modules

### Changed
- `ScanConfig` gains `log_cb: Option<Box<dyn Fn(&str) + Send + Sync>>` field —
  CLI sets this to `None`; GUI wires it to the IT terminal channel
- `normalizer::normalize()` renamed/aligned with updated lib.rs scan pipeline
- Workspace `Cargo.toml` adds `gui` as member

### Pending (deferred to v0.3.0)
- CVE enrichment via NVD API (Office 2016 `max_cvss` still `null`)
- Code signing certificate (DigiCert/Sectigo) — required before municipal delivery;
  unsigned `.exe` will be blocked by enterprise AV (McAfee, Defender ATP)
- Inno Setup portable `.exe` packaging
- Tauri 2 GUI security audit

### Legal anchors verified against
- Ley 21.663 full text (DO 08/04/2024, BCN)
- Ley 21.459 full text (DO 20/06/2022, last amended 01/04/2025)
- ANCI Instrucciones Generales N°1–4 (2025)
- DS N°295/2024 Reglamento de Reporte de Incidentes

---

## [0.1.0] — 2026-03-28

### Added
- `muniani-core` library crate with full scan pipeline
- `os_abstraction` layer — Windows (Win32/WMI) and Unix (/proc, dpkg, rpm)
- Probes: `host_discovery`, `drive_enum`, `service_probe`, `sw_inventory`, `os_check`
- `normalizer` — deduplicates raw findings into typed `AssetGraph`
- `compliance_engine` — maps findings to `Vec<Gap>` with Art. 8°/9° anchors
- `questionnaire` — declarative controls for Art. 8° lit. c, h, i and IG N°1–4
- Art. 27° significance filter for correct CSIRT reporting tagging
- `report_builder` — PDF informe de brechas + CSIRT JSON
- UTM fine scale table per Art. 40° Ley 21.663 (OIV and PSE tiers)
- Ley 21.459 Art. 2° safe harbor disclaimer in every PDF report
- `muniani-cli` binary with interactive questionnaire and progress reporting
- Windows + Linux support via `#[cfg]` platform gates
- 18 unit tests across core modules

### Legal anchors verified against
- Ley 21.663 full text (DO 08/04/2024, BCN)
- Ley 21.459 full text (DO 20/06/2022, last amended 01/04/2025)
- ANCI Instrucciones Generales N°1–4 (2025)
- DS N°295/2024 Reglamento de Reporte de Incidentes