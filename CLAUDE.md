# CLAUDE.md

Guidance for Claude Code (claude.ai/code) when working in this repository.

## What this is

**MuniANCI** is a single Tauri 2 desktop application for Chilean State bodies,
aligned to **Ley 21.663 (Marco de Ciberseguridad)**. It bundles two modules:

- **Scanner** (Rust workspace: `core` / `cli` / `gui`) — active network scan plus a
  declarative questionnaire, producing a PDF gap report and a CSIRT-ready JSON
  report. The institution name and tier are compiled into the binary at build time
  via `MUNIANI_INSTITUTION` / `MUNIANI_TIER`.
- **Asistente** (`assistant/`) — a fully-offline RAG legal assistant (formerly the
  standalone product **MuniGPT**), surfaced as the third GUI tab. Its Python backend
  (FastAPI + embedded llama.cpp + LanceDB) runs as a **Tauri sidecar**, not a
  separate app. The one network-capable path is `/search` (DuckDuckGo), off by
  default; nothing institutional leaves the machine.

The Asistente was merged in via `git subtree` under `assistant/`, preserving its
history. The engineering plan and remaining phases live in
`docs/MERGE-PLAN-MuniGPT.md` (authoritative). Backend conventions and internals are
documented in `assistant/CLAUDE.md`.

## Architecture

```
MuniANCI (Tauri process, Rust)
├── core scanner            (in-process, Rust)
├── Tauri commands          start_scan, export_report, app_branding, assistant_status
└── sidecar backend Python  uvicorn main:app  ->  llama.cpp + LanceDB
        ▲ SSE /chat, /status, /config, /search
     gui/frontend (one React/Vite app)
        ├── Vista Municipal / Vista Técnica  -> invoke() to Rust commands
        └── Asistente                        -> fetch/SSE to 127.0.0.1:8000
```

- `gui/src/assistant.rs` — sidecar lifecycle (spawn, poll `/status`, reap the
  process tree on exit). Overridable via `MUNIGPT_BACKEND_DIR`, `MUNIGPT_PYTHON`,
  `MUNIGPT_HOST`, `MUNIGPT_PORT`.
- `gui/src/commands/branding.rs` — the compiled `MUNIANI_INSTITUTION` / `MUNIANI_TIER`,
  the `app_branding` command, and the single source both modules read.

## Per-client branding (one value drives both modules)

`MUNIANI_INSTITUTION` is compiled into the binary. On a branded build the host
spawns the Asistente sidecar with `MUNIGPT_MUNICIPIO` set to that same institution,
so the backend's prompt personalization and per-comuna DB selection (`db_<slug>`)
follow the scanner. The backend resolves the municipio as
`MUNIGPT_MUNICIPIO` env → `config.json`. On an un-branded build nothing is forced,
so the backend keeps its own `config.json` (the Providencia demo still works).
The GUI header shows the institution via `app_branding`.

## Commands

```powershell
# Build the scanner CLI
cargo build --release -p muniani-cli

# Build / run the GUI (needs the frontend built or the Vite dev server up)
cd gui\frontend; npm install; npm run build
cargo build -p muniani-gui          # debug (loads devUrl; needs `npm run dev`)
cargo tauri build                    # release (embeds the frontend + installer)

# Tests
cargo test                                           # core + cli + gui
cd assistant\backend
..\.venv\Scripts\python.exe -m pytest                # backend unit tests
..\.venv\Scripts\python.exe acceptance_m1.py         # M1 retrieval acceptance (needs db/ + models)
```

The Asistente backend, its models, the llama.cpp binary, the corpus, and the vector
DBs (`db/`, `db_<comuna>/`) are gitignored (too large — shipped by the installer,
present locally only). `acceptance_m1.py` targets the national corpus in `db/`; if
the local `config.json` points elsewhere, force it with `MUNIGPT_DB_DIR=db`.

## Working conventions

These come from the repo owner's global preferences and apply to all work here:

- **HARD RULE — address Felipe in English in this repo.** Chat responses, questions in the
  option UI, and progress reports are in English. This is about how you talk to him, not
  about what you write into the product: code comments stay in Spanish, doc comments on
  functions stay in English, and commit messages, `CHANGELOG.md`, `README.md`, `ROADMAP.md`
  and everything the municipality reads stay in Chilean Spanish. Set by Felipe on
  2026-07-24.
- **No emojis** anywhere — code, comments, docs, commit messages, chat.
- **No AI attribution** — never add a `Co-Authored-By` trailer, a "Generated with"
  line, or any AI credit in commits, PRs, code, or docs.
