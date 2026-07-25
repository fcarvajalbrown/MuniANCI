//! Embedded TrueType fonts for the PDF, so Spanish prints as Spanish.
//!
//! ## Por qué existe
//!
//! El informe usaba las catorce fuentes estándar de PDF (Helvetica, Courier). Ninguna
//! puede representar `ñ` ni las vocales acentuadas con la codificación por defecto, así
//! que el generador venía aplanando el texto desde marzo de 2026: "Contraseñas por
//! defecto" salía impreso "Contrasenas por defecto", y "Art. 9°" perdía el grado.
//!
//! Es el documento que la municipalidad le presenta a la ANCI. Un informe de
//! cumplimiento que no sabe escribir el nombre de sus propios controles se lee como
//! descuidado, y con razón.
//!
//! ## Qué se embebe y por qué esta familia
//!
//! **IBM Plex**, bajo SIL Open Font License 1.1, que permite embeber y redistribuir.
//! No es una elección estética: es la familia que la interfaz del producto ya usa, así
//! que el PDF y la pantalla se ven como el mismo producto, y su licencia ya estaba
//! resuelta en este repositorio.
//!
//! Las fuentes del sistema quedaban descartadas de entrada: Arial, Calibri y Segoe UI
//! son de Microsoft y no se pueden redistribuir dentro de un producto.
//!
//! ## Por qué WinAnsiEncoding y no Identity-H
//!
//! Porque el castellano entero cabe en cp1252. Un font CID con `Identity-H` obligaría a
//! mapear cada carácter a su glifo y a mantener un `ToUnicode` para que el texto se
//! pueda copiar y buscar; con `WinAnsiEncoding` el byte 0xF1 **es** la `ñ`, y buscar
//! "Contraseñas" en el lector funciona sin nada más.
//!
//! ## El tamaño
//!
//! Cada cara son unos 200 kB. Se embeben comprimidas con Flate —el mismo `flate2` que
//! ya usa el índice de CVE—, lo que las deja en torno a la mitad. El informe pasa de
//! ~20 kB a unos pocos cientos de kB: es lo que cuesta que el documento diga "Ñuñoa".

use lopdf::{dictionary, Document, Object, Stream};

/// The three faces the report draws with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cara {
    /// Texto corrido.
    Regular,
    /// Títulos y énfasis.
    Negrita,
    /// Monoespaciada: notas al pie, avisos de atribución, evidencia.
    Mono,
}

impl Cara {
    /// The embedded font bytes.
    ///
    /// `include_bytes!` y no lectura en disco: el PDF tiene que poder generarse en un
    /// PC municipal donde solo se copió el ejecutable.
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Cara::Regular => include_bytes!("../../vendor/fonts/IBMPlexSans-Regular.ttf"),
            Cara::Negrita => include_bytes!("../../vendor/fonts/IBMPlexSans-Bold.ttf"),
            Cara::Mono => include_bytes!("../../vendor/fonts/IBMPlexMono-Regular.ttf"),
        }
    }

    /// Name used in the PDF font dictionary.
    pub fn base_font(self) -> &'static str {
        match self {
            Cara::Regular => "IBMPlexSans",
            Cara::Negrita => "IBMPlexSans-Bold",
            Cara::Mono => "IBMPlexMono",
        }
    }

    /// The resource name the content stream refers to.
    ///
    /// Se conservan los nombres cortos que el generador ya usaba (`FR`, `FB`, `FM`)
    /// para no tocar cada llamada de dibujo.
    pub fn recurso(self) -> &'static str {
        match self {
            Cara::Regular => "FR",
            Cara::Negrita => "FB",
            Cara::Mono => "FM",
        }
    }

    pub fn todas() -> [Cara; 3] {
        [Cara::Regular, Cara::Negrita, Cara::Mono]
    }
}

/// Primer y último código que se describen en el diccionario de la fuente.
///
/// De 32 (espacio) a 255: cubre ASCII imprimible y todo Latin-1, que es donde viven las
/// vocales acentuadas, la `ñ`, los signos de apertura y el símbolo de grado.
const PRIMER_CODIGO: u8 = 32;
const ULTIMO_CODIGO: u8 = 255;

