//! Maps a normalized AssetGraph + questionnaire answers to compliance gaps.
use crate::questionnaire::QuestionnaireResponse;
use crate::types::{AppliesTo, AssetGraph, DriveKind, Exigibilidad, Gap, Severity, Tier};

/// Valor con el que los chequeos objetivos rellenan `Gap::exigibilidad`.
///
/// Los chequeos no reciben el tier, así que la exigibilidad real se normaliza en
/// una sola pasada (`apply_exigibilidad`) antes de devolver los resultados. Mismo
/// patrón que ya usaba `requires_csirt_report` con el filtro de significancia.
const EXIGIBILIDAD_PENDIENTE: Exigibilidad = Exigibilidad::Exigible;

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

    // Objective checks don't know the tier — resolve it in one pass. Must run
    // before the questionnaire gaps are appended: those already carry their own
    // exigibilidad, resolved per question.
    apply_exigibilidad(&mut gaps, tier);

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

/// Art. 7° — SMB/NFS/WebDAV share accessible without creds.
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
        legal_anchor:         "Art. 7° Ley 21.663 — medidas permanentes de prevención".into(),
        applies_to:           AppliesTo::All,
        exigibilidad:         EXIGIBILIDAD_PENDIENTE,
        infraction_class:     None,
        evidence:             anon,
        requires_csirt_report: false,
    });
}

/// Art. 7° — Windows admin shares (C$, ADMIN$, IPC$) exposed.
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
        legal_anchor:         "Art. 7° Ley 21.663 — medidas permanentes de prevención".into(),
        applies_to:           AppliesTo::All,
        exigibilidad:         EXIGIBILIDAD_PENDIENTE,
        infraction_class:     None,
        evidence:             admin,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. a) — cloud sync process running (data leaving perimeter).
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
        exigibilidad:         EXIGIBILIDAD_PENDIENTE,
        infraction_class:     None,
        evidence:             procs,
        requires_csirt_report: false,
    });
}

/// Art. 7°; para OIV además IG N°4 art. sexto — firewall inactive on local host.
///
/// La IG N°4 art. sexto es una obligación permanente ("deberán instalar y mantener
/// en operación cortafuegos"), no solo de respuesta a incidentes, pero está dirigida
/// únicamente a los OIV; para el resto el anclaje es el Art. 7°.
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
        legal_anchor:         "Art. 7° Ley 21.663; para OIV además IG N°4 art. sexto — cortafuegos con bloqueo por defecto".into(),
        applies_to:           AppliesTo::All,
        exigibilidad:         EXIGIBILIDAD_PENDIENTE,
        infraction_class:     None,
        evidence:             inactive,
        requires_csirt_report: false,
    });
}

/// Art. 7° — Telnet (23), FTP (21) cleartext auth services.
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
        legal_anchor:         "Art. 7° Ley 21.663; para OIV además IG N°4 art. cuarto lit. c) — cifrado robusto".into(),
        applies_to:           AppliesTo::All,
        exigibilidad:         EXIGIBILIDAD_PENDIENTE,
        infraction_class:     None,
        evidence:             bad,
        requires_csirt_report: false,
    });
}

/// Art. 7°; NIST SP 800-52 rev2 — TLS 1.0 or 1.1 in use.
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
        legal_anchor:         "Art. 7° Ley 21.663; NIST SP 800-52 rev2 (criterio técnico)".into(),
        applies_to:           AppliesTo::OivAndPse,
        exigibilidad:         EXIGIBILIDAD_PENDIENTE,
        infraction_class:     None,
        evidence:             bad,
        requires_csirt_report: false,
    });
}

/// Art. 7° — expired or self-signed certificate on exposed service.
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
        legal_anchor:         "Art. 7° Ley 21.663; buena práctica (criterio técnico)".into(),
        applies_to:           AppliesTo::OivAndPse,
        exigibilidad:         EXIGIBILIDAD_PENDIENTE,
        infraction_class:     None,
        evidence:             bad,
        requires_csirt_report: false,
    });
}

/// Art. 7° — end-of-life operating system detected.
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
        legal_anchor:         "Art. 7° Ley 21.663; para OIV además Art. 8° lit. a) y d)".into(),
        applies_to:           AppliesTo::All,
        exigibilidad:         EXIGIBILIDAD_PENDIENTE,
        infraction_class:     None,
        evidence:             eol,
        requires_csirt_report: false,
    });
}

/// Art. 7° — installed software with known EOL version.
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
        legal_anchor:         "Art. 7° Ley 21.663; para OIV además Art. 8° lit. a) y d)".into(),
        applies_to:           AppliesTo::All,
        exigibilidad:         EXIGIBILIDAD_PENDIENTE,
        infraction_class:     None,
        evidence:             eol,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. a); ISO 27001 A.10.1 — fixed drive without encryption at rest.
///
/// Ya no se descarta para los no-OIV: se informa como madurez voluntaria, que es
/// justamente lo que el modelo dual permite decir sin afirmar un incumplimiento.
fn check_drive_encryption(graph: &AssetGraph, _tier: Tier, gaps: &mut Vec<Gap>) {
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
        exigibilidad:         EXIGIBILIDAD_PENDIENTE,
        infraction_class:     None,
        evidence:             unencrypted,
        requires_csirt_report: false,
    });
}

