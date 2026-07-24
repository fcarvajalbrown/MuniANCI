//! Historical record of evaluations, per comuna.
//!
//! ## Para qué
//!
//! Un escaneo dice cómo está la municipalidad hoy. La pregunta que en realidad hace
//! quien recibe el informe es otra: **¿estamos mejor que la vez pasada?** Sin
//! histórico no hay forma de responderla, y sin respuesta el informe no sostiene una
//! conversación de presupuesto.
//!
//! ## Por qué SQLite y no un JSON
//!
//! Porque el escáner ya barre la red local (`Scope::Lan` recorre el /24 completo).
//! Un barrido semanal de ~250 equipos acumula decenas de miles de filas al año, y un
//! JSON que hay que releer y reescribir entero en cada escaneo deja de servir a esa
//! escala. SQLite viaja compilado dentro del binario (`rusqlite` con `bundled`), así
//! que no exige instalar nada en el PC municipal ni una licencia de servidor.
//!
//! Ninguna norma chilena obliga a un motor: el Decreto 12 (Norma Técnica de
//! Interoperabilidad de la Ley 21.180) regula **cómo los órganos del Estado
//! intercambian** datos, no cómo los guardan por dentro. Lo que sí puede tener
//! exigencia es el formato de salida el día que este histórico se comparta con otro
//! órgano; eso es exportación y está diferido en el ROADMAP.
//!
//! ## Qué se guarda, y qué decide TI
//!
//! Siempre el resumen medible del escaneo. El desglose por activo —qué control
//! quedó abierto sobre qué equipo o recurso— es opcional: acumula un registro
//! histórico de qué máquina tuvo qué problema, y esa es una decisión de política de
//! cada municipalidad, no nuestra. Igual la retención. Ambas viven en
//! `munianci.config.json`.

use crate::config::HistoricoConfig;
use crate::types::{Exigibilidad, ScanResult};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Nombre del archivo, parametrizado por comuna.
pub fn nombre_archivo(institucion: &str) -> String {
    format!("historico_{}.db", slug(institucion))
}

/// Turns an institution name into a filesystem-safe slug.
///
/// Mismo criterio que el `db_<comuna>` del backend del Asistente: minúsculas, sin
/// tildes y con guion bajo, para que una municipalidad se llame igual en los dos
/// módulos del producto.
pub fn slug(institucion: &str) -> String {
    let mut out = String::new();
    let mut ultimo_guion = false;
    for c in institucion.trim().to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            otro => otro,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
            ultimo_guion = false;
        } else if !ultimo_guion && !out.is_empty() {
            out.push('_');
            ultimo_guion = true;
        }
    }
    out.trim_end_matches('_').to_string()
}

/// The measurable summary of one scan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resumen {
    pub fecha: String,
    pub puntaje: i32,
    pub base: i32,
    /// Promedio de madurez, `None` cuando ningún dominio pudo medirse.
    pub madurez: Option<f32>,
    pub exigibles: usize,
    pub madurez_gaps: usize,
    pub criticas: usize,
    pub altas: usize,
    pub medias: usize,
    pub cve_explotadas: usize,
    pub hosts: usize,
}

impl Resumen {
    /// Extracts the summary from a scan result.
    pub fn de(result: &ScanResult) -> Self {
        use crate::types::Severity;
        let exig: Vec<_> = result.gaps.iter()
            .filter(|g| g.exigibilidad == Exigibilidad::Exigible)
            .collect();
        Self {
            fecha: result.scanned_at.to_rfc3339(),
            puntaje: result.score.score,
            base: result.score.base,
            madurez: result.maturity.average(),
            exigibles: exig.len(),
            madurez_gaps: result.gaps.len() - exig.len(),
            criticas: exig.iter().filter(|g| g.severity == Severity::Critical).count(),
            altas: exig.iter().filter(|g| g.severity == Severity::High).count(),
            medias: exig.iter().filter(|g| g.severity == Severity::Medium).count(),
            cve_explotadas: result.asset_graph.software.iter().flat_map(|s| &s.cves)
                .chain(result.asset_graph.os_info.iter().flat_map(|o| &o.cves))
                .filter(|c| c.known_exploited)
                .count(),
            hosts: result.asset_graph.hosts.len(),
        }
    }
}

