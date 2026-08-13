//! MuniGPT CLI — run a compliance scan and produce PDF + JSON reports.
use anyhow::{Context, Result};
use clap::Parser;
use munigpt_core::{
    historico,
    questionnaire::{Answer, QuestionnaireResponse, catalogue},
    report_builder,
    types::{ScanConfig, Scope, Tier},
};
use std::io::{self, Write};

#[derive(Parser)]
#[command(
    name    = "munigpt",
    about   = "MuniGPT — escáner de cumplimiento Ley 21.663 / ANCI Chile",
    // Se lee del crate: una version escrita a mano queda obsoleta al primer release.
    version = env!("CARGO_PKG_VERSION"),
    author  = "Felipe Carvajal Brown",
)]
struct Cli {
    #[arg(long, value_enum, default_value = "pse", help = "Clasificación de la institución")]
    tier: CliTier,

    #[arg(long, value_enum, default_value = "local", help = "Alcance del escaneo")]
    scope: CliScope,

    #[arg(long, default_value = munigpt_core::config::DEFAULT_INSTITUTION, help = "Nombre de la institución")]
    name: String,

    #[arg(long, default_value = "informe_brechas.pdf", help = "Ruta del PDF de salida")]
    pdf: String,

    #[arg(long, default_value = "csirt_report.json", help = "Ruta del JSON de salida")]
    json: String,

