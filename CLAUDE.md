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

- **No emojis** anywhere — code, comments, docs, commit messages, chat.
- **No AI attribution** — never add a `Co-Authored-By` trailer, a "Generated with"
  line, or any AI credit in commits, PRs, code, or docs.
- **Never invent facts** — no made-up legal articles, norma ids, citations, numbers,
  URLs, or references. Both a house rule and the core product requirement: if a
  detail isn't verified or provided, stop and ask. It mirrors the system prompt in
  `assistant/backend/main.py`, which forbids the model from inventing legal refs.
- **Not a lawyer — no legal advice.** This sits in the Chilean municipal-law domain,
  but do not give legal opinions or interpretations; defer to a qualified lawyer.
- **Present decisions as interactive options** (the arrow-selectable question UI),
  not plain-text lists, with exactly one option marked "(Recommended)" first and the
  reasoning stated.
- **Commit and push actively (hard rule).** Break work into logical
  Conventional-Commit units, commit each verified unit directly on `main`, and push
  to origin as you go — don't wait to be asked. PRs are the exception: never open a
  pull request unless explicitly asked in that turn.
- **Nothing is "done" without real command output** — a build, test, or run must be
  observed passing before it's reported as working.
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
- **Refresh `README.md` before every release.** As part of cutting each release (before
  tagging), sweep `README.md` and correct anything that is no longer applicable,
  abandoned, or out of date — stale commands, removed components, superseded
  architecture, old version numbers, dead links. The README must describe the product
  as it actually is at that tag, not as it was.
