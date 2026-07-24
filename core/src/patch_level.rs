//! Determining how up to date a machine's operating system patches are.
//!
//! ## Por qué hace falta
//!
//! El matcher CPE→CVE compara producto y versión. En un SO de Microsoft la
//! release viaja en el nombre del producto (`windows_10_22h2`) y el campo versión
//! es `-`, así que **cualquier** Windows 10 22H2 arrastra todas las CVE publicadas
//! contra 22H2 desde 2021, parchado o no. En una prueba real eso le adjudicó a un
//! equipo al día 81 CVE con explotación observada, entre ellas PrintNightmare
//! (2021), corregida hace años en esa máquina.
//!
//! La fecha del último parche acumulativo permite descartar las que ya están
//! cubiertas: las actualizaciones acumulativas de Windows contienen todas las
//! correcciones anteriores de su rama de servicio.
//!
//! ## Los límites de la aproximación (van al informe, no se ocultan)
//!
//! - No cubre correcciones fuera de banda ni actualizaciones opcionales.
//! - Una CVE publicada **antes** de la fecha de parche pero todavía sin corrección
//!   disponible se descartaría por error. Es el caso de los 0-day en espera.
//! - Solo aplica al sistema operativo. Para el software instalado la versión ya
//!   dice si la corrección está o no.
//!
//! ## El formato de fecha de WMI es un campo minado
//!
//! `Win32_QuickFixEngineering.InstalledOn` es un **string**, no una fecha: llega en
//! el formato de la cultura del sistema y, para hotfixes instalados fuera del
//! agente de Windows Update, puede llegar como un FILETIME hexadecimal.
//!
//! Dos trampas concretas, y las dos importan para el destinatario de este producto:
//!
//! 1. **El separador cambia.** Puede llegar `7/15/2026`, `15-07-2026` o `15.07.2026`
//!    según la configuración. Un parser que solo entienda `/` no lee ninguna fecha
//!    en un equipo con formato chileno, y entonces el filtro no descarta nada y el
//!    informe vuelve a sobreestimar el riesgo.
//! 2. **El orden día/mes cambia.** Chile escribe DD-MM-YYYY, Estados Unidos
//!    MM/DD/YYYY. `01-11-2026` es el 1 de noviembre en Chile y el 11 de enero allá.
//!
//! ### Por qué el orden NO se lee de la configuración regional
//!
//! Parecía lo obvio, y está medido que falla. En un equipo de prueba con locale
//! `en-GB` (patrón `dd/MM/yyyy`, día primero) WMI devolvió `7/15/2026` y
//! `4/21/2026`: 15 y 21 no pueden ser meses, o sea **WMI escribió MM/DD/YYYY
//! ignorando la configuración regional del sistema**. Haber creído en el patrón del
//! sistema daba como último parche el 2 de diciembre de 2026, una fecha futura, y
//! con eso se descartaban todas las CVE del SO.
//!
//! Por eso el orden se deduce de **los propios datos**: basta una entrada con un
//! componente mayor que 12 para fijar cuál es el día en toda la lista. Cuando la
//! lista entera es ambigua se toma la lectura **más antigua**: eso descarta menos
//! CVE, nunca más. Un informe de cumplimiento puede sobreestimar el riesgo, pero no
//! puede declarar parchado algo que quizá no lo está.
//!
//! Como última red, una fecha de instalación **futura** se descarta: es imposible, y
//! delata un formato mal leído antes de que llegue al informe.

use chrono::NaiveDate;

/// Whether the day or the month comes first in the strings being parsed.
///
/// Se deduce del conjunto de fechas observado, no de la configuración regional:
/// ver la nota del encabezado sobre por qué el locale del sistema miente aquí.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateOrder {
    /// DD-MM-YYYY, como escribe Chile.
    DayFirst,
    /// MM/DD/YYYY, como escribe Estados Unidos.
    MonthFirst,
    /// No se pudo determinar: se resuelve por la lectura más antigua.
    #[default]
    Unknown,
}

/// Segundos entre la época de FILETIME (1601-01-01) y la de Unix (1970-01-01).
const FILETIME_EPOCH_OFFSET_SECS: i64 = 11_644_473_600;

/// The most recent installation date among the raw strings WMI reports.
///
/// Devuelve `None` cuando ninguna entrada se pudo interpretar: eso significa "no
/// se sabe el nivel de parches", que no es lo mismo que "sin parches", y el
/// llamador debe tratarlo como tal.
pub fn latest_install_date(raw: &[String]) -> Option<NaiveDate> {
    latest_install_date_asof(raw, chrono::Utc::now().date_naive())
}

