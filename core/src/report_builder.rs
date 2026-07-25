//! Generates PDF informe de brechas and CSIRT JSON from a ScanResult.
use crate::config::{Config, Papel};
use crate::historico::Delta;
use crate::types::{AppliesTo, Exigibilidad, InfractionClass, ScanResult, Severity, Tier};
use anyhow::{Context, Result};
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};
use std::fs::File;
use std::io::BufWriter;

// El tamaño de página lo fija la configuración de TI: una municipalidad chilena
// imprime en oficio o carta, no en A4. Ver `crate::config::Papel`.
const MARGIN: f64 = 51.0; // ~18mm
const LINE: f64 = 13.5;

const UTM_LEVE_OIV: u32      = 10_000;
const UTM_GRAVE_OIV: u32     = 20_000;
const UTM_GRAVISIMA_OIV: u32 = 40_000;
const UTM_LEVE_PSE: u32      =  5_000;
const UTM_GRAVE_PSE: u32     = 10_000;
const UTM_GRAVISIMA_PSE: u32 = 20_000;

pub fn build(result: &ScanResult, pdf_path: &str, json_path: &str, progress_cb: impl Fn(u8)) -> Result<()> {
    build_con(result, &Config::default(), pdf_path, json_path, progress_cb)
}

/// Same, honouring the municipality's configuration.
pub fn build_con(
    result: &ScanResult,
    config: &Config,
    pdf_path: &str,
    json_path: &str,
    progress_cb: impl Fn(u8),
) -> Result<()> {
    progress_cb(0);
    write_json(result, json_path)?;
    progress_cb(30);
    write_pdf_completo(result, &config.informe, config.informe.tamano_papel_tecnico, pdf_path)?;
    progress_cb(70);
    write_executive_pdf_con(
        result,
        &config.poam,
        &config.informe,
        config.informe.tamano_papel_ejecutivo,
        &executive_path(pdf_path),
    )?;
    progress_cb(100);
    Ok(())
}

/// Derives the executive report's path from the technical one.
///
/// `informe_brechas.pdf` -> `informe_brechas_ejecutivo.pdf`. Se deriva en vez de
/// pedir una ruta más para que los dos documentos de un mismo escaneo queden
/// siempre juntos y con el mismo nombre base.
pub fn executive_path(pdf_path: &str) -> String {
    match pdf_path.strip_suffix(".pdf") {
        Some(base) => format!("{base}_ejecutivo.pdf"),
        None => format!("{pdf_path}_ejecutivo.pdf"),
    }
}

fn write_json(result: &ScanResult, path: &str) -> Result<()> {
    let file = File::create(path).with_context(|| format!("cannot create {path}"))?;
    serde_json::to_writer_pretty(file, result).context("JSON serialisation failed")?;
    Ok(())
}

const NEGRO: (f64, f64, f64) = (0.0, 0.0, 0.0);

/// Altura mínima que el contenido debe dejar libre al pie de la página.
///
/// Debajo van los avisos de atribución de NVD, MITRE y el ASD, que son condición
/// de licencia. Ningún hallazgo puede invadir esa franja.
const PISO: f64 = 96.0;

/// Sets the fill colour for whatever is drawn next.
fn tinta(ops: &mut Vec<Operation>, (r, g, b): (f64, f64, f64)) {
    ops.push(Operation::new("rg", vec![r.into(), g.into(), b.into()]));
}

/// Draws a hairline rule.
///
/// Una regla de 0,7 pt bajo cada título da jerarquía visual con una cantidad de
/// tóner despreciable: un fondo relleno del mismo ancho gastaría cientos de veces
/// más, y en una municipalidad el cartucho de color se paga.
fn regla(ops: &mut Vec<Operation>, x: f64, y: f64, ancho: f64, grosor: f64, color: (f64, f64, f64)) {
    tinta(ops, color);
    ops.push(Operation::new(
        "re",
        vec![x.into(), y.into(), ancho.into(), grosor.into()],
    ));
    ops.push(Operation::new("f", vec![]));
    tinta(ops, NEGRO);
}

/// Draws a small filled square, used as a severity marker.
fn cuadro(ops: &mut Vec<Operation>, x: f64, y: f64, lado: f64, color: (f64, f64, f64)) {
    regla(ops, x, y, lado, lado, color);
}

/// Draws the Gobierno de Chile identifying band.
///
/// Es la "banda identificadora" de la página 27 del Manual de Normas Gráficas: la
/// síntesis del isologo, cuya función declarada es remarcar el carácter
/// gubernamental de la pieza, y cuyo uso el propio manual define como no
/// obligatorio e independiente del isologo. Sirve acá porque el informe lo emite un
/// órgano del Estado.
///
/// Se dibuja en su **proporción propia** (20a de ancho por 2a de alto, o sea 10:1)
/// y no estirada a lo ancho de la hoja: una banda de página completa serían 61 pt
/// de alto en papel oficio, un bloque de color macizo. Así son 8 pt de alto y el
/// gasto de tóner es despreciable. El reparto interno azul/rojo se tomó de la
/// figura del manual, donde el contenedor azul ocupa algo menos de la mitad.
fn banda_identificadora(ops: &mut Vec<Operation>, x: f64, y: f64, ancho: f64, p: &crate::config::Paleta) {
    let alto = ancho / 10.0;
    let azul = ancho * 0.44;
    regla(ops, x, y, azul, alto, p.primario);
    regla(ops, x + azul, y, ancho - azul, alto, p.alerta);
}

/// Draws one line of text without needing the caller's local macro.
fn texto(ops: &mut Vec<Operation>, fuente: &str, size: i64, x: f64, y: f64, t: &str) {
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new("Tf", vec![fuente.into(), size.into()]));
    ops.push(Operation::new("Td", vec![(x as i64).into(), (y as i64).into()]));
    ops.push(Operation::new("Tj", vec![Object::string_literal(to_pdf_safe(t).as_bytes())]));
    ops.push(Operation::new("ET", vec![]));
}

/// Stamps the attribution block and the footer at the bottom of a page.
///
/// Va en **todas** las páginas, no solo en la última: los avisos de NVD, MITRE y
/// el ASD son condición de licencia de los datos que el informe muestra, y una
/// página suelta que circule por sí sola tiene que llevarlos igual.
fn pie_de_pagina(
    ops: &mut Vec<Operation>,
    ancho_util: f64,
    p: &crate::config::Paleta,
    numero: usize,
    total: usize,
) {
    let avisos = avisos();
    let mut ay = 26.0 + (avisos.len() as f64 - 1.0) * 7.0;
    regla(ops, MARGIN, ay + 10.0, ancho_util, 0.5, p.apagado);
    for l in &avisos {
        texto(ops, "FM", 6, MARGIN, ay, l);
        ay -= 7.0;
    }
    texto(ops, "FM", 7, MARGIN, 16.0, &format!(
        "MuniANCI v{} - Felipe Carvajal Brown - uso interno reservado   |   Pagina {numero} de {total}",
        env!("CARGO_PKG_VERSION")));
}