- **Never invent facts** — no made-up legal articles, norma ids, citations, numbers,
  URLs, or references. Both a house rule and the core product requirement: if a
  detail isn't verified or provided, stop and ask. It mirrors the system prompt in
  `assistant/backend/main.py`, which forbids the model from inventing legal refs.
- **Chilean normativa is read from the PDF, never from the web.** Neither source the
  project relies on can be read programmatically: **BCN LeyChile**
  (`bcn.cl/leychile/...`) returns its "este proceso demora demasiado" error screen to a
  fetch, and the Secretaría de Gobierno Digital's **Wiki Guías**
  (`wikiguias.digital.gob.cl`) is a JavaScript app that returns only the page title. Both
  look like a successful fetch and yield nothing. So don't burn attempts on them: ask
  Felipe for the PDF up front, save it into `docs/` (it is re-included by `.gitignore`),
  and cite the local path alongside the URL. Learned the slow way on 2026-07-25 while
  reading the Decreto 7, the DFL 1 and the guía técnica.
  Corollary, and it has bitten twice: **secondary sources get the details wrong.** One
  called Decreto 7 "Decreto N°27"; a search summary put the NIST CSF 2.0 catalogue in
  `oscal-content` when it lives in the CPRT, and reported Nuclei's Windows zip as
  301,8 MB when it is 43.474.670 bytes. If a number, an identifier or a location is going
  to hold up a decision, verify it against the primary source before stating it.
- **Not a lawyer — no legal advice.** This sits in the Chilean municipal-law domain,
  but do not give legal opinions or interpretations; defer to a qualified lawyer.
- **Present decisions as interactive options** (the arrow-selectable question UI),
  not plain-text lists, with exactly one option marked "(Recommended)" first and the
  reasoning stated.
- **Commit and push actively (hard rule).** Break work into logical
  Conventional-Commit units, commit each verified unit directly on `main`, and push
  to origin as you go — don't wait to be asked. PRs are the exception: never open a
  pull request unless explicitly asked in that turn.
- **Every binary that enters the repo needs its extension marked `binary` in
  `.gitattributes`.** Windows has `core.autocrlf` on by default, so git normalises what
  it thinks is text, and adding a binary prints "LF will be replaced by CRLF". That is
  not cosmetic: it rewrote the bytes of three PDFs already committed here — including the
  texts of the **Ley 21.663** and the **Ley 21.459** — so anyone cloning the repo got
  corrupt copies of the primary sources the product cites, while the local copies looked
  fine. Found and fixed on 2026-07-25. When adding a new binary type, add the extension
  to `.gitattributes` **before** staging it, then prove the round-trip instead of assuming
  it, comparing the working file to what git actually stored:

  ```bash
  git cat-file -p HEAD:<ruta> | sha256sum   # debe coincidir con sha256sum <ruta>
  ```

  Don't add `* text=auto` to fix it — that renormalises the whole tree in one commit.
- **Nothing is "done" without real command output** — a build, test, or run must be
  observed passing before it's reported as working.
- **Research before touching code, for every milestone.** Before writing code for
  any roadmap milestone (0.X.0), run the same kind of research pass done for the
  0.9.0 RAG work: as many web searches as the topic actually needs (not one or two),
  a structured writeup per candidate technique/library (what it is, why it helps,
  offline/CPU feasibility verdict, effort, sources), explicit calls on what's not
  worth doing and why, and a prioritized shortlist — presented to Felipe as options
  before starting, not folded silently into the milestone's existing description.
  Applies to every milestone (scanner, ANCI compliance, Asistente, packaging), not
  just RAG/Asistente ones. Complements ROADMAP.md's HARD RULE to ask Felipe via UI
  before starting a 0.X run — research first, then confirm before starting.
- **Never hardcode a version string, and never invent an organisation name.** This
  product generates PDFs that leave the machine and land in front of municipal
  officials, and versions move fast (0.1 increments per milestone), so both mistakes
  age badly and in public.
  - **Versions** come from `env!("CARGO_PKG_VERSION")` (Rust) or the equivalent
    build-time value, never a literal. A hardcoded `"v0.1"` sat in the PDF footer, the
    CLI banner and `--version` while the project was already at 0.4.0.
  - **Authorship** is `Felipe Carvajal Brown` — a person, not a company. There is no
    "Felipe Carvajal Brown Software"; that string was invented and had spread to the
    PDF footer, the CLI author field, the GUI footer and the Tauri `publisher`. If a
    real razón social ever exists, Felipe supplies the exact name; do not coin one.
  - Same rule for any other user-facing constant: institution names, legal citations,
    URLs. If it asserts something about the real world and it is not verified, ask.
