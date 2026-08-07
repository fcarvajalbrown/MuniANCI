use munigpt_core::config::{Config, IdentidadConfig};
use serde::Serialize;

pub fn institution() -> String {
    resolver(&identidad()).institution
}

pub fn tier() -> String {
    resolver(&identidad()).tier
}

pub fn institucion_forzada() -> Option<String> {
    forzada(&identidad())
}

fn identidad() -> IdentidadConfig {
    Config::load().0.identidad
}

fn institucion_compilada() -> Option<&'static str> {
    option_env!("MUNIGPT_INSTITUTION").or(option_env!("MUNIANI_INSTITUTION"))
}

fn tier_compilado() -> Option<&'static str> {
    option_env!("MUNIGPT_TIER").or(option_env!("MUNIANI_TIER"))
}

fn resolver(id: &IdentidadConfig) -> Branding {
    Branding {
        institution: id.institucion_o(institucion_compilada()),
        tier: id.tier_o(tier_compilado()),
    }
}

fn forzada(id: &IdentidadConfig) -> Option<String> {
    id.configurada()
        .or_else(|| institucion_compilada().map(str::to_string))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branding {
    pub institution: String,
    pub tier: String,
}

#[tauri::command]
pub fn app_branding() -> Branding {
    resolver(&identidad())
}

#[cfg(test)]
mod tests {
    use super::*;
    use munigpt_core::config::IdentidadConfig;

    #[test]
    fn el_archivo_manda_sobre_lo_compilado() {
        let id = IdentidadConfig {
            institucion: Some("Fuerza Aerea de Chile".into()),
            tier: Some("oiv".into()),
        };
        assert_eq!(resolver(&id).institution, "Fuerza Aerea de Chile");
        assert_eq!(resolver(&id).tier, "oiv");
    }

    #[test]
    fn sin_archivo_ni_build_el_defecto_es_neutro_y_pse() {
        let id = IdentidadConfig::default();
        let b = resolver(&id);
        assert_eq!(b.institution, munigpt_core::config::DEFAULT_INSTITUTION);
        assert_eq!(b.tier, "pse");
    }

    #[test]
    fn el_asistente_solo_se_fuerza_cuando_hay_identidad_explicita() {
        let vacia = IdentidadConfig::default();
        assert_eq!(forzada(&vacia), institucion_compilada().map(str::to_string));

        let puesta = IdentidadConfig {
            institucion: Some("Ejercito de Chile".into()),
            tier: None,
        };
        assert_eq!(forzada(&puesta), Some("Ejercito de Chile".to_string()));
    }
}
