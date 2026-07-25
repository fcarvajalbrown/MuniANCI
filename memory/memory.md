# MuniANCI Development Session Memory
## Felipe Carvajal Brown — March 2026

---

## Project Overview

**MuniANCI** is a cybersecurity compliance scanner for Chilean municipalities under **Ley 21.663 (Marco de Ciberseguridad)**. Combines active network scanning with a declarative questionnaire to produce a PDF gap report and CSIRT JSON.

**Stack:** Rust workspace (`core` lib + `cli` binary + `gui` Tauri 2), `printpdf 0.9.1`, `native-tls 0.2`, `rayon`, `serde`, `windows` crate, `nix` crate. GUI: React/TypeScript/Vite + Tauri 2.

**Repo:** `C:\Users\Beetlejuice\Desktop\MuniANCI\`

---

## Workspace Layout

```
muniani/
├── Cargo.toml                              # workspace members = ["core", "cli", "gui"]
├── core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs
│       ├── questionnaire.rs
│       ├── normalizer.rs
│       ├── compliance_engine.rs
│       ├── report_builder.rs
│       ├── eol_enrichment.rs
│       ├── data/
│       │   └── eol_db.json                # bundled EOL database (include_str!)
│       ├── os_abstraction/
│       │   ├── mod.rs
│       │   ├── windows.rs
│       │   └── unix.rs
│       └── probes/
│           ├── mod.rs
│           ├── host_discovery.rs
│           ├── drive_enum.rs
│           ├── service_probe.rs
│           ├── sw_inventory.rs
│           └── os_check.rs
├── cli/
│   ├── Cargo.toml
│   └── src/main.rs
└── gui/
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── build.rs
    ├── icons/
    └── src/
        ├── main.rs
        ├── lib.rs
        └── commands/
            ├── mod.rs
            ├── start_scan.rs
            └── export_report.rs
    └── frontend/
        ├── index.html
        ├── package.json
        ├── vite.config.ts
        ├── tsconfig.json
        └── src/
            ├── main.tsx
            ├── App.tsx
            ├── app.css
            ├── types.ts
            ├── vite-env.d.ts
            └── components/
                ├── WorkerTab.tsx
                └── ItTab.tsx
        └── public/
            └── fonts/                     # self-hosted IBM Plex Sans + Mono
```

---

## Key Dependencies (core/Cargo.toml)

```toml
printpdf = "0.9.1"
native-tls = "0.2"
serde_json = "1"
windows = "0.62.1"      # features: Win32_NetworkManagement_WNet, Win32_Storage_FileSystem,
                        # Win32_System_Wmi, Win32_Foundation, Win32_System_Registry,
                        # Win32_System_SystemInformation, Win32_System_Com,
                        # Win32_System_Ole, Win32_System_Variant
nix = "0.31.1"          # features: mount, net
rayon = "1"
```

## Key Dependencies (gui/Cargo.toml)

```toml
tauri = { version = "2", features = ["devtools"] }
tauri-plugin-dialog = "2"
tauri-plugin-shell = "2"
tokio = { version = "1", features = ["full"] }
thiserror = "2"
```

---

## Architecture Decisions

- `ScanConfig` (has closures, not serialisable) + `ScanMeta` (serialisable, stored in ScanResult)
- `ScanConfig` has two callbacks: `progress_cb: Option<Box<dyn Fn(u8) + Send + Sync>>` and `log_cb: Option<Box<dyn Fn(&str) + Send + Sync>>`
- CLI sets `log_cb: None`; GUI wires both via Tauri Channel with AtomicU8 to track last pct
- `ScanResult.meta: ScanMeta` not `config: ScanConfig`
- `backup_agent_running: Option<bool>` in `OsInfo` — None=WMI failed, Some(false)=no agent, Some(true)=confirmed
- Rayon parallelism via nested `rayon::join()` in `lib.rs`
- Two gap sources: objective (scanner probes) + declarative (questionnaire) → both feed `compliance_engine::evaluate()`
- Art. 27° significance filter: `requires_csirt_report` only fires for OIV/PSE + Critical + network-reachable controls
- LAN sweep parallelized with rayon, 150ms TCP timeout per host
- Questionnaire runs by default — `--no-questionnaire` flag exists but is NOT the default
- EOL enrichment runs after `normalizer::normalize()` and before `compliance_engine::evaluate()`
- Per-client build: `MUNIANI_INSTITUTION` and `MUNIANI_TIER` are compile-time env vars baked into GUI binary

---

## GUI Architecture (Tauri 2)

- Two tabs: Vista Municipal (worker) + Vista Técnica (IT)
- Worker tab: traffic-light gap summary, UTM fines, CSIRT notice, legal context — no technical data
- IT tab: full gap table with evidence (clickable rows expand), live log terminal, asset detail, export bar
- Progress streamed via `Channel<ScanProgress { pct: u8, log: String }>`
- `start_scan` command: async, `spawn_blocking` for core scan, AtomicU8 shared between progress_cb and log_cb
- `export_report` command: native save dialog, writes PDF or JSON, opens folder in Explorer after save
- Design: NIST/NSA aesthetic — IBM Plex Sans + IBM Plex Mono, dark navy palette, federal blue accent
- Logo: placeholder `▣` — final logo TBD in v0.3.0
- Fonts: self-hosted in `frontend/public/fonts/` — no Google Fonts (offline municipalities)
- `tauri.conf.json`: `plugins.dialog: null`, `plugins.shell.open: true`, window starts maximized
- Dev: `cargo tauri dev` from `gui\` — frontend at `http://localhost:5173`
- CSP: `default-src 'self'` — no external resources in production

