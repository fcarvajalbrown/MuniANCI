# Handoff — v0.8.0 is prepped but not tagged, and the panel has never been clicked

You are in `C:\Projects\MuniANCI`, branch `main`, tree clean, everything pushed. Read
`ROADMAP.md`, the repo `CLAUDE.md` and the global `CLAUDE.md` before touching anything.
This replaces the previous handoff, whose release question is now answered.

This file is in English on purpose: the global rule says anything written for Felipe to
read is English, handoffs included. The product, the commits and the docs stay in Chilean
Spanish. The older Spanish handoff predates that rule.

═══════════════════════════════════════════════════════════════════════
STATE
═══════════════════════════════════════════════════════════════════════

**v0.8.0 is fully prepared and deliberately not tagged.** Felipe asked for prep only and
holds the decision to tag and publish, which the repo rule requires anyway because a
release is outward-facing.

Done and pushed:

| | |
|---|---|
| Versions | All four fields at `0.8.0` (workspace, `gui/Cargo.toml`, `package.json`, `tauri.conf.json`) |
| CHANGELOG | `[0.8.0] — 2026-08-03`, covering all 31 commits since `v0.7.0` |
| Release notes | `cargo run -q -p notas-release -- 0.8.0`, humanizer pass done |
| ROADMAP | Milestone marked, deep scanning renumbered to 0.8.5 |
| README | Build command, `MUNIANI_ADMIN_HASH`, `installer/` description |
| ADRs | `docs/adr/` created, 0001-0003 |

**The release covers two bodies of work, not one.** Half the range is the Asistente
packaging, finished 2026-07-25 and never released because the version question sat
unanswered. The other half is the IT settings panel, runtime identity and the defence
corpus, done 2026-08-03.

To finish the release:

```powershell
cargo run -q -p notas-release -- 0.8.0 > notas.md
git tag v0.8.0
git push origin v0.8.0
gh release create v0.8.0 --notes-file notas.md
```

═══════════════════════════════════════════════════════════════════════
WHAT IS NOT VERIFIED — read this before claiming anything works
═══════════════════════════════════════════════════════════════════════

**1. The settings panel was exercised by Felipe on 2026-08-03 and held up**, including a
deliberate attempt to break it. That closes what was the largest open risk in this release.
Note the limits of that check: it was a debug build, so it did not exercise the real
password path, and the frontend still has no automated tests because this repo has no
frontend test harness. What a release build adds is the Argon2id lock; force it in debug
with `$env:MUNIANI_FORCE_LOCK = "1"; cargo run -p muniani-gui`. A debug run also needs the
Vite dev server (`npm --prefix gui/frontend run dev`), because `devUrl` points at
`localhost:5173`.

A debug build **bypasses the password on purpose** and shows a banner saying so. To exercise
the real unlock path: `$env:MUNIANI_FORCE_LOCK = "1"; cargo run -p muniani-gui`. A debug run
also needs the Vite dev server (`npm --prefix gui/frontend run dev`), because `devUrl`
points at `localhost:5173`.

**2. The port fallback versus the CSP — fixed, but not confirmed in a webview.**
`1579ad3` made the app survive port 8000 being occupied, and `puerto_utilizable`
(`gui/src/assistant.rs:292`) picks another port, while `gui/tauri.conf.json` pinned
`connect-src` to `http://127.0.0.1:8000`. The app would have come up with an Asistente tab
unable to reach its own backend, failing only in the webview console.

Both `csp` and `devCsp` now allow `http://127.0.0.1:*`. The origin is still the local
machine, so nothing off-box became reachable. **What was verified:** the JSON parses and
the two `puerto_utilizable` tests pass. **What was not:** nobody has run the app with 8000
occupied and watched the Asistente answer on the fallback port. Reproduce with
`python -m http.server 8000` in one shell and the app in another. Until someone does that,
treat the fix as reasoned rather than proven.

**3. Inherited from the previous handoff and still open.** These were never closed:

- **Does `historico_<comuna>.db` survive a reinstall?** This is the important one.
  `ruta_historico()` (`gui/src/commands/monitoreo.rs:49`) writes it next to the executable,
  inside the install directory, and it holds months of accumulated measurements that do not
  regenerate. `munianci.config.json` sits there too, by documented design. The only thing
  protecting them is that NSIS removes only what it installed. Evidence in favour: the
  `models` folder survived an uninstall. The `.db` was never tested. Test: install, scan,
  record size and hash of the `.db`, reinstall the same installer, verify. If it does not
  survive, no municipality should install a new version over an old one until it is fixed.
