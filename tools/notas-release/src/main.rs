//! Extracts a CHANGELOG section and unwraps it for a GitHub release body.
//!
//! GitHub renderiza el cuerpo de un release con los saltos de linea duros
//! activados, mientras que un archivo `.md` los reflowea. Por eso una seccion
//! escrita a 85 columnas, que se lee perfecta en el repositorio, se publica
//! con el texto ocupando media pagina y el borde derecho dentado. Le paso al
//! release 0.4.0 y no se arregla acordandose: se arregla con esto.
//!
//! ```text
//! cargo run -q -p notas-release -- 0.5.0 > notas.md
//! gh release create v0.5.0 --notes-file notas.md
//! ```

use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(version) = std::env::args().nth(1) else {
        eprintln!("uso: notas-release <version> [ruta-del-changelog]");
        eprintln!("ejemplo: cargo run -q -p notas-release -- 0.5.0 > notas.md");
        return ExitCode::FAILURE;
    };
    let ruta = std::env::args().nth(2).unwrap_or_else(|| "CHANGELOG.md".into());

    let texto = match std::fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("no se pudo leer {ruta}: {e}");
            return ExitCode::FAILURE;
        }
    };

    match seccion(&texto, &version) {
        Some(s) => {
            print!("{}", desenvolver(&s));
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("{ruta} no tiene una seccion [{version}]");
            ExitCode::FAILURE
        }
    }
}

/// Returns the CHANGELOG block for `version`, heading included.
///
/// Acepta tanto `0.5.0` como `v0.5.0`, porque el tag lleva la v y la seccion no.
/// Se reconstruye desde `lines()` y no cortando el texto por indices: el archivo
/// viene con CRLF en Windows y contar bytes a mano se desalinea una posicion por
/// linea, lo que arrastraba el separador `---` de la version anterior.
fn seccion(changelog: &str, version: &str) -> Option<String> {
    let v = version.strip_prefix('v').unwrap_or(version);
    let encabezado = format!("## [{v}]");

    let inicio = changelog.lines().position(|l| l.starts_with(&encabezado))?;
    let resto: Vec<&str> = changelog.lines().skip(inicio).collect();
    let mut fin = resto
        .iter()
        .skip(1)
        .position(|l| l.starts_with("## ["))
        .map_or(resto.len(), |n| n + 1);

    // El separador `---` entre versiones pertenece al archivo, no a las notas.
    while fin > 0 && matches!(resto[fin - 1].trim(), "" | "---") {
        fin -= 1;
    }
    Some(resto[..fin].join("\n"))
}

/// Joins wrapped paragraph lines so GitHub renders them at full width.
///
/// Lo que **no** se toca, porque el salto de linea ahi es contenido y no
/// maquetado: bloques de codigo, tablas, encabezados y las lineas en blanco que
/// separan parrafos. Cada item de lista se une con sus lineas de continuacion
/// pero nunca con el item siguiente.
fn desenvolver(md: &str) -> String {
    let mut salida = String::with_capacity(md.len());
    let mut parrafo = String::new();
    let mut en_codigo = false;

    let cerrar = |parrafo: &mut String, salida: &mut String| {
        if !parrafo.is_empty() {
            salida.push_str(parrafo);
            salida.push('\n');
            parrafo.clear();
        }
    };

    for linea in md.lines() {
        let t = linea.trim();

        if t.starts_with("```") {
            cerrar(&mut parrafo, &mut salida);
            en_codigo = !en_codigo;
            salida.push_str(linea);
            salida.push('\n');
            continue;
        }
        if en_codigo {
            salida.push_str(linea);
            salida.push('\n');
            continue;
        }

        // Fronteras duras: nada se une a traves de ellas.
        if t.is_empty() || t.starts_with('#') || t.starts_with('|') || t == "---" {
            cerrar(&mut parrafo, &mut salida);
            salida.push_str(linea);
            salida.push('\n');
            continue;
        }

        if empieza_item(t) {
            // Un item nuevo cierra el anterior, no lo continua.
            cerrar(&mut parrafo, &mut salida);
            parrafo.push_str(linea);
        } else if parrafo.is_empty() {
            parrafo.push_str(linea);
        } else {
            parrafo.push(' ');
            parrafo.push_str(t);
        }
    }
    cerrar(&mut parrafo, &mut salida);
    salida
}

