// Runs a full MuniANCI scan and streams progress + log lines back via a Tauri Channel.
use muniani_core::{
    questionnaire::QuestionnaireResponse,
    scan,
    types::{ScanConfig, ScanResult, Scope, Tier},
};
use serde::Serialize;
use tauri::{ipc::Channel, AppHandle};

/// Payload sent on every progress tick through the Channel.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub pct: u8,
    pub log: String,
}

/// Human-readable Spanish log lines matched to progress percentages.
fn log_for_pct(pct: u8) -> &'static str {
    match pct {
        0..=5   => "Iniciando escaneo...",
        6..=20  => "Descubriendo hosts en la red...",
        21..=40 => "Enumerando unidades y recursos compartidos...",
        41..=55 => "Analizando servicios y certificados TLS...",
        56..=65 => "Relevando software instalado...",
        66..=74 => "Verificando sistema operativo y firewall...",
        75..=80 => "Enriqueciendo datos de fin de vida (EOL)...",
        81..=89 => "Evaluando brechas de cumplimiento...",
        _       => "Generando informe...",
    }
}

/// Serialisable error type so Tauri can forward it to the frontend.
#[derive(Debug, Serialize, thiserror::Error)]
pub enum ScanError {
    #[error("Error durante el escaneo: {0}")]
    Core(String),
}

/// Tauri command — invoke from the frontend with a Channel to receive progress events.
/// Returns the full ScanResult (gaps included) when the scan completes.
#[tauri::command]
pub async fn start_scan(
    _app: AppHandle,
    on_progress: Channel<ScanProgress>,
) -> Result<ScanResult, ScanError> {
    // Progress callback bridges core u8 ticks into Channel events.
    let channel = on_progress.clone();
    let progress_cb = move |pct: u8| {
        let _ = channel.send(ScanProgress {
            pct,
            log: log_for_pct(pct).to_string(),
        });
    };

    // Config is baked in per-client delivery — not user-configurable in the GUI.
    let config = ScanConfig {
        institution_name: env!("MUNIANI_INSTITUTION").to_string(),
        tier:             tier_from_env(),
        scope:            Scope::Local,
        progress_cb:      Some(Box::new(progress_cb)),
    };

    let questionnaire = QuestionnaireResponse::default();

    // Run blocking core scan on a dedicated thread — keeps the async runtime free.
    let result = tokio::task::spawn_blocking(move || scan(config, questionnaire))
        .await
        .map_err(|e| ScanError::Core(e.to_string()))?
        .map_err(|e| ScanError::Core(e.to_string()))?;

    // Final tick so the frontend progress bar reaches 100%.
    let _ = on_progress.send(ScanProgress { pct: 100, log: "Escaneo completado.".into() });

    Ok(result)
}

/// Reads MUNIANI_TIER at compile time — defaults to PSE if unset.
fn tier_from_env() -> Tier {
    match option_env!("MUNIANI_TIER").unwrap_or("pse") {
        "oiv" => Tier::Oiv,
        "unclassified" => Tier::Unclassified,
        _ => Tier::Pse,
    }
}