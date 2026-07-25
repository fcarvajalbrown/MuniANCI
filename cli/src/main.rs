//! MuniANCI CLI — run a compliance scan and produce PDF + JSON reports.
use anyhow::{Context, Result};
use clap::Parser;
use muniani_core::{
    questionnaire::{Answer, QuestionnaireResponse, catalogue},
    report_builder,
    types::{ScanConfig, Scope, Tier},
};
use std::io::{self, Write};

#[derive(Parser)]
#[command(
    name    = "munianci",
    about   = "MuniANCI — escáner de cumplimiento Ley 21.663 / ANCI Chile",
    // Se lee del crate: una version escrita a mano queda obsoleta al primer release.
    version = env!("CARGO_PKG_VERSION"),
    author  = "Felipe Carvajal Brown",
)]
struct Cli {
    #[arg(long, value_enum, default_value = "pse", help = "Clasificación de la institución")]
    tier: CliTier,

    #[arg(long, value_enum, default_value = "local", help = "Alcance del escaneo")]
    scope: CliScope,

    #[arg(long, default_value = "Municipalidad de Ñuñoa", help = "Nombre de la institución")]
    name: String,

    #[arg(long, default_value = "informe_brechas.pdf", help = "Ruta del PDF de salida")]
    pdf: String,

    #[arg(long, default_value = "csirt_report.json", help = "Ruta del JSON de salida")]
    json: String,

    #[arg(long, default_value = "poam.json",
          help = "Ruta del plan de remediación en formato OSCAL POA&M")]
    poam: String,

    #[arg(long, help = "Omitir cuestionario declarativo (asume todo no cumplido)")]
    no_questionnaire: bool,

    #[arg(long, value_name = "RUTA",
          help = "Escribir un archivo de configuración de ejemplo y salir")]
    escribir_config: Option<String>,

    #[arg(long, value_name = "CARPETA",
          help = "Generar un paquete de evidencia fechado y sellado por hash en esta carpeta")]
    evidencia: Option<String>,

    #[arg(long, help = "Registrar el reescaneo periódico en el Programador de tareas y salir")]
    programar: bool,

    #[arg(long, help = "Quitar el reescaneo periódico del Programador de tareas y salir")]
    desprogramar: bool,

    #[arg(long, help = "Modo no interactivo, para las corridas del reescaneo programado")]
    programado: bool,
}

#[derive(clap::ValueEnum, Clone)]
enum CliTier { Oiv, Pse, Unclassified }

