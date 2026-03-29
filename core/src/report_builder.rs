//! Generates PDF informe de brechas and CSIRT JSON from a ScanResult.
use crate::types::{AppliesTo, ScanResult, Severity, Tier};
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

fn write_pdf(result: &ScanResult, path: &str) -> Result<()> {
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
            ops.push(Operation::new("Tj", vec![Object::string_literal($text.as_bytes())]));
            ops.push(Operation::new("ET", vec![]));
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
    let critical = result.gaps.iter().filter(|g| g.severity == Severity::Critical).count();
    let high     = result.gaps.iter().filter(|g| g.severity == Severity::High).count();
    let medium   = result.gaps.iter().filter(|g| g.severity == Severity::Medium).count();
    let csirt    = result.gaps.iter().filter(|g| g.requires_csirt_report).count();

    line!("FB", 11, MARGIN, y, "RESUMEN EJECUTIVO");
    y -= LINE;
    for l in [
        format!("Total brechas: {}  (Criticas: {}  Altas: {}  Medias: {})", result.gaps.len(), critical, high, medium),
        format!("Con reporte CSIRT obligatorio (Art. 9): {}", csirt),
        format!("Hosts: {}  Servicios: {}  Unidades: {}",
            result.asset_graph.hosts.len(), result.asset_graph.services.len(), result.asset_graph.drives.len()),
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

    // Gaps
    line!("FB", 11, MARGIN, y, "BRECHAS DETECTADAS");
    y -= LINE;
    for (i, gap) in result.gaps.iter().enumerate() {
        if y < MARGIN + 60.0 { break; } // overflow guard — v0.2 adds pages
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
        line!("FR", 8, MARGIN, y, &format!("   Aplica a:  {}", applies_to_label(&gap.applies_to)));
        y -= LINE;
        if !gap.evidence.is_empty() {
            let ev = gap.evidence.join(", ");
            let ev_d = if ev.len() > 80 { format!("{}...", &ev[..80]) } else { ev };
            line!("FM", 8, MARGIN, y, &format!("   Evidencia: {}", ev_d));
            y -= LINE;
        }
        y -= 4.0;
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

    // Footer — pinned near bottom
    line!("FM", 7, MARGIN, 18.0,
        "MuniANCI v0.1 - Felipe Carvajal Brown Software - uso interno reservado");

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
}