# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

MuniGPT is a fully-offline RAG assistant for Chilean municipal employees, developed by Felipe Carvajal Brown in the context of Ley 21.663 (Marco de Ciberseguridad). The core design constraint drives every decision: **no institutional data leaves the machine**. Everything (LLM inference, embeddings, vector search) runs locally via a bundled **llama.cpp** server + LanceDB. The one network-capable path is the optional `/search` endpoint — web search via **DDGS** (DuckDuckGo, an unofficial free client, no API key), only the query string ever leaves the machine — gated behind the `webSearchEnabled` config flag and the "Búsqueda web" toolbar pill; it defaults **off**. The "Fuentes oficiales" per-comuna source lookup remains parked behind a "Pronto disponible!" pill.

All user-facing text and LLM output is in Spanish. The system prompt forbids the model from inventing legal articles or references and requires it to answer only from retrieved context. It answers directly (no reflexive clarifying questions); for a vague procedural/payment query (e.g. "cómo pagar su parte?") a deterministic keyword classifier (not the LLM — see `main.py` `_is_ambiguous`) offers fixed category chips grounded in the corpus (aseo domiciliario, permiso de circulación, patente municipal, patente de alcoholes, derechos de propaganda) before answering. For "cómo/dónde pagar" procedural questions it points to the municipal channel (Tesorería / Dirección de Administración y Finanzas, or the comuna portal) without inventing specific offices, URLs or amounts.

## Commands

All backend commands run from inside `backend/` (paths in the code are relative to it — e.g. `rag.py` opens `db/`, `main.py` reads `../config.json`).

```powershell
# One-time setup
python -m venv venv && venv\Scripts\activate
pip install -r backend/requirements.txt        # runtime deps
pip install -r backend/requirements-dev.txt    # + pytest, for running tests
# No `ollama pull`: inference uses the bundled llama.cpp binary at backend/bin/
# llama-server.exe with GGUF models in backend/models/ (both gitignored, shipped
# by the installer). Model filenames come from config.json's "models" block.

# Download legal corpus from BCN (needs internet, run once)
cd backend
python corpus_fetcher.py                       # all tiers 0,1,2
python corpus_fetcher.py --tiers 0 1           # subset
python corpus_fetcher.py --municipio "Municipalidad de Chillán"   # + local ordenanzas

# Build the vector DB
python ingest.py --reset                       # wipe and rebuild db/
python ingest.py                               # append to existing db/
python ingest.py --corpus-dir C:/path/to/corpus --db-dir C:/path/to/db

# Run the API
uvicorn main:app --port 8000 --reload

# Tests & acceptance
pytest                                         # backend/tests/ (rag, ingest, audit)
python acceptance_m1.py                        # ~15 Spanish queries through retrieve()
```

This backend now runs as a **Tauri sidecar** of the MuniGPT host, not as a
standalone app. Its chat UI lives in `gui/frontend` (the "Asistente" tab) and its
process lifecycle (spawn, poll `/status`, reap the process tree on exit) is handled
in Rust by `gui/src/assistant.rs`. The former standalone React frontend
(`frontend/`) and Electron shell (`electron/`) have been **removed** — the chat
components were copied into `gui/frontend`, so there is nothing to import from here.
To run the assistant end to end, build and launch the MuniGPT GUI (see the root
`README.md` / `CLAUDE.md`); the sidecar starts the backend automatically.

`inference.py` starts the llama-server subprocesses lazily on first use (one for
chat, one for embeddings) and reaps them at exit — nothing external needs to be
running first. Scripts fail fast with a clear message if the binary or a required
model file is missing.

## Architecture

The request flow is: **corpus_fetcher.py** downloads PDFs → **ingest.py** chunks + embeds them into LanceDB → **main.py** serves chat, calling **rag.py** to retrieve context per query → the bundled **llama.cpp** server (via `inference.py`) generates the answer.

**`inference.py`** — local inference layer, imported by `main.py`, `rag.py`, and `ingest.py`. Manages the bundled official llama.cpp `llama-server` binary (`backend/bin/`) rather than an in-process Python binding (prebuilt `llama-cpp-python` wheels need AVX-512 that many target machines lack; the official binary does runtime CPU dispatch). Runs two lazily-started, localhost, OpenAI-compatible server processes — one chat, one embeddings (`--embedding`) — reaped at exit. Query-time and index-time embeddings go through the identical model and the correct nomic task prefixes (`search_query:` vs `search_document:`), a hard requirement for retrieval quality. Chat model is chosen by total RAM (FR-15): a low-RAM fallback below `lowRamThresholdGb` (default 12 GB).