---

## printpdf 0.9.1 API (CRITICAL — always use these)

- `doc.add_font(BuiltinFont::Helvetica)` — NOT `add_builtin_font`
- Ops: `Op::SetFontAndFontSize` + `Op::WriteTextBuiltinFont` + `Op::BeginTextSection` + `Op::EndTextSection` + `Op::SetTextCursor`
- `Point` fields need `Mm(...).into()` to convert to `Pt`
- `save()` takes `(&PdfSaveOptions::default(), &mut Vec::<PdfWarnMsg>::new())`
- `PdfPage::new(Mm(W), Mm(H), ops)` then `doc.with_pages(vec![page]).save(...)`
- Only `report_builder.rs` uses printpdf
- PDF strings must be sanitized via `to_pdf_safe()` — builtin Type1 fonts use WinAnsiEncoding, UTF-8 multi-byte chars corrupt
- `report_builder::write_pdf` must be `pub` — used by `export_report.rs` in GUI
- `write_pdf` signature: `pub fn write_pdf(result: &ScanResult, path: &str) -> Result<()>`
- Call from GUI: `report_builder::write_pdf(&result, &path.to_string_lossy())`

---

## windows.rs API Notes (CRITICAL)

- `RegOpenKeyExW` reserved param = `Some(0)` not `0`
- `RegEnumKeyExW` takes `Some(PWSTR(...))` directly
- `RtlGetVersion` imported from `Win32::System::SystemInformation` directly (not extern block)
- `RESOURCEUSAGE_CONTAINER.0` for u32 cast in `dwUsage`
- Drive types: raw u32 match — `3` = Fixed, `2` = Removable
- WMI `VARIANT`: access via `v.Anonymous.Anonymous.vt` (no `as_raw()`), match directly on `inner.vt` — do NOT wrap in `VARENUM()`
- `EOAC_NONE` is in `Win32::System::Com` not `Ole`
- `VT_NULL` is in `Win32::System::Variant` not `Com`
- `ConnectServer` optional BSTR params: pass `&BSTR::default()` not `None`
- WMI stubs replaced with real COM implementation: `wmi_query()`, `wmi_scalar_u32()`, `wmi_string_list()`
- Firewall detection uses registry (no WMI needed): `HKLM\SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters\FirewallPolicy\{Domain,Standard,Public}Profile\EnableFirewall`

---

## v0.2 Status — COMPLETE (2026-03-31)

### ✅ All completed
- WMI COM implementation (`wmi_query`, `wmi_scalar_u32`, `wmi_string_list`)
- Real firewall detection via registry
- `backup_agent_running: Option<bool>` wired through os_check → OsInfo → compliance_engine
- LAN sweep parallelized with rayon
- TLS cert chain validation via `native-tls` — Expired, SelfSigned, ExpiredAndSelfSigned
- PDF encoding fix — `to_pdf_safe()`
- BitLocker suppressed for PSE tier
- EOL enrichment — 38 products, bundled eol_db.json, fixes Office 2016 false negative
- Tauri 2 GUI — Vista Municipal + Vista Técnica, live log terminal, export PDF/JSON
- ScanConfig `log_cb` — technical log lines streamed to IT terminal
- 28 unit tests passing

---

## v0.3.0 Priorities (in order)

1. **Security audit** — Tauri IPC surface, command exposure, capability permissions, all core Rust modules
2. **`check_utm` command** — fetch current UTM from SII if online, fallback to bundled approximate (~CLP $66,000)
3. **`legal_refs.json` corpus** — bundle Ley 21.663, Ley 21.459, ANCI IGs as structured JSON (include_str!), replace hardcoded citation strings
4. **Logo design and color scheme** — define final MuniANCI brand identity
5. **PDF layout redesign** — NIST/NSA colors, final logo, proper typography (post logo)
6. **More Tauri commands** — think of additional commands to expose (TBD)
7. **Code signing cert** — DigiCert/Sectigo ~$200-400/yr, required before municipal delivery; unsigned .exe blocked by McAfee/Defender ATP
8. **Inno Setup packaging** — portable .exe output (NOT an installer wizard)

