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

/// How a control moved between the previous measurement and this one.
///
/// Los cuatro estados son los que usan Qualys (New / Active / Fixed / Re-Opened) y
/// Tenable (New / Active / Fixed / Resurfaced) para lo mismo, más un quinto que este
/// producto necesita y ellos resuelven de otra forma: ver [`Estado::SinVerificar`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    /// Abierta ahora y nunca vista antes.
    Nueva,
    /// Abierta ahora y en la medición anterior.
    Persistente,
    /// Estaba abierta en la anterior y ya no.
    Resuelta,
    /// Volvió a abrirse después de haber estado resuelta.
    ///
    /// Es el estado que importa. Un control que se corrigió y volvió a caerse dice algo
    /// del proceso de la municipalidad, no del parque de equipos: la corrección no se
    /// aplicó del todo, o no se sostuvo. En auditoría es el "hallazgo reiterado", y pesa
    /// más que uno nuevo.
    ///
    /// Dura una sola medición: si sigue abierta en la siguiente pasa a
    /// [`Estado::Persistente`], igual que en Qualys.
    Reaparecida,
    /// Estaba abierta antes, no aparece ahora, y este escaneo no alcanzó a mirar donde
    /// habría que mirar.
    ///
    /// No es lo mismo que resuelta y no puede informarse como tal. Un control técnico
    /// desaparece de los resultados tanto cuando se corrigió como cuando el escaneo no
    /// llegó hasta él —alcance más angosto, equipos apagados—, y decirle a una
    /// municipalidad que corrigió algo que nadie miró es exactamente el error que un
    /// informe dirigido a la ANCI no puede cometer. Mismo criterio que Rapid7, que no
    /// marca "Fixed" lo que un escaneo incompleto dejó de ver.
    SinVerificar,
}

impl Estado {
    /// One-word label for the report, in Spanish.
    pub fn etiqueta(self) -> &'static str {
        match self {
            Estado::Nueva        => "nueva",
            Estado::Persistente  => "persistente",
            Estado::Resuelta     => "resuelta",
            Estado::Reaparecida  => "reaparecida",
            Estado::SinVerificar => "sin verificar",
        }
    }

    /// Whether the control is open right now.
    pub fn abierta(self) -> bool {
        matches!(self, Estado::Nueva | Estado::Persistente | Estado::Reaparecida)
    }
}

/// One control and how it moved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlEnDeriva {
    pub control: String,
    pub estado: Estado,
    /// Fecha en que se la vio cerrada, para una reaparecida.
    pub resuelta_el: Option<String>,
}

/// What changed between the previous measurement and this one, control by control.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deriva {
    /// Fecha de la medición contra la que se compara. `None` en el primer escaneo.
    pub desde: Option<String>,
    /// Alcance de la medición anterior y de esta, tal como quedaron registrados.
    pub alcance_antes: Option<String>,
    pub alcance_ahora: Option<String>,
    /// Si este escaneo cubrió al menos lo que cubría el anterior.
    pub cobertura_comparable: bool,
    pub controles: Vec<ControlEnDeriva>,
}

impl Deriva {
    /// How many controls are in a given state.
    pub fn cuantos(&self, estado: Estado) -> usize {
        self.controles.iter().filter(|c| c.estado == estado).count()
    }

    /// Controls in a given state, in the order they were read.
    pub fn en(&self, estado: Estado) -> impl Iterator<Item = &ControlEnDeriva> {
        self.controles.iter().filter(move |c| c.estado == estado)
    }

    /// Whether there is a previous measurement to compare against at all.
    pub fn hay_comparacion(&self) -> bool {
        self.desde.is_some()
    }

    /// One-line verdict in plain Spanish, for the report header.
    pub fn resumen(&self) -> String {
        if !self.hay_comparacion() {
            return "primera medicion: no hay con que comparar".into();
        }
        let mut partes = vec![
            format!("{} nueva(s)", self.cuantos(Estado::Nueva)),
            format!("{} reaparecida(s)", self.cuantos(Estado::Reaparecida)),
            format!("{} resuelta(s)", self.cuantos(Estado::Resuelta)),
            format!("{} persistente(s)", self.cuantos(Estado::Persistente)),
        ];
        let sin = self.cuantos(Estado::SinVerificar);
        if sin > 0 {
            partes.push(format!("{sin} sin verificar"));
        }
        partes.join(", ")
    }
}

/// Serialises a scope the way the history stores it.
fn alcance_a_texto(scope: crate::types::Scope) -> &'static str {
    match scope {
        crate::types::Scope::Local => "local",
        crate::types::Scope::Lan => "lan",
    }
}

