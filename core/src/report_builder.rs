//! Generates PDF informe de brechas and CSIRT JSON from a ScanResult.
use crate::types::{AppliesTo, Gap, ScanResult, Severity, Tier};
use anyhow::{Context, Result};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

// Page geometry (A4 portrait, millimetres).
const W: f32 = 210.0;
const H: f32 = 297.0;
const MARGIN: f32 = 18.0;
const LINE: f32 = 6.0;

// UTM fine scale per Art. 40° Ley 21.663 (OIV figures).
const UTM_LEVE_OIV: u32     = 10_000;
const UTM_GRAVE_OIV: u32    = 20_000;
const UTM_GRAVISIMA_OIV: u32= 40_000;
const UTM_LEVE_PSE: u32     =  5_000;
const UTM_GRAVE_PSE: u32    = 10_000;
const UTM_GRAVISIMA_PSE: u32= 20_000;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Writes the PDF gap report to `pdf_path` and the CSIRT JSON to `json_path`.
pub fn build(
    result: &ScanResult,
    pdf_path: &str,
    json_path: &str,
    progress_cb: impl Fn(u8),
) -> Result<()> {
    progress_cb(0);
    write_json(result, json_path)?;
    progress_cb(40);
    write_pdf(result, pdf_path)?;
    progress_cb(100);
    Ok(())
}

// ---------------------------------------------------------------------------
// JSON — CSIRT report format
// ---------------------------------------------------------------------------