**`main.py`** — FastAPI app, five endpoints:
- `POST /chat` — the core endpoint. Builds a **topic-aware retrieval query** from the recent user turns (via `_retrieval_query`, so multi-turn follow-ups like "menciona 5 ejemplos" keep the conversation topic instead of retrieving on the bare phrase), calls `rag.retrieve()`, injects the retrieved legal text into an augmented user message, then streams the model's response back as SSE. The stream sends a `citations` event first (so the frontend can render sources immediately), then `token` events, then a `done` event.
- `POST /ingest` — triggers a corpus ingest run.
- `POST /search` — web search via DDGS (DuckDuckGo), gated on `webSearchEnabled` in config.json (503 if off). `DDGS.text()` is a blocking call, run on a worker thread; a `DDGSException` (rate-limited, timeout, etc.) surfaces as a 502. Appends `{timestamp, query, resultCount}` to `backend/logs/search_audit.log`, one JSON line per outbound query (FR-07). DDGS is an unofficial client with no API key; it can throttle (202/403) and DuckDuckGo's ToS discourages automated use — a compliance call for Felipe, not enforced in code.
- `GET /status` — health check; the desktop shell polls this to know the backend is ready.
- `GET /config` — serves `config.json` (per-municipality branding + flags) to the frontend.

**`rag.py`** — hybrid retrieval, the heart of the system. `retrieve()` embeds the query via `inference.py`, runs **both** vector search and BM-25 full-text search against the same LanceDB table, then merges (vector results first, deduped by `(source, chunk_index)`, capped at `TOP_K=5`). FTS degrades gracefully to empty if the tantivy index is missing. LanceDB is synchronous, so the two searches run sequentially. Not a standalone script — imported by main.py.

**`ingest.py`** — builds the DB. Recursively scans `corpus/` for PDFs/TXTs (tier subdirectories are cosmetic — ingest flattens them). Chunks to ~500 chars with 50-char overlap, splitting on sentence boundaries. Embeds each chunk one-at-a-time via `inference.py` (embeddings are the slow step). Schema: `text, embedding, source, chunk_index, char_offset`. `source` is just the filename and is what appears in citations. Finally builds the FTS index that `rag.fts_search` depends on.

**`corpus_fetcher.py`** — downloads Chilean law PDFs from BCN's (leychile.cl) public export endpoint by `idNorma`. The corpus is defined as hardcoded tier lists (`TIER_0_GENERAL`, `TIER_1_CORE`, `TIER_2_EXTENDED`) — each entry is `{idNorma, filename, desc}`. To add a law, add an entry with its BCN norma id. BCN returns HTML error pages with HTTP 200 for bad ids, so the downloader sniffs content-type + size to detect failures. Municipality ordenanzas are discovered dynamically via BCN's CSV search endpoint.

**`eval/`** — offline eval harness (ROADMAP 0.4.0), the gate that makes Asistente
changes measurable. `eval/golden_set.json` is a corpus-grounded set (each question
maps to the real corpus file that answers it — no invented legal content; PENDING
OWNER APPROVAL). `eval/eval_harness.py` runs the golden set through the real
`rag.retrieve()` and scores recall@k / MRR / precision deterministically (no LLM), with
`--min-recall` as a release gate. Baseline over 45 questions on `db/`: recall@k=0.978,
MRR=0.87, mean_precision=0.77 (one genuine miss — transparencia activa — left in place
as real signal for the Horizonte reranker work). `eval/eval_judge.py` adds the LLM-judged layer
on top: it runs the real RAG pipeline per question and scores Ragas faithfulness /
answer_relevancy / context_precision with the bundled llama.cpp server as the judge
(fully offline), plus an abstention-decline check. Validated end-to-end; needs
`requirements-eval.txt` (pin langchain to 0.3.x — see that file). It is a heavy,
manual/offline activity (single-worker CPU judging, ~minutes per question), NOT a CI
gate; use `--limit N` for a smoke run. Score quality tracks the judge model (the demo
config forces the small 1.7B).

**`fetch_models.py` + `models.manifest.json`** — model distribution (D2). Two paths,
both gated by the REAL SHA256 in the manifest (measured from the local files): an
offline pack copyable from a USB/share for air-gapped municipios (`--pack DIR`, no
network), and resumable HTTP download-on-first-run (`aria2c` if vendored/on PATH, else
a built-in httpx Range download). Download only runs for entries whose `source.confirmed`
is true, so unconfirmed candidate URLs are never fetched. Any file whose SHA256 doesn't
match is rejected. Models dir: `MUNIGPT_MODELS_DIR` env, else `backend/models/`.

**`sanitize.py`** — indirect prompt-injection defenses for the RAG path (OWASP LLM
2025: RAG alone is not a defense). Two layers. **Index-time** (`sanitize_for_index`,
called by `ingest.py` before chunking): strips zero-width/bidi/control characters and
neutralizes instruction-override phrases and role markers, because `/ingest` lets IT
drop semi-trusted Tier-3 PDFs / ordenanzas into the corpus. **Prompt-time
spotlighting** (`build_data_block` + `clean_for_context`, called by
`rag.build_context`): wraps the retrieved context in `SPOTLIGHT_OPEN`/`SPOTLIGHT_CLOSE`
delimiters and strips those delimiters from chunks so a chunk can't close the block and
escape; `main.py`'s system prompt tells the model to treat that block as data, never
instructions. Retrieval-time cleaning also protects DBs built before layer 1 existed.

