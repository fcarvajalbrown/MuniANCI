//! Generates PDF informe de brechas and CSIRT JSON from a ScanResult.
use crate::types::{AppliesTo, Exigibilidad, ScanResult, Severity, Tier};
use anyhow::{Context, Result};
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};
use std::fs::File;
use std::io::BufWriter;

// A4 in points (1mm = 2.8346pt)
const PW: f64 = 595.0;
const PH: f64 = 842.0;
const MARGIN: f64 = 51.0; // ~18mm
const LINE: f64 = 13.5;

const UTM_LEVE_OIV: u32      = 10_000;
const UTM_GRAVE_OIV: u32     = 20_000;
const UTM_GRAVISIMA_OIV: u32 = 40_000;
const UTM_LEVE_PSE: u32      =  5_000;
const UTM_GRAVE_PSE: u32     = 10_000;
const UTM_GRAVISIMA_PSE: u32 = 20_000;

pub fn build(result: &ScanResult, pdf_path: &str, json_path: &str, progress_cb: impl Fn(u8)) -> Result<()> {
    progress_cb(0);
    write_json(result, json_path)?;
    progress_cb(40);
    write_pdf(result, pdf_path)?;
    progress_cb(100);
    Ok(())
}

fn write_json(result: &ScanResult, path: &str) -> Result<()> {
    let file = File::create(path).with_context(|| format!("cannot create {path}"))?;
    serde_json::to_writer_pretty(file, result).context("JSON serialisation failed")?;
    Ok(())
}

