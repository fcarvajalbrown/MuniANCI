//! Weekly rescan, scheduled without administrator privileges.
//!
//! ## Por qué una tarea por usuario y no un servicio
//!
//! Instalar un servicio de Windows exige privilegios de administrador, y el área de TI
//! de una municipalidad no siempre los tiene sobre el equipo donde corre el producto.
//! Una tarea registrada para la propia cuenta del usuario no los exige: la referencia
//! Win32 del Programador de tareas dice que solo un **disparador de arranque** obliga a
//! pertenecer al grupo Administradores.
//!
//! Verificado el 2026-07-25 en un PowerShell no elevado: la tarea se creó (código 0),
//! quedó en `Ready` con `Logon Mode: Interactive only`, y se borró sin problemas.
//!
//! ## Por qué se registra con XML y no con banderas
//!
//! Porque tres valores por defecto del Programador de tareas rompen el reescaneo en un
//! PC municipal, y **ninguno se puede fijar desde la línea de comandos**:
//!
//! | Ajuste | Por defecto | Qué provoca |
//! |---|---|---|
//! | `DisallowStartIfOnBatteries` | `true` | Un notebook desconectado a la hora programada no escanea |
//! | `StopIfGoingOnBatteries` | `true` | Si alguien lo desenchufa a mitad del barrido, se corta |
//! | `StartWhenAvailable` | `false` | El escaneo que se saltó por tener el equipo apagado no se recupera |
//!
//! Hay una cuarta razón, independiente: `schtasks /sd` interpreta la fecha según la
//! configuración regional del equipo. En este mismo equipo el Programador muestra
//! `26/07/2026`. Es la misma clase de error que la regresión de WMI documentada en
//! [`crate::patch_level`]. El XML usa ISO 8601 y no depende de la región.
//!
//! ## Esto se ve desde un EDR, y hay que decirlo
//!
//! Crear una tarea programada es la sub-técnica **T1053.005** de MITRE ATT&CK y queda
//! registrada en el **evento 4698** de Windows. En una municipalidad con antivirus
//! corporativo o EDR, MuniANCI creando una tarea puede levantar una alerta de
//! persistencia. Por eso [`ADVERTENCIA`] existe y la interfaz la muestra **antes** de
//! crear nada: una herramienta de cumplimiento que dispara una alerta sin avisar se
//! gana la desconfianza del área que más necesita confiar en ella.

use crate::config::MonitoreoConfig;

/// Nombre con que la tarea aparece en el Programador de tareas.
///
/// Explícito y aburrido a propósito: quien audite el equipo tiene que poder leer qué es
/// sin buscarlo, y quien opere el EDR tiene que poder reconocerlo en el evento 4698.
pub const NOMBRE_TAREA: &str = "MuniANCI - reescaneo de cumplimiento";

/// Lo que hay que decirle a TI municipal antes de crear la tarea.
pub const ADVERTENCIA: &str = "Se creara una tarea programada en Windows llamada \
    \"MuniANCI - reescaneo de cumplimiento\", para la cuenta de este usuario y sin \
    privilegios de administrador. Algunos antivirus corporativos y sistemas EDR \
    registran la creacion de tareas programadas como una senal de persistencia \
    (evento 4698 de Windows, tecnica T1053.005 de MITRE ATT&CK). Si su municipalidad \
    tiene EDR, avise al area que lo opera antes de continuar.";

/// What went wrong trying to schedule.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// El sistema rechazó la creación: GPO, EDR o política local.
    ///
    /// No es un fallo del producto y no puede presentarse como uno. La interfaz cae al
    /// aviso de escaneo vencido, que existe justamente para esto.
    #[error("Windows rechazo crear la tarea programada. Suele ser una politica de grupo \
             o un antivirus corporativo. Detalle: {0}")]
    Rechazada(String),

    #[error("no se pudo ejecutar schtasks.exe: {0}")]
    NoSePudoEjecutar(String),

    #[error("la programacion automatica solo esta implementada en Windows")]
    NoSoportado,

    #[error("{0}")]
    Io(String),
}

type Result<T> = std::result::Result<T, Error>;

