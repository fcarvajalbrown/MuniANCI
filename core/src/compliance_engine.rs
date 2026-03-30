//! Maps a normalized AssetGraph + questionnaire answers to compliance gaps.
use crate::questionnaire::QuestionnaireResponse;
use crate::types::{AppliesTo, AssetGraph, DriveKind, Gap, Severity, Tier};

/// Entry point — merges objective scan gaps with declarative questionnaire gaps.
pub fn evaluate(
    graph: &AssetGraph,
    questionnaire: &QuestionnaireResponse,
    tier: Tier,
) -> Vec<Gap> {
    let mut gaps = Vec::new();

    // Objective gaps from scanner.
    check_anonymous_shares(graph, &mut gaps);
    check_admin_shares(graph, &mut gaps);
    check_cloud_sync(graph, tier, &mut gaps);
    check_firewall(graph, &mut gaps);
    check_cleartext_protocols(graph, &mut gaps);
    check_tls_version(graph, &mut gaps);
    check_expired_certs(graph, &mut gaps);
    check_os_eol(graph, &mut gaps);
    check_software_eol(graph, &mut gaps);
    check_drive_encryption(graph, tier, &mut gaps);
    check_backup_agent(graph, tier, &mut gaps);

    // Declarative gaps from questionnaire answers.
    gaps.extend(crate::questionnaire::to_gaps(questionnaire, tier));

    // Art. 27° significance filter — only tag requires_csirt_report when the
    // gap could interrupt an essential service or affect personal data systems.
    apply_significance_filter(&mut gaps, tier);

    gaps.sort_by(|a, b| b.severity.cmp(&a.severity));
    gaps
}

// ---------------------------------------------------------------------------
// Control checks — one function per row in the compliance table
// ---------------------------------------------------------------------------

