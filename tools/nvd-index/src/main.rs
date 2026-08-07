//! Build-time converter for the pinned NVD snapshot.
//!
//! No se enlaza nunca en la app: produce el índice compacto que sí viaja con el
//! producto, a partir de `vendor/nvd/CVE-all.json.xz` (~2,95 GB sin comprimir).
//!
//! Dos modos, y el orden importa:
//!
//! 1. `catalog` extrae del snapshot todos los pares `vendor:product` distintos
//!    con su conteo de CVE. Sirve para **descubrir** cómo se llama realmente un
//!    producto en CPE en vez de adivinarlo, que es justamente lo que no se puede
//!    hacer en un informe de cumplimiento.
//! 2. `build` filtra el snapshot a los productos de la tabla curada y emite el
//!    índice. Falla si alguna entrada de la tabla no aparece en los datos, de modo
//!    que un CPE inventado no puede sobrevivir al build.
//!
//! El JSON se recorre por streaming con un visitor de secuencia: cargar el
//! documento completo en memoria pediría varios GB.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::de::{DeserializeSeed, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Convierte el snapshot NVD en el índice compacto de MuniGPT")]
struct Cli {
    /// Ruta al snapshot xz descargado en vendor/nvd/.
    #[arg(long, default_value = "vendor/nvd/CVE-all.json.xz")]
    snapshot: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Extrae los pares vendor:product presentes en el snapshot, con conteo.
    Catalog {
        /// Archivo de salida (TSV: conteo, part, vendor, product).
        #[arg(long, default_value = "vendor/nvd/cpe-catalog.tsv")]
        out: PathBuf,
    },
    /// Verifica que cada entrada de la tabla curada exista en el snapshot.
    ///
    /// Es la compuerta que impide que un CPE inventado llegue a un informe:
    /// termina con código distinto de cero si alguna entrada no aparece.
    Validate {
        /// Catálogo generado por `catalog`.
        #[arg(long, default_value = "vendor/nvd/cpe-catalog.tsv")]
        catalog: PathBuf,
    },
    /// Filtra el snapshot a los productos curados y emite el índice que viaja
    /// con el producto.
    Build {
        /// Archivo de salida. Va con gzip y no con xz porque este archivo se
        /// embebe en el binario de la app, y flate2 descomprime en Rust puro:
        /// xz obligaria a enlazar liblzma en el producto final.
        #[arg(long, default_value = "core/src/data/cve_index.json.gz")]
        out: PathBuf,
        /// Largo máximo de la descripción conservada, en caracteres.
        #[arg(long, default_value_t = 300)]
        max_description: usize,
    },
    /// Convierte el catálogo KEV de CISA en el snapshot embebido.
    ///
    /// Entrada: el JSON tal cual lo publica CISA. Salida: el mismo catálogo
    /// reducido a los campos que el informe justifica, comprimido con gzip por la
    /// misma razón que el índice CVE (flate2 descomprime en Rust puro).
    Kev {
        /// Catálogo descargado de cisa.gov.
        #[arg(long, default_value = "vendor/nvd/known_exploited_vulnerabilities.json")]
        input: PathBuf,
        /// Archivo de salida, versionado y embebido en el binario.
        #[arg(long, default_value = "core/src/data/kev.json.gz")]
        out: PathBuf,
    },
    /// Busca en el catálogo los productos cuyo nombre contiene un término.
    Find {
        /// Término a buscar en vendor o product.
        term: String,
        /// Catálogo generado por `catalog`.
        #[arg(long, default_value = "vendor/nvd/cpe-catalog.tsv")]
        catalog: PathBuf,
        /// Máximo de resultados.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Catalog { out } => catalog(&cli.snapshot, out),
        Command::Validate { catalog } => validate(catalog),
        Command::Build { out, max_description } => build(&cli.snapshot, out, *max_description),
        Command::Kev { input, out } => kev(input, out),
        Command::Find { term, catalog, limit } => find(term, catalog, *limit),
    }
}

// ---------------------------------------------------------------------------
// validate
// ---------------------------------------------------------------------------

