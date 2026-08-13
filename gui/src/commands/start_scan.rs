// Runs a full MuniGPT scan and streams progress + technical log lines via a Tauri Channel.
use munigpt_core::{
    questionnaire::QuestionnaireResponse,
    scan,
    types::{ScanConfig, ScanResult, Scope, Tier},
};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};
use tauri::{ipc::Channel, AppHandle};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub pct: u8,
    pub log: String,
}

// Simple label for the worker tab progress bar — not shown in IT terminal.
fn log_for_pct(pct: u8) -> &'static str {
    match pct {
        0..=5   => "Iniciando escaneo...",
        6..=20  => "Descubriendo hosts en la red...",
        21..=64 => "Sondeando servicios y unidades...",
        65..=74 => "Relevando software y SO...",
        75..=89 => "Evaluando brechas de cumplimiento...",
        _       => "Generando informe...",
    }
}

#[derive(Debug, Serialize, thiserror::Error)]
pub enum ScanError {
    #[error("Error durante el escaneo: {0}")]
    Core(String),
}

#[tauri::command]
pub async fn start_scan(
    _app: AppHandle,
    on_progress: Channel<ScanProgress>,
) -> Result<ScanResult, ScanError> {
    // Shared atomic so log_cb can read the last known pct without resetting the bar.
    let last_pct = Arc::new(AtomicU8::new(0));

    // progress_cb — updates the bar and sends a simple label (worker tab uses this).
    let pct_tracker = last_pct.clone();
    let channel     = on_progress.clone();
    let progress_cb = move |pct: u8| {
        pct_tracker.store(pct, Ordering::Relaxed);
        let _ = channel.send(ScanProgress {
            pct,
            log: log_for_pct(pct).to_string(),
        });
    };

    // log_cb — sends a technical line at the current pct (IT terminal uses this).
    let pct_tracker2 = last_pct.clone();
    let log_channel  = on_progress.clone();
    let log_cb = move |msg: &str| {
        let pct = pct_tracker2.load(Ordering::Relaxed);
        let _ = log_channel.send(ScanProgress {
            pct,
            log: msg.to_string(),
        });
    };

    let (config_ti, _) = munigpt_core::config::Config::load();

    let questionnaire = QuestionnaireResponse::desde_config(&config_ti.cuestionario);

    let config = ScanConfig {
        institution_name: super::branding::institution(),
        tier:        tier_resuelto(),
        scope:       Scope::Local,
        red:         config_ti.red,
        progress_cb: Some(Box::new(progress_cb)),
        log_cb:      Some(Box::new(log_cb)),
    };

    let mut result = tokio::task::spawn_blocking(move || scan(config, questionnaire))
        .await
        .map_err(|e| ScanError::Core(e.to_string()))?
        .map_err(|e| ScanError::Core(e.to_string()))?;

    if config_ti.historico.habilitado {
        registrar(
            &mut result,
            &config_ti.historico,
            &on_progress,
            last_pct.load(Ordering::Relaxed),
        );
    }

    let _ = on_progress.send(ScanProgress {
        pct: 100,
        log: "Escaneo completado.".into(),
    });

    Ok(result)
}

fn registrar(
    result: &mut ScanResult,
    config: &munigpt_core::config::HistoricoConfig,
    canal: &Channel<ScanProgress>,
    pct: u8,
) {
    use munigpt_core::historico;

    let linea = |texto: String| {
        let _ = canal.send(ScanProgress { pct, log: texto });
    };

    let ruta = historico::ruta_junto_al_ejecutable(&result.meta.institution_name);
    match historico::registrar_y_comparar(&ruta, result, config) {
        Ok(reg) => {
            if reg.purgadas > 0 {
                linea(format!(
                    "Histórico: {} medición(es) purgadas por retención.",
                    reg.purgadas
                ));
            }
            linea(format!(
                "Histórico: {} medición(es) registradas para esta institución.",
                reg.mediciones
            ));
            result.delta = reg.delta;
            result.deriva = reg.deriva;
        }
        Err(e) => {
            let detalle = format!("{e:#}");
            linea(format!("[!] No se pudo actualizar el histórico: {detalle}"));
            result.historico_error = Some(detalle);
        }
    }
}

fn tier_resuelto() -> Tier {
    match super::branding::tier().as_str() {
        "oiv"          => Tier::Oiv,
        "unclassified" => Tier::Unclassified,
        _              => Tier::Pse,
    }
}