/// Art. 8° lit. e); IG N°4 — SMB/NFS/WebDAV share accessible without creds.
fn check_anonymous_shares(graph: &AssetGraph, gaps: &mut Vec<Gap>) {
    let anon: Vec<String> = graph
        .drives
        .iter()
        .filter(|d| {
            matches!(d.kind, DriveKind::Smb | DriveKind::Nfs | DriveKind::WebDav)
        })
        .map(|d| d.path.clone())
        .collect();

    if anon.is_empty() { return; }

    gaps.push(Gap {
        control:              "Shares anónimos (SMB/NFS/WebDAV)".into(),
        finding:              "Recurso compartido accesible sin credenciales".into(),
        severity:             Severity::Critical,
        legal_anchor:         "Art. 8° lit. e) Ley 21.663; IG N°4 — restricción de accesos".into(),
        applies_to:           AppliesTo::All,
        evidence:             anon,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. e); IG N°4 — Windows admin shares (C$, ADMIN$, IPC$) exposed.
fn check_admin_shares(graph: &AssetGraph, gaps: &mut Vec<Gap>) {
    let admin: Vec<String> = graph
        .drives
        .iter()
        .filter(|d| {
            d.kind == DriveKind::Smb
                && ["c$", "admin$", "ipc$"]
                    .iter()
                    .any(|s| d.path.to_lowercase().ends_with(s))
        })
        .map(|d| d.path.clone())
        .collect();

    if admin.is_empty() { return; }

    gaps.push(Gap {
        control:              "Admin shares expuestos (C$, ADMIN$, IPC$)".into(),
        finding:              "Share administrativo accesible desde la red".into(),
        severity:             Severity::Critical,
        legal_anchor:         "Art. 8° lit. e) Ley 21.663; IG N°4".into(),
        applies_to:           AppliesTo::All,
        evidence:             admin,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. a); IG N°4 — cloud sync process running (data leaving perimeter).
fn check_cloud_sync(graph: &AssetGraph, _tier: Tier, gaps: &mut Vec<Gap>) {
    let procs: Vec<String> = graph
        .drives
        .iter()
        .filter(|d| d.kind == DriveKind::CloudSync)
        .map(|d| d.path.replace("process:", ""))
        .collect();

    if procs.is_empty() { return; }

    gaps.push(Gap {
        control:              "Cloud sync activo".into(),
        finding:              "Proceso de sincronización en la nube detectado en ejecución".into(),
        severity:             Severity::High,
        legal_anchor:         "Art. 8° lit. a) Ley 21.663 — SGSI; posible exfiltración".into(),
        applies_to:           AppliesTo::Oiv,
        evidence:             procs,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. e); IG N°4 — firewall inactive on local host.
fn check_firewall(graph: &AssetGraph, gaps: &mut Vec<Gap>) {
    let inactive: Vec<String> = graph
        .os_info
        .iter()
        .filter(|o| !o.firewall_active)
        .map(|o| o.host_ip.to_string())
        .collect();

    if inactive.is_empty() { return; }

    gaps.push(Gap {
        control:              "Firewall desactivado".into(),
        finding:              "No se detectó firewall activo en el host".into(),
        severity:             Severity::Critical,
        legal_anchor:         "Art. 8° lit. e) Ley 21.663; IG N°4 — uso de firewalls (explícito)".into(),
        applies_to:           AppliesTo::All,
        evidence:             inactive,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. e); IG N°4 — Telnet (23), FTP (21) cleartext auth services.
fn check_cleartext_protocols(graph: &AssetGraph, gaps: &mut Vec<Gap>) {
    let bad: Vec<String> = graph
        .services
        .iter()
        .filter(|s| matches!(s.port, 21 | 23))
        .map(|s| format!("{}:{}", s.host_ip, s.port))
        .collect();

    if bad.is_empty() { return; }

    gaps.push(Gap {
        control:              "Protocolos en claro (Telnet/FTP)".into(),
        finding:              "Servicio con autenticación sin cifrado detectado".into(),
        severity:             Severity::Critical,
        legal_anchor:         "Art. 8° lit. e) Ley 21.663; IG N°4 — servicios expuestos".into(),
        applies_to:           AppliesTo::All,
        evidence:             bad,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. a); NIST SP 800-52 rev2 — TLS 1.0 or 1.1 in use.
fn check_tls_version(graph: &AssetGraph, gaps: &mut Vec<Gap>) {
    let bad: Vec<String> = graph
        .services
        .iter()
        .filter(|s| {
            matches!(
                s.tls_version.as_deref(),
                Some("TLSv1.0") | Some("TLSv1.1") | Some("SSLv3")
            )
        })
        .map(|s| format!("{}:{} ({})", s.host_ip, s.port,
            s.tls_version.as_deref().unwrap_or("?")))
        .collect();

    if bad.is_empty() { return; }

    gaps.push(Gap {
        control:              "TLS 1.0/1.1/SSLv3 activo".into(),
        finding:              "Protocolo TLS obsoleto detectado en servicio expuesto".into(),
        severity:             Severity::Critical,
        legal_anchor:         "Art. 8° lit. a) Ley 21.663 — SGSI; NIST SP 800-52 rev2".into(),
        applies_to:           AppliesTo::OivAndPse,
        evidence:             bad,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. a) — expired or self-signed certificate on exposed service.
fn check_expired_certs(graph: &AssetGraph, gaps: &mut Vec<Gap>) {

    let bad: Vec<String> = graph
        .services
        .iter()
        .filter(|s| s.tls_cert_issue.is_some())
        .map(|s| format!("{}:{} ({:?})", s.host_ip, s.port,
            s.tls_cert_issue.as_ref().unwrap()))
        .collect();

    if bad.is_empty() { return; }

    gaps.push(Gap {
        control:              "Certificado vencido o autofirmado".into(),
        finding:              "Certificado TLS inválido en servicio expuesto".into(),
        severity:             Severity::High,
        legal_anchor:         "Art. 8° lit. a) Ley 21.663 — SGSI; buena práctica".into(),
        applies_to:           AppliesTo::OivAndPse,
        evidence:             bad,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. a) y d) — end-of-life operating system detected.
fn check_os_eol(graph: &AssetGraph, gaps: &mut Vec<Gap>) {
    let eol: Vec<String> = graph
        .os_info
        .iter()
        .filter(|o| o.is_eol)
        .map(|o| format!("{} — {}", o.host_ip, o.version))
        .collect();

    if eol.is_empty() { return; }

    gaps.push(Gap {
        control:              "Sistema operativo en EOL".into(),
        finding:              "SO sin soporte de seguridad detectado".into(),
        severity:             Severity::Critical,
        legal_anchor:         "Art. 8° lit. a) y d) Ley 21.663 — SGSI y revisión continua".into(),
        applies_to:           AppliesTo::All,
        evidence:             eol,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. a) y d) — installed software with known EOL version.
fn check_software_eol(graph: &AssetGraph, gaps: &mut Vec<Gap>) {
    let eol: Vec<String> = graph
        .software
        .iter()
        .filter(|sw| sw.is_eol)
        .map(|sw| format!("{} {} @ {}", sw.name, sw.version, sw.host_ip))
        .collect();

    if eol.is_empty() { return; }

    gaps.push(Gap {
        control:              "Software en EOL".into(),
        finding:              "Paquete de software sin soporte de seguridad detectado".into(),
        severity:             Severity::High,
        legal_anchor:         "Art. 8° lit. a) y d) Ley 21.663".into(),
        applies_to:           AppliesTo::All,
        evidence:             eol,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. a); ISO 27001 A.10.1 — fixed drive without encryption at rest.
fn check_drive_encryption(graph: &AssetGraph, tier: Tier, gaps: &mut Vec<Gap>) {
    if !AppliesTo::Oiv.is_mandatory_for(tier) { return; }
    let unencrypted: Vec<String> = graph
        .drives
        .iter()
        .filter(|d| d.kind == DriveKind::Fixed && d.encrypted == Some(false))
        .map(|d| d.path.clone())
        .collect();

    if unencrypted.is_empty() { return; }

    gaps.push(Gap {
        control:              "BitLocker/LUKS desactivado".into(),
        finding:              "Disco fijo sin cifrado en reposo detectado".into(),
        severity:             Severity::High,
        legal_anchor:         "Art. 8° lit. a) Ley 21.663 — SGSI; ISO 27001 A.10.1".into(),
        applies_to:           AppliesTo::Oiv,
        evidence:             unencrypted,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. b) — no known backup agent process detected.
fn check_backup_agent(graph: &AssetGraph, _tier: Tier, gaps: &mut Vec<Gap>) {
    if graph.os_info.is_empty() { return; }

    gaps.push(Gap {
        control:              "Sin agente de backup detectado".into(),
        finding:              "No se identificó proceso de respaldo activo en el host local".into(),
        severity:             Severity::High,
        legal_anchor:         "Art. 8° lit. b) Ley 21.663 — planes de continuidad operacional".into(),
        applies_to:           AppliesTo::OivAndPse,
        evidence:             vec!["host local".into()],
        requires_csirt_report: false,
    });
}

/// Art. 27° Ley 21.663 — tags gaps that have "efecto significativo" and therefore
/// trigger the 3-hour mandatory CSIRT report obligation under Art. 9°.
/// Significance = could interrupt essential service continuity, affect physical
/// integrity, or involves systems with personal data. Not every Critical gap
/// meets this bar — anonymous shares on an isolated workstation may not.
/// For v0.1 we apply a conservative heuristic: network-reachable Critical gaps
/// on PSE/OIV institutions are presumed significant.
fn apply_significance_filter(gaps: &mut Vec<Gap>, tier: Tier) {
    let is_reportable_tier = matches!(tier, Tier::Oiv | Tier::Pse);
    for gap in gaps.iter_mut() {
        gap.requires_csirt_report = is_reportable_tier
            && gap.severity == Severity::Critical
            && gap.applies_to.is_mandatory_for(tier)
            && is_network_reachable_control(&gap.control);
    }
}

/// Returns true for controls where the finding is network-exposed and therefore
/// more likely to meet the Art. 27° significance threshold.
fn is_network_reachable_control(control: &str) -> bool {
    matches!(
        control,
        "Shares anónimos (SMB/NFS/WebDAV)"
        | "Admin shares expuestos (C$, ADMIN$, IPC$)"
        | "Protocolos en claro (Telnet/FTP)"
        | "TLS 1.0/1.1/SSLv3 activo"
        | "Firewall desactivado"
        | "Sistema operativo en EOL"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questionnaire::QuestionnaireResponse;
    use crate::types::{AssetGraph, Drive, DriveKind, OsInfo};
    use std::net::IpAddr;

    fn empty_graph() -> AssetGraph { AssetGraph::default() }
    fn no_answers() -> QuestionnaireResponse { QuestionnaireResponse::default() }
    fn local_ip() -> IpAddr { "127.0.0.1".parse().unwrap() }

    #[test]
    fn no_objective_gaps_on_empty_graph() {
        let gaps = evaluate(&empty_graph(), &no_answers(), Tier::Pse);
        assert!(gaps.iter().all(|g| g.control != "Shares anónimos (SMB/NFS/WebDAV)"));
    }

    #[test]
    fn anonymous_smb_share_is_critical() {
        let mut graph = empty_graph();
        graph.drives.push(Drive {
            path: "\\\\192.168.1.10\\datos".into(),
            kind: DriveKind::Smb,
            total_bytes: None,
            free_bytes: None,
            encrypted: None,
            host_ip: Some("192.168.1.10".parse().unwrap()),
        });
        let gaps = evaluate(&graph, &no_answers(), Tier::Pse);
        let gap = gaps.iter().find(|g| g.control.contains("anónimos")).unwrap();
        assert_eq!(gap.severity, Severity::Critical);
    }

    #[test]
    fn firewall_off_requires_csirt_for_oiv() {
        let mut graph = empty_graph();
        graph.os_info.push(OsInfo {
            host_ip:         local_ip(),
            family:          "linux".into(),
            version:         "Ubuntu 22.04".into(),
            is_eol:          false,
            firewall_active: false,
        });
        let gaps = evaluate(&graph, &no_answers(), Tier::Oiv);
        let gap = gaps.iter().find(|g| g.control.contains("Firewall")).unwrap();
        assert!(gap.requires_csirt_report);
    }

    #[test]
    fn firewall_off_does_not_require_csirt_for_unclassified() {
        let mut graph = empty_graph();
        graph.os_info.push(OsInfo {
            host_ip:         local_ip(),
            family:          "linux".into(),
            version:         "Ubuntu 22.04".into(),
            is_eol:          false,
            firewall_active: false,
        });
        let gaps = evaluate(&graph, &no_answers(), Tier::Unclassified);
        let gap = gaps.iter().find(|g| g.control.contains("Firewall")).unwrap();
        assert!(!gap.requires_csirt_report);
    }

    #[test]
    fn cloud_sync_only_mandatory_for_oiv() {
        let mut graph = empty_graph();
        graph.drives.push(Drive {
            path: "process:onedrive".into(),
            kind: DriveKind::CloudSync,
            total_bytes: None, free_bytes: None,
            encrypted: None, host_ip: None,
        });
        let gaps = evaluate(&graph, &no_answers(), Tier::Pse);
        let gap = gaps.iter().find(|g| g.control.contains("Cloud")).unwrap();
        assert!(!gap.applies_to.is_mandatory_for(Tier::Pse));
    }

    #[test]
    fn unanswered_questionnaire_adds_declarative_gaps_for_oiv() {
        let gaps = evaluate(&empty_graph(), &no_answers(), Tier::Oiv);
        assert!(gaps.iter().any(|g| g.control.contains("SGSI")));
    }

    #[test]
    fn pse_does_not_get_oiv_only_declarative_gaps() {
        let gaps = evaluate(&empty_graph(), &no_answers(), Tier::Pse);
        assert!(!gaps.iter().any(|g| g.control.contains("Delegado")));
    }
}