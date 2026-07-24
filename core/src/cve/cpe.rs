//! CPE 2.3 parsing and version comparison.
//!
//! Solo se modela lo que el matching necesita: `part`, `vendor`, `product` y
//! `version`. Los demás componentes de un CPE 2.3 formateado se conservan sin
//! interpretar, porque para decidir si una CVE afecta a un paquete instalado no
//! aportan y sí introducirían formas nuevas de equivocarse.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// The `*` wildcard: "any value".
pub const ANY: &str = "*";

/// A CPE 2.3 identifier, reduced to the fields used for matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cpe {
    /// `a` (application), `o` (operating system) or `h` (hardware).
    pub part: String,
    pub vendor: String,
    pub product: String,
    pub version: String,
}

impl Cpe {
    /// Builds an application CPE from a vendor/product pair.
    pub fn application(vendor: &str, product: &str, version: &str) -> Self {
        Cpe {
            part: "a".into(),
            vendor: vendor.to_ascii_lowercase(),
            product: product.to_ascii_lowercase(),
            version: version.to_ascii_lowercase(),
        }
    }

    /// Parses a CPE 2.3 formatted string (`cpe:2.3:a:vendor:product:version:...`).
    ///
    /// Returns `None` for anything that is not a well-formed CPE 2.3 string. It
    /// does **not** try to rescue CPE 2.2 URIs or malformed input: a wrong CPE
    /// means wrong CVEs on a compliance report, so an unparseable identifier is
    /// dropped rather than guessed at.
    pub fn parse(s: &str) -> Option<Cpe> {
        let s = s.trim();
        let rest = s.strip_prefix("cpe:2.3:")?;

        // Split on ':' but honour the '\:' escape used inside CPE components.
        let mut fields: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut escaped = false;
        for c in rest.chars() {
            if escaped {
                current.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
                current.push(c);
            } else if c == ':' {
                fields.push(std::mem::take(&mut current));
            } else {
                current.push(c);
            }
        }
        fields.push(current);

        if fields.len() < 5 {
            return None;
        }
        let part = fields[0].to_ascii_lowercase();
        if !matches!(part.as_str(), "a" | "o" | "h") {
            return None;
        }

        Some(Cpe {
            part,
            vendor: fields[1].to_ascii_lowercase(),
            product: fields[2].to_ascii_lowercase(),
            version: fields[3].to_ascii_lowercase(),
        })
    }

    /// Whether this CPE identifies the same product as `other`, ignoring version.
    ///
    /// A `*` (ANY) on the criteria side matches anything; a `*` on the installed
    /// side does not, because "unknown vendor" must never match every vendor.
    pub fn same_product_as(&self, installed: &Cpe) -> bool {
        field_matches(&self.part, &installed.part)
            && field_matches(&self.vendor, &installed.vendor)
            && field_matches(&self.product, &installed.product)
    }
}

/// Criteria-side field comparison: ANY matches everything, otherwise exact.
fn field_matches(criteria: &str, installed: &str) -> bool {
    criteria == ANY || criteria == installed
}

impl std::fmt::Display for Cpe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cpe:2.3:{}:{}:{}:{}", self.part, self.vendor, self.product, self.version)
    }
}

// ---------------------------------------------------------------------------
// Version comparison
// ---------------------------------------------------------------------------

/// Compares two version strings the way vulnerability data expects.
///
/// No es semver: la base NVD mezcla `2014`, `10.0.19045`, `1.2.3a`, `8.0.0-rc1`.
/// Se comparan componente a componente, numéricamente cuando ambos lados son
/// números (para que `10` > `9`, que una comparación textual invertiría) y
/// alfabéticamente en caso contrario.
///
/// Cuando un lado se queda sin componentes, el más corto es menor salvo que el
/// resto del otro sean ceros: `1.2` y `1.2.0` son la misma versión.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let mut ita = split_version(a).into_iter();
    let mut itb = split_version(b).into_iter();

    loop {
        match (ita.next(), itb.next()) {
            (None, None) => return Ordering::Equal,
            // `a` es la más larga: el desempate lo decide su cola.
            (Some(x), None) => return compare_tail(x, &mut ita),
            // `b` es la más larga: mismo criterio, invertido.
            (None, Some(y)) => return compare_tail(y, &mut itb).reverse(),
            (Some(x), Some(y)) => match compare_component(&x, &y) {
                Ordering::Equal => continue,
                other => return other,
            },
        }
    }
}

/// Compares the longer version against the shorter one, given the components the
/// longer one still has left.
///
/// Tres casos, y el tercero es el que hace falta distinguir:
/// - ceros a la derecha no cambian nada: `1.2.0` == `1.2`;
/// - un número distinto de cero la hace mayor: `1.2.1` > `1.2`;
/// - un componente alfabético la hace **menor**, porque es un marcador de
///   pre-release: `1.0-rc1` < `1.0`.
fn compare_tail(first: String, rest: &mut impl Iterator<Item = String>) -> Ordering {
    let mut current = Some(first);
    while let Some(c) = current {
        match c.parse::<u64>() {
            Ok(0)  => current = rest.next(),
            Ok(_)  => return Ordering::Greater,
            Err(_) => return Ordering::Less,
        }
    }
    Ordering::Equal
}