/// Art. 8° lit. c) — no known backup agent process detected.
fn check_backup_agent(graph: &AssetGraph, _tier: Tier, gaps: &mut Vec<Gap>) {
    let no_backup = graph.os_info.iter().any(|o| !matches!(o.backup_agent_running, Some(true)));
    if !no_backup { return; }

    gaps.push(Gap {
        control:              "Sin agente de backup detectado".into(),
        finding:              "No se identificó proceso de respaldo activo en el host local".into(),
        severity:             Severity::High,
        legal_anchor:         "Art. 8° lit. c) Ley 21.663 — planes de continuidad operacional".into(),
        applies_to:           AppliesTo::OivAndPse,
        exigibilidad:         EXIGIBILIDAD_PENDIENTE,
        infraction_class:     None,
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
            // Una brecha que no es exigible no puede gatillar el deber de
            // reportar del Art. 9°.
            && gap.exigibilidad == Exigibilidad::Exigible
            && gap.severity == Severity::Critical
            && gap.applies_to.is_mandatory_for(tier)
            && is_network_reachable_control(&gap.control);
    }
}

/// Resolves `Gap::exigibilidad` for the objective checks, which are written
/// without knowledge of the scanned tier.
///
/// Es el corazón del modelo dual: un control que no obliga a esta institución se
/// informa igual, pero como madurez voluntaria en vez de como incumplimiento.
fn apply_exigibilidad(gaps: &mut [Gap], tier: Tier) {
    for gap in gaps.iter_mut() {
        gap.exigibilidad = gap.applies_to.exigibilidad_for(tier);
        if gap.exigibilidad == Exigibilidad::MadurezVoluntaria {
            gap.infraction_class = None;
        }
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
    use crate::types::{AssetGraph, Drive, DriveKind, Exigibilidad, OsInfo};
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
            backup_agent_running: None,
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
            backup_agent_running: None,
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
    fn pse_gets_oiv_only_declarative_gaps_as_voluntary_maturity() {
        let gaps = evaluate(&empty_graph(), &no_answers(), Tier::Pse);
        let delegado = gaps.iter().find(|g| g.control.contains("Delegado")).unwrap();
        assert_eq!(delegado.exigibilidad, Exigibilidad::MadurezVoluntaria);
    }

    #[test]
    fn objective_oiv_only_check_is_voluntary_for_a_municipality() {
        // BitLocker apagado ya no desaparece del informe de un municipio: se
        // informa como madurez, no como incumplimiento del Art. 8°.
        let mut graph = empty_graph();
        graph.drives.push(Drive {
            path: "C:\\".into(),
            kind: DriveKind::Fixed,
            total_bytes: None,
            free_bytes: None,
            encrypted: Some(false),
            host_ip: None,
        });
        let gaps = evaluate(&graph, &no_answers(), Tier::Pse);
        let bitlocker = gaps
            .iter()
            .find(|g| g.control.contains("BitLocker"))
            .expect("debe informarse como madurez, no desaparecer");
        assert_eq!(bitlocker.exigibilidad, Exigibilidad::MadurezVoluntaria);
        assert!(!bitlocker.requires_csirt_report);
    }

    #[test]
    fn objective_all_tier_check_is_binding_on_a_municipality() {
        let mut graph = empty_graph();
        graph.os_info.push(OsInfo {
            host_ip:         local_ip(),
            family:          "windows".into(),
            version:         "Windows 10.0 build 19045".into(),
            is_eol:          false,
            firewall_active: false,
            backup_agent_running: Some(true),
        });
        let gaps = evaluate(&graph, &no_answers(), Tier::Pse);
        let firewall = gaps.iter().find(|g| g.control.contains("Firewall")).unwrap();
        assert_eq!(firewall.exigibilidad, Exigibilidad::Exigible);
    }

    #[test]
    fn voluntary_maturity_never_requires_csirt_report() {
        for tier in [Tier::Oiv, Tier::Pse, Tier::Unclassified] {
            let gaps = evaluate(&empty_graph(), &no_answers(), tier);
            for gap in gaps.iter().filter(|g| g.exigibilidad == Exigibilidad::MadurezVoluntaria) {
                assert!(!gap.requires_csirt_report, "{} @ {tier:?}", gap.control);
            }
        }
    }

    #[test]
    fn no_gap_cites_ig4_as_binding_on_non_oiv() {
        // La IG N°4 está dirigida solo a OIV: si un hallazgo exigible a todos la
        // cita, debe hacerlo señalando expresamente que aplica a los OIV.
        let gaps = evaluate(&empty_graph(), &no_answers(), Tier::Pse);
        for gap in gaps.iter().filter(|g| g.legal_anchor.contains("IG N°4")) {
            assert!(
                gap.legal_anchor.contains("para OIV"),
                "anclaje legal excedido en {}: {}",
                gap.control,
                gap.legal_anchor
            );
        }
    }
}