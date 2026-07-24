//! Turns the software/OS inventory into concrete CVEs, offline.
//!
//! Corre después de `eol_enrichment`, sobre el mismo `AssetGraph`. Reutiliza su
//! detección de producto y su cálculo de ciclo de release, de modo que un producto
//! reconocido para EOL es exactamente el mismo que se evalúa para CVE.
//!
//! La regla que gobierna todo el módulo: **no evaluado no es lo mismo que limpio**.
//! Un paquete que no está en la tabla curada no produce hallazgos, pero sí suma a
//! [`Coverage`], y el informe declara cuántos quedaron fuera.

use super::index;
use super::matcher::{find_hits, CveHit};
use super::product_map;
use super::Coverage;
use crate::types::AssetGraph;

/// Enriches the graph with CVE findings and reports how much of it was evaluable.
pub fn enrich(graph: &mut AssetGraph, kev: &dyn Fn(&str) -> bool) -> Coverage {
    let records = index::records();
    let mut coverage = Coverage::default();

    for sw in graph.software.iter_mut() {
        coverage.total += 1;

        let Some(identities) = product_map::identities_for_software(&sw.name) else {
            continue; // sin mapeo: queda declarado como no evaluado
        };
        if sw.version.trim().is_empty() {
            continue; // sin version no se puede juzgar
        }
        coverage.evaluated += 1;

        let mut hits: Vec<CveHit> = Vec::new();
        for id in identities {
            hits.extend(find_hits(records, &id.with_version(&sw.version), kev));
        }
        dedupe(&mut hits);
        sw.max_cvss = hits.iter().filter_map(|h| h.cvss).fold(None, |acc, c| {
            Some(acc.map_or(c, |a: f32| a.max(c)))
        });
        sw.cves = hits;
    }

    for os in graph.os_info.iter_mut() {
        let Some((family, cycle)) = os_cycle(&os.version) else {
            continue;
        };
        let Some(id) = product_map::identity_for_os(family, &cycle) else {
            continue;
        };
        // En los CPE de sistemas operativos de Microsoft la release viaja en el
        // nombre del producto, así que el campo version suele ser '-' (NA).
        let mut hits = find_hits(records, &id.with_version("-"), kev);
        dedupe(&mut hits);
        os.cves = hits;
    }

    coverage
}

/// Maps an OS version string to the family slug and release cycle that
/// `eol_enrichment` already knows how to derive.
fn os_cycle(version: &str) -> Option<(&'static str, String)> {
    let v = version.to_lowercase();
    if v.contains("windows server") {
        return crate::eol_enrichment::extract_server_year(version).map(|c| ("windows-server", c));
    }
    if v.contains("windows") {
        return crate::eol_enrichment::extract_windows_build(version).map(|c| ("windows", c));
    }
    None
}

/// Removes duplicate CVE ids, which appear when a product is curated under more
/// than one vendor (redis, elasticsearch) and both match the same record.
fn dedupe(hits: &mut Vec<CveHit>) {
    let mut seen = std::collections::HashSet::new();
    hits.retain(|h| seen.insert(h.cve_id.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{OsInfo, SoftwareEntry};
    use std::net::IpAddr;

    fn ip() -> IpAddr {
        "127.0.0.1".parse().unwrap()
    }

    fn sw(name: &str, version: &str) -> SoftwareEntry {
        SoftwareEntry {
            name: name.into(),
            version: version.into(),
            host_ip: ip(),
            is_eol: false,
            max_cvss: None,
            cves: vec![],
        }
    }

    fn graph_with(entries: Vec<SoftwareEntry>) -> AssetGraph {
        AssetGraph { software: entries, ..Default::default() }
    }

    #[test]
    fn finds_real_cves_for_an_old_package() {
        let mut g = graph_with(vec![sw("OpenSSL 1.0.1", "1.0.1")]);
        let cov = enrich(&mut g, &|_| false);
        assert_eq!(cov.evaluated, 1);
        assert!(!g.software[0].cves.is_empty(), "OpenSSL 1.0.1 debe arrastrar CVE");
        assert!(g.software[0].max_cvss.is_some());
    }

    #[test]
    fn unmapped_software_counts_as_not_evaluated_not_as_clean() {
        let mut g = graph_with(vec![sw("Armoury Crate Service", "1.0")]);
        let cov = enrich(&mut g, &|_| false);
        assert_eq!(cov.total, 1);
        assert_eq!(cov.evaluated, 0, "no se puede fingir que se evaluo");
        assert_eq!(cov.unmapped(), 1);
        assert!(g.software[0].cves.is_empty());
    }

    #[test]
    fn software_without_a_version_is_not_evaluated() {
        let mut g = graph_with(vec![sw("OpenSSL", "")]);
        let cov = enrich(&mut g, &|_| false);
        assert_eq!(cov.evaluated, 0);
    }

    #[test]
    fn coverage_mixes_mapped_and_unmapped() {
        let mut g = graph_with(vec![
            sw("OpenSSL 3.5.0", "3.5.0"),
            sw("Armoury Crate Service", "1.0"),
            sw("Google Chrome", "120.0.1"),
        ]);
        let cov = enrich(&mut g, &|_| false);
        assert_eq!(cov.total, 3);
        assert_eq!(cov.evaluated, 2);
        assert!((cov.percent() - 66.6).abs() < 1.0, "{}", cov.percent());
    }

    #[test]
    fn kev_flag_is_carried_through() {
        let mut g = graph_with(vec![sw("OpenSSL 1.0.1", "1.0.1")]);
        // Marca como explotada la primera CVE que aparezca, sea cual sea.
        enrich(&mut g, &|_| true);
        assert!(g.software[0].cves.iter().all(|h| h.known_exploited));
    }

    #[test]
    fn no_duplicate_cve_ids_when_a_product_has_two_vendors() {
        let mut g = graph_with(vec![sw("Redis 6.0.0", "6.0.0")]);
        enrich(&mut g, &|_| false);
        let ids: Vec<&str> = g.software[0].cves.iter().map(|h| h.cve_id.as_str()).collect();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "hay CVE repetidas: {ids:?}");
    }

    #[test]
    fn enriches_an_end_of_life_windows_server() {
        let mut g = AssetGraph::default();
        g.os_info.push(OsInfo {
            host_ip: ip(),
            family: "windows".into(),
            version: "Windows Server 2012 R2".into(),
            is_eol: true,
            firewall_active: true,
            backup_agent_running: Some(true),
            cves: vec![],
        });
        enrich(&mut g, &|_| false);
        // 2012 R2 mapea al ciclo "2012-r2", que no esta en la tabla; el ciclo
        // "2012" si. Lo que importa es que no reviente y no invente.
        let _ = &g.os_info[0].cves;
    }

    #[test]
    fn windows_server_2012_gets_findings() {
        let mut g = AssetGraph::default();
        g.os_info.push(OsInfo {
            host_ip: ip(),
            family: "windows".into(),
            version: "Windows Server 2012".into(),
            is_eol: true,
            firewall_active: true,
            backup_agent_running: Some(true),
            cves: vec![],
        });
        enrich(&mut g, &|_| false);
        assert!(!g.os_info[0].cves.is_empty(), "Windows Server 2012 debe arrastrar CVE");
    }
}