pub fn write_pdf(result: &ScanResult, path: &str) -> Result<()> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    // Register three Type1 builtin fonts
    let f_regular = doc.add_object(dictionary! {
        "Type"     => "Font",
        "Subtype"  => "Type1",
        "BaseFont" => "Helvetica",
    });
    let f_bold = doc.add_object(dictionary! {
        "Type"     => "Font",
        "Subtype"  => "Type1",
        "BaseFont" => "Helvetica-Bold",
    });
    let f_mono = doc.add_object(dictionary! {
        "Type"     => "Font",
        "Subtype"  => "Type1",
        "BaseFont" => "Courier",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "FR" => f_regular,
            "FB" => f_bold,
            "FM" => f_mono,
        },
    });

    // Build page content as a list of PDF operations
    let mut ops: Vec<Operation> = Vec::new();
    let mut y = PH - MARGIN - 20.0; // start near top

    // Helper: emit BT ... ET block for a single line of text
    // font: "FR"|"FB"|"FM", size: pt, x/y: pt from bottom-left
    macro_rules! line {
        ($font:expr, $size:expr, $x:expr, $y:expr, $text:expr) => {{
            ops.push(Operation::new("BT", vec![]));
            ops.push(Operation::new("Tf", vec![$font.into(), ($size as i64).into()]));
            ops.push(Operation::new("Td", vec![($x as i64).into(), ($y as i64).into()]));
            ops.push(Operation::new("Tj", vec![Object::string_literal(to_pdf_safe($text).as_bytes())]));
            ops.push(Operation::new("ET", vec![]))
        }};
    }

    // Header
    line!("FB", 16, MARGIN, y, "INFORME DE BRECHAS DE CIBERSEGURIDAD");
    y -= 22.0;
    line!("FR", 10, MARGIN, y, &format!("Institucion: {}", result.meta.institution_name));
    y -= LINE;
    line!("FR", 9, MARGIN, y, &format!("Clasificacion: {}  |  Fecha: {}",
        result.meta.tier, result.scanned_at.format("%d/%m/%Y %H:%M UTC")));
    y -= LINE + 6.0;

    // Legal disclaimer
    line!("FB", 9, MARGIN, y, "AVISO LEGAL:");
    y -= LINE;
    for l in [
        "Generado con fines de auditoria interna - Ley 21.663.",
        "Uso en redes del Estado requiere inscripcion ANCI (Art. 2 Ley 21.459).",
        "Clasificar como RESERVADO.",
    ] {
        line!("FM", 7, MARGIN, y, l);
        y -= LINE - 2.0;
    }
    y -= 6.0;

    // Summary
    let exigibles: Vec<_> = result.gaps.iter()
        .filter(|g| g.exigibilidad == Exigibilidad::Exigible)
        .collect();
    let madurez: Vec<_> = result.gaps.iter()
        .filter(|g| g.exigibilidad == Exigibilidad::MadurezVoluntaria)
        .collect();
    let critical = exigibles.iter().filter(|g| g.severity == Severity::Critical).count();
    let high     = exigibles.iter().filter(|g| g.severity == Severity::High).count();
    let medium   = exigibles.iter().filter(|g| g.severity == Severity::Medium).count();
    let csirt    = result.gaps.iter().filter(|g| g.requires_csirt_report).count();
    let score    = &result.score;

    line!("FB", 11, MARGIN, y, "RESUMEN EJECUTIVO");
    y -= LINE;
    for l in [
        format!("Puntaje de cumplimiento: {} de {} (base menos deducciones ponderadas)",
            score.score, score.base),
        format!("Brechas exigibles: {}  (Criticas: {}  Altas: {}  Medias: {})",
            exigibles.len(), critical, high, medium),
        format!("Brechas de madurez voluntaria (no exigibles a esta institucion): {}", madurez.len()),
        format!("Con reporte CSIRT obligatorio (Art. 9): {}", csirt),
        format!("Hosts: {}  Servicios: {}  Unidades: {}",
            result.asset_graph.hosts.len(), result.asset_graph.services.len(), result.asset_graph.drives.len()),
        // Se declara siempre: un informe que no dice cuanto NO pudo evaluar
        // induce a leer los huecos como ausencia de problemas.
        format!("Cobertura CVE: {}", result.cve_coverage),
        // Marcar una CVE como "explotada activamente" es una afirmacion fuerte:
        // se declara contra que catalogo, y de que fecha, se hizo.
        result.kev_provenance.clone(),
    ] {
        line!("FR", 9, MARGIN, y, &l);
        y -= LINE;
    }
    if csirt > 0 {
        y -= 3.0;
        line!("FB", 9, MARGIN, y, "*** ATENCION: Reportar al CSIRT Nacional en max. 3 horas (Art. 9) ***");
        y -= LINE;
    }
    y -= 8.0;

    // Maturity per domain. Dice DONDE esta el problema, que es lo que el puntaje
    // agregado no puede decir: un 82/100 puede ser cinco dominios sanos y uno roto.
    line!("FB", 11, MARGIN, y, "MADUREZ POR DOMINIO (0 a 3)");
    y -= LINE;
    match result.maturity.average() {
        Some(avg) => line!("FR", 9, MARGIN, y,
            &format!("Promedio: {avg:.1} de 3, sobre {} dominio(s) medido(s).",
                result.maturity.domains.len() - result.maturity.unmeasured().len())),
        None => line!("FR", 9, MARGIN, y, "Ningun dominio pudo medirse en este escaneo."),
    }
    y -= LINE;
    for d in &result.maturity.domains {
        if y < MARGIN + 70.0 { break; }
        line!("FB", 9, MARGIN, y, &format!("{}  -  {}", d.level, d.domain));
        y -= LINE - 2.0;
        line!("FM", 7, MARGIN, y, &format!("   {} | {}", d.domain.legal_anchor(), d.rationale));
        y -= LINE - 1.0;
    }
    if !result.maturity.unmeasured().is_empty() {
        // Mismo criterio que la cobertura CVE: un dominio sin datos no es un
        // dominio en cero, y el informe tiene que decirlo en vez de insinuarlo.
        line!("FM", 7, MARGIN, y,
            "Los dominios \"No medido\" quedan fuera del promedio: no se recogieron datos, no son un incumplimiento.");
        y -= LINE;
    }
    y -= 8.0;

    // Gaps — first what is legally binding, then what is voluntary maturity.
    // The distinction is the whole point: for an institution that is not an OIV,
    // calling an Art. 8 item "no cumple" would assert a breach that does not exist.
    let mut shown = 0usize;
    let total = result.gaps.len();

    for (titulo, grupo, nota) in [
        ("BRECHAS EXIGIBLES", &exigibles,
         "Obligaciones vigentes para esta institucion."),
        ("MADUREZ VOLUNTARIA (NO EXIGIBLE)", &madurez,
         "Deberes del Art. 8, exigibles solo a los OIV. Se miden como referencia."),
    ] {
        if grupo.is_empty() { continue; }
        if y < MARGIN + 80.0 { break; }
        line!("FB", 11, MARGIN, y, titulo);
        y -= LINE;
        line!("FM", 7, MARGIN, y, nota);
        y -= LINE;

        for (i, gap) in grupo.iter().enumerate() {
            if y < MARGIN + 70.0 { break; } // overflow guard — v0.2 adds pages
            let sev = match gap.severity {
                Severity::Critical => "[CRITICO]",
                Severity::High     => "[ALTO]",
                Severity::Medium   => "[MEDIO]",
            };
            let csirt_tag = if gap.requires_csirt_report { " *** CSIRT ***" } else { "" };
            line!("FB", 9, MARGIN, y, &format!("{}. {} {}{}", i + 1, sev, gap.control, csirt_tag));
            y -= LINE;
            line!("FR", 8, MARGIN, y, &format!("   Hallazgo:  {}", gap.finding));
            y -= LINE;
            line!("FM", 8, MARGIN, y, &format!("   Ancla:     {}", gap.legal_anchor));
            y -= LINE;
            let clasif = match gap.infraction_class {
                Some(c) => format!("{c}"),
                None    => "sin clasificacion legal (criterio tecnico)".into(),
            };
            line!("FR", 8, MARGIN, y, &format!("   Aplica a:  {}  |  Infraccion: {}",
                applies_to_label(&gap.applies_to), clasif));
            y -= LINE;
            if !gap.evidence.is_empty() {
                let ev = gap.evidence.join(", ");
                let ev_d = if ev.len() > 80 { format!("{}...", &ev[..80]) } else { ev };
                line!("FM", 8, MARGIN, y, &format!("   Evidencia: {}", ev_d));
                y -= LINE;
            }
            y -= 4.0;
            shown += 1;
        }
        y -= 4.0;
    }

    // Never let the page limit hide findings without saying so.
    if shown < total && y > MARGIN + 30.0 {
        line!("FB", 8, MARGIN, y, &format!(
            "NOTA: se muestran {shown} de {total} brechas por limite de pagina. El JSON las incluye todas."));
        y -= LINE;
    }

    // UTM table
    let (leve, grave, gravisima) = match result.meta.tier {
        Tier::Oiv => (UTM_LEVE_OIV, UTM_GRAVE_OIV, UTM_GRAVISIMA_OIV),
        _         => (UTM_LEVE_PSE, UTM_GRAVE_PSE,  UTM_GRAVISIMA_PSE),
    };
    if y > MARGIN + 50.0 {
        y -= 6.0;
        line!("FB", 10, MARGIN, y, "ESCALA DE SANCIONES (Art. 40 Ley 21.663)");
        y -= LINE;
        for (label, utm) in [("Leve", leve), ("Grave", grave), ("Gravisima", gravisima)] {
            line!("FR", 8, MARGIN, y, &format!("  {:<12} hasta {:>6} UTM", label, utm));
            y -= LINE;
        }
        line!("FM", 7, MARGIN, y, "1 UTM aprox. CLP $66.000 - verificar en SII.");
    }

    // Attribution notices required by the NVD and CVE Program terms of use.
    // No son opcionales: son condicion de la licencia bajo la que se redistribuyen
    // esos datos dentro del producto.
    line!("FM", 6, MARGIN, 42.0, crate::maturity::ESSENTIAL_EIGHT_ATTRIBUTION);
    line!("FM", 6, MARGIN, 34.0, crate::cve::NVD_NOTICE);
    line!("FM", 6, MARGIN, 26.0, crate::cve::CVE_NOTICE);

    // Footer — pinned near bottom
    line!("FM", 7, MARGIN, 18.0,
        &format!("MuniANCI v{} - Felipe Carvajal Brown - uso interno reservado",
            env!("CARGO_PKG_VERSION")));

    // Encode content stream and assemble page
    let content = Content { operations: ops };
    let content_id = doc.add_object(
        Stream::new(dictionary! {}, content.encode().context("content encode failed")?)
    );
    let page_id = doc.add_object(dictionary! {
        "Type"      => "Page",
        "Parent"    => pages_id,
        "MediaBox"  => vec![0.into(), 0.into(), PW.into(), PH.into()],
        "Contents"  => content_id,
        "Resources" => resources_id,
    });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
        "Type"  => "Pages",
        "Kids"  => vec![page_id.into()],
        "Count" => 1i64,
    }));
    let catalog_id = doc.add_object(dictionary! {
        "Type"  => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let file = File::create(path).with_context(|| format!("cannot create {path}"))?;
    doc.save_to(&mut BufWriter::new(file)).context("PDF save failed")?;
    Ok(())
}

fn applies_to_label(a: &AppliesTo) -> &'static str {
    match a {
        AppliesTo::All       => "Todos",
        AppliesTo::OivAndPse => "PSE y OIV",
        AppliesTo::Oiv       => "Solo OIV",
    }
}

