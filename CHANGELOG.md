# Changelog

All notable changes to MuniANCI will be documented here.
Format: [Semantic Versioning](https://semver.org).

---

## [0.5.0] — 2026-07-24 — potencia del escáner y cumplimiento ANCI

El escáner deja de listar problemas y empieza a decir cuáles importan: qué CVE se están
explotando hoy, cuáles ya corrigió el último acumulativo de Windows, qué equipos hay
realmente en la red, y qué hacer primero. Del lado legal, separa lo que la Ley 21.663
exige a una municipalidad de lo que solo es buena práctica, porque las municipalidades no
son OIV. Alcance decidido tras un pase de investigación con lectura íntegra de la ley, la
Res. Ex. N°87 y las Instrucciones Generales N°1 y N°4 (ROADMAP 0.5.0,
`docs/research/0.5.0-escaner-y-cumplimiento-anci.md`).

### Added
- **Enriquecimiento CVE offline** — snapshot de NVD convertido en tiempo de build a un
  índice compacto propio, con el matching CPE→CVE implementado en Rust dentro de `core`.
  El mapeo nombre→CPE usa una tabla curada: si un producto no está en la tabla no se
  afirma nada sobre él, y el informe declara qué porcentaje del inventario quedó sin
  evaluar. Alta precisión antes que alta cobertura, porque desde abril de 2026 el NIST
  dejó de enriquecer buena parte de los CVE y el matching difuso produce falsos positivos
  entre ecosistemas.
- **Catálogo KEV de CISA** — distingue "300 CVE" de "4 CVE que se están explotando hoy", y
  ordena el plan de remediación. Viaja embebido y se sustituye en caliente con el JSON tal
  cual lo publica CISA (`MUNIANI_KEV_FILE` o junto al ejecutable), porque se actualiza cada
  pocos días. El informe declara contra qué catálogo, y de qué fecha, se hizo la afirmación.
- **Descubrimiento de red nativo en Windows** — `SendARP` e `IcmpSendEcho2` vía las APIs
  Win32 de IP Helper, sin exigir privilegios de administrador ni Npcap. El sondeo escala de
  la evidencia más fuerte a la más débil: ARP en capa 2, que el firewall del equipo no
  filtra y es lo único que entrega la dirección MAC; después ICMP; y TCP como último
  recurso. Medido en un /24 real con 4 equipos encendidos: el descubrimiento anterior veía
  1 host remoto y ninguna MAC, el nativo ve los 4 con MAC única. Impresoras, cámaras IP y
  equipos de red dejan de ser invisibles.
- **Modelo dual de cumplimiento** — lo exigible a una municipalidad (Art. 7°, Art. 9° e
  Instrucción General N°1) se evalúa como incumplimiento con consecuencia legal; el Art. 8°
  se mide como madurez voluntaria y se etiqueta como no exigible. Las municipalidades están
  obligadas por los Arts. 4°, 7° y 9°, pero la Res. Ex. N°87 las excluyó expresamente del
  primer proceso de calificación de OIV, así que el Art. 8° y las IG N°3 y N°4 no las
  obligan hoy. El tier es un dato con fecha: el Art. 6° obliga a recalificar al menos cada
  tres años.
- **Madurez 0 a 3 por dominio** — dice *dónde* está el problema, que es lo que un puntaje
  agregado no puede decir: un 82 de 100 puede ser cinco dominios sanos y uno roto. La forma
  se tomó del Essential Eight australiano, con su atribución CC BY en el informe; los
  dominios se derivan de los Arts. 7° y 8° y no copian los ocho controles del ASD.
- **Plan de remediación priorizado en OSCAL POA&M 1.2.2** — cada brecha con su acción,
  responsable y plazo, ordenadas por CVE en el catálogo KEV, luego calificación legal del
  incumplimiento según el Art. 39°, luego severidad. Los plazos sugeridos son criterio
  operativo y no legal: el único plazo perentorio del régimen es el reporte del Art. 9°.
- **Superficie de configuración para TI municipal** — `munianci.config.json` junto al
  ejecutable, editable con el Bloc de notas, sin rebuild ni instalador. Cubre los plazos del
  plan de remediación, el tamaño de papel y los colores del informe, el histórico, y el
  barrido de red. `munianci --escribir-config <ruta>` genera un archivo de ejemplo con todos
  los valores por defecto y una explicación de cada campo, porque nadie configura lo que no
  sabe que existe.
- **Informe ejecutivo de una plana**, aparte del técnico: responde tres preguntas (dónde
  estamos, qué arriesgamos, qué hacer primero) para quien firma, no para quien parchea.
  Papel chileno —oficio para el técnico, carta para el ejecutivo— y la paleta del Kit
  Gobierno de Chile, usada con moderación para no gastar tóner de color.
- **Histórico de evaluaciones por comuna en SQLite embebido**, con el delta respecto de la
  medición anterior en ambos informes. En SQLite y no en JSON porque un barrido semanal de
  un /24 acumula decenas de miles de filas al año. TI controla desde la configuración si se
  guarda el desglose por activo y cuántos meses se retiene.
- **Puntaje agregado anclado en la escala legal** — mecánica SPRS (base fija menos
  deducciones ponderadas), pero con los pesos tomados del Art. 39°: gravísima −5, grave −3,
  leve −1, en vez de una ponderación inventada. Los controles técnicos sin correlato en el
  Art. 39° usan una tabla propia, documentada como criterio técnico y no presentada como
  exigencia legal.
- **`tools/notas-release`** — genera el cuerpo del release desde el CHANGELOG. GitHub
  renderiza los release con saltos de línea duros, así que una sección envuelta a 85
  columnas se publica a media página; le pasó al 0.4.0.

### Changed
- **Las CVE del sistema operativo se filtran por el nivel de parches instalado.** Sin esto
  el catálogo KEV era contraproducente: en un CPE de Microsoft la release va en el nombre
  del producto y el campo versión es `-`, así que cualquier Windows 10 22H2 arrastraba todas
  las CVE publicadas contra esa release desde 2021. Medido en un equipo al día: 2.336 CVE y
  81 marcadas como explotadas activamente, entre ellas PrintNightmare, corregida ahí hace
  años. Ahora se descartan las publicadas antes del último acumulativo instalado, porque los
  acumulativos de Windows contienen todo lo anterior de su rama. Los límites van declarados
  en el informe: no cubre parches fuera de banda ni opcionales, y una CVE publicada antes
  del acumulativo pero aún sin corrección se descartaría por error. Sin fecha legible no se
  descarta nada.
- **La detección de versión TLS ahora funciona.** El sondeo fijaba `TLSv1.2` en todo
  handshake exitoso, de modo que el control "TLS 1.0/1.1/SSLv3 activo" —marcado como
  crítico— no podía dispararse nunca. Se reemplazó por un `ClientHello` construido a mano
  por versión, que detecta las versiones *habilitadas* y no la negociada. `rustls` no servía:
  no soporta TLS anterior a 1.2.
- **Cada pregunta va anclada a su artículo** con un ejemplo de evidencia, corrigiendo los
  anclajes excedidos: la IG N°4 estaba citada en controles aplicables a todos los tiers pero
  obliga solo a OIV.
- **El inventario declara con qué evidencia vio cada host** (`discovered_by` en el JSON):
  ARP prueba presencia física en el segmento, ICMP prueba que la pila IP responde, TCP solo
  prueba que un puerto acepta conexión. Sin esto, un activo sin MAC se lee como error del
  escáner en vez de como un equipo que probablemente tenga el firewall filtrando el ping.
- **Se eliminaron un nombre de empresa inventado y las versiones escritas a mano.** El pie
  del PDF, el banner de la CLI y el `publisher` del instalador decían "Felipe Carvajal Brown
  Software", que no existe, y arrastraban un `v0.1` mientras el proyecto iba en 0.4.0.

### Rendimiento
Medido el 2026-07-24 en una LAN real (/24, 4 equipos encendidos, 16 núcleos), no estimado.

| | Antes (solo TCP) | 0.5.0 (ARP → ICMP → TCP) |
|---|---|---|
| Hosts remotos descubiertos | 1 | 4 |
| Con dirección MAC | 0 | 4 |
| Escaneo completo | 18 s | 81 s |

El barrido de LAN es más lento y ve dos veces y media más activos. El costo es inherente a
`SendARP`, no al limitador de ritmo: con `red.arp_pps` en 0 el escaneo baja a 70 s, apenas
11 s menos. Subir los hilos tampoco ayuda (64 → 81 s, 128 → 88 s, 253 → 82 s) porque Windows
serializa la resolución de vecinos por dentro. Una municipalidad que prefiera el
comportamiento anterior pone `red.arp` en `false`.

### Seguridad de red
El barrido ARP sale limitado a **10 sondas por segundo** de fábrica. Dynamic ARP Inspection,
habitual en switches Cisco, limita el ARP en puertos de acceso y al superar el umbral deja
el puerto en err-disable: sin el límite, el escáner puede dejar sin red al equipo desde el
que corre hasta que el área de redes lo rehabilite. El archivo de configuración explica el
riesgo, y no solo nombra el campo. **Coordine el primer escaneo con LAN completa con el área
de redes**: un barrido de un /24 es una firma de reconocimiento y va a generar alerta en el
IDS. El payload del ping se identifica como MuniANCI en vez de imitar a `ping.exe`, porque
un escáner que se declara ante el SOC es más fácil de autorizar que uno que se disfraza.

### Diferido, con la razón escrita en el ROADMAP
- **La rama Linux del descubrimiento con `pnet`.** El producto se distribuye como app Tauri
  para PCs Windows y el soporte Linux completo ya estaba asignado al Horizonte. Sobre todo,
  no había forma de probarla en terreno. Linux conserva el ladder TCP anterior, sin cambio
  de comportamiento.
- **La taxonomía de incidentes de la Res. Ex. N°7/2025 en el JSON CSIRT.** El texto de la
  resolución no está verificado contra fuente oficial, y codificar categorías legales
  aproximadas sería peor que no tener ninguna: el JSON va al CSIRT Nacional con apariencia
  de estar alineado a la norma.
- **OSCAL Assessment Results.** Su campo `reviewed-controls` exige identificadores que
  resuelvan contra un catálogo OSCAL, y no existe un catálogo OSCAL de la Ley 21.663.
  Emitirlo produciría IDs que no resuelven contra nada: un documento con apariencia de
  estándar que no lo es. Queda condicionado a publicar antes ese catálogo.
- **El export del histórico al formato de interoperabilidad del Estado** (Decreto 12,
  Ley 21.180). Regula el intercambio entre órganos, no el almacenamiento, y hoy no hay
  destinatario identificado.
- **Nuclei**, a 0.7.0, donde está el escaneo de aplicaciones web y sus plantillas rinden.

### Anclajes legales verificados contra fuente primaria
- Plazos del Art. 9°: alerta temprana 3 horas, actualización 72 horas, informe final 15 días
  corridos. Reglamento aplicable: DS N°295 de 2024 (D.O. 01-03-2025).
- Multas del Art. 40°: leves hasta 5.000 UTM (10.000 OIV), graves 10.000 (20.000 OIV),
  gravísimas 20.000 (40.000 OIV).
- Estatus de las municipalidades: no son OIV. Res. Ex. N°87, sección VII numeral 3.
- Redistribución de datos NVD y CVE: permitida con los avisos de NVD y MITRE, que el informe
  imprime en todas sus páginas por condición de licencia.

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