/// Days of the week, as the Task Scheduler schema names them.
///
/// Se traduce acá y no en `config` porque el nombre en castellano es lo que TI escribe
/// en `munianci.config.json`, y el nombre en inglés es un detalle del esquema XML de
/// Microsoft que no tiene por qué salir a la superficie.
fn dia_xml(dia: &str) -> &'static str {
    match dia.trim().to_lowercase().as_str() {
        "lunes" => "Monday",
        "martes" => "Tuesday",
        "miercoles" | "miércoles" => "Wednesday",
        "jueves" => "Thursday",
        "viernes" => "Friday",
        "sabado" | "sábado" => "Saturday",
        // Domingo por defecto: es cuando la red municipal está más tranquila, y un
        // barrido de LAN completo compite con el trabajo de la gente.
        _ => "Sunday",
    }
}

/// Normalises `HH:MM`, falling back to 03:00 when the value is unusable.
///
/// Una hora mal escrita en la configuración no puede impedir que se programe el
/// reescaneo: se cae al valor por defecto, igual que hace `config::rgb` con un color
/// mal escrito.
fn hora_valida(hora: &str) -> String {
    let partes: Vec<&str> = hora.trim().split(':').collect();
    if partes.len() == 2 {
        if let (Ok(h), Ok(m)) = (partes[0].parse::<u32>(), partes[1].parse::<u32>()) {
            if h < 24 && m < 60 {
                return format!("{h:02}:{m:02}");
            }
        }
    }
    "03:00".to_string()
}

/// Builds the task definition XML.
///
/// Se arma como texto y no con una biblioteca de XML: son treinta líneas de esquema
/// fijo, y una dependencia más en un binario que se instala en PCs municipales se paga
/// en superficie de auditoría.
pub fn xml_tarea(ejecutable: &str, argumentos: &str, config: &MonitoreoConfig) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Reescaneo periodico de cumplimiento Ley 21.663. Generado por MuniANCI v{version}.</Description>
  </RegistrationInfo>
  <Triggers>
    <CalendarTrigger>
      <StartBoundary>{fecha}T{hora}:00</StartBoundary>
      <Enabled>true</Enabled>
      <ScheduleByWeek>
        <DaysOfWeek><{dia} /></DaysOfWeek>
        <WeeksInterval>{semanas}</WeeksInterval>
      </ScheduleByWeek>
    </CalendarTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <Enabled>true</Enabled>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <ExecutionTimeLimit>PT4H</ExecutionTimeLimit>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{cmd}</Command>
      <Arguments>{args}</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        version = env!("CARGO_PKG_VERSION"),
        // Fecha fija y en ISO: `StartBoundary` es obligatorio para un disparador de
        // calendario, pero solo marca desde cuando rige, no la proxima corrida.
        fecha = "2026-01-01",
        hora = hora_valida(&config.hora),
        dia = dia_xml(&config.dia_semana),
        semanas = config.intervalo_semanas.max(1),
        cmd = escapar(ejecutable),
        args = escapar(argumentos),
    )
}

/// Escapes the five XML entities.
///
/// La ruta del ejecutable la elige quien instala y puede traer `&`, y un `&` crudo
/// deja el XML mal formado y la tarea sin crear.
fn escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Encodes the XML as UTF-16LE with a BOM, which is what `schtasks /xml` expects.
fn utf16(texto: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for unidad in texto.encode_utf16() {
        bytes.extend_from_slice(&unidad.to_le_bytes());
    }
    bytes
}

/// Whether the task is currently registered.
#[cfg(windows)]
pub fn programada() -> Result<bool> {
    let salida = schtasks(&["/query", "/tn", NOMBRE_TAREA])?;
    Ok(salida.status.success())
}

#[cfg(not(windows))]
pub fn programada() -> Result<bool> {
    Err(Error::NoSoportado)
}