/// Whether `ahora` covers at least as much ground as `antes`.
///
/// Un `None` es un histórico anterior a 0.6.0, que no registraba el alcance. Se trata
/// como insuficiente a propósito: sin saber qué se miró, no se puede afirmar que algo
/// dejó de estar.
fn cobertura_suficiente(antes: Option<&str>, ahora: Option<&str>) -> bool {
    fn rango(a: Option<&str>) -> Option<u8> {
        match a {
            Some("local") => Some(0),
            Some("lan") => Some(1),
            _ => None,
        }
    }
    match (rango(antes), rango(ahora)) {
        (Some(a), Some(b)) => b >= a,
        _ => false,
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
                hosts           INTEGER NOT NULL,
                alcance         TEXT
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
             CREATE INDEX IF NOT EXISTS idx_brecha_control ON brecha(control);
             CREATE TABLE IF NOT EXISTS riesgo (
                id           TEXT PRIMARY KEY,
                control      TEXT NOT NULL,
                estado       TEXT NOT NULL,
                responsable  TEXT,
                plazo        TEXT,
                nota         TEXT,
                cerrado_el   TEXT,
                actualizado  TEXT NOT NULL
             );",
        )?;
        self.migrar_alcance()?;
        Ok(())
    }

    /// Adds the `alcance` column to a history created before 0.6.0.
    ///
    /// `CREATE TABLE IF NOT EXISTS` no agrega columnas a una tabla que ya existe, así
    /// que un histórico escrito por 0.5.0 no la tiene. Las mediciones viejas quedan con
    /// `NULL`, que la deriva lee como "no sé qué alcance tuvo" y trata igual que un
    /// alcance insuficiente: prefiere no afirmar que algo se resolvió.
    fn migrar_alcance(&self) -> Result<()> {
        let ya_esta: bool = self
            .conn
            .prepare("PRAGMA table_info(escaneo)")?
            .query_map([], |f| f.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .iter()
            .any(|c| c == "alcance");

        if !ya_esta {
            self.conn.execute("ALTER TABLE escaneo ADD COLUMN alcance TEXT", [])?;
        }
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
                exigibles, madurez_gaps, criticas, altas, medias, cve_explotadas, hosts,
                alcance)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                r.fecha, result.meta.institution_name, result.meta.tier.to_string(),
                r.puntaje, r.base, r.madurez.map(|v| v as f64),
                r.exigibles as i64, r.madurez_gaps as i64,
                r.criticas as i64, r.altas as i64, r.medias as i64,
                r.cve_explotadas as i64, r.hosts as i64,
                alcance_a_texto(result.meta.scope),
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

    /// Classifies every control against the previous measurement.
    ///
    /// Compara las dos mediciones más recientes. Corre sobre `escaneo` y `brecha`, que
    /// ya existían: lo único que 0.6.0 agregó al esquema es la columna `alcance`, y un
    /// histórico que no la tenga sigue produciendo deriva, solo que sin afirmar
    /// resoluciones de controles técnicos.
    pub fn deriva(&self) -> Result<Deriva> {
        let ultimos = self.ultimos_dos()?;
        let (ahora, antes) = match ultimos.as_slice() {
            [a, b] => (a, b),
            // Con una sola medición no hay deriva, y eso no es un error.
            _ => return Ok(Deriva::default()),
        };

        let abiertas_ahora = self.controles_de(ahora.0)?;
        let abiertas_antes = self.controles_de(antes.0)?;
        let vistas_antes_de = self.controles_antes_de(antes.0)?;
        let comparable = cobertura_suficiente(antes.2.as_deref(), ahora.2.as_deref());

        let mut controles = Vec::new();

        for control in &abiertas_ahora {
            let estado = if abiertas_antes.contains(control) {
                Estado::Persistente
            } else if vistas_antes_de.contains(control) {
                // Estuvo abierta, se cerró, volvió. Es el hallazgo reiterado.
                Estado::Reaparecida
            } else {
                Estado::Nueva
            };
            controles.push(ControlEnDeriva {
                control: control.clone(),
                // Solo una reaparecida tiene fecha de cierre que mostrar.
                resuelta_el: match estado {
                    Estado::Reaparecida => self.ultima_medicion_sin(control, ahora.0)?,
                    _ => None,
                },
                estado,
            });
        }

        for control in &abiertas_antes {
            if abiertas_ahora.contains(control) {
                continue;
            }
            // Un control declarativo que desaparece sí se resolvió: si nadie lo hubiera
            // respondido seguiría figurando como brecha. Uno técnico solo puede darse
            // por resuelto si este escaneo miró al menos lo mismo que el anterior.
            let estado = if comparable || crate::questionnaire::es_declarativo(control) {
                Estado::Resuelta
            } else {
                Estado::SinVerificar
            };
            controles.push(ControlEnDeriva {
                control: control.clone(),
                estado,
                resuelta_el: None,
            });
        }

        controles.sort_by(|a, b| a.control.cmp(&b.control));

        Ok(Deriva {
            desde: Some(antes.1.clone()),
            alcance_antes: antes.2.clone(),
            alcance_ahora: ahora.2.clone(),
            cobertura_comparable: comparable,
            controles,
        })
    }

    /// The two most recent scans as `(id, fecha, alcance)`, newest first.
    fn ultimos_dos(&self) -> Result<Vec<(i64, String, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, fecha, alcance FROM escaneo ORDER BY fecha DESC, id DESC LIMIT 2",
        )?;
        let filas = stmt
            .query_map([], |f| Ok((f.get(0)?, f.get(1)?, f.get(2)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(filas)
    }

    /// Distinct controls left open by one scan.
    fn controles_de(&self, escaneo_id: i64) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT control FROM brecha WHERE escaneo_id = ?1")?;
        let filas = stmt
            .query_map([escaneo_id], |f| f.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(filas)
    }

    /// Controls seen open in any scan older than `escaneo_id`.
    fn controles_antes_de(&self, escaneo_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT b.control FROM brecha b
             JOIN escaneo e ON e.id = b.escaneo_id
             WHERE e.id < ?1",
        )?;
        let filas = stmt
            .query_map([escaneo_id], |f| f.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(filas)
    }

    /// Date of the most recent scan before `hasta` where the control was not open.
    ///
    /// Es la fecha que el informe muestra junto a una reaparecida: "estaba resuelta el
    /// tanto". Sin ella la palabra "reaparecida" obliga a creerle al programa.
    fn ultima_medicion_sin(&self, control: &str, hasta: i64) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.fecha FROM escaneo e
             WHERE e.id < ?1
               AND NOT EXISTS (
                   SELECT 1 FROM brecha b
                   WHERE b.escaneo_id = e.id AND b.control = ?2
               )
             ORDER BY e.fecha DESC, e.id DESC LIMIT 1",
        )?;
        let fecha = stmt
            .query_map(params![hasta, control], |f| f.get(0))?
            .next()
            .transpose()?;
        Ok(fecha)
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

    // -----------------------------------------------------------------------
    // Registro de riesgos — seguimiento de un hallazgo hasta cerrarlo
    // -----------------------------------------------------------------------

    /// Estado que lleva TI para un hallazgo, si ya lo anotó.
    pub fn riesgo(&self, id: &str) -> Result<Option<Riesgo>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, control, estado, responsable, plazo, nota, cerrado_el, actualizado
             FROM riesgo WHERE id = ?1",
        )?;
        let mut filas = stmt.query([id])?;
        match filas.next()? {
            Some(f) => Ok(Some(Riesgo {
                id: f.get(0)?,
                control: f.get(1)?,
                estado: EstadoRiesgo::desde_texto(&f.get::<_, String>(2)?),
                responsable: f.get(3)?,
                plazo: f.get(4)?,
                nota: f.get(5)?,
                cerrado_el: f.get(6)?,
                actualizado: f.get(7)?,
            })),
            None => Ok(None),
        }
    }

    /// Todo el registro, para la pantalla de TI y el informe.
    pub fn riesgos(&self) -> Result<Vec<Riesgo>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, control, estado, responsable, plazo, nota, cerrado_el, actualizado
             FROM riesgo ORDER BY control",
        )?;
        let filas = stmt.query_map([], |f| {
            Ok(Riesgo {
                id: f.get(0)?,
                control: f.get(1)?,
                estado: EstadoRiesgo::desde_texto(&f.get::<_, String>(2)?),
                responsable: f.get(3)?,
                plazo: f.get(4)?,
                nota: f.get(5)?,
                cerrado_el: f.get(6)?,
                actualizado: f.get(7)?,
            })
        })?;
        Ok(filas.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    /// Anota o actualiza el estado de un hallazgo.
    ///
    /// `cerrado_el` lo pone el propio método al pasar a un estado terminal, y lo borra
    /// al salir de él: dejar que el llamador lo maneje sería dejar que un riesgo quede
    /// "cerrado" sin fecha, o con fecha estando abierto.
    pub fn anotar_riesgo(&mut self, r: &Riesgo) -> Result<()> {
        let ahora = chrono::Utc::now().to_rfc3339();
        let cerrado = if r.estado.es_terminal() {
            // Se conserva la fecha original si ya estaba cerrado: reabrir y volver a
            // cerrar no debe borrar cuándo se cerró la primera vez.
            r.cerrado_el.clone().or(Some(ahora.clone()))
        } else {
            None
        };
        self.conn.execute(
            "INSERT INTO riesgo (id, control, estado, responsable, plazo, nota, cerrado_el, actualizado)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                control = ?2, estado = ?3, responsable = ?4, plazo = ?5,
                nota = ?6, cerrado_el = ?7, actualizado = ?8",
            rusqlite::params![
                r.id, r.control, r.estado.texto(), r.responsable,
                r.plazo, r.nota, cerrado, ahora,
            ],
        )?;
        Ok(())
    }
}

