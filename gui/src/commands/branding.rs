use muniani_core::config::{Config, IdentidadConfig};
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

fn resolver(id: &IdentidadConfig) -> Branding {
    Branding {
        institution: id.institucion_o(option_env!("MUNIANI_INSTITUTION")),
        tier: id.tier_o(option_env!("MUNIANI_TIER")),
    }
}

fn forzada(id: &IdentidadConfig) -> Option<String> {
    id.configurada()
        .or_else(|| option_env!("MUNIANI_INSTITUTION").map(str::to_string))
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
    use muniani_core::config::IdentidadConfig;

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
        assert_eq!(b.institution, muniani_core::config::DEFAULT_INSTITUTION);
        assert_eq!(b.tier, "pse");
    }

    #[test]
    fn el_asistente_solo_se_fuerza_cuando_hay_identidad_explicita() {
        let vacia = IdentidadConfig::default();
        assert_eq!(forzada(&vacia), option_env!("MUNIANI_INSTITUTION").map(str::to_string));

        let puesta = IdentidadConfig {
            institucion: Some("Ejercito de Chile".into()),
            tier: None,
        };
        assert_eq!(forzada(&puesta), Some("Ejercito de Chile".to_string()));
    }
}