/// Registers the weekly rescan for the current user.
#[cfg(windows)]
pub fn programar(ejecutable: &str, argumentos: &str, config: &MonitoreoConfig) -> Result<()> {
    let xml = xml_tarea(ejecutable, argumentos, config);

    // El archivo va al directorio temporal del usuario y se borra al terminar: la ruta
    // se la pasamos a schtasks, que lo lee y no lo necesita despues.
    let ruta = std::env::temp_dir().join("munianci_tarea.xml");
    std::fs::write(&ruta, utf16(&xml)).map_err(|e| Error::Io(e.to_string()))?;

    let salida = schtasks(&[
        "/create",
        "/tn",
        NOMBRE_TAREA,
        "/xml",
        &ruta.to_string_lossy(),
        "/f",
    ]);
    let _ = std::fs::remove_file(&ruta);

    let salida = salida?;
    if salida.status.success() {
        Ok(())
    } else {
        Err(Error::Rechazada(mensaje(&salida)))
    }
}

#[cfg(not(windows))]
pub fn programar(_e: &str, _a: &str, _c: &MonitoreoConfig) -> Result<()> {
    Err(Error::NoSoportado)
}

/// Removes the task. `false` when there was nothing to remove.
#[cfg(windows)]
pub fn desprogramar() -> Result<bool> {
    if !programada()? {
        return Ok(false);
    }
    let salida = schtasks(&["/delete", "/tn", NOMBRE_TAREA, "/f"])?;
    if salida.status.success() {
        Ok(true)
    } else {
        Err(Error::Rechazada(mensaje(&salida)))
    }
}

#[cfg(not(windows))]
pub fn desprogramar() -> Result<bool> {
    Err(Error::NoSoportado)
}

/// Runs `schtasks.exe` without flashing a console window.
///
/// `CREATE_NO_WINDOW`: sin esto, al funcionario que aprieta el botón en la interfaz le
/// parpadea una ventana negra.
#[cfg(windows)]
fn schtasks(args: &[&str]) -> Result<std::process::Output> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    std::process::Command::new("schtasks.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| Error::NoSePudoEjecutar(e.to_string()))
}