#[derive(clap::ValueEnum, Clone)]
enum CliScope { Local, Lan }

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Modo utilitario: escribe el ejemplo comentado y termina. Nadie configura lo
    // que no sabe que existe, y este archivo se explica solo.
    if let Some(ruta) = &cli.escribir_config {
        let path = std::path::Path::new(ruta);
        muniani_core::config::Config::escribir_ejemplo(path)
            .with_context(|| format!("no se pudo escribir {ruta}"))?;
        println!("Configuración de ejemplo escrita en {ruta}");
        println!("Déjala junto al ejecutable como {}, o apunta {} a esta ruta.",
            muniani_core::config::CONFIG_FILE_NAME,
            muniani_core::config::CONFIG_ENV);
        return Ok(());
    }

    let (config_ti, origen_config) = muniani_core::config::Config::load();

    // Modos utilitarios del reescaneo programado: hacen una cosa y terminan.
    if cli.programar || cli.desprogramar {
        return gestionar_programacion(&cli, &config_ti.monitoreo);
    }

    let tier = match cli.tier {
        CliTier::Oiv          => Tier::Oiv,
        CliTier::Pse          => Tier::Pse,
        CliTier::Unclassified => Tier::Unclassified,
    };
    let scope = match cli.scope {
        CliScope::Local => Scope::Local,
        CliScope::Lan   => Scope::Lan,
    };

    print_banner();
    println!("  Institución : {}", cli.name);
    println!("  Tier        : {tier}");
    println!("  Alcance     : {scope:?}");
    println!("  Config TI   : {origen_config}");
    println!();

    // Questionnaire phase.
    // Una corrida programada no tiene a nadie delante: preguntar la dejaria colgada
    // esperando una respuesta que no va a llegar.
    let questionnaire = if cli.no_questionnaire || cli.programado {
        println!("[!] Cuestionario omitido — todos los controles declarativos se asumen no cumplidos.");
        QuestionnaireResponse::default()
    } else {
        run_questionnaire(tier)?
    };

    // Scan phase.
    println!("\n[*] Iniciando escaneo...\n");
    let config = ScanConfig {
        institution_name: cli.name.clone(),
        tier,
        scope,
        red: config_ti.red.clone(),
        progress_cb: Some(Box::new(|pct| {
            print!("\r    Progreso: {pct:>3}%");
            io::stdout().flush().ok();
        })),
        log_cb: None, // CLI prints its own progress line — technical logs not needed here
    };

    let mut result = muniani_core::scan(config, questionnaire)
        .context("El escaneo falló")?;

    println!("\r    Progreso: 100%\n");

    // Histórico por comuna: se registra esta medición y se compara con la anterior.
    // Va acá y no dentro de core::scan porque el motor de escaneo no tiene por qué
    // saber si existen mediciones previas.
    if config_ti.historico.habilitado {
        match registrar_historico(&result, &config_ti.historico) {
            Ok((delta, deriva, total)) => {
                result.delta = delta;
                result.deriva = deriva;
                println!("  Histórico: {total} medición(es) registradas para esta institución.");
            }
            // Un histórico que falla no puede impedir la entrega del informe.
            Err(e) => eprintln!("[!] No se pudo actualizar el histórico: {e:#}"),
        }
    }

    // Summary.
    let critical = result.gaps.iter().filter(|g| matches!(g.severity, muniani_core::types::Severity::Critical)).count();
    let high     = result.gaps.iter().filter(|g| matches!(g.severity, muniani_core::types::Severity::High)).count();
    let medium   = result.gaps.iter().filter(|g| matches!(g.severity, muniani_core::types::Severity::Medium)).count();
    let csirt    = result.gaps.iter().filter(|g| g.requires_csirt_report).count();

    println!("  Brechas detectadas : {}", result.gaps.len());
    println!("    Críticas : {critical}  Altas : {high}  Medias : {medium}");

    imprimir_deriva(result.deriva.as_ref());

    println!("\n  Madurez por dominio (0 a 3):");
    for marco in muniani_core::maturity::Marco::all() {
        println!("\n    {}", marco.title());
        let dominios = result.maturity.domains_de(marco);
        for d in &dominios {
            println!("      {:<10} {:<38} {}", d.level.to_string(), d.domain, d.rationale);
        }
        let medidos = dominios.iter().filter(|d| d.level.value().is_some()).count();
        // Un promedio por marco y no uno solo: juntar la Ley 21.663 con el Decreto 7
        // daria un numero que no significa nada.
        match result.maturity.average_de(marco) {
            Some(avg) => println!("      Promedio: {avg:.1}/3 sobre {medidos} dominio(s) medido(s)."),
            None => println!("      Ningún dominio de este marco pudo medirse."),
        }
    }
    if csirt > 0 {
        println!("\n  *** {csirt} brecha(s) requieren reporte al CSIRT Nacional en ≤3h (Art. 9°) ***\n");
    }

    // Plan de remediación priorizado.
    let plan = muniani_core::poam::plan(&result.gaps, &config_ti.poam);
    if !plan.is_empty() {
        println!("\n  Plan de remediación (primeros {}):", plan.len().min(5));
        for item in plan.iter().take(5) {
            println!("    {}. [{} días] {}", item.orden, item.plazo_dias, item.gap.control);
            println!("       {}", item.motivo);
        }
    }

    // Output phase.
    println!("\n[*] Generando reportes...");
    report_builder::build_con(&result, &config_ti, &cli.pdf, &cli.json, |_| {})?;
    muniani_core::poam::write(&result, &config_ti.poam, std::path::Path::new(&cli.poam))
        .with_context(|| format!("no se pudo escribir {}", cli.poam))?;
    println!("    PDF tecnico   → {}  [{}]",
        cli.pdf, config_ti.informe.tamano_papel_tecnico.nombre());
    println!("    PDF ejecutivo → {}  [{}]",
        report_builder::executive_path(&cli.pdf), config_ti.informe.tamano_papel_ejecutivo.nombre());
    println!("    JSON          → {}", cli.json);
    println!("    POA&M         → {} (OSCAL {})", cli.poam, muniani_core::poam::OSCAL_VERSION);

    // Paquete de evidencia: los mismos entregables, fechados y sellados por hash, en
    // una carpeta que la municipalidad puede presentar. Va al final porque reusa los
    // generadores de arriba y no tiene sentido armarlo si alguno falló.
    if let Some(carpeta) = &cli.evidencia {
        match muniani_core::evidencia::escribir(&result, &config_ti, std::path::Path::new(carpeta)) {
            Ok(p) => {
                println!("\n[*] Paquete de evidencia:");
                println!("    Carpeta   → {}", p.ruta.display());
                println!("    Archivos  : {} ({} bytes, Oxum {})",
                    p.archivos.len(), p.bytes(), p.oxum);
                println!("    Manifiesto: {}", muniani_core::evidencia::MANIFIESTO);
                println!("    Verifíquelo con `certutil -hashfile <archivo> SHA256`.");
                println!("    Es verificación de integridad, no firma electrónica: ver {}.",
                    muniani_core::evidencia::INSTRUCCIONES);
            }
            // Que falle el paquete no puede invalidar los informes ya escritos.
            Err(e) => eprintln!("[!] No se pudo generar el paquete de evidencia: {e:#}"),
        }
    }

    println!("\n[+] Listo.\n");

    Ok(())
}