/// Font metrics read from the TTF, scaled to the 1000-unit em PDF expects.
struct Metricas {
    anchos: Vec<i64>,
    ascenso: i64,
    descenso: i64,
    altura_mayusculas: i64,
    caja: [i64; 4],
}

/// Reads the advance widths and vertical metrics from the font file.
///
/// Se leen del TTF y no se escriben a mano: unos anchos inventados producen un PDF que
/// abre igual pero con el texto pisándose, que es peor que uno que no abre.
fn metricas(bytes: &[u8]) -> Option<Metricas> {
    let cara = ttf_parser::Face::parse(bytes, 0).ok()?;
    let em = f64::from(cara.units_per_em());
    let escala = |v: f64| (v * 1000.0 / em).round() as i64;

    let mut anchos = Vec::with_capacity(usize::from(ULTIMO_CODIGO - PRIMER_CODIGO) + 1);
    for codigo in PRIMER_CODIGO..=ULTIMO_CODIGO {
        // WinAnsiEncoding coincide con Latin-1 en todo el rango que le importa al
        // castellano, así que el código es el punto Unicode.
        let ancho = cara
            .glyph_index(char::from(codigo))
            .and_then(|g| cara.glyph_hor_advance(g))
            .map(|a| escala(f64::from(a)))
            .unwrap_or(0);
        anchos.push(ancho);
    }

    let caja = cara.global_bounding_box();
    Some(Metricas {
        anchos,
        ascenso: escala(f64::from(cara.ascender())),
        descenso: escala(f64::from(cara.descender())),
        altura_mayusculas: cara
            .capital_height()
            .map(|h| escala(f64::from(h)))
            .unwrap_or(700),
        caja: [
            escala(f64::from(caja.x_min)),
            escala(f64::from(caja.y_min)),
            escala(f64::from(caja.x_max)),
            escala(f64::from(caja.y_max)),
        ],
    })
}

/// Registers one embedded font and returns its dictionary id.
fn agregar(doc: &mut Document, cara: Cara) -> Option<lopdf::ObjectId> {
    let bytes = cara.bytes();
    let m = metricas(bytes)?;

    // El TTF va comprimido con Flate. `Length1` tiene que declarar el largo
    // *descomprimido*, que es lo que el lector espera encontrar al inflarlo.
    let mut archivo = Stream::new(
        dictionary! { "Length1" => bytes.len() as i64 },
        bytes.to_vec(),
    );
    let _ = archivo.compress();
    let archivo_id = doc.add_object(archivo);

    // Bit 3 (valor 4) = fuente simbólica; bit 6 (32) = no simbólica. Con
    // WinAnsiEncoding corresponde la no simbólica.
    const NO_SIMBOLICA: i64 = 32;

    let descriptor = doc.add_object(dictionary! {
        "Type"        => "FontDescriptor",
        "FontName"    => Object::Name(cara.base_font().into()),
        "Flags"       => NO_SIMBOLICA,
        "FontBBox"    => m.caja.iter().map(|v| Object::Integer(*v)).collect::<Vec<_>>(),
        "ItalicAngle" => 0,
        "Ascent"      => m.ascenso,
        "Descent"     => m.descenso,
        "CapHeight"   => m.altura_mayusculas,
        // StemV no se puede leer del TTF y ningun lector lo usa para dibujar; 80 es
        // el valor habitual para un peso regular.
        "StemV"       => 80,
        "FontFile2"   => archivo_id,
    });

    Some(doc.add_object(dictionary! {
        "Type"           => "Font",
        "Subtype"        => "TrueType",
        "BaseFont"       => Object::Name(cara.base_font().into()),
        "FirstChar"      => i64::from(PRIMER_CODIGO),
        "LastChar"       => i64::from(ULTIMO_CODIGO),
        "Widths"         => m.anchos.iter().map(|w| Object::Integer(*w)).collect::<Vec<_>>(),
        "Encoding"       => Object::Name("WinAnsiEncoding".into()),
        "FontDescriptor" => descriptor,
    }))
}