/// Converts UTF-8 to WinAnsiEncoding-safe ASCII.
/// printpdf builtin Type1 fonts use WinAnsiEncoding — multi-byte UTF-8
/// sequences corrupt if passed raw. Maps common Spanish/legal chars to
/// ASCII equivalents so the PDF renders cleanly.
fn to_pdf_safe(s: &str) -> String {
    s.chars().map(|c| match c {
        'á' | 'à' | 'ä' | 'â' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'Á' | 'À' | 'Ä' | 'Â' => 'A',
        'É' | 'È' | 'Ë' | 'Ê' => 'E',
        'Í' | 'Ì' | 'Ï' | 'Î' => 'I',
        'Ó' | 'Ò' | 'Ö' | 'Ô' => 'O',
        'Ú' | 'Ù' | 'Ü' | 'Û' => 'U',
        'ñ' => 'n',
        'Ñ' => 'N',
        '¿' => '?',
        '¡' => '!',
        '°' => ' ',
        '—' => '-',
        '\u{2019}' | '\u{2018}' => '\'',
        _ if c.is_ascii() => c,
        _ => '?',
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssetGraph, ScanMeta, ScanResult, Scope};
    use chrono::Utc;

    fn dummy() -> ScanResult {
        ScanResult {
            meta: ScanMeta {
                institution_name: "Municipalidad de Prueba".into(),
                tier:  Tier::Pse,
                scope: Scope::Local,
            },
            asset_graph: AssetGraph::default(),
            gaps:        vec![],
            cve_coverage: crate::cve::Coverage::default(),
            kev_provenance: crate::cve::kev::catalogue().provenance(),
            score:       crate::scoring::ComplianceScore::from_gaps(&[]),
            maturity:    crate::maturity::MaturityProfile::from_gaps(&[], &[]),
            scanned_at:  Utc::now(),
        }
    }

    #[test]
    fn json_roundtrip() {
        let r = dummy();
        let tmp = std::env::temp_dir().join("muniani_test.json");
        write_json(&r, tmp.to_str().unwrap()).unwrap();
        assert!(std::fs::read_to_string(&tmp).unwrap().contains("Municipalidad de Prueba"));
    }

    #[test]
    fn utm_scale() {
        assert_eq!(UTM_LEVE_OIV, 10_000);
        assert_eq!(UTM_GRAVISIMA_OIV, 40_000);
    }

    /// Renders the PDF and returns the decoded text of its content stream.
    fn pdf_text(result: &ScanResult, name: &str) -> String {
        let tmp = std::env::temp_dir().join(name);
        write_pdf(result, tmp.to_str().unwrap()).unwrap();
        let doc = Document::load(&tmp).unwrap();
        let (_, page_id) = doc.get_pages().into_iter().next().unwrap();
        String::from_utf8_lossy(&doc.get_page_content(page_id).unwrap()).into_owned()
    }

    // El PDF es el entregable que sale de la maquina: que una seccion compile no
    // prueba que llegue a la pagina. Se lee el contenido ya renderizado.
    #[test]
    fn the_pdf_carries_the_maturity_section() {
        let mut r = dummy();
        r.maturity = crate::maturity::MaturityProfile::from_gaps(
            &[],
            &[crate::maturity::Domain::MedidasPermanentes],
        );
        let text = pdf_text(&r, "muniani_test_madurez.pdf");
        assert!(text.contains("MADUREZ POR DOMINIO"), "falta el titulo");
        assert!(text.contains("Nivel 3"), "falta el nivel del dominio medido");
        assert!(text.contains("No medido"), "falta la marca de dominio sin datos");
        assert!(text.contains("Promedio"), "falta el promedio");
    }

    // La atribucion CC BY del modelo del ASD es condicion de la licencia bajo la
    // que se adapta la escala, no un adorno.
    #[test]
    fn the_pdf_carries_the_required_attributions() {
        let text = pdf_text(&dummy(), "muniani_test_avisos.pdf");
        assert!(text.contains("Essential Eight"), "falta la atribucion CC BY del ASD");
        assert!(text.contains("NVD"), "falta el aviso de NVD");
        assert!(text.contains("MITRE"), "falta el aviso del CVE Program");
    }
}