/// Registers or removes the scheduled rescan, then exits.
///
/// La advertencia sobre el EDR se imprime **antes** de crear nada: crear una tarea
/// programada es la técnica T1053.005 de MITRE y queda en el evento 4698 de Windows, así
/// que TI municipal tiene que poder avisarle a quien opere el antivirus corporativo.
fn gestionar_programacion(cli: &Cli, config: &muniani_core::config::MonitoreoConfig) -> Result<()> {
    use muniani_core::programacion;

    print_banner();

    if cli.desprogramar {
        return match programacion::desprogramar() {
            Ok(true) => {
                println!("  Reescaneo programado eliminado del Programador de tareas.\n");
                Ok(())
            }
            Ok(false) => {
                println!("  No habia ningun reescaneo programado.\n");
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("{e}")),
        };
    }

    println!("  AVISO");
    println!("  {}\n", programacion::ADVERTENCIA);

    let exe = std::env::current_exe()
        .context("no se pudo determinar la ruta del ejecutable")?
        .to_string_lossy()
        .to_string();

    // Se le pasa el mismo alcance y la misma institucion con que se invoco: una tarea
    // que escanea menos que el escaneo manual produciria deriva "sin verificar" para
    // siempre.
    let args = format!(
        "--programado --no-questionnaire --scope {} --name \"{}\"",
        match cli.scope { CliScope::Local => "local", CliScope::Lan => "lan" },
        cli.name,
    );

    match programacion::programar(&exe, &args, config) {
        Ok(()) => {
            println!("  Reescaneo programado: cada {} semana(s), {} a las {}.",
                config.intervalo_semanas.max(1), config.dia_semana, config.hora);
            println!("  Tarea: {}", programacion::NOMBRE_TAREA);
            println!("  Comando: {exe} {args}");
            println!("\n  Se puede quitar con `munianci --desprogramar`.\n");
            Ok(())
        }
        Err(e) => {
            eprintln!("[!] {e}");
            eprintln!("    El escaneo manual sigue funcionando igual. La aplicacion avisara");
            eprintln!("    cuando la medicion lleve mas de {} dias sin renovarse.", config.aviso_vencido_dias);
            std::process::exit(1);
        }
    }
}

/// Records the scan and returns the change against the previous measurement.
///
/// El archivo vive junto al ejecutable, con el mismo criterio que el resto de la
/// configuración: un solo lugar donde TI encuentra todo lo del producto.
fn registrar_historico(
    result: &muniani_core::types::ScanResult,
    config: &muniani_core::config::HistoricoConfig,
) -> Result<(
    Option<muniani_core::historico::Delta>,
    Option<muniani_core::historico::Deriva>,
    usize,
)> {
    use muniani_core::historico::{Delta, Historico, Resumen, nombre_archivo};

    let dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let ruta = dir.join(nombre_archivo(&result.meta.institution_name));

    let mut historico = Historico::abrir(&ruta)?;
    // Se lee el anterior ANTES de insertar: si no, el "anterior" sería este mismo.
    let previo = historico.ultimo()?;
    historico.registrar(result, config)?;
    let purgadas = historico.purgar(config)?;
    if purgadas > 0 {
        println!("  Histórico: {purgadas} medición(es) purgadas por retención.");
    }

    let delta = previo.map(|antes| Delta::entre(&antes, &Resumen::de(result)));
    // La deriva se calcula DESPUES de insertar: compara las dos ultimas mediciones,
    // y esta ya es una de ellas.
    let deriva = historico.deriva()?;
    let deriva = deriva.hay_comparacion().then_some(deriva);
    Ok((delta, deriva, historico.cuantos()?))
}