    #[arg(long, default_value = "poam.json",
          help = "Ruta del plan de remediación en formato OSCAL POA&M")]
    poam: String,

    #[arg(long, help = "No preguntar el cuestionario declarativo; se usan las respuestas guardadas si las hay")]
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
        munigpt_core::config::Config::escribir_ejemplo(path)
            .with_context(|| format!("no se pudo escribir {ruta}"))?;
        println!("Configuración de ejemplo escrita en {ruta}");
        println!("Déjala junto al ejecutable como {}, o apunta {} a esta ruta.",
            munigpt_core::config::CONFIG_FILE_NAME,
            munigpt_core::config::CONFIG_ENV);
        return Ok(());
    }

    let (config_ti, origen_config) = munigpt_core::config::Config::load();

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
    let guardadas = QuestionnaireResponse::desde_config(&config_ti.cuestionario);
    let questionnaire = if cli.no_questionnaire || cli.programado {
        if guardadas.answers.is_empty() {
            println!("[!] Cuestionario no preguntado y sin respuestas guardadas — los controles declarativos quedan sin responder.");
        } else {
            println!("[*] Cuestionario no preguntado — rigen las {} respuesta(s) guardadas.",
                guardadas.answers.len());
        }
        guardadas
    } else {
        let respondido = run_questionnaire(tier, &guardadas)?;
        guardar_cuestionario(&config_ti, &respondido);
        respondido
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

    let mut result = munigpt_core::scan(config, questionnaire)
        .context("El escaneo falló")?;

    println!("\r    Progreso: 100%\n");

    // Histórico por comuna: se registra esta medición y se compara con la anterior.
    // Va acá y no dentro de core::scan porque el motor de escaneo no tiene por qué
    // saber si existen mediciones previas.
    if config_ti.historico.habilitado {
        let ruta = historico::ruta_junto_al_ejecutable(&result.meta.institution_name);
        match historico::registrar_y_comparar(&ruta, &result, &config_ti.historico) {
            Ok(reg) => {
                if reg.purgadas > 0 {
                    println!("  Histórico: {} medición(es) purgadas por retención.", reg.purgadas);
                }
                println!(
                    "  Histórico: {} medición(es) registradas para esta institución.",
                    reg.mediciones
                );
                if let Some(aviso) = reg.aviso {
                    eprintln!("[!] Histórico registrado con reparos: {aviso}");
                    result.historico_error = Some(aviso);
                }
                result.delta = reg.delta;
                result.deriva = reg.deriva;
            }
            // Un histórico que falla no puede impedir la entrega del informe.
            Err(e) => {
                let detalle = format!("{e:#}");
                eprintln!("[!] No se pudo actualizar el histórico: {detalle}");
                result.historico_error = Some(detalle);
            }
        }
    }

    // Summary.
    let critical = result.gaps.iter().filter(|g| matches!(g.severity, munigpt_core::types::Severity::Critical)).count();
    let high     = result.gaps.iter().filter(|g| matches!(g.severity, munigpt_core::types::Severity::High)).count();
    let medium   = result.gaps.iter().filter(|g| matches!(g.severity, munigpt_core::types::Severity::Medium)).count();
    let csirt    = result.gaps.iter().filter(|g| g.requires_csirt_report).count();

    println!("  Brechas detectadas : {}", result.gaps.len());
    println!("    Críticas : {critical}  Altas : {high}  Medias : {medium}");

    imprimir_deriva(result.deriva.as_ref());

    println!("\n  Madurez por dominio (0 a 3):");
    for marco in munigpt_core::maturity::Marco::all() {
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
    // Bloque informativo de otra norma: no toca el puntaje ni la madurez.
    if let Some(l) = &result.ley21180 {
        println!("\n  Ley 21.180 (transformación digital) — dato informativo, no afecta el puntaje:");
        match l.grupo {
            Some(g) => println!("    {g} del Art. 5° del DFL N°1, año {}", l.anio),
            None => println!("    Institución no identificada en el Art. 5° del DFL N°1"),
        }
        for f in &l.fases {
            println!("    {}", f.descripcion());
        }
        println!("    {}", l.nota);
    }

    if csirt > 0 {
        println!("\n  *** {csirt} brecha(s) requieren reporte al {} en ≤3h (Art. 9°) ***\n",
            config_ti.informe.destinatario_csirt_o());
    }

    // Plan de remediación priorizado.
    let plan = munigpt_core::poam::plan(&result.gaps, &config_ti.poam);
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
    // El POA&M sale con el estado que TI lleva por hallazgo, no siempre "open": si
    // alguien cerro o acepto un riesgo en la aplicacion, el documento que entrega la
    // municipalidad tiene que decirlo.
    let registro = leer_registro_riesgos(&result.meta.institution_name);
    munigpt_core::poam::write_con(
        &result, &config_ti.poam, &registro, std::path::Path::new(&cli.poam),
    )
    .with_context(|| format!("no se pudo escribir {}", cli.poam))?;
    println!("    PDF tecnico   → {}  [{}]",
        cli.pdf, config_ti.informe.tamano_papel_tecnico.nombre());
    println!("    PDF ejecutivo → {}  [{}]",
        report_builder::executive_path(&cli.pdf), config_ti.informe.tamano_papel_ejecutivo.nombre());
    println!("    JSON          → {}", cli.json);
    println!("    POA&M         → {} (OSCAL {})", cli.poam, munigpt_core::poam::OSCAL_VERSION);

    // Paquete de evidencia: los mismos entregables, fechados y sellados por hash, en
    // una carpeta que la municipalidad puede presentar. Va al final porque reusa los
    // generadores de arriba y no tiene sentido armarlo si alguno falló.
    if let Some(carpeta) = &cli.evidencia {
        match munigpt_core::evidencia::escribir(&result, &config_ti, std::path::Path::new(carpeta)) {
            Ok(p) => {
                println!("\n[*] Paquete de evidencia:");
                println!("    Carpeta   → {}", p.ruta.display());
                println!("    Archivos  : {} ({} bytes, Oxum {})",
                    p.archivos.len(), p.bytes(), p.oxum);
                println!("    Manifiesto: {}", munigpt_core::evidencia::MANIFIESTO);
                println!("    Verifíquelo con `certutil -hashfile <archivo> SHA256`.");
                println!("    Es verificación de integridad, no firma electrónica: ver {}.",
                    munigpt_core::evidencia::INSTRUCCIONES);
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
fn gestionar_programacion(cli: &Cli, config: &munigpt_core::config::MonitoreoConfig) -> Result<()> {
    use munigpt_core::programacion;

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
            println!("\n  Se puede quitar con `munigpt --desprogramar`.\n");
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

fn leer_registro_riesgos(institucion: &str) -> Vec<munigpt_core::historico::Riesgo> {
    use munigpt_core::historico::Historico;

    let ruta = historico::ruta_junto_al_ejecutable(institucion);
    if !ruta.exists() {
        return Vec::new();
    }
    match Historico::abrir(&ruta).and_then(|h| h.riesgos()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[!] No se pudo leer el registro de riesgos: {e:#}");
            Vec::new()
        }
    }
}

/// Prints the control-by-control drift against the previous measurement.
///
/// El orden no es alfabético: primero lo que empeoró. Una reaparecida arriba de
/// todo porque es la que dice que una corrección no se sostuvo, y eso es lo que
/// alguien tiene que ir a mirar hoy.
fn imprimir_deriva(deriva: Option<&munigpt_core::historico::Deriva>) {
    use munigpt_core::historico::Estado;

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

fn guardar_cuestionario(
    config_ti: &munigpt_core::config::Config,
    respondido: &QuestionnaireResponse,
) {
    let Some(ruta) = munigpt_core::config::ruta_escritura() else {
        eprintln!("[!] No se pudo determinar dónde guardar las respuestas; no quedaron registradas.");
        return;
    };
    let mut nueva = config_ti.clone();
    nueva.cuestionario = respondido.a_config();
    match nueva.guardar(&ruta) {
        Ok(()) => println!("  Respuestas guardadas en {}", ruta.display()),
        Err(e) => eprintln!("[!] No se pudieron guardar las respuestas en {}: {e}", ruta.display()),
    }
}

fn respuesta_desde_entrada(entrada: &str, previa: Option<bool>) -> bool {
    match entrada.trim().to_lowercase().chars().next() {
        Some('s') => true,
        Some('n') => false,
        _ => previa.unwrap_or(false),
    }
}

fn run_questionnaire(tier: Tier, previas: &QuestionnaireResponse) -> Result<QuestionnaireResponse> {
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
    if previas.answers.is_empty() {
        println!("    Responda s (sí/cumple) o n (no/no cumple). Enter = no cumple.\n");
    } else {
        println!("    {} respuesta(s) guardada(s) se ofrecen como valor por omisión.",
            previas.answers.len());
        println!("    Responda s (sí/cumple) o n (no/no cumple). Enter = mantener lo guardado.\n");
    }

    let mut answers = Vec::new();
    for (i, q) in questions.iter().enumerate() {
        let etiqueta = q.applies_to.exigibilidad_for(tier);
        let previa = previas.get(q.id);
        println!("  [{}/{}] [{}] {}", i + 1, questions.len(), etiqueta, q.text);
        println!("        Anclaje:  {}", q.legal_anchor);
        println!("        Evidencia: {}", q.evidence_example);
        if let Some(p) = previa {
            println!("        Guardado:  {}", if p.compliant { "cumple" } else { "no cumple" });
        }
        print!("  > ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let compliant = respuesta_desde_entrada(&input, previa.map(|p| p.compliant));

        let notes = match previa {
            Some(p) if p.compliant == compliant => p.notes.clone(),
            _ => None,
        };

        answers.push(Answer { question_id: q.id, compliant, notes });

        let consecuencia = match (compliant, etiqueta) {
            (true, _)  => "Cumple".to_string(),
            (false, munigpt_core::types::Exigibilidad::Exigible) => {
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_s_declara_cumplimiento_aunque_lo_guardado_diga_lo_contrario() {
        assert!(respuesta_desde_entrada("s\n", Some(false)));
        assert!(respuesta_desde_entrada("Si\n", Some(false)));
    }

    #[test]
    fn una_n_declara_incumplimiento_aunque_lo_guardado_diga_lo_contrario() {
        assert!(!respuesta_desde_entrada("n\n", Some(true)));
        assert!(!respuesta_desde_entrada("NO\n", Some(true)));
    }

    #[test]
    fn enter_mantiene_lo_guardado() {
        assert!(respuesta_desde_entrada("\n", Some(true)));
        assert!(!respuesta_desde_entrada("\n", Some(false)));
    }

    #[test]
    fn enter_sin_nada_guardado_no_da_por_cumplido() {
        assert!(!respuesta_desde_entrada("\n", None));
        assert!(!respuesta_desde_entrada("   \n", None));
    }

    #[test]
    fn una_entrada_que_no_se_entiende_no_inventa_un_cumplimiento() {
        assert!(!respuesta_desde_entrada("quizas\n", None));
        assert!(respuesta_desde_entrada("quizas\n", Some(true)));
    }
}
