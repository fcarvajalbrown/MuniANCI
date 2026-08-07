use munigpt_core::config::{
    Config, HistoricoConfig, IdentidadConfig, InformeConfig, MonitoreoConfig, PoamConfig, RedConfig,
};
use munigpt_core::ti;
use serde::Serialize;
use std::sync::Mutex;
use std::time::Instant;

const ESPERA_MAXIMA_S: u64 = 30;
const FALLOS_ANTES_DE_ESPERAR: u32 = 3;

#[derive(Default)]
pub struct AjustesState {
    sesion: Mutex<bool>,
    fallos: Mutex<Fallos>,
}

#[derive(Default)]
struct Fallos {
    cuenta: u32,
    ultimo: Option<Instant>,
}

impl AjustesState {
    pub fn abierta(&self) -> bool {
        *self.sesion.lock().unwrap()
    }

    pub fn abrir(&self) {
        *self.sesion.lock().unwrap() = true;
    }

    pub fn cerrar(&self) {
        *self.sesion.lock().unwrap() = false;
    }

    pub fn registrar_fallo(&self) {
        let mut f = self.fallos.lock().unwrap();
        f.cuenta = f.cuenta.saturating_add(1);
        f.ultimo = Some(Instant::now());
    }

    pub fn registrar_acierto(&self) {
        let mut f = self.fallos.lock().unwrap();
        f.cuenta = 0;
        f.ultimo = None;
    }

    pub fn espera_pendiente(&self) -> u64 {
        let f = self.fallos.lock().unwrap();
        let Some(ultimo) = f.ultimo else {
            return 0;
        };
        if f.cuenta < FALLOS_ANTES_DE_ESPERAR {
            return 0;
        }
        let castigo = (1u64 << (f.cuenta - FALLOS_ANTES_DE_ESPERAR).min(6)).min(ESPERA_MAXIMA_S);
        castigo.saturating_sub(ultimo.elapsed().as_secs().min(castigo))
    }
}

fn afecta_informe(antes: &Config, despues: &Config) -> bool {
    antes.identidad != despues.identidad || antes.poam != despues.poam || antes.red != despues.red
}

fn requiere_reinicio_asistente(antes: &Config, despues: &Config) -> bool {
    antes.identidad.institucion != despues.identidad.institucion
}