/// Builds the `/Font` resource dictionary with all three faces embedded.
///
/// Si una cara no se pudiera leer, se cae a la fuente estándar equivalente en vez de
/// no emitir el informe: un PDF sin tildes sigue siendo un PDF que se entrega.
pub fn diccionario(doc: &mut Document) -> lopdf::Dictionary {
    let mut fuentes = lopdf::Dictionary::new();
    for cara in Cara::todas() {
        match agregar(doc, cara) {
            Some(id) => fuentes.set(cara.recurso(), id),
            None => {
                let respaldo = doc.add_object(dictionary! {
                    "Type"     => "Font",
                    "Subtype"  => "Type1",
                    "BaseFont" => match cara {
                        Cara::Regular => "Helvetica",
                        Cara::Negrita => "Helvetica-Bold",
                        Cara::Mono    => "Courier",
                    },
                    "Encoding" => Object::Name("WinAnsiEncoding".into()),
                });
                fuentes.set(cara.recurso(), respaldo);
            }
        }
    }
    fuentes
}

/// Encodes text as the WinAnsi (cp1252) bytes the embedded fonts expect.
///
/// Reemplaza al aplanado de tildes que regía desde marzo de 2026. Todo el castellano
/// —vocales acentuadas, `ñ`, `¿`, `¡`, `°`— vive en Latin-1, donde el byte coincide con
/// el punto Unicode. Los pocos signos tipográficos que cp1252 ubica en 0x80-0x9F se
/// mapean a mano; lo que no exista en la codificación cae a `?`, que es visible y no
/// corrompe el archivo.
pub fn win_ansi(texto: &str) -> Vec<u8> {
    texto
        .chars()
        .map(|c| match c {
            '\u{20AC}' => 0x80, // €
            '\u{201A}' => 0x82,
            '\u{201E}' => 0x84,
            '\u{2026}' => 0x85, // …
            '\u{2020}' => 0x86,
            '\u{2021}' => 0x87,
            '\u{2030}' => 0x89,
            '\u{2018}' => 0x91, // comillas simples tipográficas
            '\u{2019}' => 0x92,
            '\u{201C}' => 0x93, // comillas dobles tipográficas
            '\u{201D}' => 0x94,
            '\u{2022}' => 0x95, // •
            '\u{2013}' => 0x96, // en dash
            '\u{2014}' => 0x97, // em dash
            '\u{2122}' => 0x99,
            c if (c as u32) <= 0xFF => c as u8,
            _ => b'?',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_face_carries_a_real_truetype_file() {
        for cara in Cara::todas() {
            let b = cara.bytes();
            assert!(b.len() > 50_000, "{:?} parece truncada: {} bytes", cara, b.len());
            // Firma de un TTF con contornos TrueType.
            assert_eq!(&b[..4], &[0x00, 0x01, 0x00, 0x00], "{:?} no es un TTF", cara);
            assert!(ttf_parser::Face::parse(b, 0).is_ok(), "{cara:?} no se pudo parsear");
        }
    }

    // Si el font no trae los glifos del castellano, embeberlo no arregla nada.
    #[test]
    fn the_fonts_actually_contain_the_spanish_glyphs() {
        for cara in Cara::todas() {
            let f = ttf_parser::Face::parse(cara.bytes(), 0).unwrap();
            for c in ['ñ', 'Ñ', 'á', 'é', 'í', 'ó', 'ú', 'ü', 'Á', '¿', '¡', '°'] {
                assert!(f.glyph_index(c).is_some(), "{cara:?} no tiene glifo para {c}");
            }
        }
    }

    #[test]
    fn the_widths_cover_every_declared_code_and_are_scaled_to_the_pdf_em() {
        let m = metricas(Cara::Regular.bytes()).unwrap();
        assert_eq!(m.anchos.len(), usize::from(ULTIMO_CODIGO - PRIMER_CODIGO) + 1);
        // El espacio y la ene tienen ancho; ninguno puede quedar en cero.
        let ancho = |c: char| m.anchos[c as usize - usize::from(PRIMER_CODIGO)];
        assert!(ancho(' ') > 0, "el espacio sin ancho pega las palabras");
        assert!(ancho('ñ') > 0, "{}", ancho('ñ'));
        assert!(ancho('A') > 0);
        // Escalados al em de 1000 que espera el PDF, no a las unidades del TTF.
        assert!(m.anchos.iter().all(|w| *w < 2000), "anchos sin escalar");
        assert!(m.ascenso > 0 && m.descenso < 0, "{} {}", m.ascenso, m.descenso);
    }

    // En una monoespaciada todos los glifos miden lo mismo. Si no, el terminal y el
    // pie del informe quedan desalineados.
    #[test]
    fn the_mono_face_is_actually_monospaced() {
        let m = metricas(Cara::Mono.bytes()).unwrap();
        let ancho = |c: char| m.anchos[c as usize - usize::from(PRIMER_CODIGO)];
        assert_eq!(ancho('i'), ancho('W'));
        assert_eq!(ancho('ñ'), ancho('n'));
    }

    // Lo que este modulo existe para arreglar.
    #[test]
    fn spanish_survives_the_encoding_instead_of_being_flattened() {
        assert_eq!(win_ansi("ñ"), vec![0xF1]);
        assert_eq!(win_ansi("Ñ"), vec![0xD1]);
        assert_eq!(win_ansi("á"), vec![0xE1]);
        assert_eq!(win_ansi("¿"), vec![0xBF]);
        assert_eq!(win_ansi("¡"), vec![0xA1]);
        assert_eq!(win_ansi("°"), vec![0xB0]);
        assert_eq!(win_ansi("Contraseñas"), b"Contrase\xF1as".to_vec());
        assert_eq!(win_ansi("Ñuñoa"), b"\xD1u\xF1oa".to_vec());
        assert_eq!(win_ansi("Art. 9°"), b"Art. 9\xB0".to_vec());
    }

    #[test]
    fn ascii_passes_through_untouched() {
        assert_eq!(win_ansi("DERIVA POR CONTROL"), b"DERIVA POR CONTROL".to_vec());
    }

    // Los signos tipograficos que cp1252 no pone donde Unicode.
    #[test]
    fn typographic_marks_land_on_their_cp1252_slots() {
        assert_eq!(win_ansi("—"), vec![0x97]);
        assert_eq!(win_ansi("–"), vec![0x96]);
        assert_eq!(win_ansi("\u{2019}"), vec![0x92]);
        assert_eq!(win_ansi("…"), vec![0x85]);
    }

    // Un caracter fuera de la codificacion tiene que verse, no corromper el archivo.
    #[test]
    fn what_cp1252_cannot_hold_becomes_a_visible_question_mark() {
        assert_eq!(win_ansi("中"), b"?".to_vec());
        assert_eq!(win_ansi("a中b"), b"a?b".to_vec());
    }

    #[test]
    fn the_font_dictionary_declares_all_three_resources() {
        let mut doc = Document::with_version("1.5");
        let d = diccionario(&mut doc);
        for cara in Cara::todas() {
            assert!(d.get(cara.recurso().as_bytes()).is_ok(), "falta {}", cara.recurso());
        }
    }

    // El archivo embebido tiene que ir comprimido y declarar su largo original, o el
    // lector no sabe cuanto inflar.
    #[test]
    fn the_embedded_file_declares_its_uncompressed_length() {
        let mut doc = Document::with_version("1.5");
        diccionario(&mut doc);
        let declarados: Vec<i64> = doc
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter_map(|s| s.dict.get(b"Length1").ok()?.as_i64().ok())
            .collect();
        assert_eq!(declarados.len(), 3, "una por cara");
        for (cara, largo) in Cara::todas().iter().zip(declarados.iter()) {
            assert!(
                declarados.contains(&(cara.bytes().len() as i64)),
                "Length1 {largo} no corresponde a ninguna cara"
            );
        }
    }
}
