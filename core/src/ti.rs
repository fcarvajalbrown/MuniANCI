use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const FORZAR_CANDADO_ENV: &str = "MUNIGPT_FORCE_LOCK";

/// Nombre anterior de la variable, de cuando el producto se llamaba MuniANCI.
pub const FORZAR_CANDADO_ENV_LEGACY: &str = "MUNIANI_FORCE_LOCK";

pub const OVERRIDE_FILE_NAME: &str = "ti-password.hash";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Override {
    huella: String,
    hash: String,
}

pub fn hash_compilado() -> Option<&'static str> {
    option_env!("MUNIGPT_ADMIN_HASH").or(option_env!("MUNIANI_ADMIN_HASH"))
}

pub fn candado_activo() -> bool {
    if std::env::var_os(FORZAR_CANDADO_ENV).is_some()
        || std::env::var_os(FORZAR_CANDADO_ENV_LEGACY).is_some()
    {
        return true;
    }
    !cfg!(debug_assertions)
}

pub fn hashear(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

pub fn verificar(password: &str, phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn huella(phc: Option<&str>) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(phc.unwrap_or("<sin hash compilado>").as_bytes());
    h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

pub fn ruta_override() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(|base| PathBuf::from(base).join("MuniGPT").join(OVERRIDE_FILE_NAME))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .map(|base| PathBuf::from(base).join(".munigpt").join(OVERRIDE_FILE_NAME))
    }
}

pub fn resolver_hash(override_path: &Path, compilado: Option<&str>) -> Option<String> {
    if let Some(o) = leer_override(override_path) {
        if o.huella == huella(compilado) {
            return Some(o.hash);
        }
    }
    compilado.map(str::to_string)
}

pub fn leer_hash_efectivo(override_path: &Path) -> Option<String> {
    resolver_hash(override_path, hash_compilado())
}

pub fn escribir_override(override_path: &Path, phc: &str) -> std::io::Result<()> {
    escribir_override_con_huella(override_path, phc, &huella(hash_compilado()))
}

pub fn escribir_override_con_huella(
    override_path: &Path,
    phc: &str,
    huella: &str,
) -> std::io::Result<()> {
    let contenido = Override {
        huella: huella.to_string(),
        hash: phc.to_string(),
    };
    let json = serde_json::to_string_pretty(&contenido)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if let Some(dir) = override_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = override_path.with_extension("tmp");
    std::fs::write(&tmp, json + "\n")?;
    std::fs::rename(&tmp, override_path)
}

fn leer_override(override_path: &Path) -> Option<Override> {
    let texto = std::fs::read_to_string(override_path).ok()?;
    serde_json::from_str(crate::config::sin_bom(&texto)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_contrasena_se_verifica_contra_su_propio_hash() {
        let phc = hashear("clave de prueba").unwrap();
        assert!(verificar("clave de prueba", &phc));
        assert!(!verificar("otra clave", &phc));
    }

    #[test]
    fn dos_hashes_de_la_misma_contrasena_son_distintos() {
        let a = hashear("misma").unwrap();
        let b = hashear("misma").unwrap();
        assert_ne!(a, b, "cada hash lleva su propia sal");
        assert!(verificar("misma", &a) && verificar("misma", &b));
    }

    #[test]
    fn un_hash_corrupto_no_verifica_en_vez_de_reventar() {
        assert!(!verificar("cualquiera", "esto no es un PHC"));
        assert!(!verificar("cualquiera", ""));
    }

    #[test]
    fn la_huella_es_estable_y_distingue_builds() {
        let a = hashear("a").unwrap();
        let b = hashear("b").unwrap();
        assert_eq!(huella(Some(&a)), huella(Some(&a)));
        assert_ne!(huella(Some(&a)), huella(Some(&b)));
        assert_eq!(huella(Some(&a)).len(), 16);
        assert_ne!(huella(None), huella(Some(&a)));
    }

    #[test]
    fn el_override_gana_cuando_la_huella_calza() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(OVERRIDE_FILE_NAME);

        let compilado = hashear("de fabrica").unwrap();
        let rotada = hashear("la que puso TI").unwrap();
        escribir_override_con_huella(&path, &rotada, &huella(Some(&compilado))).unwrap();

        let efectivo = resolver_hash(&path, Some(&compilado)).unwrap();
        assert!(verificar("la que puso TI", &efectivo));
        assert!(!verificar("de fabrica", &efectivo));
    }

    #[test]
    fn un_override_de_otro_build_se_ignora() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(OVERRIDE_FILE_NAME);

        let compilado = hashear("de fabrica").unwrap();
        let ajeno = hashear("clave de desarrollo").unwrap();
        escribir_override_con_huella(&path, &ajeno, "huellaajena00000").unwrap();

        let efectivo = resolver_hash(&path, Some(&compilado)).unwrap();
        assert!(verificar("de fabrica", &efectivo), "debe mandar el hash compilado");
        assert!(!verificar("clave de desarrollo", &efectivo));
    }

    #[test]
    fn borrar_el_override_devuelve_la_contrasena_del_build() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(OVERRIDE_FILE_NAME);

        let compilado = hashear("de fabrica").unwrap();
        let rotada = hashear("rotada").unwrap();
        escribir_override_con_huella(&path, &rotada, &huella(Some(&compilado))).unwrap();
        std::fs::remove_file(&path).unwrap();

        let efectivo = resolver_hash(&path, Some(&compilado)).unwrap();
        assert!(verificar("de fabrica", &efectivo));
    }

    #[test]
    fn sin_compilado_ni_override_no_hay_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(OVERRIDE_FILE_NAME);
        assert!(resolver_hash(&path, None).is_none());
    }

    #[test]
    fn un_override_ilegible_no_revienta() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(OVERRIDE_FILE_NAME);
        std::fs::write(&path, "{ esto no es json").unwrap();

        let compilado = hashear("de fabrica").unwrap();
        let efectivo = resolver_hash(&path, Some(&compilado)).unwrap();
        assert!(verificar("de fabrica", &efectivo));
    }

    #[test]
    fn forzar_el_candado_lo_activa_siempre() {
        let previo = std::env::var_os(FORZAR_CANDADO_ENV);
        unsafe { std::env::set_var(FORZAR_CANDADO_ENV, "1") };
        let activo = candado_activo();
        match previo {
            Some(v) => unsafe { std::env::set_var(FORZAR_CANDADO_ENV, v) },
            None => unsafe { std::env::remove_var(FORZAR_CANDADO_ENV) },
        }
        assert!(activo, "MUNIGPT_FORCE_LOCK debe vencer al bypass de depuracion");
    }
}
