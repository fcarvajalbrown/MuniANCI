# Handoff — next goal is 0.9.0, and Felipe triggers it

You are in `C:\Projects\MuniANCI`, branch `main`. Read `ROADMAP.md`, the repo `CLAUDE.md`
and the global `CLAUDE.md` before touching anything. This file is in English on purpose:
anything written for Felipe to read is English. The product, the commits and the docs stay
in Chilean Spanish.

═══════════════════════════════════════════════════════════════════════
THE GOAL — 0.9.0, CALIDAD DEL ASISTENTE (RAG)
═══════════════════════════════════════════════════════════════════════

**Do not start it. Felipe triggers it.** `ROADMAP.md` carries the HARD RULE: ask him
through the option UI before starting any 0.X run, and the repo `CLAUDE.md` requires a
research pass first — as many searches as the topic needs, a structured writeup per
candidate technique, explicit rejects with reasons, and a prioritized shortlist brought to
him as options before any code.

**The night of 2026-08-03 produced real evidence that scopes this milestone.** It is not
theory any more:

1. **Retrieval misses on the product's own flagship question.** Asked *"¿Qué obliga el
   artículo 9 de la Ley 21.663?"*, both chat models answered that the article was not in
   the retrieved context. It is: the corpus file has two matches for Artículo 9, and the
   retrieval even cited `ley_21663_ciberseguridad.txt` — the wrong chunk. This is the
   reranker / RRF / structure-aware chunking work in 0.9.0, measured against the harness.
2. **There is no relevance threshold and no abstention gate.** `rag.retrieve()` always
   returns its top 5 chunks. An off-topic question still reaches the model wrapped in
   official-looking legal text, and the model accommodates it. See the incident below.
3. **A mitigation already landed, and its limits define more work.** `citas.py` blocks
   invented article numbers (commit `e8ca37f`). It does not check which norma an article
   belongs to, and it does not touch other inventions — the model still expands CSIRT-DN
   wrongly, differently on each run. Both are 0.9.0 material.

═══════════════════════════════════════════════════════════════════════
THE INCIDENT THAT MADE THIS URGENT
═══════════════════════════════════════════════════════════════════════

Asked *"cuál es el clima"*, the Asistente answered that the climate was *"el ambiente de
trabajo y operación de las Fuerzas Armadas, establecido en el artículo 32 del Reglamento
de Ciberseguridad de la Defensa Nacional"*, and rendered the PDF as its source.

Verified against the decree itself: **it has 23 articles, and the word "clima" appears
zero times.** A fabricated legal citation attributed to a real norma, in the product whose
central promise is that it does not do that.

`citas.py` now blocks it deterministically: every article number in an answer must appear
literally in a retrieved chunk, or the answer is replaced by a refusal naming the article
it could not support. `/chat` therefore buffers the answer before emitting it — a token
already shown cannot be withdrawn — so there is no progressive typing any more. Sources
still appear immediately.

═══════════════════════════════════════════════════════════════════════
SHIPPED ON 2026-08-03, ALL PUSHED
═══════════════════════════════════════════════════════════════════════

| Commit | What |
|---|---|
| `669efa2` | The history survives a reinstall — measured, see below |
| `87f8dfb` | The settings panel no longer discards what IT typed |
| `102d387` | The header no longer shows the previous institution until restart |
| `470f559` | The chat model loads at startup instead of on the first question |
| `e3bbf84` | The frozen sidecar now carries the Defence corpora |
| `e8ca37f` | No answer is delivered citing an article it cannot support |

**`historico_<comuna>.db` survives a reinstall.** Two handoffs carried this as the largest
data-loss risk; it is closed. A real scan produced a 53.248-byte `.db`
(`545EDA63…2CE45B6B`) and a 4.605-byte `munianci.config.json` (`02FAE0D1…990965BA1`); both
came through `uninstall.exe /P _?=$INSTDIR` plus a reinstall byte-identical. The generated
NSIS deletes only what it installed — one `Delete` per bundled file, non-recursive `RMDir`,
and no `RMDir /r "$INSTDIR"` anywhere. Limits: it was 0.7.0 over 0.7.0 rather than a true
upgrade, it ran passive so the "delete app data" checkbox was never ticked (it targets
`$LOCALAPPDATA\cl.felipecarvajalbrown.muniani`, a different path from the install
directory, so it cannot reach the `.db`), only NSIS was tested, and the `.db` came from the
CLI rather than a GUI scan.

**The model preloads.** Measured, same question: 40,7 s cold, 27,1 s warm, preload
finishing 8,1 s after the backend starts with nobody touching anything. `/status` exposes
`modelosPrecargados`, because its `ready` flag only ever meant that the model *files* and
the binary were present.

═══════════════════════════════════════════════════════════════════════
THE PORTABLE BUILD
═══════════════════════════════════════════════════════════════════════

`C:\Users\Beetlejuice\Desktop\MuniANCI-portable`, 4,44 GB, with a Desktop shortcut. Built
for a presentation on 2026-08-04. Not an installer, not a documented build type — assembled
by hand from the pieces below.

```
muniani-gui.exe          target\release\muniani-gui.exe
backend\                 assistant\backend\dist\munigpt-backend\  (tools\empaquetar-asistente.ps1)
backend\models\          both chat GGUF plus the embedding model
config.json              Asistente config; models.chatDefault selects the chat model
munianci.config.json     scanner identity
```

Verified on the finished copy: preload 12,6 s, corpus `db_fuerza-aerea-de-chile`, answers
in 5,8 - 10,7 s, citation guard present (the whole answer arrives as a single `token`
event, which the old code could not do), and it falls back off port 8000 on its own.