/// Checks every curated entry against the extracted catalog.
fn validate(catalog_path: &PathBuf) -> Result<()> {
    let observed = read_catalog(catalog_path)?;
    let entries = munigpt_core::cve::product_map::all_identities();

    let mut missing: Vec<String> = Vec::new();
    let mut drifted: Vec<String> = Vec::new();

    for (slug, id) in &entries {
        let key = (id.part.clone(), id.vendor.clone(), id.product.clone());
        match observed.get(&key) {
            None => missing.push(format!(
                "  {slug}: {}:{}:{} NO existe en el snapshot",
                id.part, id.vendor, id.product
            )),
            Some(&count) => {
                // Un conteo que se mueve es normal (el snapshot avanza a diario);
                // que se desplome a cero o cambie de orden de magnitud, no.
                let recorded = id.cves_at_extraction;
                if count * 2 < recorded {
                    drifted.push(format!(
                        "  {slug}: {}:{} paso de {recorded} a {count} CVE",
                        id.vendor, id.product
                    ));
                }
            }
        }
    }

    println!("Entradas curadas verificadas : {}", entries.len());
    if !drifted.is_empty() {
        println!("\nConteos con caida sospechosa (revisar, no bloquea):");
        for d in &drifted {
            println!("{d}");
        }
    }
    if !missing.is_empty() {
        println!("\nEntradas que NO existen en los datos:");
        for m in &missing {
            println!("{m}");
        }
        bail!(
            "{} entrada(s) de la tabla curada no existen en el snapshot. \
             Un CPE que no esta en los datos produciria CVE falsas: corrige la tabla.",
            missing.len()
        );
    }
    println!("\nTodas las entradas existen en el snapshot.");
    Ok(())
}

