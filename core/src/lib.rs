//! muniani-core public API — call scan() and get a ScanResult back.
pub mod compliance_engine;
pub mod normalizer;
pub mod os_abstraction;
pub mod probes;
pub mod questionnaire;
pub mod report_builder;
pub mod types;

use anyhow::Result;
use chrono::Utc;
use rayon::prelude::*;
use types::{
    AssetGraph, FindingPayload, RawFinding, ScanConfig, ScanResult, Scope,
};

/// Runs a full scan and returns the complete result.
pub fn scan(config: ScanConfig, questionnaire: questionnaire::QuestionnaireResponse) -> Result<ScanResult> {
    config.report_progress(5);

    // Phase 1 — host discovery (must run first; LAN probes need the host list).
    let host_findings = probes::host_discovery::run(config.scope)?;
    config.report_progress(20);

    // Extract discovered IPs for downstream probes.
    let host_ips: Vec<std::net::IpAddr> = host_findings
        .iter()
        .filter_map(|f| {
            if let FindingPayload::Host(h) = &f.payload {
                Some(h.ip)
            } else {
                None
            }
        })
        .collect();

    // Phase 2 — remaining probes run in parallel via rayon.
    let probe_results: Vec<Result<Vec<RawFinding>>> = vec![
        || probes::drive_enum::run(config.scope, &host_ips),
        || probes::service_probe::run(&host_ips),
        || probes::sw_inventory::run(&host_ips),
        || probes::os_check::run(),
    ]
    .into_par_iter()
    .map(|f| f())
    .collect();

    config.report_progress(65);

    // Collect all raw findings, propagating first error.
    let mut all_findings = host_findings;
    for result in probe_results {
        all_findings.extend(result?);
    }

    // Phase 3 — normalize.
    let asset_graph = normalizer::normalize(all_findings);
    config.report_progress(75);

    // Phase 4 — compliance evaluation (objective + declarative).
    let gaps = compliance_engine::evaluate(&asset_graph, &questionnaire, config.tier);
    config.report_progress(90);

    let result = ScanResult {
        config,
        asset_graph,
        gaps,
        scanned_at: Utc::now(),
    };

    Ok(result)
}