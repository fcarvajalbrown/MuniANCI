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

    #[arg(long, help = "Omitir cuestionario declarativo (asume todo no cumplido)")]
    no_questionnaire: bool,
}

#[derive(clap::ValueEnum, Clone)]
enum CliTier { Oiv, Pse, Unclassified }

#[derive(clap::ValueEnum, Clone)]
enum CliScope { Local, Lan }

fn main() -> Result<()> {
    let cli = Cli::parse();

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
    println!();

    // Questionnaire phase.
    let questionnaire = if cli.no_questionnaire {
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
        progress_cb: Some(Box::new(|pct| {
            print!("\r    Progreso: {pct:>3}%");
            io::stdout().flush().ok();
        })),
        log_cb: None, // CLI prints its own progress line — technical logs not needed here
    };

    let result = muniani_core::scan(config, questionnaire)
        .context("El escaneo falló")?;

    println!("\r    Progreso: 100%\n");

    // Summary.
    let critical = result.gaps.iter().filter(|g| matches!(g.severity, muniani_core::types::Severity::Critical)).count();
    let high     = result.gaps.iter().filter(|g| matches!(g.severity, muniani_core::types::Severity::High)).count();
    let medium   = result.gaps.iter().filter(|g| matches!(g.severity, muniani_core::types::Severity::Medium)).count();
    let csirt    = result.gaps.iter().filter(|g| g.requires_csirt_report).count();

    println!("  Brechas detectadas : {}", result.gaps.len());
    println!("    Críticas : {critical}  Altas : {high}  Medias : {medium}");
    if csirt > 0 {
        println!("\n  *** {csirt} brecha(s) requieren reporte al CSIRT Nacional en ≤3h (Art. 9°) ***\n");
    }

    // Output phase.
    println!("[*] Generando reportes...");
    report_builder::build(&result, &cli.pdf, &cli.json, |_| {})?;
    println!("    PDF  → {}", cli.pdf);
    println!("    JSON → {}", cli.json);
    println!("\n[+] Listo.\n");

    Ok(())
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