fn restaurar(actual: &Config, seccion: &str) -> Config {
    let mut c = actual.clone();
    match seccion {
        "identidad" => c.identidad = IdentidadConfig::default(),
        "poam" => c.poam = PoamConfig::default(),
        "informe" => c.informe = InformeConfig::default(),
        "historico" => c.historico = HistoricoConfig::default(),
        "red" => c.red = RedConfig::default(),
        "monitoreo" => c.monitoreo = MonitoreoConfig::default(),
        _ => {}
    }
    c
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoTi {
    pub con_candado: bool,
    pub password_configurada: bool,
    pub desbloqueado: bool,
    pub espera_s: u64,
    pub origen: String,
    pub ruta: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoGuardar {
    pub requiere_reinicio_asistente: bool,
    pub afecta_informe: bool,
    pub ruta: String,
}

fn hash_efectivo() -> Option<String> {
    let path = ti::ruta_override()?;
    ti::leer_hash_efectivo(&path)
}

fn exigir_sesion(state: &tauri::State<'_, AjustesState>) -> Result<(), String> {
    if !ti::candado_activo() || state.abierta() {
        return Ok(());
    }
    Err("El panel de ajustes esta bloqueado.".into())
}

#[tauri::command]
pub fn ti_estado(state: tauri::State<'_, AjustesState>) -> EstadoTi {
    let (_, origen) = Config::load();
    EstadoTi {
        con_candado: ti::candado_activo(),
        password_configurada: hash_efectivo().is_some(),
        desbloqueado: !ti::candado_activo() || state.abierta(),
        espera_s: state.espera_pendiente(),
        origen: origen.to_string(),
        ruta: munigpt_core::config::ruta_escritura().map(|p| p.display().to_string()),
    }
}

#[tauri::command]
pub fn ti_desbloquear(
    password: String,
    state: tauri::State<'_, AjustesState>,
) -> Result<bool, String> {
    if state.espera_pendiente() > 0 {
        return Err(format!(
            "Demasiados intentos fallidos. Espere {} segundos.",
            state.espera_pendiente()
        ));
    }
    let Some(hash) = hash_efectivo() else {
        return Err("Este equipo aun no tiene contrasena de TI configurada.".into());
    };
    if ti::verificar(&password, &hash) {
        state.registrar_acierto();
        state.abrir();
        Ok(true)
    } else {
        state.registrar_fallo();
        Ok(false)
    }
}

#[tauri::command]
pub fn ti_bloquear(state: tauri::State<'_, AjustesState>) {
    state.cerrar();
}

#[tauri::command]
pub fn ti_definir_password(
    password: String,
    state: tauri::State<'_, AjustesState>,
) -> Result<(), String> {
    if hash_efectivo().is_some() {
        return Err("Ya hay una contrasena configurada; use Cambiar contrasena.".into());
    }
    if password.chars().count() < 8 {
        return Err("La contrasena debe tener al menos 8 caracteres.".into());
    }
    let phc = ti::hashear(&password)?;
    let path = ti::ruta_override().ok_or("No se pudo determinar donde guardar la contrasena.")?;
    ti::escribir_override(&path, &phc).map_err(|e| e.to_string())?;
    state.abrir();
    Ok(())
}

#[tauri::command]
pub fn ti_cambiar_password(
    actual: String,
    nueva: String,
    state: tauri::State<'_, AjustesState>,
) -> Result<(), String> {
    exigir_sesion(&state)?;
    let Some(hash) = hash_efectivo() else {
        return Err("Este equipo aun no tiene contrasena de TI configurada.".into());
    };
    if !ti::verificar(&actual, &hash) {
        state.registrar_fallo();
        return Err("La contrasena actual no coincide.".into());
    }
    if nueva.chars().count() < 8 {
        return Err("La contrasena debe tener al menos 8 caracteres.".into());
    }
    let phc = ti::hashear(&nueva)?;
    let path = ti::ruta_override().ok_or("No se pudo determinar donde guardar la contrasena.")?;
    ti::escribir_override(&path, &phc).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ti_leer(state: tauri::State<'_, AjustesState>) -> Result<Config, String> {
    exigir_sesion(&state)?;
    Ok(Config::load().0)
}

#[tauri::command]
pub fn ti_guardar(
    nueva: Config,
    state: tauri::State<'_, AjustesState>,
) -> Result<ResultadoGuardar, String> {
    exigir_sesion(&state)?;
    if nueva.identidad.institucion_o(None).trim().is_empty() {
        return Err("El nombre de la institucion no puede quedar vacio.".into());
    }
    let (antes, _) = Config::load();
    let ruta = munigpt_core::config::ruta_escritura()
        .ok_or("No se pudo determinar donde guardar la configuracion.")?;
    nueva.guardar(&ruta).map_err(|e| e.to_string())?;
    Ok(ResultadoGuardar {
        requiere_reinicio_asistente: requiere_reinicio_asistente(&antes, &nueva),
        afecta_informe: afecta_informe(&antes, &nueva),
        ruta: ruta.display().to_string(),
    })
}

#[tauri::command]
pub fn ti_restaurar_defectos(
    seccion: String,
    state: tauri::State<'_, AjustesState>,
) -> Result<Config, String> {
    exigir_sesion(&state)?;
    let (actual, _) = Config::load();
    Ok(restaurar(&actual, &seccion))
}

#[tauri::command]
pub fn ti_abrir_archivo(
    app: tauri::AppHandle,
    state: tauri::State<'_, AjustesState>,
) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;
    exigir_sesion(&state)?;
    let ruta = munigpt_core::config::ruta_escritura()
        .ok_or("No se pudo determinar donde esta la configuracion.")?;
    if !ruta.exists() {
        Config::load().0.guardar(&ruta).map_err(|e| e.to_string())?;
    }
    app.shell()
        .open(ruta.display().to_string(), None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asistente_reiniciar(
    app: tauri::AppHandle,
    state: tauri::State<'_, AjustesState>,
) -> Result<(), String> {
    exigir_sesion(&state)?;
    crate::assistant::shutdown(&app);
    crate::assistant::start(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_backoff_crece_y_se_reinicia_al_acertar() {
        let s = AjustesState::default();
        assert_eq!(s.espera_pendiente(), 0);

        s.registrar_fallo();
        s.registrar_fallo();
        s.registrar_fallo();
        assert!(s.espera_pendiente() > 0, "tres fallos deben imponer espera");

        s.registrar_acierto();
        assert_eq!(s.espera_pendiente(), 0);
    }

    #[test]
    fn el_backoff_tiene_techo() {
        let s = AjustesState::default();
        for _ in 0..40 {
            s.registrar_fallo();
        }
        assert!(s.espera_pendiente() <= ESPERA_MAXIMA_S, "el techo son 30 segundos");
    }

    #[test]
    fn la_sesion_empieza_cerrada() {
        let s = AjustesState::default();
        assert!(!s.abierta());
        s.abrir();
        assert!(s.abierta());
        s.cerrar();
        assert!(!s.abierta());
    }

    #[test]
    fn solo_identidad_poam_y_red_marcan_el_escaneo_vencido() {
        let base = Config::default();

        let mut otra = base.clone();
        otra.identidad.institucion = Some("Ejercito de Chile".into());
        assert!(afecta_informe(&base, &otra));

        let mut otra = base.clone();
        otra.poam.plazo_dias_critica = 45;
        assert!(afecta_informe(&base, &otra));

        let mut otra = base.clone();
        otra.red.arp = false;
        assert!(afecta_informe(&base, &otra));

        let mut otra = base.clone();
        otra.informe.color_primario = "#112233".into();
        assert!(!afecta_informe(&base, &otra));

        let mut otra = base.clone();
        otra.monitoreo.hora = "04:00".into();
        assert!(!afecta_informe(&base, &otra));

        let mut otra = base.clone();
        otra.historico.retencion_meses = 12;
        assert!(!afecta_informe(&base, &otra));
    }

    #[test]
    fn cambiar_la_institucion_pide_reiniciar_el_asistente() {
        let base = Config::default();
        let mut otra = base.clone();
        otra.identidad.institucion = Some("Fuerza Aerea de Chile".into());
        assert!(requiere_reinicio_asistente(&base, &otra));

        let mut solo_tier = base.clone();
        solo_tier.identidad.tier = Some("oiv".into());
        assert!(!requiere_reinicio_asistente(&base, &solo_tier));
    }

    #[test]
    fn restaurar_una_seccion_no_toca_las_demas() {
        let mut c = Config::default();
        c.poam.plazo_dias_critica = 45;
        c.red.arp_pps = 0;

        let restaurada = restaurar(&c, "poam");
        assert_eq!(restaurada.poam, PoamConfig::default());
        assert_eq!(restaurada.red.arp_pps, 0, "la otra seccion se conserva");
    }

    #[test]
    fn restaurar_una_seccion_desconocida_no_cambia_nada() {
        let mut c = Config::default();
        c.poam.plazo_dias_critica = 45;
        assert_eq!(restaurar(&c, "inexistente"), c);
    }
}
