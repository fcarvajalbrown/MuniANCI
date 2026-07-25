//! Dated, tamper-evident evidence package the municipality can hand to ANCI.
//!
//! ## Qué resuelve
//!
//! La ANCI puede pedir informes y hacer visitas inspectivas. Lo que se le presenta
//! tiene que estar fechado, completo y verificable, no ser un PDF suelto que alguien
//! mandó por correo y que nadie puede distinguir de otro editado después.
//!
//! Este módulo junta en una carpeta fechada todo lo que produjo un escaneo y le agrega
//! un manifiesto SHA-256.
//!
//! ## Qué NO es
//!
//! **No es una firma electrónica.** Bajo la Ley 19.799 solo una firma electrónica
//! avanzada de un prestador acreditado le da a un documento de un órgano del Estado la
//! calidad de instrumento público. Esto es otra cosa: verificación de integridad.
//! Permite detectar que un archivo cambió, y nada más.
//!
//! Tampoco se firma con un par de claves propio, y no es un olvido. La clave privada
//! viviría en el mismo PC municipal que la evidencia que firma, así que quien pudiera
//! alterar el informe podría volver a firmarlo. Agregaría gestión de claves sin agregar
//! seguridad.
//!
//! Y hay un límite que el propio paquete declara: **quien genera los hashes es quien
//! controla su verificación**. El manifiesto prueba que la carpeta no se alteró después
//! de escribirse; no prueba que el escaneo haya ocurrido como dice.
//!
//! ## Por qué se verifica con `certutil` y no con nuestro binario
//!
//! Porque un sello que solo puede comprobar la herramienta que lo puso no sirve de
//! nada. `certutil` y `Get-FileHash` vienen de fábrica en Windows, así que la
//! municipalidad —o quien la audite— verifica el paquete sin instalar nada.
//!
//! El manifiesto usa el formato de `sha256sum` (`<hash> *<archivo>`) y **se escribe con
//! LF**, aunque el resto del repositorio use CRLF: las implementaciones viejas y
//! no-GNU de `sha256sum -c` toman el CR como parte del nombre del archivo y la
//! verificación falla entera.
//!
//! ## Por qué una carpeta y no BagIt ni un ZIP
//!
//! BagIt (RFC 8493) es el estándar que hace esto y se evaluó en serio, pero obliga a
//! meter la carga útil en `data/`: el funcionario que abre la carpeta vería otra
//! carpeta en vez del PDF, a cambio de una validación que hoy nadie en la cadena usa.
//! Se le tomó prestado el `Payload-Oxum`, que detecta un paquete truncado antes de
//! calcular un solo hash. El ZIP no aporta nada que el Explorador de Windows no haga
//! con dos clics.

use crate::config::Config;
use crate::historico::{Delta, Deriva, Resumen};
use crate::types::ScanResult;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Nombre del manifiesto. En formato `sha256sum`, para que lo lea cualquier cosa.
pub const MANIFIESTO: &str = "MANIFIESTO.sha256";

/// Instrucciones en castellano llano, para quien no sepa qué es un hash.
pub const INSTRUCCIONES: &str = "COMO-VERIFICAR.txt";

/// El resumen del histórico que viaja con el paquete.
pub const RESUMEN_HISTORICO: &str = "resumen_historico.json";

/// One file inside the package, with the digest that pins it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Archivo {
    pub nombre: String,
    pub bytes: u64,
    /// SHA-256 en hexadecimal minúscula, que es lo que emite `certutil`.
    pub sha256: String,
}

/// What the package contains, returned so the caller can report it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paquete {
    pub ruta: PathBuf,
    pub archivos: Vec<Archivo>,
    /// `<octetos>.<cantidad de archivos>`, tomado del `Payload-Oxum` de BagIt.
    ///
    /// Detecta un paquete truncado o incompleto sin calcular un solo hash, que es
    /// para lo que el RFC 8493 lo define.
    pub oxum: String,
}

impl Paquete {
    /// Total size of the packaged evidence, in bytes.
    pub fn bytes(&self) -> u64 {
        self.archivos.iter().map(|a| a.bytes).sum()
    }
}

/// The history summary that travels with the package.
///
/// Va aparte del JSON CSIRT porque responde otra pregunta: el CSIRT quiere el estado de
/// hoy, y quien audita quiere saber de dónde viene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResumenHistorico {
    pub medicion: Resumen,
    pub delta: Option<Delta>,
    pub deriva: Option<Deriva>,
}