/// Cómo va un hallazgo camino a cerrarse.
///
/// Los nombres salen del modelo POA&M de OSCAL, que ya define este ciclo de vida
/// (`risk/status`), en vez de inventar uno propio: así el estado que lleva la
/// municipalidad se emite tal cual en el documento que entrega.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoRiesgo {
    /// Abierto y sin trabajo declarado. Es el estado por defecto de todo hallazgo.
    Abierto,
    /// TI está averiguando si corresponde, o cómo corregirlo.
    Investigando,
    /// Corregido y verificado.
    Cerrado,
    /// Se revisó y el hallazgo no era real.
    ///
    /// Estado terminal, pero **distinto de cerrado**, y la diferencia importa: un falso
    /// positivo dice que la herramienta se equivocó, no que la municipalidad corrigió
    /// algo. Confundirlos infla el trabajo declarado ante una fiscalización.
    FalsoPositivo,
    /// Aceptado a sabiendas, con su justificación en la nota.
    ///
    /// No es cumplimiento y no se cuenta como tal: es una decisión de la institución
    /// que queda registrada con nombre y fecha.
    Aceptado,
}

impl EstadoRiesgo {
    pub fn texto(self) -> &'static str {
        match self {
            EstadoRiesgo::Abierto => "abierto",
            EstadoRiesgo::Investigando => "investigando",
            EstadoRiesgo::Cerrado => "cerrado",
            EstadoRiesgo::FalsoPositivo => "falso_positivo",
            EstadoRiesgo::Aceptado => "aceptado",
        }
    }

    /// Un texto desconocido cae en `Abierto` y no revienta.
    ///
    /// Una base escrita por una versión posterior puede traer un estado que esta no
    /// conoce. Tratarlo como abierto es el error seguro: muestra el hallazgo en vez de
    /// esconderlo.
    pub fn desde_texto(s: &str) -> Self {
        match s {
            "investigando" => EstadoRiesgo::Investigando,
            "cerrado" => EstadoRiesgo::Cerrado,
            "falso_positivo" => EstadoRiesgo::FalsoPositivo,
            "aceptado" => EstadoRiesgo::Aceptado,
            _ => EstadoRiesgo::Abierto,
        }
    }

    /// Si el hallazgo ya no está en curso.
    pub fn es_terminal(self) -> bool {
        matches!(
            self,
            EstadoRiesgo::Cerrado | EstadoRiesgo::FalsoPositivo | EstadoRiesgo::Aceptado
        )
    }

    /// El `risk/status` de OSCAL que le corresponde.
    ///
    /// El modelo define `open`, `investigating`, `remediating`, `deviation-requested`,
    /// `deviation-approved` y `closed`. Un aceptado se emite como desviación aprobada
    /// —que es lo que es— y no como cerrado, porque cerrado afirmaría una corrección
    /// que no ocurrió.
    pub fn oscal_status(self) -> &'static str {
        match self {
            EstadoRiesgo::Abierto => "open",
            EstadoRiesgo::Investigando => "investigating",
            EstadoRiesgo::Cerrado => "closed",
            EstadoRiesgo::FalsoPositivo => "closed",
            EstadoRiesgo::Aceptado => "deviation-approved",
        }
    }
}