/// Same, with "today" injected so the future-date guard is testable.
pub fn latest_install_date_asof(raw: &[String], today: NaiveDate) -> Option<NaiveDate> {
    let order = infer_order(raw);
    raw.iter()
        .filter_map(|s| parse_installed_on(s, order))
        // Un parche instalado en el futuro no existe. Si aparece uno, la cadena se
        // leyó mal, y creerle descartaria CVE que si estan vigentes.
        .filter(|d| *d <= today)
        .max()
}

/// Works out the day/month order from the dates themselves.
///
/// Una sola entrada con un componente mayor que 12 basta para fijar el orden de
/// toda la lista, porque WMI las escribe todas con el mismo formato. Si la
/// evidencia se contradice, no se decide nada: es preferible caer en la lectura
/// conservadora que elegir la mitad de las fechas al azar.
pub fn infer_order(raw: &[String]) -> DateOrder {
    let mut day_first = 0usize;
    let mut month_first = 0usize;

    for s in raw {
        let s = s.trim();
        let parts: Vec<&str> = s.split(['/', '-', '.']).collect();
        if parts.len() != 3 {
            continue; // FILETIME, YYYYMMDD o basura: no aportan evidencia
        }
        if parts[0].trim().len() == 4 {
            continue; // ISO (YYYY-MM-DD): no es ambigua, no aporta evidencia
        }
        let (Ok(a), Ok(b)) = (parts[0].trim().parse::<u32>(), parts[1].trim().parse::<u32>()) else {
            continue;
        };
        if a > 12 && b <= 12 {
            day_first += 1;
        } else if b > 12 && a <= 12 {
            month_first += 1;
        }
    }

    match (day_first, month_first) {
        (0, 0) => DateOrder::Unknown,
        (d, 0) if d > 0 => DateOrder::DayFirst,
        (0, m) if m > 0 => DateOrder::MonthFirst,
        // Evidencia contradictoria: la lista mezcla formatos y no hay un orden
        // unico que aplicar.
        _ => DateOrder::Unknown,
    }
}

/// Parses one `InstalledOn` value in any of the shapes Windows actually emits.
pub fn parse_installed_on(raw: &str, order: DateOrder) -> Option<NaiveDate> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }

    // ISO, que es lo que entrega el registro de Windows Update.
    if let Some(d) = s.split(['T', ' ']).next() {
        if let Ok(date) = NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            return Some(date);
        }
    }

    // Fecha corta local. El separador depende de la configuración regional:
    // `/` en inglés, `-` en Chile, `.` en varias locales europeas.
    if s.contains(['/', '-', '.']) {
        return parse_local_short_date(s, order);
    }

    // FILETIME hexadecimal: 100 ns desde 1601-01-01.
    if s.len() >= 15 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        return u64::from_str_radix(s, 16).ok().and_then(from_filetime);
    }

    // YYYYMMDD compacto.
    if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) {
        return NaiveDate::parse_from_str(s, "%Y%m%d").ok();
    }

    None
}

/// Handles `a<sep>b<sep>YYYY`, resolving the day/month order.
///
/// Primero manda el dato duro: un componente mayor que 12 no puede ser un mes.
/// Si eso no basta, manda el orden declarado por el sistema. Si tampoco hay orden,
/// se elige la lectura que da la fecha **más antigua**, por la razón del encabezado.
fn parse_local_short_date(s: &str, order: DateOrder) -> Option<NaiveDate> {
    let parts: Vec<&str> = s.split(['/', '-', '.']).collect();
    if parts.len() != 3 {
        return None;
    }
    let a: u32 = parts[0].trim().parse().ok()?;
    let b: u32 = parts[1].trim().parse().ok()?;
    let year: i32 = parts[2].trim().parse().ok()?;
    if !(1980..=2200).contains(&year) {
        return None;
    }

    match (a > 12, b > 12) {
        // Uno de los dos no puede ser mes: el orden queda determinado por el dato.
        (true, false) => NaiveDate::from_ymd_opt(year, b, a),
        (false, true) => NaiveDate::from_ymd_opt(year, a, b),
        // Ambos > 12: ninguno es un mes válido.
        (true, true) => None,
        // Ambiguo: decide el orden del sistema, y si no lo hay, la más antigua.
        (false, false) => match order {
            DateOrder::DayFirst => NaiveDate::from_ymd_opt(year, b, a),
            DateOrder::MonthFirst => NaiveDate::from_ymd_opt(year, a, b),
            DateOrder::Unknown => {
                let as_md = NaiveDate::from_ymd_opt(year, a, b);
                let as_dm = NaiveDate::from_ymd_opt(year, b, a);
                match (as_md, as_dm) {
                    (Some(x), Some(y)) => Some(x.min(y)),
                    (x, y) => x.or(y),
                }
            }
        },
    }
}