`config.json` currently selects `Qwen3-1.7B-Q4_K_M.gguf`. Changing that one line to
`Qwen3-4B-Instruct-Q4_K_M.gguf` switches models with no rebuild and no download; the 4B
answers better but took 27,1 s against the 1.7B's 9,0 s.

Three questions tested against it, all answering from the right primary sources:
- ¿Qué es el CSIRT de la Defensa Nacional y de quién depende?
- ¿A quién debe reportar un incidente de ciberseguridad un organismo de las Fuerzas Armadas?
- ¿Qué establece la Política General de Seguridad de la Información de las Fuerzas Armadas?

The second cites Artículo 17, which was checked against the PDF and is correct.

═══════════════════════════════════════════════════════════════════════
OPEN — READ BEFORE CLAIMING ANYTHING WORKS
═══════════════════════════════════════════════════════════════════════

- **`Cargo.toml` and `Cargo.lock` are uncommitted.** A workspace-wide dependency upgrade
  Felipe started, plus `lopdf` pinned back to `0.42` by this session. `lopdf` 0.44.0 is the
  newest published and does not compile against `time` 0.3.47, which is itself the highest
  `time` its own requirement allows — upstream broken, not waiting for an update. The
  toolchain was updated from 1.94.0 to stable **1.97.1** to get past `libsqlite3-sys`
  0.38.1, whose build script uses the then-unstable `cfg_select!`. Decide what to do with
  the upgrade; the portable does not depend on it.
- **"El Asistente no alcanzó a iniciarse" appeared once and was never explained.** It
  showed on a relaunch of the portable; the backend for that same window was healthy one to
  two seconds later, and questions answered normally afterwards. Root cause not
  established. Reproduce before trusting the startup path in front of anyone.
- **The CSP wildcard is still unproven in a webview.** `csp` and `devCsp` allow
  `http://127.0.0.1:*`, and the sidecar demonstrably falls back off port 8000. But every
  successful webview conversation so far happened while the backend held 8000. Nobody has
  watched the Asistente tab answer while the sidecar sits on a fallback port. Reproduce
  with `python -m http.server 8000` in one shell and the app in another.
- **Frontend has no automated tests.** The panel and header fixes pass `tsc` and
  `vite build` and were exercised by hand; there is no harness in this repo to assert their
  behaviour.
- **v0.8.0 is prepared and still not tagged.** Versions, CHANGELOG, release notes, README
  and the ADRs are all done. Publishing is outward-facing, so it is Felipe's call.
- **`assistant\config.json` now selects the 1.7B model.** It is gitignored and
  per-machine. Change it back if the 4B should be the default again.
- **The CHANGELOG's 0.7.0 section still says deep scanning is deferred "a 0.8.0"**, which
  was renumbered to 0.8.5. True when published; rewriting a released section diverges the
  file from the published GitHub release. Decide, do not silently edit.
- **`gui/src/commands/ajustes.rs:246`** uses the deprecated `tauri_plugin_shell::Shell::open`.
  Works, emits a build warning.
- **The defence corpora have no personnel statute.** Ley N° 18.948 is cited in the 2025
  reglamento's own *Vistos* but is not indexed, so a staff-duties question has no correct
  source. Short ingest; recipe in `assistant/backend/corpus_defensa/README.md`. Needs the
  PDF from Felipe, since BCN LeyChile cannot be fetched.

═══════════════════════════════════════════════════════════════════════
BUILDING
═══════════════════════════════════════════════════════════════════════

| Command | Output | Contains |
|---|---|---|
| `cargo build --release -p muniani-cli` | `target\release\muniani-cli.exe` | Scanner, CLI only |
| `cargo tauri build --no-bundle` | `target\release\muniani-gui.exe` | GUI, runnable, no installer |
| `cargo tauri build` | NSIS + MSI | Scanner only |
| `cargo tauri build --config tauri.asistente.conf.json` | NSIS + MSI | Scanner + Asistente |
| `tools\empaquetar-asistente.ps1` | `assistant\backend\dist\munigpt-backend\` | Frozen sidecar, 928,6 MB |

`bundle.targets` is `"all"`, so one run emits NSIS and MSI both — that is where the ~1,5 GB
per round comes from. Clean up after a packaging session; `assistant\backend\dist\` is kept
on purpose, because the installer and the portable are built from it.

Backend tests: `cd assistant\backend; ..\.venv\Scripts\python.exe -m pytest` — **110 green**
as of this handoff.

Felipe does not want an automatic Desktop shortcut per build. Report the path and ask.

═══════════════════════════════════════════════════════════════════════
REPO RULES THAT BITE
═══════════════════════════════════════════════════════════════════════

- **Never invent.** No norma, figure or citation without the primary source. BCN LeyChile
  and the Wiki Guías both return plausible-looking nothing to a fetch.
- **Not legal advice.**
- **No comments in code.** None, including doc comments. Existing ones are debt.
- **No emojis, no AI attribution** anywhere, including commit trailers.
- **Commit and push actively**, Conventional Commits, straight onto `main`. Never open a PR
  unless asked in that turn.
- **Nothing is done without real command output.**
- **Never paste a CHANGELOG section into a GitHub release**; run `notas-release`.
- **Mark milestones in `ROADMAP.md`** when they ship.
- **New binaries need their extension in `.gitattributes` before staging**, then prove the
  round trip with `git cat-file -p HEAD:<path> | sha256sum`.
- English to Felipe; Spanish in the product, the commits and the docs.