**`convert.py`** — a throwaway utility that converts every corpus PDF to TXT via PyMuPDF (`fitz`). Non-destructive (writes the TXT alongside the PDF; earlier versions deleted the original). Not part of the main pipeline — `ingest.py` already reads PDFs directly, so this is only for cases where pypdf extraction is poor and PyMuPDF does better.

**Chat UI (now in `gui/frontend`, not here)** — the React + Vite + TypeScript chat
components (`Chat.tsx`, `Message.tsx`, `SearchToggle.tsx`, `ComingSoonPill.tsx`,
`api.ts`) were copied into the MuniGPT GUI frontend during the merge and deleted
from this subtree. `api.ts` `streamChat` is a fetch + ReadableStream SSE parser that
consumes `/chat` (FR-04) and also dispatches a `disambiguate` SSE event
(deterministic category chips, no LLM); `webSearch` posts to `/search`. Citations
(source filename + chunk, FR-03/FR-12), web results, and disambiguation chips are
rendered by `Message.tsx`; the still-parked "Fuentes oficiales" per-comuna lookup by
`ComingSoonPill.tsx`. Per-municipality branding and `webSearchEnabled` come from
`GET /config`. These files now live under `gui/frontend/src/` — edit them there.

**Process lifecycle (now in `gui/src/assistant.rs`, not Electron)** — the backend
runs as a Tauri sidecar. The Rust host spawns the Python backend, polls `/status`
until `ready`, and reaps the whole process tree (uvicorn + llama-server children) on
exit. The former `electron/` desktop shell that did this has been removed.

**Packaging** — the Asistente's own former Inno Setup installer, from the days it
shipped separately (`assistant/installer/munigpt.iss`), has been removed. A single unified Tauri installer that bundles the scanner + this
backend (llama.cpp binary, GGUF models, corpus, `db/`) is a later merge phase
(Phase 5 in `docs/MERGE-PLAN-MuniGPT.md`) and is not built in-repo yet.

## Important notes

- **Corpus, `db/`, `backend/bin/`, and `backend/models/` are gitignored** (too large — shipped via installer, not git). `config.json` and `.env` are also gitignored, so per-install secrets (e.g. `license.licenseKey`) live only on the machine. `config.example.json` is the committed template. Web search needs no secret (DDGS requires no API key) — it's controlled purely by the `webSearchEnabled` flag.
- **Model choices are config-driven, not hardcoded.** `inference.py` reads the `models` block from `config.json` (falling back to `config.example.json`, then built-in defaults): `chatDefault` (`Qwen3-4B-Instruct-Q4_K_M.gguf`), `chatLowRam` (`Qwen3-1.7B-Q4_K_M.gguf`), `embedding` (`nomic-embed-text-v2-moe.Q4_K_M.gguf`), plus `lowRamThresholdGb`, `nCtx`, `nThreads`. Runs CPU-only, no GPU. To change a model, edit config and drop the GGUF into `backend/models/`.
- **Ollama has been fully removed** — replaced by the bundled llama.cpp server (commit `0ceb3ca`). The README, code, and this file all reflect that; older references to Ollama or `qwen2.5:3b`/`nomic-embed-text` are historical.
- **Offline licensing (FR-08) is not implemented yet.** `config.json` carries a `license` block placeholder and `requirements.txt` lists `cryptography`, but the actual license-verification scheme is a gated decision (see `docs/CHECKLIST_1.0.md` section B1) and must not be invented.
- **Status:** the backend is code-complete (M1). The chat UI (M2) now lives in the MuniGPT GUI (`gui/frontend`) and its lifecycle is a Tauri sidecar (M3, in `gui/src/assistant.rs`), replacing the removed standalone frontend + Electron shell. `docs/CHECKLIST_1.0.md` is the historical MuniGPT Definition-of-Done; the unified installer and remaining merge work are tracked in `docs/MERGE-PLAN-MuniGPT.md`.

## Working conventions

These come from the repo owner's global preferences and apply to all work in this repo:

- **No emojis** anywhere — code, comments, docs, commit messages, PR descriptions, or chat.
- **No AI attribution** — never add a `Co-Authored-By: Claude` trailer, a "Generated with Claude Code" line, or any mention/credit of an AI in commits, PRs, code, comments, or docs.
- **Never invent facts** — no made-up legal articles, norma ids, citations, numbers, or references. This is both a house rule and the core product requirement: if a detail isn't verified or provided, stop and ask rather than fill the gap. It mirrors the system prompt in `main.py`, which forbids the model from inventing legal references.
- **Not a lawyer — no legal advice.** This project sits in the Chilean municipal-law domain, but do not give legal opinions or interpretations. On any IP, licensing, contract, or liability question, decline and defer to a qualified lawyer.
- **Present decisions as interactive options** (the arrow-selectable question UI), not plain-text lists, whenever offering the user a choice — with exactly one option marked "(Recommended)" first, and the reasoning stated.
- **Commit/push only when asked.** This is a solo repo: when told to commit, work directly on `main` unless asked otherwise (do not spin up feature branches for small changes).