/// Una fila del registro de riesgos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Riesgo {
    /// UUID v5 del hallazgo. Ver [`crate::poam::id_de_riesgo`].
    pub id: String,
    /// Nombre del control, para poder leer el registro sin cruzarlo con un escaneo.
    pub control: String,
    pub estado: EstadoRiesgo,
    pub responsable: Option<String>,
    /// Fecha comprometida, en ISO 8601. Criterio operativo de TI, no plazo legal.
    pub plazo: Option<String>,
    pub nota: Option<String>,
    /// Cuándo pasó a un estado terminal. Lo administra [`Historico::anotar_riesgo`].
    pub cerrado_el: Option<String>,
    pub actualizado: String,
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
        resultado_con(gaps, dias_atras, Scope::Lan)
    }

    fn resultado_con(gaps: Vec<Gap>, dias_atras: i64, scope: Scope) -> ScanResult {
        ScanResult {
            meta: ScanMeta {
                institution_name: "Municipalidad de Nunoa".into(),
                tier: Tier::Pse,
                scope,
            },
            asset_graph: AssetGraph::default(),
            maturity: MaturityProfile::from_gaps(&gaps, &[Domain::MedidasPermanentes]),
            ley21180: None,
            score: crate::scoring::ComplianceScore::from_gaps(&gaps),
            gaps,
            cve_coverage: crate::cve::Coverage::default(),
            kev_provenance: "prueba".into(),
            taxonomia_anci: crate::taxonomia::TaxonomiaAnci::default(),
            delta: None,
            deriva: None,
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
        assert_eq!(slug("  Puerto Montt  "), "puerto_montt");
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

    // -----------------------------------------------------------------------
    // Deriva por control
    // -----------------------------------------------------------------------

    fn estado_de(d: &Deriva, control: &str) -> Option<Estado> {
        d.controles.iter().find(|c| c.control == control).map(|c| c.estado)
    }

    #[test]
    fn a_first_scan_has_no_drift_and_that_is_not_an_error() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(&resultado(vec![gap("Firewall", Severity::High, vec![])], 0), &config(true, 0))
            .unwrap();

        let d = h.deriva().unwrap();
        assert!(!d.hay_comparacion());
        assert!(d.controles.is_empty());
        assert!(d.resumen().contains("primera medicion"));
    }

    #[test]
    fn an_empty_history_yields_an_empty_drift() {
        let h = Historico::en_memoria().unwrap();
        assert_eq!(h.deriva().unwrap(), Deriva::default());
    }

    #[test]
    fn a_control_open_in_both_scans_is_persistent() {
        let mut h = Historico::en_memoria().unwrap();
        let g = gap("Firewall", Severity::High, vec![]);
        h.registrar(&resultado(vec![g.clone()], 30), &config(true, 0)).unwrap();
        h.registrar(&resultado(vec![g], 0), &config(true, 0)).unwrap();

        assert_eq!(estado_de(&h.deriva().unwrap(), "Firewall"), Some(Estado::Persistente));
    }

    #[test]
    fn a_control_never_seen_before_is_new() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(&resultado(vec![gap("Viejo", Severity::High, vec![])], 30), &config(true, 0))
            .unwrap();
        h.registrar(
            &resultado(vec![gap("Viejo", Severity::High, vec![]), gap("Recien", Severity::High, vec![])], 0),
            &config(true, 0),
        )
        .unwrap();

        let d = h.deriva().unwrap();
        assert_eq!(estado_de(&d, "Recien"), Some(Estado::Nueva));
        assert_eq!(d.cuantos(Estado::Nueva), 1);
    }

    #[test]
    fn a_control_gone_from_the_latest_scan_is_resolved() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(&resultado(vec![gap("Firewall", Severity::High, vec![])], 30), &config(true, 0))
            .unwrap();
        h.registrar(&resultado(vec![], 0), &config(true, 0)).unwrap();

        let d = h.deriva().unwrap();
        assert_eq!(estado_de(&d, "Firewall"), Some(Estado::Resuelta));
        assert!(d.cobertura_comparable, "los dos escaneos fueron LAN");
    }

    // EL caso del hito. Una brecha que se corrigio y volvio a caerse no es una
    // brecha nueva: habla del proceso de la municipalidad, no de sus equipos.
    #[test]
    fn a_control_fixed_and_then_broken_again_is_reported_as_reappeared() {
        let mut h = Historico::en_memoria().unwrap();
        let g = gap("BitLocker", Severity::Critical, vec![]);

        h.registrar(&resultado(vec![g.clone()], 90), &config(true, 0)).unwrap(); // abierta
        h.registrar(&resultado(vec![], 60), &config(true, 0)).unwrap();          // corregida
        h.registrar(&resultado(vec![g], 0), &config(true, 0)).unwrap();          // vuelve

        let d = h.deriva().unwrap();
        assert_eq!(
            estado_de(&d, "BitLocker"),
            Some(Estado::Reaparecida),
            "no puede informarse como nueva: ya se habia corregido una vez"
        );
        assert_eq!(d.cuantos(Estado::Nueva), 0);
    }

    // "Reaparecida" sin fecha obliga a creerle al programa. Con fecha, el area de
    // TI puede ir a mirar que paso entre esas dos mediciones.
    #[test]
    fn a_reappeared_control_names_the_date_it_had_been_closed() {
        let mut h = Historico::en_memoria().unwrap();
        let g = gap("BitLocker", Severity::Critical, vec![]);
        h.registrar(&resultado(vec![g.clone()], 90), &config(true, 0)).unwrap();
        h.registrar(&resultado(vec![], 60), &config(true, 0)).unwrap();
        h.registrar(&resultado(vec![g], 0), &config(true, 0)).unwrap();

        let d = h.deriva().unwrap();
        let c = d.en(Estado::Reaparecida).next().unwrap();
        let cerrada = c.resuelta_el.as_ref().expect("tiene que decir cuando estuvo cerrada");
        let dias = (Utc::now()
            - chrono::DateTime::parse_from_rfc3339(cerrada).unwrap().with_timezone(&Utc))
        .num_days();
        assert!((55..65).contains(&dias), "deberia apuntar a la medicion de hace 60 dias, no {dias}");
    }

    // Qualys: una reaparecida que sigue abierta en el escaneo siguiente pasa a
    // activa. Quedarse en "reaparecida" para siempre convertiria el estado en ruido.
    #[test]
    fn a_reappeared_control_that_stays_open_becomes_persistent_next_time() {
        let mut h = Historico::en_memoria().unwrap();
        let g = gap("BitLocker", Severity::Critical, vec![]);
        h.registrar(&resultado(vec![g.clone()], 90), &config(true, 0)).unwrap();
        h.registrar(&resultado(vec![], 60), &config(true, 0)).unwrap();
        h.registrar(&resultado(vec![g.clone()], 30), &config(true, 0)).unwrap();
        assert_eq!(estado_de(&h.deriva().unwrap(), "BitLocker"), Some(Estado::Reaparecida));

        h.registrar(&resultado(vec![g], 0), &config(true, 0)).unwrap();
        assert_eq!(estado_de(&h.deriva().unwrap(), "BitLocker"), Some(Estado::Persistente));
    }

    // El error que un informe dirigido a la ANCI no puede cometer: decir que se
    // corrigio algo que en realidad nadie miro.
    #[test]
    fn a_narrower_rescan_does_not_claim_technical_controls_were_fixed() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(
            &resultado_con(vec![gap("Shares anonimos (SMB/NFS/WebDAV)", Severity::Critical, vec!["10.0.0.5"])], 30, Scope::Lan),
            &config(true, 0),
        )
        .unwrap();
        h.registrar(&resultado_con(vec![], 0, Scope::Local), &config(true, 0)).unwrap();

        let d = h.deriva().unwrap();
        assert!(!d.cobertura_comparable, "de LAN a local se miro menos");
        assert_eq!(
            estado_de(&d, "Shares anonimos (SMB/NFS/WebDAV)"),
            Some(Estado::SinVerificar)
        );
        assert_eq!(d.cuantos(Estado::Resuelta), 0);
        assert!(d.resumen().contains("sin verificar"));
    }

    #[test]
    fn a_wider_rescan_can_claim_resolutions() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(
            &resultado_con(vec![gap("Firewall", Severity::High, vec![])], 30, Scope::Local),
            &config(true, 0),
        )
        .unwrap();
        h.registrar(&resultado_con(vec![], 0, Scope::Lan), &config(true, 0)).unwrap();

        let d = h.deriva().unwrap();
        assert!(d.cobertura_comparable, "de local a LAN se miro mas, no menos");
        assert_eq!(estado_de(&d, "Firewall"), Some(Estado::Resuelta));
    }

    // Un control declarativo que desaparece si se resolvio, aunque el alcance se
    // haya angostado: si nadie lo hubiera respondido seguiria figurando como brecha.
    #[test]
    fn a_declarative_control_is_resolved_even_when_the_scope_narrowed() {
        let declarativo = crate::questionnaire::catalogue()[0].text.clone();

        let mut h = Historico::en_memoria().unwrap();
        h.registrar(
            &resultado_con(vec![gap(&declarativo, Severity::High, vec![])], 30, Scope::Lan),
            &config(true, 0),
        )
        .unwrap();
        h.registrar(&resultado_con(vec![], 0, Scope::Local), &config(true, 0)).unwrap();

        let d = h.deriva().unwrap();
        assert!(!d.cobertura_comparable);
        assert_eq!(
            estado_de(&d, &declarativo),
            Some(Estado::Resuelta),
            "el cuestionario no depende del alcance del barrido"
        );
    }

    // Un historico escrito por 0.5.0 no tiene la columna: sin saber que se miro,
    // no se puede afirmar que algo dejo de estar.
    #[test]
    fn an_unknown_scope_is_treated_as_insufficient_coverage() {
        assert!(!cobertura_suficiente(None, Some("lan")));
        assert!(!cobertura_suficiente(Some("lan"), None));
        assert!(!cobertura_suficiente(None, None));
        assert!(cobertura_suficiente(Some("local"), Some("lan")));
        assert!(cobertura_suficiente(Some("lan"), Some("lan")));
        assert!(!cobertura_suficiente(Some("lan"), Some("local")));
    }

    // La migracion tiene que correr sobre una base creada sin la columna, y no
    // volver a correr sobre una que ya la tiene.
    #[test]
    fn a_history_written_before_the_scope_column_still_opens_and_derives() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("viejo.db");

        // Un historico 0.5.0: mismo esquema, sin `alcance`.
        {
            let conn = Connection::open(&ruta).unwrap();
            conn.execute_batch(
                "CREATE TABLE escaneo (
                    id INTEGER PRIMARY KEY, fecha TEXT NOT NULL, institucion TEXT NOT NULL,
                    tier TEXT NOT NULL, puntaje INTEGER NOT NULL, base INTEGER NOT NULL,
                    madurez REAL, exigibles INTEGER NOT NULL, madurez_gaps INTEGER NOT NULL,
                    criticas INTEGER NOT NULL, altas INTEGER NOT NULL, medias INTEGER NOT NULL,
                    cve_explotadas INTEGER NOT NULL, hosts INTEGER NOT NULL);
                 CREATE TABLE brecha (
                    escaneo_id INTEGER NOT NULL, control TEXT NOT NULL, severidad TEXT NOT NULL,
                    exigibilidad TEXT NOT NULL, activo TEXT);",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO escaneo (fecha, institucion, tier, puntaje, base, madurez,
                    exigibles, madurez_gaps, criticas, altas, medias, cve_explotadas, hosts)
                 VALUES ('2026-01-01T00:00:00+00:00','Nunoa','PSE',80,100,NULL,1,0,0,1,0,0,3)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO brecha VALUES (1,'Firewall','High','exigible',NULL)",
                [],
            )
            .unwrap();
        }

        let mut h = Historico::abrir(&ruta).unwrap();
        assert_eq!(h.cuantos().unwrap(), 1, "la migracion no puede perder mediciones");

        h.registrar(&resultado_con(vec![], 0, Scope::Lan), &config(true, 0)).unwrap();
        let d = h.deriva().unwrap();
        assert_eq!(
            estado_de(&d, "Firewall"),
            Some(Estado::SinVerificar),
            "no se sabe con que alcance corrio el escaneo viejo"
        );

        // Reabrir no puede volver a intentar el ALTER TABLE.
        drop(h);
        assert_eq!(Historico::abrir(&ruta).unwrap().cuantos().unwrap(), 2);
    }

    #[test]
    fn the_drift_names_the_date_it_compares_against_and_both_scopes() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(&resultado_con(vec![], 30, Scope::Lan), &config(true, 0)).unwrap();
        h.registrar(&resultado_con(vec![], 0, Scope::Local), &config(true, 0)).unwrap();

        let d = h.deriva().unwrap();
        assert!(d.desde.is_some());
        assert_eq!(d.alcance_antes.as_deref(), Some("lan"));
        assert_eq!(d.alcance_ahora.as_deref(), Some("local"));
    }

    #[test]
    fn the_states_agree_on_whether_the_control_is_open_right_now() {
        assert!(Estado::Nueva.abierta());
        assert!(Estado::Persistente.abierta());
        assert!(Estado::Reaparecida.abierta());
        assert!(!Estado::Resuelta.abierta());
        assert!(!Estado::SinVerificar.abierta(), "no se sabe, y no saber no es estar abierta");
    }

    // El conteo de abiertas de la deriva tiene que cuadrar con las brechas del
    // escaneo, o el informe se contradice a si mismo entre dos parrafos.
    #[test]
    fn the_open_controls_in_the_drift_match_the_latest_scan() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(&resultado(vec![gap("A", Severity::High, vec![])], 30), &config(true, 0))
            .unwrap();
        let ahora = resultado(
            vec![gap("A", Severity::High, vec![]), gap("B", Severity::Critical, vec![])],
            0,
        );
        h.registrar(&ahora, &config(true, 0)).unwrap();

        let d = h.deriva().unwrap();
        let abiertas = d.controles.iter().filter(|c| c.estado.abierta()).count();
        assert_eq!(abiertas, ahora.gaps.len());
    }

    #[test]
    fn the_drift_survives_a_json_round_trip() {
        let mut h = Historico::en_memoria().unwrap();
        let g = gap("BitLocker", Severity::Critical, vec![]);
        h.registrar(&resultado(vec![g.clone()], 90), &config(true, 0)).unwrap();
        h.registrar(&resultado(vec![], 60), &config(true, 0)).unwrap();
        h.registrar(&resultado(vec![g], 0), &config(true, 0)).unwrap();

        let d = h.deriva().unwrap();
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("reaparecida"), "el estado va en snake_case al JSON: {json}");
        assert_eq!(serde_json::from_str::<Deriva>(&json).unwrap(), d);
    }

    #[test]
    fn the_domain_levels_are_kept_per_scan() {
        let mut h = Historico::en_memoria().unwrap();
        h.registrar(&resultado(vec![], 0), &config(true, 24)).unwrap();
        let n: i64 = h.conn.query_row("SELECT COUNT(*) FROM nivel_dominio", [], |f| f.get(0)).unwrap();
        assert_eq!(n, Domain::all().len() as i64);
    }
}