/// True if the line opens a bullet or numbered list item.
fn empieza_item(t: &str) -> bool {
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        return true;
    }
    // "1. ", "12) " y demas.
    let digitos: String = t.chars().take_while(char::is_ascii_digit).collect();
    !digitos.is_empty()
        && t[digitos.len()..]
            .starts_with(['.', ')'])
        && t[digitos.len() + 1..].starts_with(' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    const EJEMPLO: &str = "\
# Changelog

---

## [0.5.0] - 2026-07-24 - potencia del escaner

Esta version agrega descubrimiento de red nativo, que
antes no existia pese a lo que decia el codigo.

### Added

- **ARP y ICMP** via APIs Win32, sin exigir privilegios
  de administrador ni Npcap.
- Segundo item, que no debe fusionarse con el anterior.

| Metodo | Evidencia |
|---|---|
| ARP | capa 2 |

```powershell
cargo test --workspace
```

---

## [0.4.0] - 2026-07-12 - empaquetado

Version anterior.

---
";

    #[test]
    fn extrae_solo_la_seccion_pedida() {
        let s = seccion(EJEMPLO, "0.5.0").unwrap();
        assert!(s.starts_with("## [0.5.0]"), "{s}");
        assert!(!s.contains("0.4.0"), "se colo la version anterior");
        assert!(!s.trim_end().ends_with("---"), "arrastro el separador:\n{s}");
    }

    #[test]
    fn un_changelog_con_crlf_no_arrastra_el_separador_anterior() {
        // El CHANGELOG del repo tiene CRLF. Contando bytes a mano el corte se
        // desalineaba una posicion por linea y el `---` de la version previa
        // aparecia encima del encabezado.
        let crlf = EJEMPLO.replace('\n', "\r\n");
        let s = seccion(&crlf, "0.5.0").unwrap();
        assert!(s.starts_with("## [0.5.0]"), "{s}");
        assert!(!s.contains('\r'), "quedaron retornos de carro:\n{s:?}");
    }

    #[test]
    fn acepta_el_tag_con_v() {
        assert_eq!(seccion(EJEMPLO, "v0.5.0"), seccion(EJEMPLO, "0.5.0"));
    }

    #[test]
    fn una_version_inexistente_no_devuelve_nada() {
        assert!(seccion(EJEMPLO, "9.9.9").is_none());
        // Y no confunde 0.5.0 con un prefijo de otra version.
        assert!(seccion(EJEMPLO, "0.5").is_none());
    }

    #[test]
    fn el_parrafo_envuelto_sale_en_una_linea() {
        let d = desenvolver(&seccion(EJEMPLO, "0.5.0").unwrap());
        assert!(
            d.contains("descubrimiento de red nativo, que antes no existia"),
            "el parrafo sigue partido:\n{d}"
        );
    }

    #[test]
    fn cada_item_de_lista_queda_en_su_linea() {
        let d = desenvolver(&seccion(EJEMPLO, "0.5.0").unwrap());
        // El item se une con su continuacion...
        assert!(d.contains("sin exigir privilegios de administrador ni Npcap."), "{d}");
        // ...pero no con el item siguiente.
        assert!(
            d.contains("\n- Segundo item"),
            "los dos items se fusionaron:\n{d}"
        );
    }

    #[test]
    fn tablas_encabezados_y_codigo_conservan_sus_saltos() {
        let d = desenvolver(&seccion(EJEMPLO, "0.5.0").unwrap());
        assert!(d.contains("| Metodo | Evidencia |\n|---|---|\n| ARP | capa 2 |"), "{d}");
        assert!(d.contains("```powershell\ncargo test --workspace\n```"), "{d}");
        assert!(d.contains("\n### Added\n"), "{d}");
    }

    #[test]
    fn ninguna_linea_de_prosa_queda_a_media_pagina() {
        // La regresion que motivo esta herramienta: el 0.4.0 se publico con
        // todos los parrafos cortados a ~85 columnas.
        let d = desenvolver(&seccion(EJEMPLO, "0.5.0").unwrap());
        let lineas: Vec<&str> = d.lines().collect();
        for (i, l) in lineas.iter().enumerate() {
            let siguiente_es_prosa = lineas
                .get(i + 1)
                .is_some_and(|n| !n.trim().is_empty() && !n.starts_with(['#', '|', '`', '-']));
            assert!(
                !(l.len() > 60 && l.len() < 90 && siguiente_es_prosa),
                "linea {i} sigue envuelta: {l}"
            );
        }
    }
}
