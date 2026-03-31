//! Post-normalization EOL enrichment — patches SoftwareEntry and OsInfo using a bundled static DB.
use crate::types::{AssetGraph, OsInfo, SoftwareEntry};
use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::HashMap;

static EOL_JSON: &str = include_str!("data/eol_db.json");

#[derive(Deserialize)]
struct Cycle {
    cycle: String,
    eol_date: Option<String>,
    is_eol: bool,
}

type EolDb = HashMap<String, Vec<Cycle>>;

fn load_db() -> EolDb {
    serde_json::from_str(EOL_JSON).expect("eol_db.json is malformed")
}

pub fn enrich(graph: &mut AssetGraph) {
    let db = load_db();
    let today = chrono::Local::now().date_naive();
    for sw in graph.software.iter_mut() { enrich_software(sw, &db, today); }
    for os in graph.os_info.iter_mut() { enrich_os(os, &db, today); }
}

fn enrich_software(sw: &mut SoftwareEntry, db: &EolDb, today: NaiveDate) {
    let slug = match detect_product_slug(&sw.name) { Some(s) => s, None => return };
    let cycles = match db.get(slug) { Some(c) => c, None => return };
    if let Some(cycle) = best_cycle_match(cycles, sw.version.trim(), today) {
        sw.is_eol = resolved_eol(cycle, today);
    }
}

fn enrich_os(os: &mut OsInfo, db: &EolDb, today: NaiveDate) {
    let v = os.version.to_lowercase();
    if v.contains("windows server") {
        if let Some(cycles) = db.get("windows-server") {
            if let Some(year) = extract_server_year(&os.version) {
                if let Some(cycle) = cycles.iter().find(|c| c.cycle == year) {
                    os.is_eol = resolved_eol(cycle, today);
                    return;
                }
            }
        }
    }
    if v.contains("windows") {
        if let Some(cycles) = db.get("windows") {
            if let Some(build) = extract_windows_build(&os.version) {
                if let Some(cycle) = best_cycle_match(cycles, &build, today) {
                    os.is_eol = resolved_eol(cycle, today);
                }
            }
        }
    }
}

fn best_cycle_match<'a>(cycles: &'a [Cycle], version: &str, _today: NaiveDate) -> Option<&'a Cycle> {
    let mut best: Option<&Cycle> = None;
    let mut best_len = 0usize;
    for cycle in cycles {
        let cn = cycle.cycle.replace('-', ".");
        let vn = version.replace('-', ".");
        if vn.starts_with(&cn) && cn.len() > best_len {
            best = Some(cycle);
            best_len = cn.len();
        }
    }
    best
}

fn resolved_eol(cycle: &Cycle, today: NaiveDate) -> bool {
    if cycle.is_eol { return true; }
    if let Some(ds) = &cycle.eol_date {
        if let Ok(d) = NaiveDate::parse_from_str(ds, "%Y-%m-%d") { return d <= today; }
    }
    false
}

fn detect_product_slug(name: &str) -> Option<&'static str> {
    let n = name.to_lowercase();
    if n.contains("microsoft office") || n.contains("office professional")
        || n.contains("office standard") || n.contains("office home") { return Some("office"); }
    if n.contains("sql server")          { return Some("mssqlserver"); }
    if n.contains("exchange server")     { return Some("msexchange"); }
    if n.contains("sharepoint")          { return Some("sharepoint"); }
    if (n.contains("microsoft .net") || n.contains(".net core") || n.contains(".net runtime"))
        && !n.contains("framework") && !n.contains("targeting pack") { return Some("dotnet"); }
    if n.contains(".net framework")      { return Some("dotnetfx"); }
    if n.contains("visual studio") && !n.contains("code") && !n.contains("vscode") { return Some("visual-studio"); }
    if n.contains("powershell") && !n.contains("windows powershell") { return Some("powershell"); }
    if n.contains("windows powershell")  { return Some("windows-powershell"); }
    if n.contains("internet explorer")   { return Some("internet-explorer"); }
    if n.contains("firefox")             { return Some("firefox"); }
    if n.contains("chrome") || n.contains("google chrome") { return Some("chrome"); }
    if n.contains("mysql")               { return Some("mysql"); }
    if n.contains("postgresql") || n.contains("postgres") { return Some("postgresql"); }
    if n.contains("mariadb")             { return Some("mariadb"); }
    if n.contains("mongodb")             { return Some("mongodb"); }
    if n.contains("redis")               { return Some("redis"); }
    if n.contains("elasticsearch")       { return Some("elasticsearch"); }
    if n.contains("oracle database") || n.contains("oracle db") { return Some("oracle-database"); }
    if n.contains("python") && !n.contains("documentation") && !n.contains("launcher") { return Some("python"); }
    if n.contains("node.js") || n.contains("nodejs") { return Some("nodejs"); }
    if n.contains("apache http") || (n.contains("apache") && n.contains("http server")) { return Some("apache-http-server"); }
    if n.contains("nginx")               { return Some("nginx"); }
    if n.contains("tomcat")              { return Some("tomcat"); }
    if n.contains("wordpress")           { return Some("wordpress"); }
    if n.contains("drupal")              { return Some("drupal"); }
    if n.contains("joomla")              { return Some("joomla"); }
    if n.contains("libreoffice")         { return Some("libreoffice"); }
    if n.contains("vcenter")             { return Some("vcenter"); }
    if n.contains("vmware cloud foundation") { return Some("vmware-cloud-foundation"); }
    if n.contains("veeam backup & replication") || n.contains("veeam backup and replication") { return Some("veeam-backup-and-replication"); }
    if n.contains("openssl")             { return Some("openssl"); }
    if n.contains("isc dhcp") || n.contains("dhcp server") { return Some("isc-dhcp"); }
    if n.contains("notepad++") || n.contains("notepad plus") { return Some("notepad-plus-plus"); }
    if n.contains("php")                 { return Some("php"); }
    None
}