/// Converts a Windows FILETIME tick count to a date.
fn from_filetime(ticks: u64) -> Option<NaiveDate> {
    let secs = (ticks / 10_000_000) as i64 - FILETIME_EPOCH_OFFSET_SECS;
    chrono::DateTime::from_timestamp(secs, 0).map(|dt| dt.date_naive())
}

/// Whether a CVE is already covered by the cumulative update installed on a host.
///
/// `published` llega como la fecha de publicación de NVD (`YYYY-MM-DD`). Si no se
/// conoce la fecha de parche, o no se conoce la de publicación, la respuesta es
/// `false`: sin dato no se afirma que algo esté corregido.
pub fn covered_by_patch(published: Option<&str>, last_patch: Option<NaiveDate>) -> bool {
    let (Some(published), Some(patch)) = (published, last_patch) else {
        return false;
    };
    let Some(day) = published.split(['T', ' ']).next() else {
        return false;
    };
    match NaiveDate::parse_from_str(day, "%Y-%m-%d") {
        // Microsoft publica la CVE junto con su corrección: publicada en o antes
        // del acumulativo instalado significa que el acumulativo la trae.
        Ok(d) => d <= patch,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    use DateOrder::{DayFirst, MonthFirst, Unknown};

    #[test]
    fn parses_the_us_style_string_wmi_returned_on_a_real_machine() {
        assert_eq!(parse_installed_on("7/15/2026", MonthFirst), Some(d(2026, 7, 15)));
    }

    // El caso chileno: separador guion y dia primero. Un parser que solo entienda
    // "/" no lee ninguna fecha en un equipo municipal, y el filtro no descarta nada.
    #[test]
    fn parses_the_chilean_short_date_with_dashes() {
        assert_eq!(parse_installed_on("15-07-2026", DayFirst), Some(d(2026, 7, 15)));
        assert_eq!(parse_installed_on("01-11-2026", DayFirst), Some(d(2026, 11, 1)));
        // Y sin saber el orden, la misma cadena sigue leyendose (mas antigua).
        assert_eq!(parse_installed_on("15-07-2026", Unknown), Some(d(2026, 7, 15)));
    }

    #[test]
    fn the_system_order_decides_an_otherwise_ambiguous_date() {
        assert_eq!(parse_installed_on("01/11/2026", DayFirst), Some(d(2026, 11, 1)));
        assert_eq!(parse_installed_on("01/11/2026", MonthFirst), Some(d(2026, 1, 11)));
    }

    #[test]
    fn without_a_known_order_an_ambiguous_date_takes_the_earlier_reading() {
        // 1/11/2026 es 11-ene o 1-nov segun la cultura. Se toma 11-ene: descarta
        // menos CVE, que es el error seguro.
        assert_eq!(parse_installed_on("1/11/2026", Unknown), Some(d(2026, 1, 11)));
        assert_eq!(parse_installed_on("11/1/2026", Unknown), Some(d(2026, 1, 11)));
    }

    #[test]
    fn a_day_over_twelve_pins_down_the_order_whatever_the_system_says() {
        // El dato manda sobre la configuracion regional: 25 no puede ser un mes.
        assert_eq!(parse_installed_on("25/12/2023", MonthFirst), Some(d(2023, 12, 25)));
        assert_eq!(parse_installed_on("12/25/2023", DayFirst), Some(d(2023, 12, 25)));
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // La lista literal que devolvio WMI en un equipo con locale en-GB
    // (patron dd/MM/yyyy). Se conserva como caso de regresion: creerle al locale
    // daba como ultimo parche el 2026-12-02, una fecha futura, y con eso se
    // descartaban TODAS las CVE del sistema operativo.
    const WMI_EN_GB_MACHINE: [&str; 16] = [
        "7/15/2026", "12/4/2023", "1/11/2026", "12/4/2023", "1/11/2026", "7/15/2026",
        "12/4/2023", "12/4/2023", "1/11/2026", "1/11/2026", "1/11/2026", "2/12/2026",
        "3/10/2026", "4/21/2026", "6/10/2026", "7/14/2026",
    ];

    #[test]
    fn the_order_comes_from_the_data_not_from_the_system_locale() {
        // 15 y 21 no pueden ser meses: la lista esta en MM/DD/YYYY pese a que el
        // equipo muestra las fechas como dd/MM/yyyy.
        assert_eq!(infer_order(&strings(&WMI_EN_GB_MACHINE)), MonthFirst);
    }

    #[test]
    fn the_real_machine_resolves_to_the_date_it_was_actually_patched() {
        let hoy = d(2026, 7, 24);
        assert_eq!(
            latest_install_date_asof(&strings(&WMI_EN_GB_MACHINE), hoy),
            Some(d(2026, 7, 15)),
        );
    }

    #[test]
    fn a_single_unambiguous_entry_settles_the_whole_list() {
        assert_eq!(infer_order(&strings(&["01/11/2026", "25/12/2023"])), DayFirst);
        assert_eq!(infer_order(&strings(&["01/11/2026", "12/25/2023"])), MonthFirst);
    }

    #[test]
    fn contradictory_evidence_decides_nothing() {
        // Una lista que mezcla formatos no tiene un orden unico que aplicar.
        assert_eq!(infer_order(&strings(&["25/12/2023", "12/25/2023"])), Unknown);
        assert_eq!(infer_order(&strings(&["01/11/2026"])), Unknown);
        assert_eq!(infer_order(&strings(&["2026-07-15"])), Unknown);
    }

    #[test]
    fn a_future_install_date_is_thrown_out_instead_of_believed() {
        let hoy = d(2026, 7, 24);
        let raw = strings(&["15-07-2026", "02-12-2026"]);
        // Sin la guarda, diciembre ganaria y se descartaria todo el catalogo.
        assert_eq!(latest_install_date_asof(&raw, hoy), Some(d(2026, 7, 15)));
    }

    #[test]
    fn rejects_what_it_cannot_read_instead_of_guessing() {
        assert_eq!(parse_installed_on("", Unknown), None);
        assert_eq!(parse_installed_on("N/A", Unknown), None);
        assert_eq!(parse_installed_on("31/31/2023", Unknown), None);
        assert_eq!(parse_installed_on("7/15/1200", Unknown), None);
    }

    #[test]
    fn parses_iso_and_compact_forms() {
        assert_eq!(parse_installed_on("2026-07-15", Unknown), Some(d(2026, 7, 15)));
        assert_eq!(parse_installed_on("2026-07-15 09:31:00", Unknown), Some(d(2026, 7, 15)));
        assert_eq!(parse_installed_on("20260715", Unknown), Some(d(2026, 7, 15)));
    }

    #[test]
    fn parses_a_hex_filetime() {
        // 2026-07-15 00:00:00 UTC en ticks de FILETIME.
        let secs = d(2026, 7, 15).and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
        let ticks = (secs + FILETIME_EPOCH_OFFSET_SECS) as u64 * 10_000_000;
        assert_eq!(parse_installed_on(&format!("{ticks:X}"), Unknown), Some(d(2026, 7, 15)));
    }

    #[test]
    fn latest_wins_over_the_rest() {
        // "15-07-2026" fija DD-MM para toda la lista, asi que 01-11 es 1 de nov.
        let raw = strings(&["12-04-2023", "15-07-2026", "basura", "01-11-2026"]);
        assert_eq!(latest_install_date_asof(&raw, d(2026, 12, 31)), Some(d(2026, 11, 1)));
    }

    #[test]
    fn nothing_readable_means_unknown_not_unpatched() {
        assert_eq!(latest_install_date_asof(&strings(&["basura"]), d(2026, 7, 24)), None);
        assert_eq!(latest_install_date_asof(&[], d(2026, 7, 24)), None);
    }

    #[test]
    fn a_cve_published_before_the_cumulative_update_is_covered() {
        assert!(covered_by_patch(Some("2021-07-01"), Some(d(2026, 7, 15))));
        assert!(covered_by_patch(Some("2026-07-15T00:00:00.000"), Some(d(2026, 7, 15))));
    }

    #[test]
    fn a_cve_published_after_the_update_is_not_covered() {
        assert!(!covered_by_patch(Some("2026-07-16"), Some(d(2026, 7, 15))));
    }

    #[test]
    fn without_a_date_nothing_is_declared_patched() {
        assert!(!covered_by_patch(None, Some(d(2026, 7, 15))));
        assert!(!covered_by_patch(Some("2021-07-01"), None));
        assert!(!covered_by_patch(Some("sin fecha"), Some(d(2026, 7, 15))));
    }
}