#[cfg(test)]
mod riesgos_tests {
    use super::*;

    fn r(id: &str, estado: EstadoRiesgo) -> Riesgo {
        Riesgo {
            id: id.into(),
            control: "Shares anónimos (SMB/NFS/WebDAV)".into(),
            estado,
            responsable: Some("Jefe de Informática".into()),
            plazo: Some("2026-09-30".into()),
            nota: None,
            cerrado_el: None,
            actualizado: String::new(),
        }
    }

    #[test]
    fn un_hallazgo_sin_anotar_no_esta_en_el_registro() {
        let h = Historico::en_memoria().unwrap();
        assert!(h.riesgo("no-existe").unwrap().is_none());
        assert!(h.riesgos().unwrap().is_empty());
    }

    #[test]
    fn el_estado_sobrevive_y_se_puede_actualizar() {
        // Es la razón de ser del registro: un hallazgo se cierra a lo largo de varios
        // escaneos, no dentro de uno.
        let mut h = Historico::en_memoria().unwrap();
        h.anotar_riesgo(&r("abc", EstadoRiesgo::Investigando)).unwrap();
        assert_eq!(h.riesgo("abc").unwrap().unwrap().estado, EstadoRiesgo::Investigando);

        h.anotar_riesgo(&r("abc", EstadoRiesgo::Cerrado)).unwrap();
        let g = h.riesgo("abc").unwrap().unwrap();
        assert_eq!(g.estado, EstadoRiesgo::Cerrado);
        assert_eq!(h.riesgos().unwrap().len(), 1, "actualizar no debe duplicar la fila");
    }