fn extract_server_year(version: &str) -> Option<String> {
    let v = version.to_lowercase();
    if v.contains("2025")                              { return Some("2025".into()); }
    if v.contains("2022")                              { return Some("2022".into()); }
    if v.contains("2019")                              { return Some("2019".into()); }
    if v.contains("2016")                              { return Some("2016".into()); }
    if v.contains("2012 r2") || v.contains("2012-r2") { return Some("2012-r2".into()); }
    if v.contains("2012")                              { return Some("2012".into()); }
    if v.contains("2008 r2") || v.contains("2008-r2") { return Some("2008-r2".into()); }
    if v.contains("2008")                              { return Some("2008".into()); }
    if v.contains("2003 r2") || v.contains("2003-r2") { return Some("2003-r2".into()); }
    if v.contains("2003")                              { return Some("2003".into()); }
    None
}

fn extract_windows_build(version: &str) -> Option<String> {
    let build: u32 = version.split_whitespace().last()?.parse().ok()?;
    let cycle = match build {
        26100..=u32::MAX => "11-24H2",
        22631..=26099    => "11-23H2",
        22621..=22630    => "11-22H2",
        22000..=22620    => "11-21H2",
        19045..=21999    => "10-22H2",
        19044            => "10-21H2",
        19043            => "10-21H1",
        19042            => "10-20H2",
        19041            => "10-2004",
        18363..=19040    => "10-1909",
        18362            => "10-1903",
        17763..=18361    => "10-1809",
        17134..=17762    => "10-1803",
        16299..=17133    => "10-1709",
        15063..=16298    => "10-1703",
        14393..=15062    => "10-1607",
        10586..=14392    => "10-1511",
        10240..=10585    => "10-1507",
        _                => return None,
    };
    Some(cycle.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate { NaiveDate::from_ymd_opt(2026, 3, 31).unwrap() }

    #[test]
    fn office_2016_is_eol() {
        let cycles = load_db();
        let cycle = best_cycle_match(cycles.get("office").unwrap(), "2016", today());
        assert!(cycle.unwrap().is_eol);
    }

    #[test]
    fn office_2024_not_eol() {
        let cycles = load_db();
        let cycle = best_cycle_match(cycles.get("office").unwrap(), "2024", today());
        assert!(!cycle.unwrap().is_eol);
    }

    #[test]
    fn windows_build_19045_maps_to_10_22h2() {
        assert_eq!(extract_windows_build("Windows 10.0 build 19045").unwrap(), "10-22H2");
    }

    #[test]
    fn windows_10_22h2_not_eol() {
        let cycles = load_db();
        let cycle = best_cycle_match(cycles.get("windows").unwrap(), "10-22H2", today());
        assert!(!resolved_eol(cycle.unwrap(), today()));
    }

    #[test]
    fn python_2_7_is_eol() {
        let cycles = load_db();
        let cycle = best_cycle_match(cycles.get("python").unwrap(), "2.7.18", today());
        assert!(cycle.unwrap().is_eol);
    }

    #[test]
    fn slug_office() {
        assert_eq!(detect_product_slug("Microsoft Office Professional Plus 2016"), Some("office"));
    }

    #[test]
    fn slug_sql_server() {
        assert_eq!(detect_product_slug("Microsoft SQL Server 2014 (64-bit)"), Some("mssqlserver"));
    }

    #[test]
    fn slug_unknown_is_none() {
        assert_eq!(detect_product_slug("Armoury Crate Service"), None);
    }

    #[test]
    fn server_year_2012_r2() {
        assert_eq!(extract_server_year("Windows Server 2012 R2"), Some("2012-r2".into()));
    }
}