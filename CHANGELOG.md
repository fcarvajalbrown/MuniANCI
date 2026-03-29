# Changelog

All notable changes to MuniANCI will be documented here.
Format: [Semantic Versioning](https://semver.org).

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

---

## [Unreleased — v0.2.0]

### Planned
- `muniani-gui` Tauri 2 shell with React/Vite gap dashboard
- Full WMI COM implementation for Windows (`wmi_scalar_u32`, `wmi_string_list`)
- TLS certificate chain validation (`TlsCertIssue` full detection)
- CVE enrichment against NVD API (CVSS ≥ 9.0 flagging)
- EOL database for software versions (PHP, Apache, OpenSSL, etc.)
- `--export-evidence` flag — hashed screenshots of findings
- Inno Setup installer for Windows distribution
- `backup_agent_running` field in `OsInfo`