    #[test]
    fn cerrar_pone_fecha_y_reabrir_la_quita() {
        let mut h = Historico::en_memoria().unwrap();
        h.anotar_riesgo(&r("abc", EstadoRiesgo::Abierto)).unwrap();
        assert!(h.riesgo("abc").unwrap().unwrap().cerrado_el.is_none());

        h.anotar_riesgo(&r("abc", EstadoRiesgo::Cerrado)).unwrap();
        let cerrado = h.riesgo("abc").unwrap().unwrap().cerrado_el;
        assert!(cerrado.is_some(), "un riesgo cerrado sin fecha no sirve de evidencia");

        // Reaparece: no puede quedar "abierto" arrastrando una fecha de cierre.
        h.anotar_riesgo(&r("abc", EstadoRiesgo::Abierto)).unwrap();
        assert!(h.riesgo("abc").unwrap().unwrap().cerrado_el.is_none());
    }

    #[test]
    fn el_falso_positivo_no_es_lo_mismo_que_cerrado() {
        // Cerrado dice que la municipalidad corrigió; falso positivo dice que la
        // herramienta se equivocó. Contarlos juntos infla el trabajo declarado.
        assert!(EstadoRiesgo::FalsoPositivo.es_terminal());
        assert!(EstadoRiesgo::Cerrado.es_terminal());
        assert_ne!(EstadoRiesgo::FalsoPositivo, EstadoRiesgo::Cerrado);
        assert!(!EstadoRiesgo::Abierto.es_terminal());
        assert!(!EstadoRiesgo::Investigando.es_terminal());
    }

