# MuniANCI Development Session Memory
## Felipe Carvajal Brown Software — March 2026

---

## Project Overview

**MuniANCI** is a cybersecurity compliance scanner for Chilean municipalities under **Ley 21.663 (Marco de Ciberseguridad)**. Combines active network scanning with a declarative questionnaire to produce a PDF gap report and CSIRT JSON.

**Stack:** Rust workspace (`core` lib + `cli` binary), `printpdf 0.9.1`, `native-tls 0.2`, `rayon`, `serde`, `windows` crate, `nix` crate.

**Repo:** `C:\Users\Beetlejuice\Desktop\MuniANCI\`

---

## Workspace Layout

```
MuniANCI/
├── Cargo.toml                        # workspace members = ["core", "cli"]
├── core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs
│       ├── questionnaire.rs
│       ├── normalizer.rs
│       ├── compliance_engine.rs
│       ├── report_builder.rs
│       ├── os_abstraction/
│       │   ├── mod.rs
│       │   ├── windows.rs
│       │   └── linux.rs
│       └── probes/
│           ├── mod.rs
│           ├── host_discovery.rs
│           ├── drive_enum.rs
│           ├── service_probe.rs
│           ├── sw_inventory.rs
│           └── os_check.rs
└── cli/
    ├── Cargo.toml
    └── src/main.rs
```

---

## Key Dependencies (core/Cargo.toml)

```toml
printpdf = "0.9.1"
native-tls = "0.2"
lopdf = "0.34"          # in Cargo.toml but printpdf is the active PDF lib
windows = "0.62.1"      # features: Win32_NetworkManagement_WNet, Win32_Storage_FileSystem,
                        # Win32_System_Wmi, Win32_Foundation, Win32_System_Registry,
                        # Win32_System_SystemInformation, Win32_System_Com,
                        # Win32_System_Ole, Win32_System_Variant
nix = "0.31.1"          # features: mount, net
rayon = "1"
```

---

## Architecture Decisions

- `ScanConfig` (has closure, not serialisable) + `ScanMeta` (serialisable, stored in ScanResult)
- `ScanResult.meta: ScanMeta` not `config: ScanConfig`
- `backup_agent_running: Option<bool>` in `OsInfo` — None=WMI failed, Some(false)=no agent, Some(true)=confirmed
- Rayon parallelism via nested `rayon::join()` in `lib.rs`
- Two gap sources: objective (scanner probes) + declarative (questionnaire) → both feed `compliance_engine::evaluate()`
- Art. 27° significance filter: `requires_csirt_report` only fires for OIV/PSE + Critical + network-reachable controls
- LAN sweep parallelized with rayon, 150ms TCP timeout per host
- Questionnaire runs by default — `--no-questionnaire` flag exists but is NOT the default (tool ships to municipalities)

---

## printpdf 0.9.1 API (CRITICAL — always use these)

- `doc.add_font(BuiltinFont::Helvetica)` — NOT `add_builtin_font`
- Ops: `Op::SetFontAndFontSize` + `Op::WriteTextBuiltinFont` + `Op::BeginTextSection` + `Op::EndTextSection` + `Op::SetTextCursor`
- `Point` fields need `Mm(...).into()` to convert to `Pt`
- `save()` takes `(&PdfSaveOptions::default(), &mut Vec::<PdfWarnMsg>::new())`
- `PdfPage::new(Mm(W), Mm(H), ops)` then `doc.with_pages(vec![page]).save(...)`
- Only `report_builder.rs` uses printpdf
- PDF strings must be sanitized via `to_pdf_safe()` — builtin Type1 fonts use WinAnsiEncoding, UTF-8 multi-byte chars corrupt

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

## v0.2 Status

### ✅ Completed
- WMI COM implementation (`wmi_query`, `wmi_scalar_u32`, `wmi_string_list`) — real Win32 COM calls
- Real firewall detection via registry — no elevation needed
- `backup_agent_running: Option<bool>` wired through: `os_check.rs` → `OsInfo` → `compliance_engine::check_backup_agent()`
- `check_backup_agent` uses `matches!(o.backup_agent_running, Some(true))` — None and Some(false) both fire gap (conservative = legally correct)
- LAN sweep parallelized with rayon
- TLS cert chain validation via `native-tls` — `check_tls()` in `service_probe.rs` does two connections: strict first (captures error), permissive second (confirms TLS). Classifies `Expired`, `SelfSigned`, or conservative `SelfSigned` from error message.
- PDF encoding fix — `to_pdf_safe()` function sanitizes UTF-8 to WinAnsi before PDF output
- BitLocker suppressed for PSE tier (OIV-only control) in compliance_engine

### 🔄 In Progress
- TLS cert validation — `native-tls` integrated and compiling. Docker test container running on port 8443 with self-signed cert. TLS connection confirmed via curl. Waiting on scan results to verify `tls_cert_issue` populates correctly.

### ⏳ Pending
- CVE/EOL enrichment via NVD API (e.g. Office 2016 `is_eol: false` is wrong)
- Tauri 2 GUI (React/Vite gap dashboard) — architecture designed, not built
- Inno Setup — portable `.exe` output (NOT an installer wizard)
- Code signing cert (DigiCert/Sectigo ~$200-400/yr) — add to CHANGELOG/README when v0.2 done

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
| Leve | 10,000 UTM | 5,000 UTM |
| Grave | 20,000 UTM | 10,000 UTM |
| Gravísima | 40,000 UTM | 20,000 UTM |

1 UTM ≈ CLP $66,000 — verify current value at SII.

---

## Run Command

```powershell
cargo run -p muniani-cli
```

---

## Important Behavioral Notes

- Tool is a **screening tool** for internal self-assessment, not legal certification
- ANCI decides compliance, not the tool
- `backup_agent_running` is `Option<bool>` — `None` AND `Some(false)` both fire the gap (conservative by design — legally correct)
- `encrypted: null` on drives = BitLocker WMI requires admin rights, `None` is correct behavior
- BitLocker gap suppressed for PSE (OIV-only control) — correct per law
- PDF always includes Ley 21.459 Art. 2° safe harbor disclaimer
- Classify all output as RESERVADO
- Must be ANCI-registered before running on any Estado network
- Affiliation: "Felipe Carvajal Brown Software"
- Security reports affiliation: "Magíster en Simulaciones Numéricas, UPM"
- Municipalities commonly have McAfee/enterprise AV — code signing cert required before delivery

---

## Developer Preferences (always follow)

- File delivery: one at a time, wait for feedback
- Fixes: diffs/snippets only — never full files unless explicitly asked
- Comments: 1-line only, no block comments
- Bug fixes: always at root cause, never patch tests
- Never write code just to get it to compile — must reflect real behavior
- This tool ships to real government institutions — correctness is non-negotiable