/// Folder name for a scan's evidence: `evidencia_<comuna>_<AAAA-MM-DD>`.
///
/// Fecha en ISO 8601 porque es el único formato donde el orden alfabético coincide con
/// el cronológico: las carpetas de un municipio quedan ordenadas solas en el
/// Explorador. El slug es el mismo de `historico::slug`, así que la evidencia, la base
/// del histórico y el `db_<comuna>` del Asistente se llaman igual.
pub fn nombre_carpeta(institucion: &str, fecha: &chrono::DateTime<chrono::Utc>) -> String {
    format!(
        "evidencia_{}_{}",
        crate::historico::slug(institucion),
        fecha.format("%Y-%m-%d")
    )
}

/// Writes the evidence package under `destino`, returning what it contains.
///
/// El manifiesto se escribe **al final**, después de que todo lo demás quedó en disco.
/// Es lo que hace detectable una corrida interrumpida: un paquete a medio escribir no
/// tiene manifiesto, y eso se ve. Escribirlo primero habría dejado carpetas incompletas
/// con un manifiesto internamente consistente, que se leen como válidas.
pub fn escribir(result: &ScanResult, config: &Config, destino: &Path) -> Result<Paquete> {
    let carpeta = destino.join(nombre_carpeta(&result.meta.institution_name, &result.scanned_at));
    std::fs::create_dir_all(&carpeta)
        .with_context(|| format!("no se pudo crear {}", carpeta.display()))?;

    let pdf = carpeta.join("informe_brechas.pdf");
    let json = carpeta.join("csirt_report.json");
    let pdf_str = pdf.to_string_lossy().to_string();

    crate::report_builder::build_con(
        result,
        config,
        &pdf_str,
        &json.to_string_lossy(),
        |_| {},
    )
    .context("no se pudieron generar los informes del paquete")?;

    crate::poam::write(result, &config.poam, &carpeta.join("poam.json"))
        .context("no se pudo escribir el plan de remediación del paquete")?;

    let resumen = ResumenHistorico {
        medicion: Resumen::de(result),
        delta: result.delta.clone(),
        deriva: result.deriva.clone(),
    };
    std::fs::write(
        carpeta.join(RESUMEN_HISTORICO),
        serde_json::to_string_pretty(&resumen)? + "\n",
    )
    .context("no se pudo escribir el resumen del histórico")?;

    // Las instrucciones van antes del manifiesto: tienen que quedar cubiertas por él.
    std::fs::write(carpeta.join(INSTRUCCIONES), instrucciones(result))
        .context("no se pudo escribir las instrucciones de verificación")?;

    let archivos = listar(&carpeta)?;
    std::fs::write(carpeta.join(MANIFIESTO), manifiesto(&archivos))
        .context("no se pudo escribir el manifiesto")?;

    Ok(Paquete {
        ruta: carpeta,
        oxum: oxum(&archivos),
        archivos,
    })
}

/// Hashes every file directly inside `dir`, sorted by name.
///
/// El orden es explícito y no el que devuelve el sistema de archivos: `read_dir` no
/// garantiza ninguno, depende de la plataforma y **puede cambiar entre llamadas**. Sin
/// ordenar, dos corridas sobre el mismo contenido producen manifiestos distintos.
fn listar(dir: &Path) -> Result<Vec<Archivo>> {
    let mut archivos = Vec::new();
    for entrada in std::fs::read_dir(dir)
        .with_context(|| format!("no se pudo leer {}", dir.display()))?
    {
        let entrada = entrada?;
        if !entrada.file_type()?.is_file() {
            continue;
        }
        let nombre = entrada.file_name().to_string_lossy().to_string();
        // El manifiesto no puede contenerse a sí mismo.
        if nombre == MANIFIESTO {
            continue;
        }
        let datos = std::fs::read(entrada.path())
            .with_context(|| format!("no se pudo leer {nombre}"))?;
        archivos.push(Archivo {
            bytes: datos.len() as u64,
            sha256: hex(&Sha256::digest(&datos)),
            nombre,
        });
    }
    archivos.sort_by(|a, b| a.nombre.cmp(&b.nombre));
    Ok(archivos)
}