/// Reads the extracted catalog into a lookup of (part, vendor, product) -> count.
fn read_catalog(path: &PathBuf) -> Result<BTreeMap<(String, String, String), u64>> {
    if !path.exists() {
        bail!(
            "no existe {}. Ejecuta primero: cargo run --release -p nvd-index -- catalog",
            path.display()
        );
    }
    let text = std::fs::read_to_string(path)?;
    let mut out = BTreeMap::new();
    for line in text.lines().skip(1) {
        let mut f = line.split('\t');
        let (Some(n), Some(part), Some(vendor), Some(product)) =
            (f.next(), f.next(), f.next(), f.next())
        else {
            continue;
        };
        let count: u64 = n.trim().parse().unwrap_or(0);
        out.insert(
            (part.to_string(), vendor.to_string(), product.to_string()),
            count,
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Snapshot streaming
// ---------------------------------------------------------------------------

/// Only the fields the converter needs. Everything else in the record is skipped
/// by serde without being materialised.
#[derive(Deserialize)]
struct CveItem {
    id: String,
    /// Fecha de publicación en NVD, ISO 8601. Habilita el filtro por nivel de
    /// parches del SO; sin ella el informe no puede distinguir un Windows al día
    /// de uno abandonado en 2021.
    #[serde(default)]
    published: Option<String>,
    #[serde(default)]
    configurations: Vec<Configuration>,
    #[serde(default)]
    descriptions: Vec<Description>,
    #[serde(default)]
    metrics: Metrics,
}

#[derive(Deserialize)]
struct Description {
    lang: String,
    value: String,
}

/// CVSS lives under a different key per version. Se prefiere v3.1, luego v3.0 y
/// por último v2; muchos registros recientes no traen ninguno, y eso se respeta
/// como `None` en vez de inventar un 0.
#[derive(Default, Deserialize)]
struct Metrics {
    #[serde(default, rename = "cvssMetricV31")]
    v31: Vec<MetricEntry>,
    #[serde(default, rename = "cvssMetricV30")]
    v30: Vec<MetricEntry>,
    #[serde(default, rename = "cvssMetricV2")]
    v2: Vec<MetricEntry>,
}

#[derive(Deserialize)]
struct MetricEntry {
    #[serde(rename = "cvssData")]
    cvss_data: CvssData,
    #[serde(default, rename = "baseSeverity")]
    base_severity: Option<String>,
}

#[derive(Deserialize)]
struct CvssData {
    #[serde(default, rename = "baseScore")]
    base_score: Option<f32>,
    #[serde(default, rename = "baseSeverity")]
    base_severity: Option<String>,
}

impl Metrics {
    fn best(&self) -> (Option<f32>, Option<String>) {
        for group in [&self.v31, &self.v30, &self.v2] {
            if let Some(m) = group.first() {
                let sev = m
                    .cvss_data
                    .base_severity
                    .clone()
                    .or_else(|| m.base_severity.clone());
                return (m.cvss_data.base_score, sev);
            }
        }
        (None, None)
    }
}

#[derive(Deserialize)]
struct Configuration {
    #[serde(default)]
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    #[serde(default, rename = "cpeMatch")]
    cpe_match: Vec<CpeMatchRaw>,
}

#[derive(Deserialize)]
struct CpeMatchRaw {
    #[serde(default)]
    vulnerable: bool,
    criteria: String,
    #[serde(default, rename = "versionStartIncluding")]
    version_start_including: Option<String>,
    #[serde(default, rename = "versionStartExcluding")]
    version_start_excluding: Option<String>,
    #[serde(default, rename = "versionEndIncluding")]
    version_end_including: Option<String>,
    #[serde(default, rename = "versionEndExcluding")]
    version_end_excluding: Option<String>,
}

/// Streams `cve_items`, handing each record to `on_item`.
///
/// Se implementa como `DeserializeSeed` sobre la secuencia para que serde no
/// acumule los 369.933 registros: cada uno se procesa y se descarta.
fn stream_items<F>(snapshot: &PathBuf, mut on_item: F) -> Result<u64>
where
    F: FnMut(CveItem),
{
    let file = File::open(snapshot)
        .with_context(|| format!("no se pudo abrir {}", snapshot.display()))?;
    let decoder = xz2::read::XzDecoder::new(BufReader::with_capacity(1 << 20, file));
    // El anidamiento del feed es poco profundo (feed > items > configurations >
    // nodes > cpeMatch), muy por debajo del límite de recursión de serde_json.
    let mut de = serde_json::Deserializer::from_reader(BufReader::with_capacity(1 << 20, decoder));

    struct Feed<'a, F>(&'a mut F);
    struct FeedVisitor<'a, F>(&'a mut F);
    struct Items<'a, F>(&'a mut F);
    struct ItemsVisitor<'a, F>(&'a mut F);

    impl<'de, F: FnMut(CveItem)> DeserializeSeed<'de> for Items<'_, F> {
        type Value = u64;
        fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<u64, D::Error> {
            d.deserialize_seq(ItemsVisitor(self.0))
        }
    }

    impl<'de, F: FnMut(CveItem)> Visitor<'de> for ItemsVisitor<'_, F> {
        type Value = u64;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "el arreglo cve_items")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<u64, A::Error> {
            let mut n = 0u64;
            while let Some(item) = seq.next_element::<CveItem>()? {
                (self.0)(item);
                n += 1;
                if n % 50_000 == 0 {
                    eprint!("\r  {n} CVE procesadas...");
                }
            }
            Ok(n)
        }
    }

    impl<'de, F: FnMut(CveItem)> DeserializeSeed<'de> for Feed<'_, F> {
        type Value = u64;
        fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<u64, D::Error> {
            d.deserialize_map(FeedVisitor(self.0))
        }
    }

    impl<'de, F: FnMut(CveItem)> Visitor<'de> for FeedVisitor<'_, F> {
        type Value = u64;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "el objeto raíz del feed")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<u64, A::Error> {
            let mut count = 0u64;
            while let Some(key) = map.next_key::<String>()? {
                if key == "cve_items" {
                    count = map.next_value_seed(Items(self.0))?;
                } else {
                    let _ = map.next_value::<serde::de::IgnoredAny>()?;
                }
            }
            Ok(count)
        }
    }

    let mut f = on_item;
    let count = Feed(&mut f)
        .deserialize(&mut de)
        .context("el snapshot no tiene la forma esperada")?;
    eprintln!("\r  {count} CVE procesadas. ");
    Ok(count)
}

// ---------------------------------------------------------------------------
// catalog
// ---------------------------------------------------------------------------