/// Which way the measurement moved between two scans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direccion {
    Mejoro,
    SinCambios,
    Empeoro,
}

/// The change between the previous scan and the current one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delta {
    /// Fecha del escaneo con el que se compara.
    pub desde: String,
    pub puntaje: i32,
    pub madurez: Option<f32>,
    pub exigibles: i64,
    pub criticas: i64,
    pub cve_explotadas: i64,
}

impl Delta {
    /// Computes the change from `antes` to `ahora`.
    pub fn entre(antes: &Resumen, ahora: &Resumen) -> Self {
        Self {
            desde: antes.fecha.clone(),
            puntaje: ahora.puntaje - antes.puntaje,
            madurez: match (antes.madurez, ahora.madurez) {
                (Some(a), Some(b)) => Some(b - a),
                _ => None,
            },
            exigibles: ahora.exigibles as i64 - antes.exigibles as i64,
            criticas: ahora.criticas as i64 - antes.criticas as i64,
            cve_explotadas: ahora.cve_explotadas as i64 - antes.cve_explotadas as i64,
        }
    }

    /// Which way things moved.
    ///
    /// Se decide acá y no leyendo el texto del veredicto: el informe pinta un
    /// marcador de color según esto, y deducir el color de una cadena hacía que
    /// "sin cambios" saliera en rojo.
    ///
    /// La explotación activa y los hallazgos críticos pesan igual que el puntaje:
    /// quedarse en los mismos puntos mientras aparecen tres vulnerabilidades
    /// explotándose no es "casi igual".
    pub fn direccion(&self) -> Direccion {
        if self.cve_explotadas > 0 || self.criticas > 0 || self.puntaje < 0 {
            return Direccion::Empeoro;
        }
        if self.puntaje > 0 || self.criticas < 0 || self.cve_explotadas < 0 {
            return Direccion::Mejoro;
        }
        Direccion::SinCambios
    }

    /// One-line verdict in plain Spanish.
    pub fn veredicto(&self) -> &'static str {
        match self.direccion() {
            Direccion::Empeoro if self.cve_explotadas > 0 || self.criticas > 0 => {
                "empeoro: hay hallazgos criticos o explotados nuevos"
            }
            Direccion::Empeoro => "empeoro respecto de la medicion anterior",
            Direccion::Mejoro => "mejoro respecto de la medicion anterior",
            Direccion::SinCambios => "sin cambios respecto de la medicion anterior",
        }
    }

    /// Formats a signed number the way a report should read it.
    pub fn signo(n: impl Into<i64>) -> String {
        let n = n.into();
        if n > 0 { format!("+{n}") } else { n.to_string() }
    }
}

/// The evaluation history for one comuna.
pub struct Historico {
    conn: Connection,
}

