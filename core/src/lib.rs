//! muniani-core public API — call scan() and get a ScanResult back.
pub mod compliance_engine;
pub mod eol_enrichment;
pub mod normalizer;
pub mod os_abstraction;
pub mod probes;
pub mod questionnaire;
pub mod report_builder;
pub mod types;

use anyhow::Result;
use chrono::Utc;
use types::{FindingPayload, ScanConfig, ScanResult};

pub fn scan(config: ScanConfig, questionnaire: questionnaire::QuestionnaireResponse) -> Result<ScanResult> {
    config.report_progress(5);

    let host_findings = probes::host_discovery::run(config.scope)?;
    config.report_progress(20);

    let host_ips: Vec<std::net::IpAddr> = host_findings
        .iter()
        .filter_map(|f| {
            if let FindingPayload::Host(h) = &f.payload { Some(h.ip) } else { None }
        })
        .collect();

    let scope = config.scope;
    let ips = host_ips.clone();
    let ((drives, services), (sw, os)) = rayon::join(
        || rayon::join(
            || probes::drive_enum::run(scope, &ips),
            || probes::service_probe::run(&ips),
        ),
        || rayon::join(
            || probes::sw_inventory::run(&ips),
            || probes::os_check::run(),
        ),
    );

    config.report_progress(65);

    let mut all_findings = host_findings;
    all_findings.extend(drives?);
    all_findings.extend(services?);
    all_findings.extend(sw?);
    all_findings.extend(os?);

    let mut asset_graph = normalizer::normalize(all_findings);
    config.report_progress(75);

    // Patch is_eol on software and OS entries before gap evaluation.
    eol_enrichment::enrich(&mut asset_graph);

    let gaps = compliance_engine::evaluate(&asset_graph, &questionnaire, config.tier);
    config.report_progress(90);

    let result = ScanResult {
        meta:        config.meta(),
        asset_graph,
        gaps,
        scanned_at:  Utc::now(),
    };

    Ok(result)
}