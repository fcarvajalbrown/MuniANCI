# Handoff — 0.9.5 tomorrow, then the portable, and Thursday is real

You are in `C:\Projects\MuniGPT`, branch `main`, tree clean. Read `ROADMAP.md`, the repo
`CLAUDE.md`, `assistant/CLAUDE.md` and the global `CLAUDE.md` before touching anything.
This file is in English on purpose: anything written for Felipe to read is English. The
product, the commits and the docs stay in Chilean Spanish.

**Hard deadline: Felipe meets the mayor of Providencia on Thursday 2026-08-13 at 10:30.**
Wednesday 2026-08-12 is the only working day left. He has it free, and he chose to spend it
on **0.9.5, the retrieval orchestrator**, and then the portable — in that order. He said
today was enough and asked for cleanup, which is done.

**There is still no runnable binary on this machine, and there never has been since the
0.8.1 rename.** The old Desktop portable was deleted. Nothing has been observed launching.
Treat that as the day's real risk, not the RAG work.

═══════════════════════════════════════════════════════════════════════
WHAT SHIPPED TODAY, 2026-08-11 — ALL PUSHED
═══════════════════════════════════════════════════════════════════════

| Commit | What |
|---|---|
| `af5709a` | The 0.9.0 reranker blocker did not exist; research doc corrected |
| `b45a394` | The harness measures fragments, not only files |
| `7afcf2b` | The hybrid fusion became real RRF |
| `85b13db` | The BM-25 index became Spanish |
| `bda4098` | **Item 4** — the articulado is chunked per article, not per 500 chars |
| `fa932f6` | **Item 5** — a query naming an article retrieves it by metadata |
| `9805a37` | `assistant/CLAUDE.md` reflects schema v2 and the article route |
| `904184e` | The frontend builds with pnpm, from its own directory |

**Tramo A items 1 to 5 are done.** The measured progression, all on `db/`:

| config | archivo recall / MRR / nDCG | fragmento recall / MRR / nDCG |
|---|---|---|
| baseline (item 1) | 0.9787 / 0.867 / 0.8931 | 0.6667 / 0.5 / 0.5436 |
| + RRF (item 2) | 1.0 / 0.8535 / 0.8836 | 0.6667 / 0.375 / 0.4488 |
| + BM-25 español (item 3) | 1.0 / 0.8901 / 0.9034 | 0.6667 / 0.5417 / 0.5718 |
| + chunking por artículo (item 4) | 0.9787 / 0.8511 / 0.8854 | 0.6667 / 0.5333 / 0.5645 |
| + ruta de artículo (item 5) | 0.9787 / 0.8511 / 0.8854 | **0.8333 / 0.7 / 0.7311** |

**q46 — "¿Qué obliga el artículo 9 de la Ley 21.663?", the flagship failure of 2026-08-03 —
now answers at rank 1 at both file and fragment level.** Two consecutive harness runs came
back byte-identical, so it is not noise.

**The lesson worth carrying: item 4 alone bought nothing.** It moved the answering chunk
from invisible to BM-25 up to fourth in its list, and RRF still buried it, because RRF
rewards appearing in both lists more than a good rank in one. Item 5 is what collected the
gain. Expect the same shape in Tramo B: a change that improves the candidate pool can score
flat until something reorders it.

**Still open in Tramo A: items 6, 7 and 8** — parent-document with a size cap (§2.4),
norma-aware `citas.py` (§2.8), and the abstention threshold (§2.7). The research doc's §7
sequences the orchestrator **after** Tramo A. Felipe has chosen to go to 0.9.5 next anyway.
**Ask him through the option UI whether he wants 6 to 8 first**, do not silently pick.

`q29` (infracciones y sanciones) is the one remaining fragment-level miss. It names no
article, so the deterministic route never fires — it is the case Tramo B's reranker exists
for. One genuine file-level miss remains, transparencia pasiva.

═══════════════════════════════════════════════════════════════════════
THE GOAL — 0.9.5, ORQUESTADOR DE RECUPERACIÓN
═══════════════════════════════════════════════════════════════════════

Everything it consumes now exists. Read, in this order:

- `docs/adr/0004-orquestador-de-recuperacion-del-asistente.md` — the decision and why it
  waits for Tramo A
- `docs/design/2026-08-04-orquestador-de-recuperacion.md` — the design
- `docs/plans/2026-08-04-orquestador-de-recuperacion.md` — **tasks 5 to 8 remain**

Already delivered on 2026-08-04 and inert until the orchestrator uses them: `config_io.py`,
`corpus.py`, `rag.buscar_en()`, `rag.fusionar()`, `plan.py`. What is missing is the
schema-constrained model turn, the orchestrator loop, its wiring into `/chat`, and its
measurement with the harness.

Two turns of model maximum plus a wall-clock budget, both configurable, with a switch back
to today's fixed path. No agent framework — a loop over the `llama-server` that already
ships. Out of scope by decision: reformulate-and-retry, and web search as a model tool.

`ROADMAP.md` carries a HARD RULE: **ask Felipe through the option UI before starting any
0.X run**, and the repo `CLAUDE.md` requires a research pass first for every milestone.
0.9.5 already has its research, design and plan written and approved, so the research
obligation is met — but the "ask before starting" rule still applies.

═══════════════════════════════════════════════════════════════════════
THE TRAP THAT WILL WASTE YOUR MORNING
═══════════════════════════════════════════════════════════════════════

**`target/release/backend/munigpt-backend.exe` is the frozen PyInstaller sidecar from
2026-08-04, and `packaged_sidecar_bin()` in `gui/src/assistant.rs:257` looks there first.**

So a freshly built `munigpt-gui.exe` will silently run **August's Asistente**, which
carries August's Python and its own bundled copies of `db/` and `db_providencia` at 11.191
rows on schema v1. None of today's work would appear, and everything would look like it was
working normally.

Three ways past it:
- `MUNIGPT_SIDECAR_BIN` pointing somewhere else, or
- move `target/release/backend/` aside so the dev fallback spawns `assistant/.venv` against
  `assistant/backend/` — today's code, today's DBs, works on this machine only, or
- rebuild the sidecar with `tools\empaquetar-asistente.ps1` (928,6 MB, 6.900 files) and copy
  the current `db_providencia` into it — the only genuinely portable answer.

Related: `backend_dir()` resolves `MUNIGPT_BACKEND_DIR` first, then
`<exe_dir>/../../assistant/backend`, so from `target/release/` the dev fallback finds the
live source with no configuration.

═══════════════════════════════════════════════════════════════════════
THE BUILD — TWO CAUSES WERE FOUND, ONE IS FIXED
═══════════════════════════════════════════════════════════════════════

1. **Fixed.** `beforeBuildCommand` was `"npm run build"` with no prefix, and Tauri runs it
   from `gui/`, which has no `package.json`. That command could never succeed. Both hooks
   now read `pnpm --dir frontend run <script>`.
2. **Still true and unverified.** `devUrl` is `http://localhost:5173` and a debug binary
   loads it. That remains the leading explanation for the "chromium couldn't load page" that
   killed the 2026-08-10 meeting. A **release** build embeds `frontendDist` and cannot fail
   that way. Build release.

The pnpm migration was verified, not assumed: `pnpm import` converted `package-lock.json`
so pnpm resolved npm's existing tree, and `pnpm run build` reproduced the 2026-08-07 `dist`
**byte-for-byte across all 11 files**, same SHA256, same content hashes
(`index-C9Vju3-t.css`, `index-DmYct4LA.js`). pnpm 11 does not run install scripts by
default, which left esbuild without its platform binary; that is enabled in
`gui/frontend/pnpm-workspace.yaml` via `allowBuilds: esbuild: true`. The `pnpm` field in
`package.json` is no longer read by pnpm 11.

**`cargo tauri build --no-bundle` has still never been run to completion.** Nothing about
the build is proven.

═══════════════════════════════════════════════════════════════════════
DATA ON DISK — WHAT IS CURRENT AND WHAT IS NOT
═══════════════════════════════════════════════════════════════════════