/// The palette entry a severity maps to.
fn color_severidad(s: Severity, p: &crate::config::Paleta) -> (f64, f64, f64) {
    match s {
        Severity::Critical => p.alerta,
        Severity::High => p.primario,
        Severity::Medium => p.apagado,
    }
}

/// Sets up an empty single-page document with the three builtin fonts registered.
///
/// Lo comparten el informe técnico y el ejecutivo: son dos documentos distintos
/// pero el andamiaje de PDF es el mismo, y duplicarlo garantizaba que uno de los
/// dos quedara desincronizado.
fn new_doc() -> (Document, lopdf::ObjectId, lopdf::ObjectId) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

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

    (doc, pages_id, resources_id)
}

/// Encodes the operations into the page and writes the document to disk.
fn finish(
    doc: &mut Document,
    pages_id: lopdf::ObjectId,
    resources_id: lopdf::ObjectId,
    ops: Vec<Operation>,
    papel: Papel,
    path: &str,
) -> Result<()> {
    finish_paginas(doc, pages_id, resources_id, vec![ops], papel, path)
}

/// Same, for a document made of several pages.
fn finish_paginas(
    doc: &mut Document,
    pages_id: lopdf::ObjectId,
    resources_id: lopdf::ObjectId,
    paginas: Vec<Vec<Operation>>,
    papel: Papel,
    path: &str,
) -> Result<()> {
    let (pw, ph) = papel.puntos();
    let mut kids: Vec<Object> = Vec::new();

    for ops in paginas {
        let content = Content { operations: ops };
        let content_id = doc.add_object(
            Stream::new(dictionary! {}, content.encode().context("content encode failed")?)
        );
        let page_id = doc.add_object(dictionary! {
            "Type"      => "Page",
            "Parent"    => pages_id,
            "MediaBox"  => vec![0.into(), 0.into(), pw.into(), ph.into()],
            "Contents"  => content_id,
            "Resources" => resources_id,
        });
        kids.push(page_id.into());
    }

    let count = kids.len() as i64;
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
        "Type"  => "Pages",
        "Kids"  => kids,
        "Count" => count,
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

pub fn write_pdf(result: &ScanResult, path: &str) -> Result<()> {
    write_pdf_con(result, Papel::default(), path)
}

pub fn write_pdf_con(result: &ScanResult, papel: Papel, path: &str) -> Result<()> {
    write_pdf_completo(result, &crate::config::InformeConfig::default(), papel, path)
}

pub fn write_pdf_completo(
    result: &ScanResult,
    informe: &crate::config::InformeConfig,
    papel: Papel,
    path: &str,
) -> Result<()> {
    let (mut doc, pages_id, resources_id) = new_doc();
    let (pw, ph) = papel.puntos();
    let p = informe.paleta();
    let ancho_util = pw - 2.0 * MARGIN;

    // Build page content as a list of PDF operations. El informe tecnico crece a
    // las paginas que haga falta: ocultar tres cuartos de los hallazgos hacia que
    // no sirviera para presentarlo.
    let mut paginas: Vec<Vec<Operation>> = Vec::new();
    let mut ops: Vec<Operation> = Vec::new();
    let mut y = ph - MARGIN - 20.0; // start near top

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

    macro_rules! titulo {
        ($size:expr, $texto:expr) => {{
            tinta(&mut ops, p.texto);
            line!("FB", $size, MARGIN, y, $texto);
            tinta(&mut ops, NEGRO);
            y -= 5.0;
            regla(&mut ops, MARGIN, y, ancho_util, 0.7, p.primario);
            y -= LINE - 2.0;
        }};
    }
    // Cierra la pagina en curso y abre otra con su encabezado de continuacion.
    macro_rules! salto {
        () => {{
            paginas.push(std::mem::take(&mut ops));
            y = ph - MARGIN - 20.0;
            banda_identificadora(&mut ops, MARGIN, y + 14.0, 60.0, &p);
            tinta(&mut ops, p.texto);
            line!("FB", 10, MARGIN, y, &format!(
                "INFORME DE BRECHAS - {} (continuacion)", result.meta.institution_name));
            tinta(&mut ops, NEGRO);
            y -= 6.0;
            regla(&mut ops, MARGIN, y, ancho_util, 0.7, p.primario);
            y -= LINE + 2.0;
        }};
    }

    // Header
    banda_identificadora(&mut ops, MARGIN, y + 22.0, 80.0, &p);
    y -= 4.0;
    tinta(&mut ops, p.texto);
    line!("FB", 16, MARGIN, y, "INFORME DE BRECHAS DE CIBERSEGURIDAD");
    tinta(&mut ops, NEGRO);
    y -= 20.0;
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

    titulo!(11, "RESUMEN EJECUTIVO");
    for l in [
        format!("Puntaje de cumplimiento: {} de {} (base menos deducciones ponderadas)",
            score.score, score.base),
        format!("Brechas exigibles: {}  (Criticas: {}  Altas: {}  Medias: {})",
            exigibles.len(), critical, high, medium),
        format!("Brechas de madurez voluntaria (no exigibles a esta institucion): {}", madurez.len()),
        format!("Con reporte CSIRT obligatorio (Art. 9): {}", csirt),
        format!("Hosts: {}  Servicios: {}  Unidades: {}",
            result.asset_graph.hosts.len(), result.asset_graph.services.len(), result.asset_graph.drives.len()),
        // Con que evidencia se afirma que cada host existe. Un host visto solo
        // por TCP puede tener el firewall filtrando ICMP, y no entrega MAC.
        resumen_descubrimiento(&result.asset_graph),
        // Se declara siempre: un informe que no dice cuanto NO pudo evaluar
        // induce a leer los huecos como ausencia de problemas.
        format!("Cobertura CVE: {}", result.cve_coverage),
        // Marcar una CVE como "explotada activamente" es una afirmacion fuerte:
        // se declara contra que catalogo, y de que fecha, se hizo.
        result.kev_provenance.clone(),
        match &result.delta {
            Some(d) => format!(
                "Evolucion desde {}: puntaje {}, brechas exigibles {}, explotadas {}",
                fecha_corta(&d.desde),
                Delta::signo(d.puntaje),
                Delta::signo(d.exigibles),
                Delta::signo(d.cve_explotadas),
            ),
            None => "Evolucion: primera medicion registrada para esta institucion.".to_string(),
        },
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

    // Deriva por control. El "Evolucion desde" de arriba dice cuanto se movio el
    // agregado; esto dice QUE se movio, que es lo unico accionable de las dos.
    if let Some(d) = &result.deriva {
        use crate::historico::Estado;

        titulo!(11, "DERIVA POR CONTROL");
        line!("FR", 9, MARGIN, y,
            &format!("Comparado con la medicion del {}: {}", fecha_corta(d.desde.as_deref().unwrap_or("")), d.resumen()));
        y -= LINE;

        // Una cobertura menor no puede pasar inadvertida: es lo que separa
        // "se corrigio" de "no se miro".
        if !d.cobertura_comparable {
            cuadro(&mut ops, MARGIN, y + 2.0, 6.0, p.alerta);
            line!("FB", 9, MARGIN + 12.0, y,
                &format!("Este escaneo cubrio menos que el anterior ({} -> {}).",
                    d.alcance_antes.as_deref().unwrap_or("desconocido"),
                    d.alcance_ahora.as_deref().unwrap_or("desconocido")));
            y -= LINE;
            line!("FR", 9, MARGIN + 12.0, y,
                "Los controles tecnicos que faltan figuran como SIN VERIFICAR, no como resueltos.");
            y -= LINE;
        }
        y -= 3.0;

        // Primero lo que empeoro. Una reaparecida arriba de todo: dice que una
        // correccion no se sostuvo, y eso es lo que hay que ir a mirar hoy.
        for (estado, subtitulo, color) in [
            (Estado::Reaparecida, "Reaparecidas (se habian corregido y volvieron)", p.alerta),
            (Estado::Nueva, "Nuevas", p.primario),
            (Estado::Resuelta, "Resueltas", p.apagado),
            (Estado::SinVerificar, "Sin verificar en este escaneo", p.apagado),
        ] {
            let items: Vec<_> = d.en(estado).collect();
            if items.is_empty() {
                continue;
            }
            // Mismo criterio que las brechas: el alto se mide ANTES de dibujar,
            // para que el ultimo renglon no se monte sobre las atribuciones del pie.
            let alto = (items.len() as f64 + 1.0) * LINE + 6.0;
            if y - alto.min(LINE * 6.0) < PISO {
                salto!();
            }

            cuadro(&mut ops, MARGIN, y + 2.0, 6.0, color);
            line!("FB", 9, MARGIN + 12.0, y, &format!("{subtitulo}: {}", items.len()));
            y -= LINE;
            for c in items {
                if y - LINE < PISO {
                    salto!();
                }
                let linea = match &c.resuelta_el {
                    Some(f) => format!("- {} (estaba resuelta el {})", limpiar(&c.control), fecha_corta(f)),
                    None => format!("- {}", limpiar(&c.control)),
                };
                line!("FR", 8, MARGIN + 12.0, y, &recortar(&linea, 92));
                y -= LINE - 2.0;
            }
            y -= 4.0;
        }
        y -= 4.0;
    }

    // Maturity per domain. Dice DONDE esta el problema, que es lo que el puntaje
    // agregado no puede decir: un 82/100 puede ser cinco dominios sanos y uno roto.
    titulo!(11, "MADUREZ POR DOMINIO (0 a 3)");
    match result.maturity.average() {
        Some(avg) => line!("FR", 9, MARGIN, y,
            &format!("Promedio: {avg:.1} de 3, sobre {} dominio(s) medido(s).",
                result.maturity.domains.len() - result.maturity.unmeasured().len())),
        None => line!("FR", 9, MARGIN, y, "Ningun dominio pudo medirse en este escaneo."),
    }
    y -= LINE;
    for d in &result.maturity.domains {
        if y < PISO + 20.0 { break; }
        // Un dominio no medido no lleva color: no es un nivel malo, es ausencia
        // de dato, y pintarlo de rojo diria lo contrario.
        let color = match d.level.value() {
            Some(0) => p.alerta,
            Some(1) => p.primario,
            Some(_) => p.apagado,
            None => p.apagado,
        };
        cuadro(&mut ops, MARGIN, y + 1.0, 6.0, color);
        line!("FB", 9, MARGIN + 11.0, y, &format!("{}  -  {}", d.level, d.domain));
        y -= LINE - 2.0;
        for (j, l) in envolver(&format!("{} | {}", d.domain.legal_anchor(), limpiar(&d.rationale)), 110)
            .into_iter().enumerate()
        {
            line!("FM", 7, MARGIN + 10.0, y, &if j == 0 { l } else { format!("  {l}") });
            y -= LINE - 3.0;
        }
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
        if y < PISO + 60.0 { salto!(); }
        titulo!(11, titulo);
        line!("FM", 7, MARGIN, y, nota);
        y -= LINE;

        for (i, gap) in grupo.iter().enumerate() {
            let sev = match gap.severity {
                Severity::Critical => "[CRITICO]",
                Severity::High     => "[ALTO]",
                Severity::Medium   => "[MEDIO]",
            };
            let csirt_tag = if gap.requires_csirt_report { " *** CSIRT ***" } else { "" };
            // Se envuelve en vez de dejar que la linea se salga por el borde
            // derecho: los controles declarativos estan redactados como preguntas
            // largas y desbordaban la hoja.
            let titulo_l = envolver(
                &format!("{}. {} {}{}", i + 1, sev, limpiar(&gap.control), csirt_tag), 86);
            let hallazgo_l = envolver(&format!("Hallazgo: {}", limpiar(&gap.finding)), 100);
            let ancla_l = envolver(&format!("Ancla: {}", gap.legal_anchor), 96);
            let evidencia = if gap.evidence.is_empty() { 0 } else { 1 };

            // El alto se calcula ANTES de dibujar. Medirlo despues, que es lo que
            // hacia el guard anterior, dejaba que la ultima brecha se montara
            // encima de los avisos de atribucion del pie.
            let alto = (titulo_l.len() + hallazgo_l.len() + ancla_l.len() + 1 + evidencia) as f64
                * (LINE - 2.0) + 5.0;
            // Una brecha no se parte entre paginas: leer el anclaje legal de un
            // hallazgo en otra hoja que su hallazgo no ayuda a nadie.
            if y - alto < PISO { salto!(); }

            cuadro(&mut ops, MARGIN, y + 1.0, 6.0, color_severidad(gap.severity, &p));
            for (j, l) in titulo_l.into_iter().enumerate() {
                line!("FB", 9, MARGIN + 11.0, y, &if j == 0 { l } else { format!("   {l}") });
                y -= LINE - 2.0;
            }
            for (j, l) in hallazgo_l.into_iter().enumerate() {
                line!("FR", 8, MARGIN + 10.0, y, &if j == 0 { l } else { format!("  {l}") });
                y -= LINE - 2.0;
            }
            for (j, l) in ancla_l.into_iter().enumerate() {
                line!("FM", 8, MARGIN + 10.0, y, &if j == 0 { l } else { format!("  {l}") });
                y -= LINE - 2.0;
            }
            let clasif = match gap.infraction_class {
                Some(c) => format!("{c}"),
                None    => "sin clasificacion legal (criterio tecnico)".into(),
            };
            line!("FR", 8, MARGIN + 10.0, y, &format!("Aplica a: {}  |  Infraccion: {}",
                applies_to_label(&gap.applies_to), clasif));
            y -= LINE - 2.0;
            if evidencia == 1 {
                // La evidencia si se recorta: puede traer decenas de paquetes y no
                // es lo que el informe necesita mostrar entero. El JSON la lleva
                // completa, y la nota de mas abajo lo dice.
                let ev = recortar(&gap.evidence.join(", "), 95);
                line!("FM", 8, MARGIN + 10.0, y, &format!("Evidencia: {ev}"));
                y -= LINE - 2.0;
            }
            y -= 5.0;
            shown += 1;
        }
        y -= 4.0;
    }

    // Con paginacion ya no se pierde ninguna, pero la nota se mantiene por si
    // alguna vez se topa el limite de paginas.
    if shown < total {
        if y < PISO + 20.0 { salto!(); }
        line!("FB", 8, MARGIN, y, &format!(
            "NOTA: se muestran {shown} de {total} brechas. El JSON y el POA&M las incluyen todas."));
        y -= LINE;
    }

    // UTM table
    let (leve, grave, gravisima) = match result.meta.tier {
        Tier::Oiv => (UTM_LEVE_OIV, UTM_GRAVE_OIV, UTM_GRAVISIMA_OIV),
        _         => (UTM_LEVE_PSE, UTM_GRAVE_PSE,  UTM_GRAVISIMA_PSE),
    };
    if y < PISO + 80.0 { salto!(); }
    y -= 6.0;
    titulo!(10, "ESCALA DE SANCIONES (Art. 40 Ley 21.663)");
    for (label, utm) in [("Leve", leve), ("Grave", grave), ("Gravisima", gravisima)] {
        line!("FR", 8, MARGIN, y, &format!("  {:<12} hasta {:>6} UTM", label, utm));
        y -= LINE;
    }
    line!("FM", 7, MARGIN, y, "1 UTM aprox. CLP $66.000 - verificar en SII.");

    paginas.push(ops);

    // Los avisos de atribucion de NVD, MITRE y el ASD son condicion de licencia y
    // se estampan en TODAS las paginas: una hoja suelta que circule por si sola
    // tiene que llevarlos igual. El numero de pagina se conoce recien aca.
    let total_paginas = paginas.len();
    for (i, pagina) in paginas.iter_mut().enumerate() {
        pie_de_pagina(pagina, ancho_util, &p, i + 1, total_paginas);
    }

    finish_paginas(&mut doc, pages_id, resources_id, paginas, papel, path)
}

// ---------------------------------------------------------------------------
// Informe ejecutivo — una plana
// ---------------------------------------------------------------------------

/// Writes the one-page executive report.
///
/// Va en un archivo aparte y no como primera página del técnico: son documentos
/// con destinatarios distintos y circulan por separado. El ejecutivo se le manda al
/// alcalde por correo; el técnico lleva hallazgos con IP y rutas de recursos
/// compartidos, y conviene tratarlo como reservado.
///
/// Responde tres preguntas de quien firma el presupuesto: dónde estamos, qué
/// arriesgamos y qué hay que autorizar. El detalle vive en el informe largo.
pub fn write_executive_pdf(
    result: &ScanResult,
    config: &crate::config::PoamConfig,
    papel: Papel,
    path: &str,
) -> Result<()> {
    write_executive_pdf_con(result, config, &crate::config::InformeConfig::default(), papel, path)
}

pub fn write_executive_pdf_con(
    result: &ScanResult,
    config: &crate::config::PoamConfig,
    informe: &crate::config::InformeConfig,
    papel: Papel,
    path: &str,
) -> Result<()> {
    let (mut doc, pages_id, resources_id) = new_doc();
    let (pw, ph) = papel.puntos();
    let p = informe.paleta();
    let ancho_util = pw - 2.0 * MARGIN;
    let mut ops: Vec<Operation> = Vec::new();
    let mut y = ph - MARGIN - 20.0;

    macro_rules! line {
        ($font:expr, $size:expr, $x:expr, $y:expr, $text:expr) => {{
            ops.push(Operation::new("BT", vec![]));
            ops.push(Operation::new("Tf", vec![$font.into(), ($size as i64).into()]));
            ops.push(Operation::new("Td", vec![($x as i64).into(), ($y as i64).into()]));
            ops.push(Operation::new("Tj", vec![Object::string_literal(to_pdf_safe($text).as_bytes())]));
            ops.push(Operation::new("ET", vec![]))
        }};
    }
    macro_rules! titulo {
        ($size:expr, $texto:expr) => {{
            tinta(&mut ops, p.texto);
            line!("FB", $size, MARGIN, y, $texto);
            tinta(&mut ops, NEGRO);
            y -= 5.0;
            regla(&mut ops, MARGIN, y, ancho_util, 0.7, p.primario);
            y -= LINE - 1.0;
        }};
    }

    banda_identificadora(&mut ops, MARGIN, y + 22.0, 80.0, &p);
    y -= 4.0;
    tinta(&mut ops, p.texto);
    line!("FB", 17, MARGIN, y, "RESUMEN EJECUTIVO DE CIBERSEGURIDAD");
    tinta(&mut ops, NEGRO);
    y -= 18.0;
    line!("FR", 11, MARGIN, y, &result.meta.institution_name);
    y -= LINE;
    line!("FM", 8, MARGIN, y, &format!(
        "Ley 21.663  |  Clasificacion: {}  |  {}",
        result.meta.tier, result.scanned_at.format("%d-%m-%Y")));
    y -= LINE + 6.0;

    // 1. Donde estamos.
    titulo!(12, "1. DONDE ESTAMOS");
    line!("FR", 10, MARGIN, y, &format!(
        "Puntaje de cumplimiento: {} de {}", result.score.score, result.score.base));
    y -= LINE;
    match result.maturity.average() {
        Some(avg) => line!("FR", 10, MARGIN, y, &format!(
            "Madurez promedio: {avg:.1} de 3, sobre {} de {} dominios medidos",
            result.maturity.domains.len() - result.maturity.unmeasured().len(),
            result.maturity.domains.len())),
        None => line!("FR", 10, MARGIN, y, "Madurez: no se pudo medir ningun dominio"),
    }
    y -= LINE;
    let exigibles = result.gaps.iter()
        .filter(|g| g.exigibilidad == Exigibilidad::Exigible).count();
    line!("FR", 10, MARGIN, y, &format!(
        "Incumplimientos exigibles a esta institucion: {exigibles}"));
    y -= LINE;

    // La pregunta real de quien recibe el informe no es cuanto sacamos, sino si
    // estamos mejor que la vez pasada.
    match &result.delta {
        Some(d) => {
            cuadro(&mut ops, MARGIN, y + 1.0, 6.0, match d.direccion() {
                crate::historico::Direccion::Mejoro => p.primario,
                crate::historico::Direccion::SinCambios => p.apagado,
                crate::historico::Direccion::Empeoro => p.alerta,
            });
            line!("FB", 10, MARGIN + 11.0, y, &format!(
                "Desde la medicion del {}: {}", fecha_corta(&d.desde), d.veredicto()));
            y -= LINE - 1.0;
            line!("FM", 8, MARGIN + 11.0, y, &format!(
                "Puntaje {}   |   Brechas exigibles {}   |   Criticas {}   |   Explotadas {}",
                Delta::signo(d.puntaje), Delta::signo(d.exigibles),
                Delta::signo(d.criticas), Delta::signo(d.cve_explotadas)));
            y -= LINE - 1.0;
        }
        None => {
            line!("FM", 8, MARGIN, y,
                "Primera medicion registrada: todavia no hay con que comparar.");
            y -= LINE - 1.0;
        }
    }
    for d in result.maturity.weakest_first().iter().take(2) {
        for (i, l) in envolver(&format!("Lo mas debil: {} ({}) - {}",
            d.domain, d.level, d.rationale), 98).into_iter().enumerate()
        {
            line!("FM", 8, MARGIN + 8.0, y, &if i == 0 { l } else { format!("  {l}") });
            y -= LINE - 3.0;
        }
    }
    y -= 8.0;

    // 2. Que arriesgamos.
    titulo!(12, "2. QUE ARRIESGAMOS");

    let explotadas = kev_count(result);
    if explotadas > 0 {
        cuadro(&mut ops, MARGIN, y + 1.0, 6.0, p.alerta);
        line!("FB", 10, MARGIN + 11.0, y, &format!(
            "URGENTE: {explotadas} vulnerabilidad(es) que se estan explotando hoy en el mundo real."));
        y -= LINE;
        line!("FM", 8, MARGIN, y,
            "Fuente: catalogo de vulnerabilidades explotadas conocidas de CISA (KEV).");
    } else {
        line!("FR", 10, MARGIN, y,
            "No se detectaron vulnerabilidades bajo explotacion activa conocida.");
    }
    y -= LINE + 4.0;

    let (leve, grave, gravisima) = match result.meta.tier {
        Tier::Oiv => (UTM_LEVE_OIV, UTM_GRAVE_OIV, UTM_GRAVISIMA_OIV),
        _         => (UTM_LEVE_PSE, UTM_GRAVE_PSE,  UTM_GRAVISIMA_PSE),
    };
    let cuenta = |c: InfractionClass| result.gaps.iter()
        .filter(|g| g.exigibilidad == Exigibilidad::Exigible && g.infraction_class == Some(c))
        .count();
    let (n_leve, n_grave, n_gravisima) = (
        cuenta(InfractionClass::Leve),
        cuenta(InfractionClass::Grave),
        cuenta(InfractionClass::Gravisima),
    );

    line!("FR", 10, MARGIN, y, "Exposicion segun la escala del Art. 40:");
    y -= LINE;
    let mut tope = 0u64;
    for (etiqueta, n, max) in [
        ("Gravisimas", n_gravisima, gravisima),
        ("Graves",     n_grave,     grave),
        ("Leves",      n_leve,      leve),
    ] {
        tope += n as u64 * max as u64;
        line!("FR", 9, MARGIN, y, &format!(
            "   {etiqueta:<12} {n:>2} incumplimiento(s)   x hasta {max:>6} UTM c/u"));
        y -= LINE - 2.0;
    }
    line!("FB", 10, MARGIN, y, &format!("   Tope teorico acumulado: {tope} UTM"));
    y -= LINE;
    // No es una prediccion de multa ni una opinion legal: es la escala del
    // articulo multiplicada por la cuenta de incumplimientos.
    line!("FM", 7, MARGIN, y,
        "Es el maximo que permite la escala si cada incumplimiento se sancionara al tope, no una multa esperada.");
    y -= LINE - 3.0;
    line!("FM", 7, MARGIN, y,
        "Determinar sanciones corresponde a la ANCI. Este documento no constituye asesoria legal.");
    y -= LINE + 6.0;

    // 3. Que hay que autorizar.
    titulo!(12, "3. LAS TRES PRIMERAS ACCIONES");
    let plan = crate::poam::plan(&result.gaps, config);
    if plan.is_empty() {
        line!("FR", 10, MARGIN, y, "No hay acciones pendientes: no se detectaron brechas.");
        y -= LINE;
    }
    for item in plan.iter().take(3) {
        // El marcador de severidad es lo unico que el alcalde tiene que poder leer
        // de un vistazo sin leer el texto.
        cuadro(&mut ops, MARGIN, y + 1.0, 6.0, color_severidad(item.gap.severity, &p));
        for (i, l) in envolver(&accion(item.gap), 82).into_iter().enumerate() {
            let texto = if i == 0 { format!("{}. {l}", item.orden) } else { format!("   {l}") };
            line!("FB", 10, MARGIN + 11.0, y, &texto);
            y -= LINE - 1.0;
        }
        for (i, l) in envolver(&item.motivo, 92).into_iter().enumerate() {
            line!("FR", 9, MARGIN + 8.0, y,
                &if i == 0 { format!("Por que: {l}") } else { format!("   {l}") });
            y -= LINE - 3.0;
        }
        for l in envolver(&format!("Plazo sugerido: {} dias   |   {}",
            item.plazo_dias, item.gap.legal_anchor), 100)
        {
            line!("FM", 8, MARGIN + 8.0, y, &l);
            y -= LINE - 3.0;
        }
        y -= 5.0;
    }
    if plan.len() > 3 {
        line!("FM", 8, MARGIN, y, &format!(
            "El plan completo tiene {} acciones y esta en el informe tecnico y en el POA&M.",
            plan.len()));
    }

    // Avisos y pie, anclados al borde inferior y envueltos para no salirse del
    // margen: una atribucion cortada por el borde no cumple la licencia.
    let avisos = avisos();
    let mut ay = 26.0 + (avisos.len() as f64 - 1.0) * 7.0;
    regla(&mut ops, MARGIN, ay + 10.0, ancho_util, 0.5, p.apagado);
    for l in &avisos {
        line!("FM", 6, MARGIN, ay, l);
        ay -= 7.0;
    }
    line!("FM", 7, MARGIN, 16.0, &format!(
        "MuniANCI v{} - Felipe Carvajal Brown - uso interno reservado",
        env!("CARGO_PKG_VERSION")));

    finish(&mut doc, pages_id, resources_id, ops, papel, path)
}

/// CVEs under observed exploitation across the whole inventory.
fn kev_count(result: &ScanResult) -> usize {
    result.asset_graph.software.iter().flat_map(|s| &s.cves)
        .chain(result.asset_graph.os_info.iter().flat_map(|o| &o.cves))
        .filter(|c| c.known_exploited)
        .count()
}

/// Summarises how the remote hosts were discovered, by evidence strength.
///
/// La calidad de la prueba no es la misma en los tres casos y el informe lo
/// dice: ARP confirma el equipo en la capa 2 del propio segmento y es el único
/// que entrega MAC; ICMP prueba que la pila IP responde; TCP solo prueba que un
/// puerto acepta conexión, y un host visto solo así probablemente tenga el
/// firewall filtrando el ping. Sin esta línea, un activo sin MAC se lee como
/// error del escáner.
fn resumen_descubrimiento(graph: &crate::types::AssetGraph) -> String {
    use crate::probes::net_discovery::DiscoveryMethod::{Arp, Icmp, Tcp};
    let cuenta = |m: crate::probes::net_discovery::DiscoveryMethod| {
        graph.hosts.iter().filter(|h| h.discovered_by == Some(m)).count()
    };
    let (arp, icmp, tcp) = (cuenta(Arp), cuenta(Icmp), cuenta(Tcp));
    if arp + icmp + tcp == 0 {
        // Escaneo local: no hubo barrido, así que no hay nada que declarar.
        return "Descubrimiento de red: solo el equipo local (sin barrido de LAN).".into();
    }
    format!(
        "Descubrimiento: {arp} en capa 2 (ARP, con MAC), {icmp} por ICMP, {tcp} solo por TCP."
    )
}

/// Renders an RFC 3339 timestamp as `dd-mm-aaaa`, the way Chile writes dates.
fn fecha_corta(rfc3339: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(rfc3339) {
        Ok(d) => d.format("%d-%m-%Y").to_string(),
        // Si no parsea se muestra tal cual: es preferible una fecha fea a una
        // fecha inventada.
        Err(_) => rfc3339.to_string(),
    }
}

/// Truncates on a character boundary — the line budget of one page is real.
fn recortar(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max.saturating_sub(3)).collect::<String>() + "..."
}

/// Splits text into lines of at most `max` characters, breaking at spaces.
///
/// Las fuentes Type1 incrustadas no traen métricas acá, así que el corte es por
/// cuenta de caracteres y no por ancho real. Es conservador a propósito: mejor una
/// línea corta que un anclaje legal cortado por el borde derecho de la hoja.
fn envolver(texto: &str, max: usize) -> Vec<String> {
    let mut lineas = Vec::new();
    let mut actual = String::new();

    for palabra in texto.split_whitespace() {
        // Una palabra sola más larga que el ancho se emite tal cual: partirla
        // dentro de un identificador legal sería peor que pasarse un poco.
        if actual.is_empty() {
            actual = palabra.to_string();
        } else if actual.chars().count() + 1 + palabra.chars().count() <= max {
            actual.push(' ');
            actual.push_str(palabra);
        } else {
            lineas.push(std::mem::take(&mut actual));
            actual = palabra.to_string();
        }
    }
    if !actual.is_empty() {
        lineas.push(actual);
    }
    if lineas.is_empty() {
        lineas.push(String::new());
    }
    lineas
}

/// The mandatory attribution notices, wrapped to fit the page width.
///
/// Se emiten en los dos informes: son condición de la licencia de los datos, no un
/// adorno, y una atribución cortada por el margen no cumple la condición.
fn avisos() -> Vec<String> {
    [
        crate::maturity::ESSENTIAL_EIGHT_ATTRIBUTION,
        crate::cve::NVD_NOTICE,
        crate::cve::CVE_NOTICE,
    ]
    .iter()
    .flat_map(|t| envolver(t, 128))
    .collect()
}

/// What the reader is being asked to do about a gap.
///
/// Sin esto, la primera acción del informe ejecutivo se leía literalmente como
/// "1. ¿Existe un procedimiento interno...?", porque el control declarativo está
/// redactado como pregunta. El alcalde necesita un verbo.
fn accion(gap: &crate::types::Gap) -> String {
    let control = gap.control.trim_start_matches('¿').trim_end_matches('?');
    if !gap.evaluated {
        format!("Verificar y documentar si {}", minuscula_inicial(control))
    } else {
        format!("Corregir: {control}")
    }
}

/// Drops the opening inverted question mark before it becomes a bare `?`.
///
/// Los controles declarativos están redactados como preguntas. `to_pdf_safe` mapea
/// `¿` a `?` porque las fuentes Type1 no lo tienen, y el resultado se leía como
/// "?La institucion aplica...", que parece un error de codificación.
fn limpiar(s: &str) -> String {
    s.replace('¿', "")
}

fn minuscula_inicial(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) => c.to_lowercase().collect::<String>() + cs.as_str(),
        None => String::new(),
    }
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
            taxonomia_anci: crate::taxonomia::TaxonomiaAnci::default(),
            score:       crate::scoring::ComplianceScore::from_gaps(&[]),
            maturity:    crate::maturity::MaturityProfile::from_gaps(&[], &[]),
            delta:       None,
            deriva:      None,
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
    fn el_informe_declara_con_que_evidencia_vio_cada_host() {
        use crate::probes::net_discovery::DiscoveryMethod::{Arp, Tcp};
        // Sin esta linea, un activo sin MAC se lee como error del escaner en vez
        // de como un host que solo contesto TCP.
        let host = |ip: &str, m| crate::types::Host {
            ip: ip.parse().unwrap(),
            hostname: None,
            mac: None,
            os_banner: None,
            discovered_by: Some(m),
            is_local: false,
        };
        let mut r = dummy();
        r.asset_graph.hosts = vec![
            host("10.0.0.1", Arp),
            host("10.0.0.2", Arp),
            host("10.0.0.3", Tcp),
        ];
        let t = pdf_text(&r, "muniani_test_descubrimiento.pdf");
        assert!(t.contains("2 en capa 2"), "{t}");
        assert!(t.contains("0 por ICMP"), "{t}");
        assert!(t.contains("1 solo por TCP"), "{t}");

        // Un escaneo local no barre la LAN: tiene que decirlo, no mentir un cero.
        let t = pdf_text(&dummy(), "muniani_test_sin_barrido.pdf");
        assert!(t.contains("sin barrido de LAN"), "{t}");
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

    fn deriva_de_prueba() -> crate::historico::Deriva {
        use crate::historico::{ControlEnDeriva, Deriva, Estado};
        let c = |control: &str, estado, resuelta_el: Option<&str>| ControlEnDeriva {
            control: control.into(),
            estado,
            resuelta_el: resuelta_el.map(String::from),
        };
        Deriva {
            desde: Some("2026-06-12T10:00:00+00:00".into()),
            alcance_antes: Some("lan".into()),
            alcance_ahora: Some("local".into()),
            cobertura_comparable: false,
            controles: vec![
                c("BitLocker sin activar", Estado::Reaparecida, Some("2026-05-08T10:00:00+00:00")),
                c("Firewall perimetral", Estado::Nueva, None),
                c("Antivirus desactualizado", Estado::Persistente, None),
                c("Contrasenas por defecto", Estado::Resuelta, None),
                c("Shares anonimos (SMB/NFS/WebDAV)", Estado::SinVerificar, None),
            ],
        }
    }

    // El PDF es el entregable que sale de la maquina. Se lee el contenido ya
    // renderizado, no se confia en que la seccion compile.
    #[test]
    fn the_pdf_carries_the_drift_section_with_every_state() {
        let mut r = dummy();
        r.deriva = Some(deriva_de_prueba());
        let t = pdf_text(&r, "muniani_test_deriva.pdf");

        assert!(t.contains("DERIVA POR CONTROL"), "falta el titulo");
        assert!(t.contains("12-06-2026"), "tiene que decir contra que fecha compara: {t}");
        assert!(t.contains("Reaparecidas"), "falta el grupo que mas importa");
        assert!(t.contains("BitLocker sin activar"), "falta el control reaparecido");
        assert!(t.contains("08-05-2026"), "una reaparecida tiene que decir cuando estuvo resuelta");
        assert!(t.contains("Nuevas"), "{t}");
        assert!(t.contains("Firewall perimetral"), "{t}");
        assert!(t.contains("Resueltas"), "{t}");
        assert!(t.contains("Sin verificar"), "{t}");
    }

    // Decirle a una municipalidad que corrigio algo que en realidad nadie miro es
    // el error que este aviso existe para evitar, y tiene que llegar a la hoja.
    #[test]
    fn the_pdf_warns_when_the_rescan_covered_less_ground() {
        let mut r = dummy();
        r.deriva = Some(deriva_de_prueba());
        let t = pdf_text(&r, "muniani_test_deriva_cobertura.pdf");
        assert!(t.contains("cubrio menos que el anterior"), "{t}");
        assert!(t.contains("lan") && t.contains("local"), "tiene que nombrar los dos alcances");
        assert!(t.contains("SIN VERIFICAR, no como resueltos"), "{t}");

        // Con cobertura suficiente el aviso no puede aparecer: seria ruido.
        let mut d = deriva_de_prueba();
        d.cobertura_comparable = true;
        d.alcance_ahora = Some("lan".into());
        r.deriva = Some(d);
        let t = pdf_text(&r, "muniani_test_deriva_ok.pdf");
        assert!(!t.contains("cubrio menos que el anterior"), "{t}");
    }

    // Un escaneo sin historico no puede dejar la seccion vacia en la hoja.
    #[test]
    fn the_pdf_omits_the_drift_section_on_a_first_scan() {
        let t = pdf_text(&dummy(), "muniani_test_sin_deriva.pdf");
        assert!(!t.contains("DERIVA POR CONTROL"), "{t}");
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

    // -----------------------------------------------------------------------
    // Informe ejecutivo
    // -----------------------------------------------------------------------

    fn ejecutivo(result: &ScanResult, name: &str) -> String {
        let tmp = std::env::temp_dir().join(name);
        write_executive_pdf(result, &crate::config::PoamConfig::default(), Papel::Carta,
            tmp.to_str().unwrap()).unwrap();
        let doc = Document::load(&tmp).unwrap();
        let (_, page_id) = doc.get_pages().into_iter().next().unwrap();
        String::from_utf8_lossy(&doc.get_page_content(page_id).unwrap()).into_owned()
    }

    fn con_brechas() -> ScanResult {
        use crate::maturity::Domain;
        use crate::types::{Gap, InfractionClass};
        let gap = |control: &str, sev: Severity, clase: Option<InfractionClass>| Gap {
            control: control.into(),
            finding: format!("hallazgo de {control}"),
            severity: sev,
            legal_anchor: "Art. 7 Ley 21.663".into(),
            applies_to: AppliesTo::All,
            exigibilidad: Exigibilidad::Exigible,
            infraction_class: clase,
            domain: Domain::MedidasPermanentes,
            evaluated: true,
            evidence: vec!["10.0.0.1".into()],
            requires_csirt_report: false,
        };
        let gaps = vec![
            gap("Shares anonimos", Severity::Critical, Some(InfractionClass::Grave)),
            gap("Firewall desactivado", Severity::Critical, Some(InfractionClass::Leve)),
            gap("Software fuera de soporte", Severity::High, None),
            gap("Certificado vencido", Severity::Medium, None),
        ];
        let mut r = dummy();
        r.maturity = crate::maturity::MaturityProfile::from_gaps(&gaps, &[Domain::MedidasPermanentes]);
        r.score = crate::scoring::ComplianceScore::from_gaps(&gaps);
        r.gaps = gaps;
        r
    }

    #[test]
    fn the_executive_report_answers_the_three_questions() {
        let text = ejecutivo(&con_brechas(), "muniani_test_ejecutivo.pdf");
        assert!(text.contains("DONDE ESTAMOS"), "falta el estado");
        assert!(text.contains("QUE ARRIESGAMOS"), "falta la exposicion");
        assert!(text.contains("TRES PRIMERAS ACCIONES"), "faltan las acciones");
        assert!(text.contains("Puntaje de cumplimiento"), "falta el puntaje");
    }

    #[test]
    fn the_executive_report_fits_on_one_page() {
        let tmp = std::env::temp_dir().join("muniani_test_una_plana.pdf");
        write_executive_pdf(&con_brechas(), &crate::config::PoamConfig::default(),
            Papel::Carta, tmp.to_str().unwrap()).unwrap();
        let doc = Document::load(&tmp).unwrap();
        assert_eq!(doc.get_pages().len(), 1, "el ejecutivo tiene que caber en una plana");
    }

    // La cifra en UTM es la escala del Art. 40 por la cuenta de incumplimientos.
    // Presentarla sin decir que es un tope teorico la convertiria en un pronostico
    // de multa, que no es, y este producto no da asesoria legal.
    #[test]
    fn the_utm_figure_says_it_is_a_ceiling_and_not_a_forecast() {
        let text = ejecutivo(&con_brechas(), "muniani_test_utm.pdf");
        assert!(text.contains("Tope teorico"), "falta la palabra tope");
        assert!(text.contains("no una multa esperada"), "falta la aclaracion");
        assert!(text.contains("no constituye asesoria legal"), "falta el descargo");
        // 1 grave (10.000) + 1 leve (5.000) para un PSE.
        assert!(text.contains("15000"), "la suma no cuadra con la escala del Art. 40");
    }

    #[test]
    fn a_clean_scan_says_so_instead_of_showing_an_empty_plan() {
        let text = ejecutivo(&dummy(), "muniani_test_limpio.pdf");
        assert!(text.contains("No hay acciones pendientes"), "falta el caso sin brechas");
        assert!(text.contains("No se detectaron vulnerabilidades bajo explotacion"),
            "falta la lectura de KEV en un equipo limpio");
    }

    /// The lowest text baseline drawn in a rendered page's content stream.
    ///
    /// Se lee del operador `Td` de cada bloque de texto: es la unica forma de
    /// comprobar el layout de verdad y no de palabra.
    fn baseline_mas_baja(contenido: &str) -> f64 {
        contenido
            .lines()
            .filter(|l| l.trim_end().ends_with("Td"))
            .filter_map(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
            .fold(f64::MAX, f64::min)
    }

    // Con muchas brechas largas, la ultima se montaba encima de los avisos de
    // atribucion del pie, que son condicion de licencia de NVD, MITRE y el ASD.
    // El guard media el espacio DESPUES de dibujar; ahora calcula el alto antes.
    #[test]
    fn no_finding_invades_the_attribution_strip() {
        use crate::maturity::Domain;
        use crate::types::Gap;
        let mut r = dummy();
        let largo = "Existe un procedimiento interno que permita cumplir los plazos de reporte \
                     del Art. 9 con alerta temprana en 3 horas, actualizacion en 72 horas e \
                     informe final en 15 dias corridos, documentado y probado?";
        r.gaps = (0..40)
            .map(|i| Gap {
                control: format!("{i}. {largo}"),
                finding: format!("Control declarativo no cumplido: {largo}"),
                severity: Severity::High,
                legal_anchor: "Art. 9 Ley 21.663; incumplir el deber de reportar es infraccion \
                               grave (Art. 38, graves N 5)".into(),
                applies_to: AppliesTo::All,
                exigibilidad: Exigibilidad::Exigible,
                infraction_class: Some(InfractionClass::Grave),
                domain: Domain::ReporteIncidentes,
                evaluated: true,
                evidence: vec!["No respondido o declarado no cumplido".into()],
                requires_csirt_report: false,
            })
            .collect();

        for (papel, nombre) in [(Papel::Oficio, "muniani_test_piso_oficio.pdf"),
                                (Papel::Carta, "muniani_test_piso_carta.pdf")] {
            let tmp = std::env::temp_dir().join(nombre);
            write_pdf_con(&r, papel, tmp.to_str().unwrap()).unwrap();
            let doc = Document::load(&tmp).unwrap();
            let paginas = doc.get_pages();
            assert!(paginas.len() > 1, "{nombre}: 40 brechas tienen que paginar");

            for (n, (_, page_id)) in paginas.iter().enumerate() {
                let texto = String::from_utf8_lossy(&doc.get_page_content(*page_id).unwrap())
                    .into_owned();
                // Los avisos y el pie viven bajo PISO a proposito; lo que no puede
                // bajar de ahi es el contenido. Se cuenta cuantas lineas caen en la
                // franja: solo las del bloque de avisos mas el pie.
                let en_la_franja = texto.lines()
                    .filter(|l| l.trim_end().ends_with("Td"))
                    .filter_map(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
                    .filter(|&b| b < PISO)
                    .count();
                assert!(en_la_franja <= 8,
                    "{nombre} pag {}: {en_la_franja} lineas invaden la franja de atribuciones",
                    n + 1);
                assert!(baseline_mas_baja(&texto) > 10.0,
                    "{nombre} pag {}: hay texto fuera de la hoja", n + 1);
                assert!(texto.contains("MITRE"),
                    "{nombre} pag {}: sin la atribucion exigida por licencia", n + 1);
                assert!(texto.contains(&format!("Pagina {} de {}", n + 1, paginas.len())),
                    "{nombre} pag {}: sin numeracion", n + 1);
            }
        }
    }

    // El defecto que motivo la paginacion: el informe mostraba 4 de 16 hallazgos.
    #[test]
    fn every_finding_reaches_the_paper() {
        use crate::maturity::Domain;
        use crate::types::Gap;
        let mut r = dummy();
        r.gaps = (0..25)
            .map(|i| Gap {
                control: format!("Control numero {i}"),
                finding: format!("Hallazgo del control {i}"),
                severity: Severity::High,
                legal_anchor: "Art. 7 Ley 21.663".into(),
                applies_to: AppliesTo::All,
                exigibilidad: Exigibilidad::Exigible,
                infraction_class: None,
                domain: Domain::MedidasPermanentes,
                evaluated: true,
                evidence: vec![],
                requires_csirt_report: false,
            })
            .collect();

        let tmp = std::env::temp_dir().join("muniani_test_todas.pdf");
        write_pdf_con(&r, Papel::Oficio, tmp.to_str().unwrap()).unwrap();
        let doc = Document::load(&tmp).unwrap();
        let todo: String = doc.get_pages().values()
            .map(|id| String::from_utf8_lossy(&doc.get_page_content(*id).unwrap()).into_owned())
            .collect();

        for i in 0..25 {
            assert!(todo.contains(&format!("Control numero {i}")),
                "falta el control {i} en el PDF");
        }
        assert!(!todo.contains("NOTA: se muestran"), "no deberia sobrar ninguna");
        // La escala de sanciones dejaba de imprimirse cuando el contenido llegaba
        // al pie; ahora salta de pagina y siempre sale.
        assert!(todo.contains("ESCALA DE SANCIONES"), "falta la escala del Art. 40");
    }

    #[test]
    fn the_executive_path_sits_next_to_the_technical_one() {
        assert_eq!(executive_path("informe_brechas.pdf"), "informe_brechas_ejecutivo.pdf");
        assert_eq!(executive_path("C:\\x\\a.pdf"), "C:\\x\\a_ejecutivo.pdf");
        assert_eq!(executive_path("sin_extension"), "sin_extension_ejecutivo.pdf");
    }
}