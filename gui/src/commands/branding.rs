// Per-client branding compiled into the binary at build time.
//
// One value drives the whole product: `MUNIANI_INSTITUTION` / `MUNIANI_TIER` are
// baked in via `option_env!` when building for a given client. The scanner
// (start_scan.rs) stamps the report with it, the Rust host passes it to the
// Asistente sidecar as `MUNIGPT_MUNICIPIO` (see assistant.rs), and the frontend
// reads it through the `app_branding` command below to show it in the header —
// so header, report, and Asistente all name the same institution.
use serde::Serialize;

/// Default institution when no `MUNIANI_INSTITUTION` was compiled in. Matches the
/// scanner default in start_scan.rs so an un-branded dev build is consistent.
pub const DEFAULT_INSTITUTION: &str = "Municipalidad de Prueba";
/// Default tier when no `MUNIANI_TIER` was compiled in.
pub const DEFAULT_TIER: &str = "pse";

/// The compiled institution name, falling back to the dev default.
pub fn institution() -> String {
    option_env!("MUNIANI_INSTITUTION")
        .unwrap_or(DEFAULT_INSTITUTION)
        .to_string()
}

/// The raw compiled tier string (`oiv` / `pse` / `unclassified`), falling back to
/// the dev default.
pub fn tier() -> &'static str {
    option_env!("MUNIANI_TIER").unwrap_or(DEFAULT_TIER)
}

/// The institution ONLY when it was actually compiled in (`None` otherwise). Used
/// by the Asistente sidecar to decide whether to override the backend's municipio:
/// on a branded build we force it to match the scanner; on an un-branded build we
/// leave the backend's own config.json (e.g. the Providencia demo) untouched.
pub fn institution_override() -> Option<&'static str> {
    option_env!("MUNIANI_INSTITUTION")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branding {
    pub institution: String,
    pub tier: String,
}

/// Tauri command: the per-client branding compiled into this binary, for the header.
#[tauri::command]
pub fn app_branding() -> Branding {
    Branding {
        institution: institution(),
        tier: tier().to_string(),
    }
}