| Path | State |
|---|---|
| `assistant/backend/db/` | **12.393 rows, schema v2**, rebuilt today, 934,5 s |
| `assistant/backend/db_providencia/` | **15.398 rows, schema v2**, rebuilt today (22 laws + 6 Providencia PDFs) |
| `assistant/backend/db_ejercito-de-chile/`, `db_fuerza-aerea-de-chile/` | **schema v1, untouched** — they warn on read and the article route declines |
| `assistant/backend/corpus/` | **24 txt + 24 sidecars** |
| `assistant/backend/dist/munigpt-backend/` | 2026-08-04, stale code and stale DBs |

**The corpus grew today and the DBs did not.** BCN finally delivered
`ley_18575_bases_administracion_estado` and `ds250_reglamento_compras`, which had never
downloaded successfully before. They are in `corpus/` with their sidecars but in **no DB**:
they were held out of today's ingest on purpose so the item 4 and 5 measurement stayed
attributable, and `ds250` is the reglamento of `ley_19886`, a strong distractor for q07 and
q39. Re-ingesting now shifts the harness baseline. That is a decision for Felipe, not a
cleanup task.

**Schema v2 fails loud on write, degrades declared on read.** Appending v2 rows into a v1
table raises, because mixing two chunk layouts in one index corrupts retrieval. Opening a v1
table works, warns once on stderr, and omits the article columns. `schema_version` lives in
`embedding_meta.json` next to `embedding_model`.

Reingest cost, measured for the first time: **934,5 s for 22 documents and 12.393 fragments**,
up from 8.186 (+51%, because articles no longer pack against each other). Budget about 20
minutes per DB rebuild.

═══════════════════════════════════════════════════════════════════════
TRAPS IN THIS ENVIRONMENT
═══════════════════════════════════════════════════════════════════════

- **PowerShell `if ($?)` after a Python command is a lie.** Python writes deprecation
  warnings to stderr, PowerShell wraps them as `NativeCommandError`, and `$?` goes false on
  a successful exit. It cost a half-built `db_providencia` today — the national laws went in
  and the Providencia PDFs silently did not. Chain with `;` and verify the row count, never
  with `if ($?)`.
- **Do not redirect a native exe's stderr** (`2>&1`) inside PowerShell for the same reason.
- **8.3 short paths break substring arithmetic.** The scratchpad resolves as `BEETLE~1`
  while `Get-ChildItem` returns `Beetlejuice`, so `$_.FullName.Substring($root.Length)`
  silently produces garbage. Compare by name and hash.
- **`H` is an alias for `Get-History`.** Naming a helper function `H` fails in a way that
  still prints your success message.
- **Here-strings do not survive a `;`-chained statement.** Use `git commit -F <file>`.

═══════════════════════════════════════════════════════════════════════
STILL OPEN, CARRIED FORWARD
═══════════════════════════════════════════════════════════════════════

- **"El Asistente no alcanzó a iniciarse" appeared once and was never explained.** Seen on a
  relaunch of the old portable; the backend was healthy one to two seconds later. Root cause
  not established. Reproduce before trusting the startup path in front of anyone.
- **The CSP wildcard is unproven in a webview.** `csp` and `devCsp` allow
  `http://127.0.0.1:*` and the sidecar demonstrably falls back off port 8000, but nobody has
  watched the Asistente tab answer while it sits on a fallback port. Reproduce with
  `python -m http.server 8000` in one shell and the app in another.
- **`assistant/config.json` says `municipio: "Organismo del Estado"`**, which slugs to a
  folder that does not exist, so `rag.db_dir()` falls back to `db/`. For a Providencia demo
  the identity has to come from `munigpt.config.json`'s `identidad` block (which wins over
  the compiled `MUNIGPT_INSTITUTION` since 0.8.0) or from `MUNIGPT_MUNICIPIO`;
  `gui/src/assistant.rs:205` passes it to the sidecar. `"Municipalidad de Providencia"`
  slugs to `db_providencia`.
- **Both chat models are on disk** and `config.json` currently forces
  `Qwen3-1.7B-Q4_K_M.gguf` for both `chatDefault` and `chatLowRam`, which is what Felipe
  wanted so he can switch with Notepad at the venue. The 4B answers better, 27,1 s against
  the 1.7B's 9,0 s.