/// The most useful line schtasks printed, for the error message.
#[cfg(windows)]
fn mensaje(salida: &std::process::Output) -> String {
    let texto = String::from_utf8_lossy(&salida.stderr);
    let texto = if texto.trim().is_empty() {
        String::from_utf8_lossy(&salida.stdout).into_owned()
    } else {
        texto.into_owned()
    };
    texto
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("sin detalle")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> MonitoreoConfig {
        MonitoreoConfig::default()
    }

    #[test]
    fn spanish_days_map_to_the_schema_names() {
        assert_eq!(dia_xml("lunes"), "Monday");
        assert_eq!(dia_xml("Miércoles"), "Wednesday");
        assert_eq!(dia_xml("miercoles"), "Wednesday", "sin tilde tambien");
        assert_eq!(dia_xml("SÁBADO"), "Saturday");
        assert_eq!(dia_xml("domingo"), "Sunday");
    }

    // Un dia mal escrito no puede impedir que se programe el reescaneo.
    #[test]
    fn an_unknown_day_falls_back_to_sunday() {
        assert_eq!(dia_xml("jueves santo"), "Sunday");
        assert_eq!(dia_xml(""), "Sunday");
    }

    #[test]
    fn the_hour_is_normalised_and_bad_values_fall_back() {
        assert_eq!(hora_valida("3:5"), "03:05");
        assert_eq!(hora_valida(" 23:59 "), "23:59");
        assert_eq!(hora_valida("24:00"), "03:00", "no existe la hora 24");
        assert_eq!(hora_valida("03:60"), "03:00");
        assert_eq!(hora_valida("tarde"), "03:00");
        assert_eq!(hora_valida(""), "03:00");
    }

    // Los tres ajustes que la linea de comandos no puede fijar, y que son la razon
    // entera de registrar por XML.
    #[test]
    fn the_xml_overrides_the_three_defaults_that_would_break_the_rescan() {
        let x = xml_tarea("C:\\muni\\munianci.exe", "--programado", &config());
        assert!(x.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"), "{x}");
        assert!(x.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"), "{x}");
        assert!(x.contains("<StartWhenAvailable>true</StartWhenAvailable>"), "{x}");
    }

    // Un disparador de arranque es lo unico que exigiria privilegios de
    // administrador, y una cuenta ajena tambien.
    #[test]
    fn the_task_never_asks_for_privileges_it_cannot_get() {
        let x = xml_tarea("C:\\muni\\munianci.exe", "--programado", &config());
        assert!(x.contains("<LogonType>InteractiveToken</LogonType>"), "{x}");
        assert!(x.contains("<RunLevel>LeastPrivilege</RunLevel>"), "{x}");
        assert!(!x.contains("BootTrigger"), "un disparador de arranque exige administrador");
        assert!(!x.contains("LogonTrigger"), "{x}");
        assert!(!x.contains("HighestAvailable"), "{x}");
        assert!(!x.contains("<UserId>"), "la cuenta la rellena Windows con la del que registra");
    }

    // schtasks /sd interpreta la fecha segun la region del equipo. El XML no.
    #[test]
    fn the_start_boundary_is_iso_and_not_locale_dependent() {
        let x = xml_tarea("x.exe", "", &config());
        assert!(x.contains("<StartBoundary>2026-01-01T03:00:00</StartBoundary>"), "{x}");
    }

    #[test]
    fn the_schedule_follows_the_configuration() {
        let c = MonitoreoConfig {
            dia_semana: "martes".into(),
            hora: "22:30".into(),
            intervalo_semanas: 2,
            ..config()
        };
        let x = xml_tarea("x.exe", "", &c);
        assert!(x.contains("<Tuesday />"), "{x}");
        assert!(x.contains("T22:30:00"), "{x}");
        assert!(x.contains("<WeeksInterval>2</WeeksInterval>"), "{x}");
    }

    // Un intervalo de cero semanas dejaria el disparador sin sentido.
    #[test]
    fn a_zero_interval_is_clamped_to_one_week() {
        let c = MonitoreoConfig { intervalo_semanas: 0, ..config() };
        assert!(xml_tarea("x.exe", "", &c).contains("<WeeksInterval>1</WeeksInterval>"));
    }

    // Una ruta con & dejaba el XML mal formado y la tarea sin crear.
    #[test]
    fn a_path_with_xml_characters_does_not_break_the_definition() {
        let x = xml_tarea("C:\\Archivos & Programas\\muni.exe", "--a \"b\" <c>", &config());
        assert!(x.contains("Archivos &amp; Programas"), "{x}");
        assert!(x.contains("&quot;b&quot;"), "{x}");
        assert!(x.contains("&lt;c&gt;"), "{x}");
        // Ningun & crudo fuera de las entidades.
        for (i, _) in x.match_indices('&') {
            let cola = &x[i..];
            assert!(
                ["&amp;", "&lt;", "&gt;", "&quot;", "&apos;"].iter().any(|e| cola.starts_with(e)),
                "& sin escapar en {}", &cola[..20.min(cola.len())]
            );
        }
    }

    // schtasks /xml no lee UTF-8: espera UTF-16 con marca de orden de bytes.
    #[test]
    fn the_xml_is_encoded_as_utf16_with_a_bom() {
        let b = utf16("<a/>");
        assert_eq!(&b[..2], &[0xFF, 0xFE], "falta la BOM");
        assert_eq!(&b[2..4], &[b'<', 0x00], "little endian");
        assert_eq!(b.len(), 2 + 4 * 2);
    }

    #[test]
    fn the_warning_names_what_an_edr_will_see() {
        assert!(ADVERTENCIA.contains("4698"), "el evento que registra Windows");
        assert!(ADVERTENCIA.contains("T1053.005"), "la tecnica de MITRE");
        assert!(ADVERTENCIA.contains("sin \nprivilegios") || ADVERTENCIA.contains("sin privilegios"));
        assert!(ADVERTENCIA.is_ascii(), "va a consola y a la interfaz");
    }

    #[test]
    fn the_task_name_is_boring_and_says_what_it_is() {
        assert!(NOMBRE_TAREA.starts_with("MuniANCI"));
        assert!(NOMBRE_TAREA.contains("reescaneo"));
    }

    // Fuera de Windows el modulo compila y responde que no aplica, para que el job
    // de `cargo check` en Linux siga verde.
    #[cfg(not(windows))]
    #[test]
    fn outside_windows_it_says_so_instead_of_failing_to_compile() {
        assert!(matches!(programada(), Err(Error::NoSoportado)));
        assert!(matches!(desprogramar(), Err(Error::NoSoportado)));
        assert!(matches!(programar("x", "", &config()), Err(Error::NoSoportado)));
    }
}