- **The `installer/asistente.nsh` hook never actually ran.** It kills `munigpt-backend.exe`
  and `llama-server.exe` and clears `$INSTDIR\backend` before copying. It compiled into the
  installer; running it needs installing twice.
- **A real query answered from an installed app.** The frozen backend answered with
  citations running loose, and the installed app reached `ready:true`, but nobody typed a
  question into the tab of an installed copy and read the answer.
- **The "not installed" panel**, forced by renaming `backend\` in the install directory.

**4. Smaller, real, and mine.** `gui/src/commands/ajustes.rs:246` uses
`tauri_plugin_shell::Shell::open`, which is deprecated in favour of `tauri-plugin-opener`.
It works; it emits a build warning.

**5. The defence corpus has no personnel statute.** `db_ejercito-de-chile` and
`db_fuerza-aerea-de-chile` deliberately exclude municipal law, so a question about staff
duties or discipline has no correct source to answer from. Ley N° 18.948 is cited in the
2025 reglamento's own *Vistos* but is not indexed. Adding it is a short ingest; see
`assistant/backend/corpus_defensa/README.md` for the rebuild recipe.

**6. `CHANGELOG.md` 0.7.0 still says the deep scanning is deferred "a 0.8.0".** That
milestone was renumbered to 0.8.5 tonight. The line was true when published and rewriting a
released section diverges the file from the published GitHub release, so it was left alone
on purpose. Decide, do not silently edit.

═══════════════════════════════════════════════════════════════════════
BUILDING (tested, not assumed)
═══════════════════════════════════════════════════════════════════════

```powershell
# 1. Freeze the sidecar and stage its assets (~1 min of PyInstaller)
powershell -ExecutionPolicy Bypass -File tools\empaquetar-asistente.ps1

# 2. Full installer. The overlay is mandatory, or the Asistente does not travel.
cd gui; cargo tauri build --config tauri.asistente.conf.json
```

`cargo build --release -p muniani-gui` is **not** a valid test build: it does not embed the
frontend, so the app falls back to `devUrl` and shows the Edge error page. For a runnable
executable without waiting for LZMA, use `cargo tauri build --no-bundle`.

Per-client builds also need `MUNIANI_ADMIN_HASH`, an Argon2id PHC string, or the release
build will ask the user to set a password on first cog press.

Disk: `target/debug` reached 46.74 GB once. Do not chain installer builds without checking
size; the repo `CLAUDE.md` has the cleanup rule.

═══════════════════════════════════════════════════════════════════════
WHERE THE ROADMAP GOES NEXT
═══════════════════════════════════════════════════════════════════════

Pending and un-started: **0.7.5** otros órganos del Estado, **0.7.6** desmunicipalizar,
**0.7.7** enrutamiento al CSIRT-DN, **0.8.5** escaneo profundo + Asistente avanzado +
orquestador, **0.9.0** calidad del RAG, **1.0.0** piloto.

0.7.6 and 0.7.7 exist because of the defence demos. 0.7.7 now has its identifier verified
from the primary source: decreto Núm. 2 de la Subsecretaría de Defensa, Diario Oficial
Núm. 44.337, 31-DIC-2025, CVE 2748664, PDF in `docs/`. Only its first two pages have been
read; the remaining seven are part of that milestone's research pass.

**HARD RULE: ask Felipe through the option UI before starting any 0.X run**, and whenever in
doubt during it. Do the research pass first, with explicit rejects, then confirm scope.

═══════════════════════════════════════════════════════════════════════
REPO RULES THAT BITE
═══════════════════════════════════════════════════════════════════════

- **Never invent.** No norma, figure or citation without reading the primary source. BCN
  LeyChile and the Wiki Guías both return plausible-looking nothing to a fetch. The FACH and
  Ejército transparency portals return 403 for HTML but serve PDFs whose URL you already
  know, so their indexes cannot be enumerated; ask Felipe for the file.
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

═══════════════════════════════════════════════════════════════════════
SUGGESTED FIRST STEP
═══════════════════════════════════════════════════════════════════════

Ask Felipe whether to tag and publish v0.8.0 as prepared. Before he answers, the honest
thing is to get the panel exercised in a running app and to reproduce the CSP port
collision, because both affect whether this release should go out as it stands. The
`historico` reinstall test matters more than either, and it needs an installer.