impl Historico {
    /// Opens (creating if needed) the history database at `path`.
    pub fn abrir(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("no se pudo abrir el historico en {}", path.display()))?;
        let h = Self { conn };
        h.crear_esquema()?;
        Ok(h)
    }

    /// In-memory history, for tests.
    pub fn en_memoria() -> Result<Self> {
        let h = Self { conn: Connection::open_in_memory()? };
        h.crear_esquema()?;
        Ok(h)
    }

    fn crear_esquema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS escaneo (
                id              INTEGER PRIMARY KEY,
                fecha           TEXT    NOT NULL,
                institucion     TEXT    NOT NULL,
                tier            TEXT    NOT NULL,
                puntaje         INTEGER NOT NULL,
                base            INTEGER NOT NULL,
                madurez         REAL,
                exigibles       INTEGER NOT NULL,
                madurez_gaps    INTEGER NOT NULL,
                criticas        INTEGER NOT NULL,
                altas           INTEGER NOT NULL,
                medias          INTEGER NOT NULL,
                cve_explotadas  INTEGER NOT NULL,
                hosts           INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_escaneo_fecha ON escaneo(fecha);
             CREATE TABLE IF NOT EXISTS nivel_dominio (
                escaneo_id INTEGER NOT NULL REFERENCES escaneo(id) ON DELETE CASCADE,
                dominio    TEXT    NOT NULL,
                nivel      INTEGER
             );
             CREATE TABLE IF NOT EXISTS brecha (
                escaneo_id   INTEGER NOT NULL REFERENCES escaneo(id) ON DELETE CASCADE,
                control      TEXT    NOT NULL,
                severidad    TEXT    NOT NULL,
                exigibilidad TEXT    NOT NULL,
                activo       TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_brecha_control ON brecha(control);",
        )?;
        Ok(())
    }

    /// The most recent scan recorded, if any.
    pub fn ultimo(&self) -> Result<Option<Resumen>> {
        let mut stmt = self.conn.prepare(
            "SELECT fecha, puntaje, base, madurez, exigibles, madurez_gaps,
                    criticas, altas, medias, cve_explotadas, hosts
             FROM escaneo ORDER BY fecha DESC, id DESC LIMIT 1",
        )?;
        let mut filas = stmt.query([])?;
        match filas.next()? {
            Some(f) => Ok(Some(Resumen {
                fecha: f.get(0)?,
                puntaje: f.get(1)?,
                base: f.get(2)?,
                madurez: f.get::<_, Option<f64>>(3)?.map(|v| v as f32),
                exigibles: f.get::<_, i64>(4)? as usize,
                madurez_gaps: f.get::<_, i64>(5)? as usize,
                criticas: f.get::<_, i64>(6)? as usize,
                altas: f.get::<_, i64>(7)? as usize,
                medias: f.get::<_, i64>(8)? as usize,
                cve_explotadas: f.get::<_, i64>(9)? as usize,
                hosts: f.get::<_, i64>(10)? as usize,
            })),
            None => Ok(None),
        }
    }

    /// Records a scan and returns its row id.
    pub fn registrar(&mut self, result: &ScanResult, config: &HistoricoConfig) -> Result<i64> {
        let r = Resumen::de(result);
        let tx = self.conn.transaction()?;

        tx.execute(
            "INSERT INTO escaneo (fecha, institucion, tier, puntaje, base, madurez,
                exigibles, madurez_gaps, criticas, altas, medias, cve_explotadas, hosts)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                r.fecha, result.meta.institution_name, result.meta.tier.to_string(),
                r.puntaje, r.base, r.madurez.map(|v| v as f64),
                r.exigibles as i64, r.madurez_gaps as i64,
                r.criticas as i64, r.altas as i64, r.medias as i64,
                r.cve_explotadas as i64, r.hosts as i64,
            ],
        )?;
        let id = tx.last_insert_rowid();

        for d in &result.maturity.domains {
            tx.execute(
                "INSERT INTO nivel_dominio (escaneo_id, dominio, nivel) VALUES (?1,?2,?3)",
                params![id, d.domain.title(), d.level.value().map(|v| v as i64)],
            )?;
        }

        for gap in &result.gaps {
            let exig = match gap.exigibilidad {
                Exigibilidad::Exigible => "exigible",
                Exigibilidad::MadurezVoluntaria => "madurez",
            };
            if config.desglose_por_activo && !gap.evidence.is_empty() {
                // Una fila por activo afectado: es lo que permite decir "esta
                // brecha lleva cinco meses abierta en estos doce equipos".
                for activo in &gap.evidence {
                    tx.execute(
                        "INSERT INTO brecha (escaneo_id, control, severidad, exigibilidad, activo)
                         VALUES (?1,?2,?3,?4,?5)",
                        params![id, gap.control, gap.severity.to_string(), exig, activo],
                    )?;
                }
            } else {
                tx.execute(
                    "INSERT INTO brecha (escaneo_id, control, severidad, exigibilidad, activo)
                     VALUES (?1,?2,?3,?4,NULL)",
                    params![id, gap.control, gap.severity.to_string(), exig],
                )?;
            }
        }

        tx.commit()?;
        Ok(id)
    }

    /// Deletes scans older than the configured retention, returning how many went.
    ///
    /// Sin esto el archivo crece para siempre sin que nadie lo mire. Una retención
    /// de 0 meses se interpreta como "no purgar".
    pub fn purgar(&self, config: &HistoricoConfig) -> Result<usize> {
        if config.retencion_meses == 0 {
            return Ok(0);
        }
        let corte = chrono::Utc::now()
            - chrono::Duration::days(config.retencion_meses as i64 * 30);
        let corte = corte.to_rfc3339();

        self.conn.execute("PRAGMA foreign_keys = ON", [])?;
        let ids: Vec<i64> = self.conn
            .prepare("SELECT id FROM escaneo WHERE fecha < ?1")?
            .query_map([&corte], |f| f.get(0))?
            .collect::<std::result::Result<_, _>>()?;

        for id in &ids {
            self.conn.execute("DELETE FROM nivel_dominio WHERE escaneo_id = ?1", [id])?;
            self.conn.execute("DELETE FROM brecha WHERE escaneo_id = ?1", [id])?;
            self.conn.execute("DELETE FROM escaneo WHERE id = ?1", [id])?;
        }
        Ok(ids.len())
    }

    /// How many scans are on record.
    pub fn cuantos(&self) -> Result<usize> {
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM escaneo", [], |f| f.get(0))?;
        Ok(n as usize)
    }

    /// Scans in which a control appeared as an open gap, oldest first.
    ///
    /// Responde "desde cuándo arrastramos esto", que es la frase que mueve un
    /// presupuesto municipal.
    pub fn abierta_desde(&self, control: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT MIN(e.fecha) FROM brecha b
             JOIN escaneo e ON e.id = b.escaneo_id
             WHERE b.control = ?1",
        )?;
        let fecha: Option<String> = stmt.query_row([control], |f| f.get(0))?;
        Ok(fecha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maturity::{Domain, MaturityProfile};
    use crate::types::{AppliesTo, AssetGraph, Gap, ScanMeta, Scope, Severity, Tier};
    use chrono::{Duration, Utc};

    fn gap(control: &str, sev: Severity, evidencia: Vec<&str>) -> Gap {
        Gap {
            control: control.into(),
            finding: "hallazgo".into(),
            severity: sev,
            legal_anchor: "Art. 7".into(),
            applies_to: AppliesTo::All,
            exigibilidad: Exigibilidad::Exigible,
            infraction_class: None,
            domain: Domain::MedidasPermanentes,
            evaluated: true,
            evidence: evidencia.into_iter().map(String::from).collect(),
            requires_csirt_report: false,
        }
    }

    fn resultado(gaps: Vec<Gap>, dias_atras: i64) -> ScanResult {
        ScanResult {
            meta: ScanMeta {
                institution_name: "Municipalidad de Nunoa".into(),
                tier: Tier::Pse,
                scope: Scope::Lan,
            },
            asset_graph: AssetGraph::default(),
            maturity: MaturityProfile::from_gaps(&gaps, &[Domain::MedidasPermanentes]),
            score: crate::scoring::ComplianceScore::from_gaps(&gaps),
            gaps,
            cve_coverage: crate::cve::Coverage::default(),
            kev_provenance: "prueba".into(),
            delta: None,
            scanned_at: Utc::now() - Duration::days(dias_atras),
        }
    }

    fn config(desglose: bool, meses: u32) -> HistoricoConfig {
        HistoricoConfig {
            habilitado: true,
            desglose_por_activo: desglose,
            retencion_meses: meses,
        }
    }

    #[test]
    fn the_slug_matches_the_assistant_convention() {
        assert_eq!(slug("Municipalidad de Ñuñoa"), "municipalidad_de_nunoa");
        assert_eq!(slug("  Providencia  "), "providencia");
        assert_eq!(slug("La Cisterna"), "la_cisterna");
        assert_eq!(nombre_archivo("Ñuñoa"), "historico_nunoa.db");
    }

    #[test]
    fn an_empty_history_has_nothing_to_compare_against() {
        let h = Historico::en_memoria().unwrap();
        assert_eq!(h.ultimo().unwrap(), None);
        assert_eq!(h.cuantos().unwrap(), 0);
    }

    #[test]
    fn a_scan_is_recorded_and_read_back() {
        let mut h = Historico::en_memoria().unwrap();
        let r = resultado(vec![gap("Firewall", Severity::Critical, vec!["10.0.0.1"])], 0);
        h.registrar(&r, &config(true, 24)).unwrap();

        let u = h.ultimo().unwrap().unwrap();
        assert_eq!(u.exigibles, 1);
        assert_eq!(u.criticas, 1);
        assert_eq!(u.puntaje, r.score.score);
        assert_eq!(h.cuantos().unwrap(), 1);
    }

    #[test]
    fn the_latest_scan_wins_over_the_older_ones() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(&resultado(vec![gap("A", Severity::Critical, vec![])], 30), &config(true, 24)).unwrap();
        h.registrar(&resultado(vec![], 0), &config(true, 24)).unwrap();
        assert_eq!(h.ultimo().unwrap().unwrap().exigibles, 0);
    }

    #[test]
    fn the_delta_reads_the_improvement_in_plain_spanish() {
        let antes = Resumen { puntaje: 70, criticas: 2, cve_explotadas: 1,
            ..Resumen::de(&resultado(vec![], 30)) };
        let ahora = Resumen { puntaje: 85, criticas: 0, cve_explotadas: 0,
            ..Resumen::de(&resultado(vec![], 0)) };
        let d = Delta::entre(&antes, &ahora);
        assert_eq!(d.puntaje, 15);
        assert_eq!(d.criticas, -2);
        assert!(d.veredicto().contains("mejoro"), "{}", d.veredicto());
        assert_eq!(Delta::signo(d.puntaje), "+15");
        assert_eq!(Delta::signo(d.criticas), "-2");
    }

    // Un puntaje que casi no se mueve mientras aparecen vulnerabilidades
    // explotandose no es "sin cambios".
    #[test]
    fn new_exploited_vulnerabilities_outweigh_a_flat_score() {
        let antes = Resumen { puntaje: 80, cve_explotadas: 0,
            ..Resumen::de(&resultado(vec![], 30)) };
        let ahora = Resumen { puntaje: 80, cve_explotadas: 3,
            ..Resumen::de(&resultado(vec![], 0)) };
        assert!(Delta::entre(&antes, &ahora).veredicto().contains("empeoro"));
    }

    // El color del marcador del informe sale de aqui: deducirlo del texto hacia
    // que "sin cambios" se pintara de rojo.
    #[test]
    fn the_direction_is_explicit_and_not_read_off_the_wording() {
        let base = Resumen::de(&resultado(vec![], 0));
        let sin = Delta::entre(&base, &base);
        assert_eq!(sin.direccion(), Direccion::SinCambios);
        assert!(sin.veredicto().contains("sin cambios"));

        let peor = Delta::entre(&Resumen { puntaje: 90, ..base.clone() },
                                &Resumen { puntaje: 70, ..base.clone() });
        assert_eq!(peor.direccion(), Direccion::Empeoro);

        let mejor = Delta::entre(&Resumen { puntaje: 70, ..base.clone() },
                                 &Resumen { puntaje: 90, ..base });
        assert_eq!(mejor.direccion(), Direccion::Mejoro);
    }

    #[test]
    fn maturity_delta_is_absent_when_a_side_was_never_measured() {
        let antes = Resumen { madurez: None, ..Resumen::de(&resultado(vec![], 30)) };
        let ahora = Resumen { madurez: Some(2.0), ..Resumen::de(&resultado(vec![], 0)) };
        assert_eq!(Delta::entre(&antes, &ahora).madurez, None);
    }

    // La decision de politica que TI controla: guardar o no que maquina tuvo que
    // problema.
    #[test]
    fn the_per_asset_breakdown_is_opt_out() {
        let g = gap("Firewall", Severity::Critical, vec!["10.0.0.1", "10.0.0.2", "10.0.0.3"]);

        let mut con = Historico::en_memoria().unwrap();
        con.registrar(&resultado(vec![g.clone()], 0), &config(true, 24)).unwrap();
        let n: i64 = con.conn.query_row("SELECT COUNT(*) FROM brecha", [], |f| f.get(0)).unwrap();
        assert_eq!(n, 3, "con desglose hay una fila por activo");

        let mut sin = Historico::en_memoria().unwrap();
        sin.registrar(&resultado(vec![g], 0), &config(false, 24)).unwrap();
        let n: i64 = sin.conn.query_row("SELECT COUNT(*) FROM brecha", [], |f| f.get(0)).unwrap();
        assert_eq!(n, 1, "sin desglose no se acumula que equipo era");
        let activo: Option<String> = sin.conn
            .query_row("SELECT activo FROM brecha", [], |f| f.get(0)).unwrap();
        assert_eq!(activo, None);
    }

    #[test]
    fn retention_drops_the_old_scans_and_their_children() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(&resultado(vec![gap("Viejo", Severity::High, vec!["x"])], 400), &config(true, 12)).unwrap();
        h.registrar(&resultado(vec![gap("Nuevo", Severity::High, vec!["y"])], 1), &config(true, 12)).unwrap();

        assert_eq!(h.purgar(&config(true, 12)).unwrap(), 1);
        assert_eq!(h.cuantos().unwrap(), 1);
        let n: i64 = h.conn.query_row("SELECT COUNT(*) FROM brecha", [], |f| f.get(0)).unwrap();
        assert_eq!(n, 1, "las brechas del escaneo purgado tienen que irse con el");
    }

    #[test]
    fn a_retention_of_zero_months_never_purges() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(&resultado(vec![], 4000), &config(true, 0)).unwrap();
        assert_eq!(h.purgar(&config(true, 0)).unwrap(), 0);
        assert_eq!(h.cuantos().unwrap(), 1);
    }

    #[test]
    fn it_can_say_how_long_a_gap_has_been_open() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(&resultado(vec![gap("Firewall", Severity::Critical, vec!["10.0.0.1"])], 150),
            &config(true, 0)).unwrap();
        h.registrar(&resultado(vec![gap("Firewall", Severity::Critical, vec!["10.0.0.1"])], 0),
            &config(true, 0)).unwrap();

        let desde = h.abierta_desde("Firewall").unwrap().unwrap();
        let dias = (Utc::now() - chrono::DateTime::parse_from_rfc3339(&desde).unwrap()
            .with_timezone(&Utc)).num_days();
        assert!(dias > 140, "deberia decir que lleva meses abierta, no {dias} dias");
        assert_eq!(h.abierta_desde("Nunca visto").unwrap(), None);
    }

    #[test]
    fn the_domain_levels_are_kept_per_scan() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(&resultado(vec![], 0), &config(true, 24)).unwrap();
        let n: i64 = h.conn.query_row("SELECT COUNT(*) FROM nivel_dominio", [], |f| f.get(0)).unwrap();
        assert_eq!(n, Domain::all().len() as i64);
    }
}