- **Frontend has no automated tests.**
- **The CHANGELOG's 0.7.0 section still defers deep scanning "a 0.8.0"**, now renumbered to
  0.8.6. True when published; rewriting a released section diverges from the GitHub release.
  Decide, do not silently edit.
- **`gui/src/commands/ajustes.rs:246`** uses the deprecated `tauri_plugin_shell::Shell::open`.
- **The defence corpora have no personnel statute.** Ley N° 18.948 is cited in the 2025
  reglamento's own *Vistos* but is not indexed. Recipe in
  `assistant/backend/corpus_defensa/README.md`. Needs the PDF from Felipe.
- **`norma` is not a key.** It is not unique — the estatuto docente, the código del trabajo
  and the LOC de municipalidades are all `"Decreto con Fuerza de Ley 1"` — and it is not how
  the laws are cited: the LOC is the Ley 18.695, juntas de vecinos the Ley 19.418, the
  Constitución is `Decreto 100`. 17 of 22 files have an artículo 9. That is why the article
  route resolves nothing by name and restricts itself to files retrieval already returned.

═══════════════════════════════════════════════════════════════════════
BUILDING AND TESTING
═══════════════════════════════════════════════════════════════════════

| Command | Output | Contains |
|---|---|---|
| `cargo build --release -p munigpt-cli` | `target\release\munigpt-cli.exe` | Scanner, CLI only |
| `cargo tauri build --no-bundle` | `target\release\munigpt-gui.exe` | GUI, runnable, no installer |
| `cargo tauri build` | NSIS + MSI | Scanner only |
| `cargo tauri build --config tauri.asistente.conf.json` | NSIS + MSI | Scanner + Asistente |
| `tools\empaquetar-asistente.ps1` | `assistant\backend\dist\munigpt-backend\` | Frozen sidecar, 928,6 MB |

```powershell
cd assistant\backend
..\.venv\Scripts\python.exe -m pytest                      # 180 green as of this handoff
$env:MUNIGPT_DB_DIR="db"; ..\.venv\Scripts\python.exe eval\eval_harness.py
..\.venv\Scripts\python.exe ingest.py --db-dir <db> --reindex-fts   # BM-25 only, no re-embed
..\.venv\Scripts\python.exe corpus_fetcher.py              # backfills missing .estructura.json sidecars
```

`corpus_fetcher.py` **never rewrites an existing `.txt`** — the golden set quotes it
verbatim, so its bytes are a fixture. It re-fetches only to write a missing sidecar.

`target/debug` was deleted today, freeing 16,01 GB; `target/release` (6,01 GB) was kept, so
release dependencies are still warm. 122 GB free. `bundle.targets` is `"all"`, so one
packaging run emits NSIS and MSI both, about 1,5 GB per round — clean up afterwards.
`assistant\backend\dist\` is kept on purpose; the installer and the portable are built from
it.

Felipe does not want an automatic Desktop shortcut per build. Report the path and ask.

═══════════════════════════════════════════════════════════════════════
REPO RULES THAT BITE
═══════════════════════════════════════════════════════════════════════

- **Never invent.** No norma, figure or citation without the primary source. BCN LeyChile's
  navigation app and the Wiki Guías both return plausible-looking nothing to a fetch. The
  `obtxml?opt=7&idNorma=` service **does** answer, but only with a browser User-Agent.
- **Not legal advice.**
- **No comments in code.** None, including docstrings. Existing ones are debt awaiting
  deletion, never licence to add more.
- **No emojis, no AI attribution** anywhere, including commit trailers.
- **Commit and push actively**, Conventional Commits, straight onto `main`. Never open a PR
  unless asked in that turn.
- **Nothing is done without real command output.**
- **Present every decision through the option UI**, one marked "(Recommended)" and first,
  with the reasoning. One topic per reply.
- **Never paste a CHANGELOG section into a GitHub release**; run `notas-release`.
- **Mark milestones in `ROADMAP.md`** when they ship.
- **New binaries need their extension in `.gitattributes` before staging**, then prove the
  round trip with `git cat-file -p HEAD:<path> | sha256sum`.
- **Superpowers skills are opt-in.** Do not invoke one unless Felipe names it that turn.
- English to Felipe; Spanish in the product, the commits and the docs.