/// Serialises the scan result to a structured JSON file for CSIRT Chile.
fn write_json(result: &ScanResult, path: &str) -> Result<()> {
    let file = File::create(path)
        .with_context(|| format!("cannot create {path}"))?;
    serde_json::to_writer_pretty(file, result)
        .context("JSON serialisation failed")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// PDF — informe de brechas
// ---------------------------------------------------------------------------

fn write_pdf(result: &ScanResult, path: &str) -> Result<()> {
    let (doc, page1, layer1) = PdfDocument::new(
        "Informe de Brechas MuniANCI",
        Mm(W), Mm(H),
        "Página 1",
    );

    let font_regular = doc.add_builtin_font(BuiltinFont::Helvetica)?;
    let font_bold    = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;
    let font_mono    = doc.add_builtin_font(BuiltinFont::Courier)?;

    let mut ctx = PageCtx {
        doc: &doc,
        font_regular: &font_regular,
        font_bold: &font_bold,
        font_mono: &font_mono,
        page: page1,
        layer: layer1,
        y: H - MARGIN,
        page_num: 1,
    };

    draw_header(&mut ctx, result)?;
    draw_legal_disclaimer(&mut ctx)?;
    draw_summary(&mut ctx, result)?;
    draw_gaps(&mut ctx, result)?;
    draw_utm_table(&mut ctx, result.config.tier)?;
    draw_footer(&mut ctx)?;

    let file = File::create(path)
        .with_context(|| format!("cannot create {path}"))?;
    doc.save(&mut BufWriter::new(file))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Page state helper
// ---------------------------------------------------------------------------

struct PageCtx<'a> {
    doc:          &'a PdfDocumentReference,
    font_regular: &'a IndirectFontRef,
    font_bold:    &'a IndirectFontRef,
    font_mono:    &'a IndirectFontRef,
    page:         PdfPageIndex,
    layer:        PdfLayerIndex,
    y:            f32,
    page_num:     u32,
}

impl<'a> PageCtx<'a> {
    fn current_layer(&self) -> PdfLayerReference {
        self.doc.get_page(self.page).get_layer(self.layer)
    }

    fn advance(&mut self, mm: f32) {
        self.y -= mm;
    }

    // Adds a new page and resets cursor. Call before any draw that might overflow.
    fn new_page(&mut self) {
        self.page_num += 1;
        let (page, layer) = self.doc.add_page(
            Mm(W), Mm(H),
            format!("Página {}", self.page_num),
        );
        self.page  = page;
        self.layer = layer;
        self.y     = H - MARGIN;
    }

    fn ensure_space(&mut self, needed: f32) {
        if self.y - needed < MARGIN {
            self.new_page();
        }
    }

    fn write_line(&mut self, text: &str, font: &IndirectFontRef, size: f32, color: Color) {
        self.ensure_space(LINE + 2.0);
        let layer = self.current_layer();
        layer.use_text(text, size, Mm(MARGIN), Mm(self.y), font);
        self.advance(LINE);
    }

    fn hline(&mut self, thickness: f32) {
        let layer = self.current_layer();
        let pts = vec![
            (Point::new(Mm(MARGIN), Mm(self.y)), false),
            (Point::new(Mm(W - MARGIN), Mm(self.y)), false),
        ];
        let line = Line { points: pts, is_closed: false };
        layer.set_outline_color(Color::Greyscale(Greyscale::new(0.6, None)));
        layer.set_outline_thickness(thickness as f64);
        layer.add_line(line);
        self.advance(2.0);
    }
}

// ---------------------------------------------------------------------------
// Section renderers
// ---------------------------------------------------------------------------

fn draw_header(ctx: &mut PageCtx, result: &ScanResult) -> Result<()> {
    let layer = ctx.current_layer();
    layer.use_text(
        "INFORME DE BRECHAS DE CIBERSEGURIDAD",
        16.0, Mm(MARGIN), Mm(ctx.y),
        ctx.font_bold,
    );
    ctx.advance(9.0);

    layer.use_text(
        &format!("Institución: {}", result.config.institution_name),
        10.0, Mm(MARGIN), Mm(ctx.y), ctx.font_regular,
    );
    ctx.advance(LINE);

    layer.use_text(
        &format!(
            "Clasificación: {}   |   Alcance: {:?}   |   Fecha: {}",
            result.config.tier,
            result.config.scope,
            result.scanned_at.format("%d/%m/%Y %H:%M UTC"),
        ),
        9.0, Mm(MARGIN), Mm(ctx.y), ctx.font_regular,
    );
    ctx.advance(LINE);
    ctx.hline(0.5);
    Ok(())
}

fn draw_legal_disclaimer(ctx: &mut PageCtx) -> Result<()> {
    ctx.ensure_space(20.0);
    ctx.write_line("AVISO LEGAL", ctx.font_bold, 9.0, Color::Greyscale(Greyscale::new(0.0, None)));

    let lines = [
        "Este informe fue generado con fines de auditoría interna conforme a Ley 21.663.",
        "El uso de esta herramienta en redes de organismos del Estado requiere inscripción",
        "previa en la ANCI y notificación conforme Art. 2° Ley 21.459 (safe harbor).",
        "Este documento contiene información sensible — clasificar como RESERVADO.",
    ];
    for line in lines {
        ctx.write_line(line, ctx.font_mono, 7.5, Color::Greyscale(Greyscale::new(0.3, None)));
    }
    ctx.advance(3.0);
    ctx.hline(0.3);
    Ok(())
}

fn draw_summary(ctx: &mut PageCtx, result: &ScanResult) -> Result<()> {
    ctx.ensure_space(30.0);
    ctx.write_line("RESUMEN EJECUTIVO", ctx.font_bold, 11.0, Color::Greyscale(Greyscale::new(0.0, None)));
    ctx.advance(1.0);

    let critical = result.gaps.iter().filter(|g| g.severity == Severity::Critical).count();
    let high     = result.gaps.iter().filter(|g| g.severity == Severity::High).count();
    let medium   = result.gaps.iter().filter(|g| g.severity == Severity::Medium).count();
    let csirt    = result.gaps.iter().filter(|g| g.requires_csirt_report).count();

    let summary_lines = [
        format!("Total de brechas detectadas: {}", result.gaps.len()),
        format!("  Críticas: {}   Altas: {}   Medias: {}", critical, high, medium),
        format!("  Brechas con reporte CSIRT obligatorio (Art. 9°): {}", csirt),
        format!("  Hosts descubiertos: {}", result.asset_graph.hosts.len()),
        format!("  Servicios detectados: {}", result.asset_graph.services.len()),
        format!("  Unidades de almacenamiento: {}", result.asset_graph.drives.len()),
    ];
    for line in &summary_lines {
        ctx.write_line(line, ctx.font_regular, 9.5, Color::Greyscale(Greyscale::new(0.0, None)));
    }

    if csirt > 0 {
        ctx.advance(2.0);
        ctx.write_line(
            "*** ATENCIÓN: Se detectaron brechas que requieren notificación al CSIRT",
            ctx.font_bold, 9.0, Color::Greyscale(Greyscale::new(0.0, None)),
        );
        ctx.write_line(
            "    Nacional en plazo máximo de 3 horas desde conocimiento (Art. 9° Ley 21.663).",
            ctx.font_bold, 9.0, Color::Greyscale(Greyscale::new(0.0, None)),
        );
    }

    ctx.advance(3.0);
    ctx.hline(0.3);
    Ok(())
}

fn draw_gaps(ctx: &mut PageCtx, result: &ScanResult) -> Result<()> {
    ctx.write_line("BRECHAS DETECTADAS", ctx.font_bold, 11.0, Color::Greyscale(Greyscale::new(0.0, None)));
    ctx.advance(1.0);

    for (i, gap) in result.gaps.iter().enumerate() {
        ctx.ensure_space(40.0);

        let sev_label = match gap.severity {
            Severity::Critical => "[CRÍTICO]",
            Severity::High     => "[ALTO]",
            Severity::Medium   => "[MEDIO]",
        };

        let csirt_tag = if gap.requires_csirt_report { " *** REPORTAR A CSIRT ***" } else { "" };

        ctx.write_line(
            &format!("{}. {} {}{}", i + 1, sev_label, gap.control, csirt_tag),
            ctx.font_bold, 9.5, Color::Greyscale(Greyscale::new(0.0, None)),
        );
        ctx.write_line(
            &format!("   Hallazgo:  {}", gap.finding),
            ctx.font_regular, 8.5, Color::Greyscale(Greyscale::new(0.0, None)),
        );
        ctx.write_line(
            &format!("   Ancla:     {}", gap.legal_anchor),
            ctx.font_mono, 8.0, Color::Greyscale(Greyscale::new(0.3, None)),
        );
        ctx.write_line(
            &format!("   Aplica a:  {}", applies_to_label(&gap.applies_to)),
            ctx.font_regular, 8.5, Color::Greyscale(Greyscale::new(0.0, None)),
        );

        if !gap.evidence.is_empty() {
            let ev = gap.evidence.join(", ");
            // Truncate long evidence strings so they fit on one line.
            let ev_display = if ev.len() > 80 { format!("{}…", &ev[..80]) } else { ev };
            ctx.write_line(
                &format!("   Evidencia: {}", ev_display),
                ctx.font_mono, 8.0, Color::Greyscale(Greyscale::new(0.2, None)),
            );
        }

        ctx.advance(2.0);
    }

    ctx.hline(0.3);
    Ok(())
}

fn draw_utm_table(ctx: &mut PageCtx, tier: Tier) -> Result<()> {
    ctx.ensure_space(50.0);
    ctx.write_line("ESCALA DE SANCIONES APLICABLE (Art. 40° Ley 21.663)", ctx.font_bold, 10.0, Color::Greyscale(Greyscale::new(0.0, None)));
    ctx.advance(1.0);

    let (leve, grave, gravisima) = match tier {
        Tier::Oiv => (UTM_LEVE_OIV, UTM_GRAVE_OIV, UTM_GRAVISIMA_OIV),
        _         => (UTM_LEVE_PSE, UTM_GRAVE_PSE,  UTM_GRAVISIMA_PSE),
    };

    let rows = [
        ("Infracción leve",     leve,      "Incumplimiento de instrucciones ANCI"),
        ("Infracción grave",    grave,     "No reportar, no implementar SGSI"),
        ("Infracción gravísima",gravisima, "Obstruir gestión incidente significativo"),
    ];

    for (label, utm, example) in rows {
        ctx.write_line(
            &format!("  {:<22} hasta {:>6} UTM   (ej: {})", label, utm, example),
            ctx.font_regular, 8.5, Color::Greyscale(Greyscale::new(0.0, None)),
        );
    }

    ctx.advance(2.0);
    ctx.write_line(
        "1 UTM ≈ CLP $66.000 (valor referencial — verificar UTM vigente en SII).",
        ctx.font_mono, 7.5, Color::Greyscale(Greyscale::new(0.4, None)),
    );
    ctx.hline(0.3);
    Ok(())
}

fn draw_footer(ctx: &mut PageCtx) -> Result<()> {
    ctx.y = MARGIN + 8.0;
    let layer = ctx.current_layer();
    layer.use_text(
        ""Generado por MuniANCI v0.1 — Felipe Carvajal Brown — uso interno reservado",
        7.0, Mm(MARGIN), Mm(ctx.y), ctx.font_mono,
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn applies_to_label(a: &AppliesTo) -> &'static str {
    match a {
        AppliesTo::All        => "Todos (PSE + OIV + no clasificados)",
        AppliesTo::OivAndPse  => "PSE y OIV",
        AppliesTo::Oiv        => "Solo OIV",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssetGraph, ScanConfig, ScanResult, Scope};
    use chrono::Utc;

    fn dummy_result() -> ScanResult {
        ScanResult {
            config: ScanConfig {
                institution_name: "Municipalidad de Prueba".into(),
                tier:             Tier::Pse,
                scope:            Scope::Local,
                progress_cb:      None,
            },
            asset_graph:  AssetGraph::default(),
            gaps:         vec![],
            scanned_at:   Utc::now(),
        }
    }

    #[test]
    fn json_output_is_valid() {
        let result = dummy_result();
        let tmp = std::env::temp_dir().join("muniani_test.json");
        write_json(&result, tmp.to_str().unwrap()).unwrap();
        let content = std::fs::read_to_string(&tmp).unwrap();
        assert!(content.contains("Municipalidad de Prueba"));
    }

    #[test]
    fn utm_table_uses_oiv_scale() {
        let (leve, _, _) = (UTM_LEVE_OIV, UTM_GRAVE_OIV, UTM_GRAVISIMA_OIV);
        assert_eq!(leve, 10_000);
    }
}