fn compare_component(a: &str, b: &str) -> Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        // A numeric component outranks an alphabetic one: 1.0 is newer than
        // 1.0-rc, which is how pre-release suffixes behave in practice.
        (Ok(_), Err(_)) => Ordering::Greater,
        (Err(_), Ok(_)) => Ordering::Less,
        (Err(_), Err(_)) => a.cmp(b),
    }
}

/// Splits a version into comparable components, breaking on separators and also
/// at digit/letter boundaries so `1.2.3a` becomes `["1","2","3","a"]`.
fn split_version(v: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut current_is_digit: Option<bool> = None;

    for c in v.trim().chars() {
        if !c.is_ascii_alphanumeric() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            current_is_digit = None;
            continue;
        }
        let is_digit = c.is_ascii_digit();
        if current_is_digit.is_some_and(|prev| prev != is_digit) && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        current_is_digit = Some(is_digit);
        current.push(c.to_ascii_lowercase());
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_formatted_cpe() {
        let cpe = Cpe::parse("cpe:2.3:a:microsoft:sql_server:2014:sp3:*:*:*:*:x64:*").unwrap();
        assert_eq!(cpe.part, "a");
        assert_eq!(cpe.vendor, "microsoft");
        assert_eq!(cpe.product, "sql_server");
        assert_eq!(cpe.version, "2014");
    }

    #[test]
    fn parses_an_operating_system_cpe() {
        let cpe = Cpe::parse("cpe:2.3:o:microsoft:windows_10:-:*:*:*:*:*:*:*").unwrap();
        assert_eq!(cpe.part, "o");
        assert_eq!(cpe.product, "windows_10");
    }

    #[test]
    fn honours_the_colon_escape_inside_a_component() {
        let cpe = Cpe::parse(r"cpe:2.3:a:vendor:pro\:duct:1.0:*:*:*:*:*:*:*").unwrap();
        assert_eq!(cpe.product, r"pro\:duct");
        assert_eq!(cpe.version, "1.0");
    }

    #[test]
    fn rejects_malformed_input_instead_of_guessing() {
        assert!(Cpe::parse("").is_none());
        assert!(Cpe::parse("not a cpe").is_none());
        assert!(Cpe::parse("cpe:/a:microsoft:office").is_none(), "CPE 2.2 URI no se rescata");
        assert!(Cpe::parse("cpe:2.3:x:vendor:product:1.0").is_none(), "part invalido");
        assert!(Cpe::parse("cpe:2.3:a:vendor").is_none(), "faltan campos");
    }

    #[test]
    fn wildcard_on_the_criteria_side_matches_anything() {
        let criteria = Cpe::parse("cpe:2.3:a:apache:*:*:*:*:*:*:*:*:*").unwrap();
        let installed = Cpe::application("apache", "tomcat", "9.0.1");
        assert!(criteria.same_product_as(&installed));
    }

    #[test]
    fn wildcard_on_the_installed_side_does_not_match_everything() {
        // Un producto desconocido no puede empatar con todos los vendors: seria
        // la fabrica de falsos positivos que el informe no puede permitirse.
        let criteria = Cpe::application("microsoft", "office", "2016");
        let installed = Cpe::application("*", "*", "1.0");
        assert!(!criteria.same_product_as(&installed));
    }

    #[test]
    fn different_vendor_does_not_match() {
        let criteria = Cpe::application("f5", "nginx", "1.0");
        let installed = Cpe::application("nginx", "nginx", "1.0");
        assert!(!criteria.same_product_as(&installed));
    }

    #[test]
    fn numeric_components_compare_numerically() {
        assert_eq!(compare_versions("10.0", "9.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.2.10", "1.2.9"), Ordering::Greater);
        assert_eq!(compare_versions("2014", "2012"), Ordering::Greater);
    }

    #[test]
    fn trailing_zeros_do_not_change_the_version() {
        assert_eq!(compare_versions("1.2", "1.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.2.0.0", "1.2"), Ordering::Equal);
    }

    #[test]
    fn a_longer_nonzero_version_is_greater() {
        assert_eq!(compare_versions("1.2.1", "1.2"), Ordering::Greater);
        assert_eq!(compare_versions("1.2", "1.2.1"), Ordering::Less);
    }

    #[test]
    fn prerelease_suffix_is_older_than_the_release() {
        assert_eq!(compare_versions("1.0", "1.0-rc1"), Ordering::Greater);
        assert_eq!(compare_versions("8.0.0-beta", "8.0.0"), Ordering::Less);
    }

    #[test]
    fn letter_suffix_splits_from_the_number() {
        assert_eq!(split_version("1.2.3a"), vec!["1", "2", "3", "a"]);
        assert_eq!(compare_versions("1.2.3b", "1.2.3a"), Ordering::Greater);
    }

    #[test]
    fn real_windows_build_string_compares() {
        assert_eq!(compare_versions("10.0.19045", "10.0.19044"), Ordering::Greater);
    }

    #[test]
    fn equal_versions_are_equal() {
        assert_eq!(compare_versions("3.4.5", "3.4.5"), Ordering::Equal);
        assert_eq!(compare_versions("", ""), Ordering::Equal);
    }
}