/// Prints the control-by-control drift against the previous measurement.
///
/// El orden no es alfabético: primero lo que empeoró. Una reaparecida arriba de
/// todo porque es la que dice que una corrección no se sostuvo, y eso es lo que
/// alguien tiene que ir a mirar hoy.
fn imprimir_deriva(deriva: Option<&muniani_core::historico::Deriva>) {
    use muniani_core::historico::Estado;

    let Some(d) = deriva else { return };

    println!("\n  Deriva desde {}:", d.desde.as_deref().unwrap_or("?"));
    println!("    {}", d.resumen());

    for (estado, titulo) in [
        (Estado::Reaparecida, "Reaparecidas (se habían corregido y volvieron)"),
        (Estado::Nueva, "Nuevas"),
        (Estado::Resuelta, "Resueltas"),
        (Estado::SinVerificar, "Sin verificar (este escaneo no las cubrió)"),
    ] {
        let items: Vec<_> = d.en(estado).collect();
        if items.is_empty() {
            continue;
        }
        println!("    {titulo}:");
        for c in items {
            match &c.resuelta_el {
                Some(f) => println!("      - {} (estaba resuelta el {})", c.control, &f[..10.min(f.len())]),
                None => println!("      - {}", c.control),
            }
        }
    }

    if !d.cobertura_comparable {
        println!(
            "    [!] Este escaneo cubrió menos que el anterior ({} -> {}). No se puede afirmar",
            d.alcance_antes.as_deref().unwrap_or("desconocido"),
            d.alcance_ahora.as_deref().unwrap_or("desconocido"),
        );
        println!("        que los controles técnicos que faltan se hayan corregido.");
    }
}

// ---------------------------------------------------------------------------
// Interactive questionnaire
// ---------------------------------------------------------------------------

fn run_questionnaire(tier: Tier) -> Result<QuestionnaireResponse> {
    // Se preguntan todas, no solo las exigibles: las que no obligan a este tier
    // se responden igual y se informan como madurez voluntaria.
    let questions = catalogue();

    if questions.is_empty() {
        return Ok(QuestionnaireResponse::default());
    }

    let exigibles = questions
        .iter()
        .filter(|q| q.applies_to.is_mandatory_for(tier))
        .count();

    println!("[*] Cuestionario declarativo ({} preguntas para tier {tier})", questions.len());
    println!("    {exigibles} exigible(s) por ley; {} se miden como madurez voluntaria.",
        questions.len() - exigibles);
    println!("    Responda s (sí/cumple) o n (no/no cumple). Enter = no cumple.\n");

    let mut answers = Vec::new();
    for (i, q) in questions.iter().enumerate() {
        let etiqueta = q.applies_to.exigibilidad_for(tier);
        println!("  [{}/{}] [{}] {}", i + 1, questions.len(), etiqueta, q.text);
        println!("        Anclaje:  {}", q.legal_anchor);
        println!("        Evidencia: {}", q.evidence_example);
        print!("  > ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let compliant = input.trim().to_lowercase().starts_with('s');

        answers.push(Answer {
            question_id: q.id,
            compliant,
            notes: None,
        });

        let consecuencia = match (compliant, etiqueta) {
            (true, _)  => "Cumple".to_string(),
            (false, muniani_core::types::Exigibilidad::Exigible) => {
                "No cumple — se registrará como brecha exigible".to_string()
            }
            (false, _) => "No cumple — se registrará como brecha de madurez (no exigible)".to_string(),
        };
        println!("    → {consecuencia}\n");
    }

    Ok(QuestionnaireResponse { answers })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn print_banner() {
    println!();
    println!("  ███╗   ███╗██╗   ██╗███╗   ██╗██╗ █████╗ ███╗   ██╗ ██████╗██╗");
    println!("  ████╗ ████║██║   ██║████╗  ██║██║██╔══██╗████╗  ██║██╔════╝██║");
    println!("  ██╔████╔██║██║   ██║██╔██╗ ██║██║███████║██╔██╗ ██║██║     ██║");
    println!("  ██║╚██╔╝██║██║   ██║██║╚██╗██║██║██╔══██║██║╚██╗██║██║     ██║");
    println!("  ██║ ╚═╝ ██║╚██████╔╝██║ ╚████║██║██║  ██║██║ ╚████║╚██████╗██║");
    println!("  ╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═══╝╚═╝╚═╝  ╚═╝╚═╝  ╚═══╝ ╚═════╝╚═╝");
    println!("  v{} — Escáner de Cumplimiento Ley 21.663 / ANCI Chile", env!("CARGO_PKG_VERSION"));
    println!("  Felipe Carvajal Brown\n");
}