/// Lowercase hex, which is what `certutil` prints.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// BagIt's Payload-Oxum: `<octetos>.<cantidad de archivos>`.
///
/// Se devuelve en [`Paquete`] y lo imprime la CLI, pero **no se escribe dentro del
/// paquete**, y no por descuido: cualquier archivo que lo contuviera quedaría cubierto
/// por el manifiesto y contaría para el propio Oxum, así que el número nunca podría
/// describir al paquete que lo lleva. Se detectó al leer la primera carpeta generada,
/// donde el texto decía cinco archivos y el manifiesto listaba seis.
fn oxum(archivos: &[Archivo]) -> String {
    format!(
        "{}.{}",
        archivos.iter().map(|a| a.bytes).sum::<u64>(),
        archivos.len()
    )
}

/// The manifest, in `sha256sum` format.
///
/// Se arma con `\n` explícito y no con `writeln!`: en Windows el CRLF haría que
/// `sha256sum -c` tomara el retorno de carro como parte del nombre del archivo y
/// fallara la verificación completa. El `*` marca modo binario, que es lo correcto
/// para los PDF.
fn manifiesto(archivos: &[Archivo]) -> String {
    let mut out = String::new();
    for a in archivos {
        out.push_str(&format!("{} *{}\n", a.sha256, a.nombre));
    }
    out
}