---

## Legal Corpus

- **Ley 21.663** (DO 08/04/2024): Art. 1° includes Municipalidades. Art. 4° = PSE scope. Art. 8° = OIV duties (lit. a-i). Art. 9° = 3h/72h/15d reporting. Art. 27° = significance threshold. Art. 40° = UTM fines.
- **Ley 21.459** (DO 20/06/2022): Art. 2° = safe harbor for ANCI-registered researchers.
- ANCI IG N°1 (jun 2025): platform registration mandatory.
- ANCI IG N°3, N°4 (dic 2025): delegate + containment measures.

---

## Compliance Controls

### Objective (scanned automatically)
| Control | Severity | Tier | Legal Anchor |
|---------|----------|------|-------------|
| Anonymous SMB/NFS/WebDAV shares | Critical | All | Art. 8° lit. e); IG N°4 |
| Admin shares exposed (C$, ADMIN$) | Critical | All | Art. 8° lit. e); IG N°4 |
| Firewall inactive | Critical | All | Art. 8° lit. e); IG N°4 |
| Cleartext protocols (Telnet/FTP) | Critical | All | Art. 8° lit. e); IG N°4 |
| TLS 1.0/1.1/SSLv3 active | Critical | OIV+PSE | Art. 8° lit. a); NIST SP 800-52 |
| OS EOL | Critical | All | Art. 8° lit. a) y d) |
| Expired/self-signed cert | High | OIV+PSE | Art. 8° lit. a) |
| Software EOL | High | All | Art. 8° lit. a) y d) |
| Drive unencrypted | High | OIV only | Art. 8° lit. a); ISO 27001 A.10.1 |
| Cloud sync active | High | OIV only | Art. 8° lit. a) |
| No backup agent | High | OIV+PSE | Art. 8° lit. b) |

### Declarative (questionnaire — runs by default)
| QuestionId | Severity | Tier | Legal Anchor |
|-----------|----------|------|-------------|
| InscritoAnci | Critical | OIV+PSE | IG N°1 ANCI (jun 2025) |
| DelegadoCiberseguridad | High | OIV | Art. 8° lit. i); IG N°3 |
| SgsiImplementado | Critical | OIV | Art. 8° lit. a) |
| RegistroAcciones | High | OIV | Art. 8° lit. b) |
| PlanContinuidad | High | OIV | Art. 8° lit. c) |
| PlanCertificado | High | OIV | Art. 8° lit. c) + Art. 28° |
| CapacitacionContinua | Medium | OIV | Art. 8° lit. h) |

---

## UTM Fine Scale (Art. 40° Ley 21.663)

| Infraction | OIV | PSE |
|-----------|-----|-----|
| Leve (Medium) | 10,000 UTM | 5,000 UTM |
| Grave (High) | 20,000 UTM | 10,000 UTM |
| Gravísima (Critical) | 40,000 UTM | 20,000 UTM |

1 UTM ≈ CLP $66,000 — verify current value at SII (www.sii.cl).

---

## Run Commands

```powershell
# CLI
cargo run -p muniani-cli

# GUI dev
cd gui
cargo tauri dev

# Tests
cargo test -p muniani-core

# Per-client GUI release build
$env:MUNIANI_INSTITUTION = "Municipalidad de X"
$env:MUNIANI_TIER = "pse"
cargo tauri build
```

---

## Important Behavioral Notes

- Tool is a **screening tool** for internal self-assessment, not legal certification
- ANCI decides compliance, not the tool
- `backup_agent_running` is `Option<bool>` — `None` AND `Some(false)` both fire the gap (conservative by design)
- `encrypted: null` on drives = BitLocker WMI requires admin rights, `None` is correct behavior
- BitLocker gap suppressed for PSE (OIV-only control) — correct per law
- PDF always includes Ley 21.459 Art. 2° safe harbor disclaimer
- Classify all output as RESERVADO
- Must be ANCI-registered before running on any Estado network
- Affiliation: "Felipe Carvajal Brown" (a person, not a company — there is no
  "Felipe Carvajal Brown Software"; that string was invented and was corrected
  repo-wide in 0.5.0)
- Security reports affiliation: "Magíster en Simulaciones Numéricas, UPM"
- Municipalities commonly have McAfee/enterprise AV — code signing cert required before delivery
- WebView2 Runtime required on Windows 10 for GUI (bundled in Win11)
- UTM monetary values shown in GUI are approximate — always show "verificar en SII" disclaimer

---

## Developer Preferences (always follow)

- File delivery: one at a time, wait for feedback
- Fixes: diffs/snippets only — never full files unless explicitly asked
- Comments: 1-line only, no block comments
- Bug fixes: always at root cause, never patch tests
- Never write code just to get it to compile — must reflect real behavior
- This tool ships to real government institutions — correctness is non-negotiable