    #[test]
    fn un_aceptado_no_se_emite_como_cerrado_en_oscal() {
        // Aceptar un riesgo es una decisión, no una corrección. Emitirlo como "closed"
        // afirmaría ante quien lea el POA&M que se remedió algo que sigue ahí.
        assert_eq!(EstadoRiesgo::Aceptado.oscal_status(), "deviation-approved");
        assert_eq!(EstadoRiesgo::Cerrado.oscal_status(), "closed");
        assert_eq!(EstadoRiesgo::Abierto.oscal_status(), "open");
        assert_eq!(EstadoRiesgo::Investigando.oscal_status(), "investigating");
    }

    #[test]
    fn un_estado_desconocido_se_lee_como_abierto() {
        // Una base escrita por una versión posterior puede traer un estado que esta no
        // conoce. Mostrarlo abierto es el error seguro; esconderlo no lo es.
        assert_eq!(EstadoRiesgo::desde_texto("teletransportado"), EstadoRiesgo::Abierto);
        assert_eq!(EstadoRiesgo::desde_texto(""), EstadoRiesgo::Abierto);
    }

    #[test]
    fn una_base_de_0_6_x_se_abre_y_gana_la_tabla() {
        // Mismo criterio que `migrar_alcance`: un histórico anterior tiene que seguir
        // abriendo, y sin perder sus mediciones.
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("viejo.db");
        {
            let c = Connection::open(&ruta).unwrap();
            c.execute_batch(
                "CREATE TABLE escaneo (
                    id INTEGER PRIMARY KEY, fecha TEXT NOT NULL, institucion TEXT NOT NULL,
                    tier TEXT NOT NULL, puntaje INTEGER NOT NULL, base INTEGER NOT NULL,
                    madurez REAL, exigibles INTEGER NOT NULL, madurez_gaps INTEGER NOT NULL,
                    criticas INTEGER NOT NULL, altas INTEGER NOT NULL, medias INTEGER NOT NULL,
                    cve_explotadas INTEGER NOT NULL, hosts INTEGER NOT NULL);
                 INSERT INTO escaneo VALUES
                    (1,'2026-01-01T00:00:00Z','Organismo del Estado','pse',
                     80,100,2.0,3,1,1,1,1,0,4);",
            )
            .unwrap();
        }
        let mut h = Historico::abrir(&ruta).unwrap();
        assert!(h.ultimo().unwrap().is_some(), "se perdió la medición anterior");
        h.anotar_riesgo(&r("abc", EstadoRiesgo::Abierto)).unwrap();
        assert_eq!(h.riesgos().unwrap().len(), 1);
    }
}