/// Plain-Spanish verification instructions.
///
/// Sin tildes y con CRLF a propósito. Este archivo lo abre el Bloc de notas en un PC
/// municipal que puede ser viejo: los saltos de línea sueltos se veían como un único
/// párrafo, y las tildes en UTF-8 sin BOM salían como símbolos.
fn instrucciones(result: &ScanResult) -> String {
    let l = [
        "COMO VERIFICAR ESTE PAQUETE DE EVIDENCIA".to_string(),
        "=========================================".to_string(),
        String::new(),
        format!("Institucion : {}", crate::historico::slug(&result.meta.institution_name)),
        format!("Escaneo     : {}", result.scanned_at.format("%Y-%m-%d %H:%M:%S UTC")),
        format!("Generado por: MuniANCI v{}", env!("CARGO_PKG_VERSION")),
        String::new(),
        "QUE ES ESTO".to_string(),
        "-----------".to_string(),
        "El archivo MANIFIESTO.sha256 contiene una huella digital (SHA-256) de cada".to_string(),
        "archivo de esta carpeta. Si un archivo cambia, aunque sea en un solo byte, su".to_string(),
        "huella cambia. Eso permite comprobar que el paquete esta tal como se emitio.".to_string(),
        String::new(),
        "COMO COMPROBARLO EN WINDOWS".to_string(),
        "---------------------------".to_string(),
        "No hay que instalar nada. Abra esta carpeta, escriba cmd en la barra de".to_string(),
        "direccion del Explorador y presione Enter. Despues, por cada archivo:".to_string(),
        String::new(),
        "    certutil -hashfile informe_brechas.pdf SHA256".to_string(),
        String::new(),
        "Compare el valor que aparece con el que trae MANIFIESTO.sha256 para ese".to_string(),
        "archivo. Tienen que ser iguales. No importa si uno esta en mayusculas y el".to_string(),
        "otro en minusculas: es el mismo valor.".to_string(),
        String::new(),
        "Tambien sirve PowerShell:".to_string(),
        String::new(),
        "    Get-FileHash informe_brechas.pdf -Algorithm SHA256".to_string(),
        String::new(),
        "QUE SIGNIFICA Y QUE NO SIGNIFICA".to_string(),
        "--------------------------------".to_string(),
        "Esto es una VERIFICACION DE INTEGRIDAD. NO es una firma electronica.".to_string(),
        String::new(),
        "Bajo la Ley 19.799, solo una firma electronica avanzada emitida por un".to_string(),
        "prestador acreditado le da a un documento de un organo del Estado la calidad".to_string(),
        "de instrumento publico. Este paquete no la tiene. Si la municipalidad".to_string(),
        "necesita ese efecto, debe firmarlo por su cuenta; consulte a un abogado.".to_string(),
        String::new(),
        "Dos limites mas, que conviene tener claros:".to_string(),
        String::new(),
        "1. La fecha de este paquete es la del reloj del equipo que lo genero. No hay".to_string(),
        "   sello de tiempo de un tercero, porque eso exigiria una conexion de red y".to_string(),
        "   este producto funciona sin salir del equipo.".to_string(),
        String::new(),
        "2. Las huellas las calculo la misma herramienta que produjo los informes. El".to_string(),
        "   manifiesto demuestra que la carpeta no se altero DESPUES de emitirse; no".to_string(),
        "   demuestra por si solo que el escaneo haya ocurrido como se describe.".to_string(),
        String::new(),
        "QUE CONTIENE LA CARPETA".to_string(),
        "-----------------------".to_string(),
        "  informe_brechas.pdf            Informe tecnico, por dominio".to_string(),
        "  informe_brechas_ejecutivo.pdf  Resumen de una plana".to_string(),
        "  csirt_report.json              Datos del escaneo en formato legible por maquina".to_string(),
        "  poam.json                      Plan de remediacion (formato OSCAL)".to_string(),
        "  resumen_historico.json         Evolucion respecto de la medicion anterior".to_string(),
        "  MANIFIESTO.sha256              Las huellas digitales".to_string(),
        "  COMO-VERIFICAR.txt             Este archivo".to_string(),
    ];
    // CRLF: este lo abre el Bloc de notas, no una herramienta de linea de comandos.
    l.join("\r\n") + "\r\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maturity::MaturityProfile;
    use crate::types::{AssetGraph, ScanMeta, Scope, Tier};
    use chrono::{TimeZone, Utc};

    fn resultado() -> ScanResult {
        ScanResult {
            meta: ScanMeta {
                institution_name: "Municipalidad de Ñuñoa".into(),
                tier: Tier::Pse,
                scope: Scope::Local,
            },
            asset_graph: AssetGraph::default(),
            gaps: vec![],
            cve_coverage: crate::cve::Coverage::default(),
            kev_provenance: "prueba".into(),
            taxonomia_anci: crate::taxonomia::TaxonomiaAnci::default(),
            score: crate::scoring::ComplianceScore::from_gaps(&[]),
            maturity: MaturityProfile::from_gaps(&[], &[]),
            delta: None,
            deriva: None,
            scanned_at: Utc.with_ymd_and_hms(2026, 7, 25, 14, 30, 0).unwrap(),
        }
    }

    fn escribir_en_temp() -> (tempfile::TempDir, Paquete) {
        let dir = tempfile::tempdir().unwrap();
        let p = escribir(&resultado(), &Config::default(), dir.path()).unwrap();
        (dir, p)
    }

    // La fecha en ISO 8601 es lo unico que hace que el orden alfabetico del
    // Explorador coincida con el cronologico.
    #[test]
    fn the_folder_is_named_after_the_comuna_and_an_iso_date() {
        let f = Utc.with_ymd_and_hms(2026, 7, 5, 0, 0, 0).unwrap();
        assert_eq!(
            nombre_carpeta("Municipalidad de Ñuñoa", &f),
            "evidencia_municipalidad_de_nunoa_2026-07-05"
        );
        // Con relleno de ceros: sin el, 2026-7-... ordena despues de 2026-10-...
        assert!(nombre_carpeta("X", &f).ends_with("2026-07-05"));
    }

    #[test]
    fn the_package_carries_every_expected_file() {
        let (_d, p) = escribir_en_temp();
        let nombres: Vec<&str> = p.archivos.iter().map(|a| a.nombre.as_str()).collect();
        for esperado in [
            "COMO-VERIFICAR.txt",
            "csirt_report.json",
            "informe_brechas.pdf",
            "informe_brechas_ejecutivo.pdf",
            "poam.json",
            "resumen_historico.json",
        ] {
            assert!(nombres.contains(&esperado), "falta {esperado} en {nombres:?}");
        }
        assert!(p.ruta.join(MANIFIESTO).exists());
    }

    // Lo que el paquete promete: que se puede detectar un cambio. Se comprueba
    // recalculando cada hash desde el disco, no confiando en lo que devolvimos.
    #[test]
    fn every_hash_in_the_manifest_matches_the_file_on_disk() {
        let (_d, p) = escribir_en_temp();
        let texto = std::fs::read_to_string(p.ruta.join(MANIFIESTO)).unwrap();

        assert_eq!(texto.lines().count(), p.archivos.len());
        for linea in texto.lines() {
            let (hash, nombre) = linea.split_once(" *").expect("formato sha256sum: {linea}");
            let datos = std::fs::read(p.ruta.join(nombre)).unwrap();
            assert_eq!(hash, hex(&Sha256::digest(&datos)), "no cuadra el hash de {nombre}");
        }
    }

    // Si esto no falla, el manifiesto no sirve para nada.
    #[test]
    fn altering_one_byte_breaks_the_manifest() {
        let (_d, p) = escribir_en_temp();
        let objetivo = p.ruta.join("csirt_report.json");
        let antes = std::fs::read(&objetivo).unwrap();

        let mut despues = antes.clone();
        *despues.last_mut().unwrap() ^= 0x01;
        std::fs::write(&objetivo, &despues).unwrap();

        let esperado = &p.archivos.iter().find(|a| a.nombre == "csirt_report.json").unwrap().sha256;
        let real = hex(&Sha256::digest(std::fs::read(&objetivo).unwrap()));
        assert_ne!(&real, esperado, "un byte cambiado tiene que cambiar la huella");
    }

    #[test]
    fn the_manifest_does_not_list_itself() {
        let (_d, p) = escribir_en_temp();
        assert!(!p.archivos.iter().any(|a| a.nombre == MANIFIESTO));
        let texto = std::fs::read_to_string(p.ruta.join(MANIFIESTO)).unwrap();
        assert!(!texto.contains(MANIFIESTO));
    }

    // Un CRLF hace que sha256sum -c tome el retorno de carro como parte del nombre
    // del archivo y falle la verificacion entera.
    #[test]
    fn the_manifest_uses_lf_even_on_windows() {
        let (_d, p) = escribir_en_temp();
        let bytes = std::fs::read(p.ruta.join(MANIFIESTO)).unwrap();
        assert!(!bytes.windows(2).any(|w| w == b"\r\n"), "el manifiesto no puede llevar CRLF");
        assert!(!bytes.contains(&b'\r'));
        assert_eq!(bytes.last(), Some(&b'\n'), "tiene que terminar en salto de linea");
    }

    // El formato de sha256sum: hash, espacio, asterisco (modo binario), nombre.
    #[test]
    fn the_manifest_is_in_sha256sum_format() {
        let (_d, p) = escribir_en_temp();
        for linea in std::fs::read_to_string(p.ruta.join(MANIFIESTO)).unwrap().lines() {
            let (hash, nombre) = linea.split_once(" *").expect("{linea}");
            assert_eq!(hash.len(), 64, "SHA-256 en hexadecimal son 64 caracteres: {linea}");
            assert!(hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "certutil emite minusculas: {linea}");
            assert!(!nombre.is_empty());
        }
    }

    // read_dir no garantiza orden y puede cambiar entre llamadas: sin ordenar, dos
    // corridas del mismo contenido darian manifiestos distintos.
    #[test]
    fn the_manifest_is_sorted_and_therefore_reproducible() {
        let (_d, p) = escribir_en_temp();
        let nombres: Vec<&String> = p.archivos.iter().map(|a| &a.nombre).collect();
        let mut ordenados = nombres.clone();
        ordenados.sort();
        assert_eq!(nombres, ordenados);
    }

    // El Payload-Oxum de BagIt: detecta un paquete truncado sin calcular un hash.
    #[test]
    fn the_oxum_counts_the_bytes_and_the_files() {
        let (_d, p) = escribir_en_temp();
        let (octetos, cuantos) = p.oxum.split_once('.').unwrap();
        assert_eq!(octetos.parse::<u64>().unwrap(), p.bytes());
        assert_eq!(cuantos.parse::<usize>().unwrap(), p.archivos.len());
        assert!(p.bytes() > 0);

        // Y cuadra con lo que hay en disco, no solo consigo mismo.
        let en_disco: Vec<_> = std::fs::read_dir(&p.ruta).unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy() != MANIFIESTO)
            .collect();
        assert_eq!(cuantos.parse::<usize>().unwrap(), en_disco.len());
    }

    // El Oxum no puede vivir dentro del paquete: el archivo que lo contuviera
    // contaria para el propio Oxum. La primera carpeta generada decia cinco
    // archivos mientras el manifiesto listaba seis.
    #[test]
    fn the_instructions_do_not_carry_a_self_referential_count() {
        let (_d, p) = escribir_en_temp();
        let t = std::fs::read_to_string(p.ruta.join(INSTRUCCIONES)).unwrap();
        assert!(!t.contains(&p.oxum), "el Oxum no puede ir dentro del paquete que cuenta");
        assert!(!t.contains(&p.bytes().to_string()), "{t}");
    }

    // El limite legal tiene que estar dicho con todas sus letras, no insinuado.
    #[test]
    fn the_instructions_say_this_is_not_an_electronic_signature() {
        let (_d, p) = escribir_en_temp();
        let t = std::fs::read_to_string(p.ruta.join(INSTRUCCIONES)).unwrap();
        assert!(t.contains("NO es una firma electronica"), "{t}");
        assert!(t.contains("19.799"), "tiene que citar la ley: {t}");
        assert!(t.contains("prestador acreditado"), "{t}");
        assert!(t.contains("consulte a un abogado"), "no damos asesoria legal: {t}");
    }

    // Los dos limites que el paquete no puede callar.
    #[test]
    fn the_instructions_declare_the_clock_and_the_self_attestation_limits() {
        let (_d, p) = escribir_en_temp();
        let t = std::fs::read_to_string(p.ruta.join(INSTRUCCIONES)).unwrap();
        assert!(t.contains("reloj del equipo"), "la fecha no la sella un tercero: {t}");
        assert!(t.contains("no\r\n   demuestra por si solo que el escaneo haya ocurrido"), "{t}");
    }

    #[test]
    fn the_instructions_show_the_commands_windows_already_has() {
        let (_d, p) = escribir_en_temp();
        let t = std::fs::read_to_string(p.ruta.join(INSTRUCCIONES)).unwrap();
        assert!(t.contains("certutil -hashfile"), "{t}");
        assert!(t.contains("Get-FileHash"), "{t}");
        // certutil emite minusculas y muchos publican en mayusculas.
        assert!(t.contains("mayusculas"), "hay que avisar que no distingue mayusculas: {t}");
    }

    // El Bloc de notas de un PC municipal viejo mostraba los saltos LF como un solo
    // parrafo, y las tildes en UTF-8 sin BOM como simbolos.
    #[test]
    fn the_instructions_are_notepad_safe() {
        let (_d, p) = escribir_en_temp();
        let bytes = std::fs::read(p.ruta.join(INSTRUCCIONES)).unwrap();
        assert!(bytes.is_ascii(), "sin tildes: el Bloc de notas viejo las rompe");
        let texto = String::from_utf8(bytes).unwrap();
        for linea in texto.split("\r\n") {
            assert!(!linea.contains('\n'), "todo salto tiene que ser CRLF: {linea:?}");
        }
    }

    // El resumen del historico responde otra pregunta que el JSON del CSIRT: de
    // donde viene esta medicion.
    #[test]
    fn the_history_summary_travels_with_the_package() {
        let (_d, p) = escribir_en_temp();
        let t = std::fs::read_to_string(p.ruta.join(RESUMEN_HISTORICO)).unwrap();
        let r: ResumenHistorico = serde_json::from_str(&t).unwrap();
        assert_eq!(r.medicion.fecha, resultado().scanned_at.to_rfc3339());
        assert_eq!(r.delta, None, "primera medicion");
        assert_eq!(r.deriva, None);
    }

    // Dos escaneos del mismo dia no pueden pisarse el paquete en silencio.
    #[test]
    fn writing_twice_over_the_same_day_leaves_a_consistent_package() {
        let dir = tempfile::tempdir().unwrap();
        let primero = escribir(&resultado(), &Config::default(), dir.path()).unwrap();
        let segundo = escribir(&resultado(), &Config::default(), dir.path()).unwrap();
        assert_eq!(primero.ruta, segundo.ruta);

        // Lo que importa no es que sean identicos byte a byte (los PDF llevan la
        // hora), sino que el manifiesto describa lo que hay en disco AHORA.
        let texto = std::fs::read_to_string(segundo.ruta.join(MANIFIESTO)).unwrap();
        for linea in texto.lines() {
            let (hash, nombre) = linea.split_once(" *").unwrap();
            let datos = std::fs::read(segundo.ruta.join(nombre)).unwrap();
            assert_eq!(hash, hex(&Sha256::digest(&datos)), "{nombre} quedo descuadrado");
        }
    }

    #[test]
    fn hex_is_lowercase_and_padded() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff]), "000fff");
        // Vector de prueba conocido de SHA-256 sobre la cadena vacia.
        assert_eq!(
            hex(&Sha256::digest(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