- **Lo que TI municipal puede ajustar va en `munianci.config.json`, no compilado.**
  Existe una superficie de configuración en runtime para el área de TI de cada
  municipalidad: un JSON junto al ejecutable (o apuntado por `MUNIANI_CONFIG`),
  editable con el Bloc de notas, sin rebuild ni instalador. Vive en
  `core/src/config.rs`; `munianci --escribir-config <ruta>` genera un ejemplo con
  todos los valores por defecto y una explicación de cada campo, porque nadie
  configura lo que no sabe que existe. Reglas al agregar un bloque nuevo: cada área
  aporta su propia sección con `#[serde(default)]` (un archivo viejo tiene que
  seguir cargando), un archivo ilegible avisa por stderr y cae a los valores por
  defecto en vez de degradar en silencio, el informe declara de dónde salió la
  configuración, y **siempre se descarta el BOM al leer** (`config::sin_bom`): el
  Bloc de notas y PowerShell escriben UTF-8 con BOM por defecto en Windows y
  `serde_json` lo rechaza, así que sin eso la primera edición de TI se pierde sin
  aviso. Lo compilado por build (`MUNIANI_INSTITUTION`, `MUNIANI_TIER`) sigue
  siendo compilado: la identidad del cliente no es configuración de TI.
- **Mark milestones on the roadmap.** When a milestone ships, update its row in
  `ROADMAP.md`'s "Resumen de hitos" table to `Completado (vX.Y.0, YYYY-MM-DD)`, so the
  roadmap always reflects reality.
- **Release each 0.1 milestone (convention).** Every 0.1 increment gets a full release,
  the same way 0.3.0 was cut: bump ALL version fields to keep them aligned (workspace
  `Cargo.toml`, `gui/Cargo.toml`, `gui/frontend/package.json`, `gui/tauri.conf.json`),
  finalize `CHANGELOG.md` (`[Unreleased]` -> `[X.Y.0]`), draft release notes and run the
  `/article-humanizer` pass on them, then `git tag vX.Y.0` and `gh release create`.
  Confirm with the owner before pushing the tag / publishing the GitHub release (it is
  outward-facing). Docs/cosmetic-only changes do not warrant a release.
- **Never paste a CHANGELOG section straight into a GitHub release.** GitHub renders
  release bodies with hard line breaks ON, so a section wrapped at 85 columns publishes
  at half the page width with a ragged right edge. This happened to v0.4.0. Generate the
  body instead:

  ```powershell
  cargo run -q -p notas-release -- 0.5.0 > notas.md
  gh release create v0.5.0 --notes-file notas.md
  ```

  `tools/notas-release` extracts the section and unwraps its paragraphs, leaving code
  blocks, tables and headings alone. The `.md` file in the repo stays wrapped; only the
  published copy is unwrapped. Do not hand-edit the wrapping instead of running it.
- **Refresh `README.md` before every release.** As part of cutting each release (before
  tagging), sweep `README.md` and correct anything that is no longer applicable,
  abandoned, or out of date — stale commands, removed components, superseded
  architecture, old version numbers, dead links. The README must describe the product
  as it actually is at that tag, not as it was.
- **No acumular instaladores ni artefactos de build: se borra lo que no se está
  probando.** Desde que el Asistente viaja en el instalador, cada corrida de
  `cargo tauri build` deja cerca de 1,5 GB en `target/release/bundle/` (NSIS **y** MSI,
  porque `bundle.targets` es `"all"`), y el mismo bundle de ~900 MB queda duplicado en
  **cuatro** lugares a la vez: `assistant/backend/dist/`, la copia que Tauri prepara en
  `target/release/backend/`, el interior de cada instalador, y lo ya instalado en
  `%LOCALAPPDATA%\MuniANCI`. Se conserva solo el instalador bajo prueba; los demás se
  borran en cuanto la ronda termina. `assistant/backend/dist/` sí se conserva, porque es
  el recurso al que apunta el overlay y sin él no hay instalador que armar.
  El que de verdad crece no son los instaladores: **medido el 2026-07-25,
  `target/debug/` pesaba 46,74 GB** y el repositorio completo 57,97 GB. Borrando el MSI
  que no se estaba probando, el directorio de trabajo de PyInstaller y `target/debug/`
  quedó en 10,44 GB, o sea 47,53 GB liberados sin perder nada que no se regenere. La
  regla operativa: antes de encadenar builds de instalador, revisar el tamaño; y al
  terminar una sesión de empaquetado, limpiar. Nada de esto se versiona, así que borrarlo
  no pierde trabajo, solo cuesta recompilar.
