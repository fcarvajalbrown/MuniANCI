//! muniani-core public API — call scan() and get a ScanResult back.
pub mod compliance_engine;
pub mod config;
pub mod cve;
pub mod eol_enrichment;
pub mod evidencia;
pub mod historico;
pub mod maturity;
pub mod normalizer;
pub mod os_abstraction;
pub mod patch_level;
pub mod poam;
pub mod probes;
pub mod questionnaire;
pub mod report_builder;
pub mod scoring;
pub mod taxonomia;
pub mod types;

use anyhow::Result;
use chrono::Utc;
use types::{FindingPayload, ScanConfig, ScanResult};

pub fn scan(config: ScanConfig, questionnaire: questionnaire::QuestionnaireResponse) -> Result<ScanResult> {
    config.report_progress(5);
    config.log("Iniciando escaneo — descubriendo hosts en la red...");

    let host_findings = probes::host_discovery::run_con(config.scope, &config.red.ajustes())?;
    let host_ips: Vec<std::net::IpAddr> = host_findings
        .iter()
        .filter_map(|f| {
            if let FindingPayload::Host(h) = &f.payload { Some(h.ip) } else { None }
        })
        .collect();
    config.log(&format!("{} host(s) descubiertos — iniciando sondeo paralelo (rayon)...", host_ips.len()));
    config.report_progress(20);

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

    let drives = drives?;
    config.log(&format!("{} unidad(es) enumerada(s) — discos fijos, shares SMB/NFS, cloud-sync.", drives.iter().filter(|f| matches!(f.payload, FindingPayload::Drive(_))).count()));
    all_findings.extend(drives);

    let services = services?;
    config.log(&format!("{} servicio(s) detectado(s) — TLS, banners, acceso anónimo verificados.", services.len()));
    all_findings.extend(services);

    let sw = sw?;
    config.log(&format!("{} paquete(s) de software relevado(s) via WMI/registro.", sw.len()));
    all_findings.extend(sw);

    let os = os?;
    config.log("SO verificado — versión, EOL, firewall (registro), agente de backup (WMI).");
    all_findings.extend(os);

    let mut asset_graph = normalizer::normalize(all_findings);
    config.log("Grafo de activos normalizado — iniciando enriquecimiento EOL...");
    config.report_progress(75);

    eol_enrichment::enrich(&mut asset_graph);
    let eol_count = asset_graph.software.iter().filter(|s| s.is_eol).count();
    config.log(&format!("EOL: {} paquete(s) en fin de vida identificado(s) (base endoflife.date mar 2026).", eol_count));

    config.log("Enriqueciendo con CVE offline (snapshot NVD embebido)...");
    let kev = cve::kev::catalogue();
    config.log(&kev.provenance());
    let cve_coverage = cve::enrichment::enrich(&mut asset_graph, &|id| kev.contains(id));
    let cve_total: usize = asset_graph.software.iter().map(|s| s.cves.len()).sum::<usize>()
        + asset_graph.os_info.iter().map(|o| o.cves.len()).sum::<usize>();
    let exploited: usize = asset_graph.software.iter().flat_map(|s| &s.cves)
        .chain(asset_graph.os_info.iter().flat_map(|o| &o.cves))
        .filter(|c| c.known_exploited)
        .count();
    config.log(&format!(
        "CVE: {cve_total} hallazgo(s), {exploited} con explotación observada (KEV). Cobertura: {cve_coverage}."
    ));
    config.report_progress(80);

    config.log("Evaluando brechas de cumplimiento contra controles Ley 21.663...");
    let gaps = compliance_engine::evaluate(&asset_graph, &questionnaire, config.tier);
    let exigibles = gaps.iter().filter(|g| g.exigibilidad == types::Exigibilidad::Exigible);
    config.log(&format!("{} brecha(s) detectada(s) — {} crítica(s), {} alta(s), {} media(s).",
        gaps.len(),
        gaps.iter().filter(|g| g.severity == types::Severity::Critical).count(),
        gaps.iter().filter(|g| g.severity == types::Severity::High).count(),
        gaps.iter().filter(|g| g.severity == types::Severity::Medium).count(),
    ));
    config.log(&format!("De ellas, {} exigible(s) a un {} y el resto se informan como madurez voluntaria.",
        exigibles.count(), config.tier,
    ));
    config.report_progress(90);

    let score = scoring::ComplianceScore::from_gaps(&gaps);
    config.log(&format!(
        "Puntaje de cumplimiento: {}/{} ({} brecha(s) exigible(s), {} de madurez voluntaria).",
        score.score, score.base, score.exigibles(), score.madurez,
    ));

    let measured = compliance_engine::measured_domains(&questionnaire);
    let maturity = maturity::MaturityProfile::from_gaps(&gaps, &measured);
    for d in &maturity.domains {
        config.log(&format!("  {} — {}: {}", d.domain, d.level, d.rationale));
    }
    match maturity.average() {
        Some(avg) => config.log(&format!(
            "Madurez promedio: {avg:.1}/3 sobre {} dominio(s) medido(s).",
            maturity.domains.len() - maturity.unmeasured().len()
        )),
        None => config.log("Madurez: ningún dominio pudo medirse."),
    }

    let result = ScanResult {
        meta:        config.meta(),
        asset_graph,
        gaps,
        cve_coverage,
        kev_provenance: kev.provenance(),
        taxonomia_anci: taxonomia::TaxonomiaAnci::default(),
        score,
        maturity,
        // El histórico lo lleva el llamador: `scan()` no sabe de mediciones previas.
        delta: None,
        deriva: None,
        scanned_at:  Utc::now(),
    };

    Ok(result)
}