/// Extracts every distinct part/vendor/product with its CVE count.
fn catalog(snapshot: &PathBuf, out: &PathBuf) -> Result<()> {
    eprintln!("Leyendo {} ...", snapshot.display());

    // (part, vendor, product) -> nº de CVE distintas que lo mencionan.
    let mut counts: BTreeMap<(String, String, String), u64> = BTreeMap::new();
    let mut skipped_unparseable = 0u64;

    let total = stream_items(snapshot, |item| {
        let mut seen: Vec<(String, String, String)> = Vec::new();
        for cfg in &item.configurations {
            for node in &cfg.nodes {
                for cm in &node.cpe_match {
                    if !cm.vulnerable {
                        continue;
                    }
                    match munigpt_core::cve::cpe::Cpe::parse(&cm.criteria) {
                        Some(c) => {
                            let key = (c.part, c.vendor, c.product);
                            if !seen.contains(&key) {
                                seen.push(key);
                            }
                        }
                        None => skipped_unparseable += 1,
                    }
                }
            }
        }
        let _ = &item.id;
        for key in seen {
            *counts.entry(key).or_insert(0) += 1;
        }
    })?;

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let mut w = BufWriter::new(File::create(out)?);
    writeln!(w, "# conteo\tpart\tvendor\tproduct")?;
    // Ordenado por conteo descendente: lo más frecuente primero.
    let mut rows: Vec<_> = counts.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    for ((part, vendor, product), n) in rows {
        writeln!(w, "{n}\t{part}\t{vendor}\t{product}")?;
    }
    w.flush()?;

    eprintln!("CVE totales           : {total}");
    eprintln!("Pares vendor:product  : {}", counts.len());
    eprintln!("CPE no parseables     : {skipped_unparseable}");
    eprintln!("Catálogo escrito en   : {}", out.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// build
// ---------------------------------------------------------------------------

/// Filters the snapshot down to the curated products and writes the shipped index.
///
/// Se conserva **solo** lo que el informe necesita. Las descripciones se truncan
/// porque son la mayor parte del volumen y el plan de remediación se apoya en el
/// identificador, el puntaje y la explotación observada, no en el texto completo.
fn build(snapshot: &PathBuf, out: &PathBuf, max_description: usize) -> Result<()> {
    use std::collections::HashSet;

    // Conjunto curado: (part, vendor, product).
    let curated: HashSet<(String, String, String)> =
        munigpt_core::cve::product_map::all_identities()
            .into_iter()
            .map(|(_, id)| (id.part.clone(), id.vendor.clone(), id.product.clone()))
            .collect();
    eprintln!("Productos curados: {}", curated.len());

    let mut kept: Vec<munigpt_core::cve::matcher::CveRecord> = Vec::new();
    let mut with_cvss = 0u64;

    let total = stream_items(snapshot, |item| {
        let mut matches: Vec<munigpt_core::cve::matcher::CpeMatch> = Vec::new();

        for cfg in &item.configurations {
            for node in &cfg.nodes {
                for cm in &node.cpe_match {
                    if !cm.vulnerable {
                        continue;
                    }
                    let Some(cpe) = munigpt_core::cve::cpe::Cpe::parse(&cm.criteria) else {
                        continue;
                    };
                    let key = (cpe.part.clone(), cpe.vendor.clone(), cpe.product.clone());
                    if !curated.contains(&key) {
                        continue;
                    }
                    matches.push(munigpt_core::cve::matcher::CpeMatch {
                        criteria: cpe,
                        range: munigpt_core::cve::matcher::VersionRange {
                            start_including: cm.version_start_including.clone(),
                            start_excluding: cm.version_start_excluding.clone(),
                            end_including: cm.version_end_including.clone(),
                            end_excluding: cm.version_end_excluding.clone(),
                        },
                        vulnerable: true,
                    });
                }
            }
        }

        if matches.is_empty() {
            return;
        }

        let (cvss, severity) = item.metrics.best();
        if cvss.is_some() {
            with_cvss += 1;
        }
        let description = item
            .descriptions
            .iter()
            .find(|d| d.lang == "en")
            .map(|d| truncate(&d.value, max_description));

        kept.push(munigpt_core::cve::matcher::CveRecord {
            id: item.id.clone(),
            cvss,
            severity,
            description,
            // Solo el día: la hora no aporta a un filtro de acumulativos mensuales
            // y multiplicaría el tamaño del índice embebido sin razón.
            published: item.published.as_ref().map(|p| {
                p.split(['T', ' ']).next().unwrap_or(p).to_string()
            }),
            matches,
        });
    })?;

    kept.sort_by(|a, b| a.id.cmp(&b.id));

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let file = File::create(out)?;
    let mut enc = flate2::write::GzEncoder::new(BufWriter::new(file), flate2::Compression::best());
    serde_json::to_writer(&mut enc, &kept).context("no se pudo serializar el indice")?;
    enc.finish()?.flush()?;

    let size = std::fs::metadata(out)?.len();
    eprintln!("CVE en el snapshot        : {total}");
    eprintln!("CVE que tocan lo curado   : {}", kept.len());
    eprintln!("  de ellas con CVSS       : {with_cvss}");
    eprintln!("  sin CVSS (politica NVD) : {}", kept.len() as u64 - with_cvss);
    eprintln!("Indice escrito en         : {} ({:.1} MB)", out.display(), size as f64 / 1_048_576.0);
    Ok(())
}

/// Truncates on a character boundary, never mid-UTF-8.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push_str("...");
    out
}

// ---------------------------------------------------------------------------
// kev
// ---------------------------------------------------------------------------

/// Converts CISA's published KEV catalogue into the snapshot shipped in the binary.
///
/// El parseo lo hace `munigpt_core::cve::kev::from_cisa_json`, la misma función que
/// usa la app al leer un catálogo puesto en disco: así el archivo externo y el
/// embebido no pueden interpretarse distinto.
fn kev(input: &PathBuf, out: &PathBuf) -> Result<()> {
    if !input.exists() {
        bail!(
            "no existe {}. Descárgalo de \
             https://www.cisa.gov/sites/default/files/feeds/known_exploited_vulnerabilities.json",
            input.display()
        );
    }

    let catalogue = munigpt_core::cve::kev::from_cisa_file(input)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if catalogue.entries.is_empty() {
        bail!("el catálogo no trae vulnerabilidades: no se escribe un snapshot vacío");
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let file = File::create(out)?;
    let mut enc = flate2::write::GzEncoder::new(BufWriter::new(file), flate2::Compression::best());
    serde_json::to_writer(&mut enc, &catalogue).context("no se pudo serializar el catálogo")?;
    enc.finish()?.flush()?;

    let ransomware = catalogue.entries.iter().filter(|e| e.ransomware).count();
    let size = std::fs::metadata(out)?.len();
    eprintln!("Versión del catálogo   : {}", catalogue.catalog_version);
    eprintln!("Publicado              : {}", catalogue.date_released);
    eprintln!("Vulnerabilidades       : {}", catalogue.entries.len());
    eprintln!("  usadas en ransomware : {ransomware}");
    eprintln!("Snapshot escrito en    : {} ({:.0} KB)", out.display(), size as f64 / 1024.0);
    Ok(())
}

// ---------------------------------------------------------------------------
// find
// ---------------------------------------------------------------------------

/// Searches the extracted catalog, so a CPE name is looked up rather than guessed.
fn find(term: &str, catalog: &PathBuf, limit: usize) -> Result<()> {
    if !catalog.exists() {
        bail!(
            "no existe {}. Ejecuta primero: cargo run -p nvd-index -- catalog",
            catalog.display()
        );
    }
    let text = std::fs::read_to_string(catalog)?;
    let needle = term.to_ascii_lowercase();

    let mut shown = 0usize;
    println!("{:>8}  {:<4} {:<28} {}", "CVEs", "part", "vendor", "product");
    for line in text.lines().skip(1) {
        let mut f = line.split('\t');
        let (Some(n), Some(part), Some(vendor), Some(product)) =
            (f.next(), f.next(), f.next(), f.next())
        else {
            continue;
        };
        if vendor.contains(&needle) || product.contains(&needle) {
            println!("{n:>8}  {part:<4} {vendor:<28} {product}");
            shown += 1;
            if shown >= limit {
                break;
            }
        }
    }
    if shown == 0 {
        println!("(sin coincidencias para {term:?})");
    }
    Ok(())
}
