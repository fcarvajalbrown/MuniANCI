# IT settings cog panel — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put a password-protected cog in the MuniANCI header that lets institutional IT edit the identity, deadlines, network and report settings without a rebuild.

**Architecture:** `munianci.config.json` already is the runtime configuration surface; this adds an `identidad` section to it, an Argon2id password gate in `core`, a set of Tauri commands that read and write the file, and an anchored dropdown panel in the React frontend. Every consumer already calls `Config::load()` fresh at its use site, so saving the file is enough — there is no cached state to invalidate.

**Tech Stack:** Rust 2024 (workspace `core` / `gui`), `serde`/`serde_json`, `argon2` (new), `sha2` (already present), Tauri 2, React + Vite + TypeScript.

**Spec:** `docs/superpowers/specs/2026-08-03-panel-ajustes-ti-design.md`

## Global Constraints

- **No comments in code.** Zero. Not line comments, not block comments, not `///` doc comments, not `TODO`. The surrounding files are full of them because they predate the rule; do not match that style. If code needs explaining, rename or extract instead.
- **No emojis** anywhere, including commit messages.
- **No AI attribution** in commits: no `Co-Authored-By`, no "Generated with".
- Commit messages, `CHANGELOG.md`, `README.md`, `ROADMAP.md` and all product-facing strings are in **Chilean Spanish**. The spec and this plan are English because Felipe reads them.
- **Never invent** a legal citation, norma id, institution name, URL or number. Every user-facing string that asserts something about the real world must trace to the local PDFs in `docs/` or to a value Felipe supplied.
- **Never hardcode a version string.** Use `env!("CARGO_PKG_VERSION")`.
- New config blocks carry `#[serde(default)]` so an older `munianci.config.json` still loads.
- **Commit each task directly on `main` and push.** No pull requests.
- Run `cargo test` from the repo root; it covers `core`, `cli` and `gui`.
- Exact default institution string: `Organismo del Estado`. Exact default tier: `pse`.

---

### Task 1: `identidad` config section and neutral defaults

**Files:**
- Modify: `core/src/config.rs`
- Test: `core/src/config.rs` (the existing `mod tests` at the bottom)

**Interfaces:**
- Consumes: nothing.
- Produces: `muniani_core::config::DEFAULT_INSTITUTION: &str`, `muniani_core::config::DEFAULT_TIER: &str`, `muniani_core::config::IdentidadConfig { institucion: Option<String>, tier: Option<String> }` with methods `institucion_o(&self, compilada: Option<&str>) -> String`, `tier_o(&self, compilado: Option<&str>) -> String` and `configurada(&self) -> Option<String>`, and the field `Config::identidad`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `core/src/config.rs`:

```rust
#[test]
fn la_identidad_del_archivo_gana_sobre_la_compilada() {
    let c: Config =
        serde_json::from_str(r#"{"identidad":{"institucion":"Fuerza Aerea de Chile"}}"#).unwrap();
    assert_eq!(
        c.identidad.institucion_o(Some("Municipalidad de Nunoa")),
        "Fuerza Aerea de Chile"
    );
}

#[test]
fn sin_archivo_manda_la_identidad_compilada() {
    let c = Config::default();
    assert_eq!(
        c.identidad.institucion_o(Some("Municipalidad de Nunoa")),
        "Municipalidad de Nunoa"
    );
}

#[test]
fn sin_archivo_ni_compilada_el_defecto_es_neutro() {
    let c = Config::default();
    assert_eq!(c.identidad.institucion_o(None), DEFAULT_INSTITUTION);
    assert_eq!(c.identidad.institucion_o(None), "Organismo del Estado");
}

#[test]
fn una_institucion_en_blanco_no_cuenta_como_configurada() {
    let c: Config = serde_json::from_str(r#"{"identidad":{"institucion":"   "}}"#).unwrap();
    assert_eq!(c.identidad.institucion_o(Some("Municipalidad de Nunoa")), "Municipalidad de Nunoa");
}

#[test]
fn el_tier_por_defecto_es_pse() {
    let c = Config::default();
    assert_eq!(c.identidad.tier_o(None), "pse");
    assert_eq!(c.identidad.tier_o(Some("oiv")), "oiv");

    let c: Config = serde_json::from_str(r#"{"identidad":{"tier":"unclassified"}}"#).unwrap();
    assert_eq!(c.identidad.tier_o(Some("oiv")), "unclassified");
}

#[test]
fn un_archivo_sin_la_seccion_identidad_carga_con_los_defectos() {
    let c: Config = serde_json::from_str(r#"{"poam":{"plazo_dias_alta":45}}"#).unwrap();
    assert_eq!(c.identidad, IdentidadConfig::default());
    assert_eq!(c.poam.plazo_dias_alta, 45);
}

#[test]
fn el_ejemplo_documenta_la_identidad() {
    let ayuda = Config::ejemplo().ayuda.join(" ");
    assert!(ayuda.contains("identidad.institucion"), "{ayuda}");
    assert!(ayuda.contains("identidad.tier"), "{ayuda}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p muniani-core config::tests`
Expected: FAIL — `cannot find type IdentidadConfig in this scope`, `no field identidad on type Config`.

- [ ] **Step 3: Add the constants, the struct and the field**

In `core/src/config.rs`, after the existing `CONFIG_FILE_NAME` constant, add:

```rust
pub const DEFAULT_INSTITUTION: &str = "Organismo del Estado";

pub const DEFAULT_TIER: &str = "pse";
```

Add the struct next to the other section structs:

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IdentidadConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub institucion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
}

impl IdentidadConfig {
    pub fn institucion_o(&self, compilada: Option<&str>) -> String {
        Self::primero_no_vacio(self.institucion.as_deref(), compilada, DEFAULT_INSTITUTION)
    }

    pub fn tier_o(&self, compilado: Option<&str>) -> String {
        Self::primero_no_vacio(self.tier.as_deref(), compilado, DEFAULT_TIER)
    }

    pub fn configurada(&self) -> Option<String> {
        self.institucion
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    fn primero_no_vacio(archivo: Option<&str>, compilado: Option<&str>, defecto: &str) -> String {
        archivo
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or(compilado)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(defecto)
            .to_string()
    }
}
```

Add the field to `Config`, first in the struct so it reads first in the written file:

```rust
pub identidad: IdentidadConfig,
```

- [ ] **Step 4: Add the identidad help lines to `Config::ejemplo`**

Set `identidad: IdentidadConfig::default()` in the `Config { .. }` literal inside `ejemplo()`, and insert these lines at the top of the `ayuda` vector, right after the three existing intro lines and the blank one:

```rust
"identidad.institucion: nombre del organismo que emite el informe. Si se omite,".into(),
"  rige el nombre compilado en este build y, si tampoco lo hay, un marcador neutro.".into(),
"  Cambiarlo aca cambia el encabezado, el informe y el Asistente a la vez.".into(),
"identidad.tier: \"oiv\", \"pse\" o \"unclassified\".".into(),
"  Por defecto \"pse\". El Art. 1 inc. 2 de la Ley 21.663 incluye a las".into(),
"  Municipalidades y a las Fuerzas Armadas en la Administracion del Estado, y el".into(),
"  Art. 4 inc. 2 declara esenciales los servicios provistos por sus organismos, de".into(),
"  modo que un organo del Estado es prestador de servicios esenciales sin que medie".into(),
"  resolucion alguna. \"oiv\" corresponde solo a quien la Agencia haya calificado".into(),
"  como operador de importancia vital por resolucion fundada (Arts. 5 y 6).".into(),
"  \"unclassified\" apaga el deber de reporte al CSIRT en todo el informe.".into(),
"".into(),
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p muniani-core config::tests`
Expected: PASS, all tests in the module including the pre-existing ones.

- [ ] **Step 6: Commit and push**

```bash
git add core/src/config.rs
git commit -m "feat(config): seccion identidad y valores por defecto neutros

La institucion y el tier dejan de ser solo compilados: munianci.config.json
puede fijarlos y gana sobre el build. El defecto de institucion pasa a un
marcador neutro, porque un cliente real no puede ser el respaldo de todo build
sin marca. El tier se mantiene en pse, que es lo que producen el Art. 1 inc. 2
y el Art. 4 inc. 2 de la Ley 21.663 para un organo del Estado sin resolucion
de la Agencia."
git push origin main
```

---

### Task 2: atomic config writer

**Files:**
- Modify: `core/src/config.rs`
- Test: `core/src/config.rs` (`mod tests`)

**Interfaces:**
- Consumes: `Config` from Task 1.
- Produces: `Config::guardar(&self, path: &Path) -> std::io::Result<()>` and `muniani_core::config::ruta_escritura() -> Option<PathBuf>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn guardar_y_releer_conserva_los_valores() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(CONFIG_FILE_NAME);

    let mut c = Config::default();
    c.identidad.institucion = Some("Fuerza Aerea de Chile".into());
    c.poam.plazo_dias_critica = 45;
    c.guardar(&path).unwrap();

    let texto = std::fs::read_to_string(&path).unwrap();
    let leido: Config = serde_json::from_str(sin_bom(&texto)).unwrap();
    assert_eq!(leido.identidad.institucion.as_deref(), Some("Fuerza Aerea de Chile"));
    assert_eq!(leido.poam.plazo_dias_critica, 45);
}

#[test]
fn guardar_repone_la_ayuda_cuando_el_archivo_no_la_traia() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(CONFIG_FILE_NAME);

    Config::default().guardar(&path).unwrap();

    let texto = std::fs::read_to_string(&path).unwrap();
    assert!(texto.contains("_ayuda"), "el archivo guardado debe documentarse solo");
    assert!(texto.contains("Dynamic ARP Inspection"), "{texto}");
}

#[test]
fn guardar_conserva_la_ayuda_que_ya_estaba() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(CONFIG_FILE_NAME);

    let mut c = Config::default();
    c.ayuda = vec!["Nota propia del area de TI.".into()];
    c.guardar(&path).unwrap();

    let texto = std::fs::read_to_string(&path).unwrap();
    assert!(texto.contains("Nota propia del area de TI."), "{texto}");
    assert!(!texto.contains("Dynamic ARP Inspection"), "no debe pisar la ayuda existente");
}

#[test]
fn guardar_sobre_un_archivo_existente_no_deja_temporal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(CONFIG_FILE_NAME);

    Config::default().guardar(&path).unwrap();
    Config::default().guardar(&path).unwrap();

    let sobrantes: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(sobrantes.is_empty(), "quedaron temporales: {sobrantes:?}");
}

#[test]
fn la_ruta_de_escritura_respeta_la_variable_de_entorno() {
    let previo = std::env::var_os(CONFIG_ENV);
    unsafe { std::env::set_var(CONFIG_ENV, "Z:/muniani-prueba/mi.json") };
    let ruta = ruta_escritura().unwrap();
    match previo {
        Some(v) => unsafe { std::env::set_var(CONFIG_ENV, v) },
        None => unsafe { std::env::remove_var(CONFIG_ENV) },
    }
    assert_eq!(ruta, PathBuf::from("Z:/muniani-prueba/mi.json"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p muniani-core config::tests`
Expected: FAIL — `no method named guardar found`, `cannot find function ruta_escritura`.

- [ ] **Step 3: Implement the writer**

Add to `impl Config` in `core/src/config.rs`:

```rust
pub fn guardar(&self, path: &std::path::Path) -> std::io::Result<()> {
    let mut salida = self.clone();
    if salida.ayuda.is_empty() {
        salida.ayuda = Config::ejemplo().ayuda;
    }
    let json = serde_json::to_string_pretty(&salida)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json + "\n")?;
    std::fs::rename(&tmp, path)
}
```

And a free function next to `candidate_paths`:

```rust
pub fn ruta_escritura() -> Option<PathBuf> {
    candidate_paths().into_iter().next()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p muniani-core config::tests`
Expected: PASS.

- [ ] **Step 5: Commit and push**

```bash
git add core/src/config.rs
git commit -m "feat(config): escritura atomica de munianci.config.json

Se escribe a un temporal y se renombra encima, para que un corte a mitad de
guardado no deje a TI con un archivo truncado. La cabecera _ayuda se repone
cuando el archivo no la traia y se respeta cuando si."
git push origin main
```

---

### Task 3: Argon2id password gate

**Files:**
- Create: `core/src/ti.rs`
- Modify: `core/src/lib.rs` (add `pub mod ti;`)
- Modify: `Cargo.toml` (workspace dependency)
- Modify: `core/Cargo.toml` (crate dependency)
- Test: `core/src/ti.rs` (`mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `muniani_core::ti::hashear(password: &str) -> Result<String, String>`
  - `muniani_core::ti::verificar(password: &str, phc: &str) -> bool`
  - `muniani_core::ti::huella(phc: Option<&str>) -> String`
  - `muniani_core::ti::hash_compilado() -> Option<&'static str>`
  - `muniani_core::ti::candado_activo() -> bool`
  - `muniani_core::ti::ruta_override() -> Option<PathBuf>`
  - `muniani_core::ti::leer_hash_efectivo(override_path: &Path) -> Option<String>`
  - `muniani_core::ti::resolver_hash(override_path: &Path, compilado: Option<&str>) -> Option<String>`
  - `muniani_core::ti::escribir_override(override_path: &Path, phc: &str) -> std::io::Result<()>`
  - `muniani_core::ti::escribir_override_con_huella(override_path: &Path, phc: &str, huella: &str) -> std::io::Result<()>`
  - `muniani_core::ti::FORZAR_CANDADO_ENV: &str` (`"MUNIANI_FORCE_LOCK"`), `muniani_core::ti::OVERRIDE_FILE_NAME: &str`

- [ ] **Step 1: Add the dependency**

In the root `Cargo.toml`, under `[workspace.dependencies]`, after the `sha2` entry:

```toml
# Hash de la contrasena del panel de TI. RustCrypto, MIT/Apache, Rust puro.
# Argon2id es el algoritmo que recomienda la RFC 9106 para contrasenas.
argon2 = "0.5"
```

In `core/Cargo.toml`, under `[dependencies]`, after `ttf-parser`:

```toml
argon2      = { workspace = true }
```

Run: `cargo build -p muniani-core`
Expected: compiles. If cargo resolves a different major version, adjust the `"0.5"` requirement to whatever `cargo tree -p argon2` reports and keep the API calls below; `hash_password`, `verify_password`, `PasswordHash::new` and `SaltString::generate` are stable across 0.5.x.

- [ ] **Step 2: Write the failing tests**

Create `core/src/ti.rs` with only the test module for now:

```rust
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
        let path = dir.path().join("ti-password.hash");

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
        let path = dir.path().join("ti-password.hash");

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
        let path = dir.path().join("ti-password.hash");

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
        let path = dir.path().join("ti-password.hash");
        assert!(resolver_hash(&path, None).is_none());
    }

    #[test]
    fn un_override_ilegible_no_revienta() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ti-password.hash");
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
        assert!(activo, "MUNIANI_FORCE_LOCK debe vencer al bypass de depuracion");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Add `pub mod ti;` to `core/src/lib.rs` next to the other `pub mod` lines, then run:

Run: `cargo test -p muniani-core ti::tests`
Expected: FAIL — `cannot find function hashear in this scope` and the rest.

- [ ] **Step 4: Implement the module**

Put this above the `mod tests` block in `core/src/ti.rs`:

```rust
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const FORZAR_CANDADO_ENV: &str = "MUNIANI_FORCE_LOCK";

pub const OVERRIDE_FILE_NAME: &str = "ti-password.hash";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Override {
    huella: String,
    hash: String,
}

pub fn hash_compilado() -> Option<&'static str> {
    option_env!("MUNIANI_ADMIN_HASH")
}

pub fn candado_activo() -> bool {
    if std::env::var_os(FORZAR_CANDADO_ENV).is_some() {
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
            .map(|base| PathBuf::from(base).join("MuniANCI").join(OVERRIDE_FILE_NAME))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .map(|base| PathBuf::from(base).join(".muniani").join(OVERRIDE_FILE_NAME))
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
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p muniani-core ti::tests`
Expected: PASS, 10 tests.

- [ ] **Step 6: Run the whole suite**

Run: `cargo test`
Expected: PASS. Nothing else touches `ti`, so this is a regression check only.

- [ ] **Step 7: Commit and push**

```bash
git add Cargo.toml core/Cargo.toml core/src/ti.rs core/src/lib.rs
git commit -m "feat(ti): candado Argon2id para el panel de ajustes

El hash viaja compilado por cliente en MUNIANI_ADMIN_HASH y TI puede rotarlo:
la rotacion escribe un override en el perfil del usuario que gana sobre el
compilado. El override guarda la huella del hash contra el que se creo y se
ignora cuando no calza, porque el archivo sobrevive a reinstalaciones y sin eso
una contrasena de desarrollo abriria despues cualquier build de cliente
instalado en la misma maquina.

Los builds de depuracion no ponen candado; MUNIANI_FORCE_LOCK=1 lo repone para
poder ejercitar el camino real sin cortar un release."
git push origin main
```

---

### Task 4: the host reads the runtime identity

**Files:**
- Modify: `gui/src/commands/branding.rs`
- Modify: `gui/src/commands/start_scan.rs:69-78` and `:95-101`
- Modify: `gui/src/assistant.rs:205-215`
- Modify: `cli/src/main.rs:26`
- Test: `gui/src/commands/branding.rs` (new `mod tests`)

**Interfaces:**
- Consumes: `IdentidadConfig::institucion_o`, `IdentidadConfig::tier_o`, `IdentidadConfig::configurada`, `DEFAULT_INSTITUTION`, `DEFAULT_TIER` from Task 1.
- Produces: `branding::institution() -> String` (now config-aware), `branding::tier() -> String` (now `String`, was `&'static str`), `branding::institucion_forzada() -> Option<String>` (replaces `institution_override`).

- [ ] **Step 1: Write the failing test**

Add at the bottom of `gui/src/commands/branding.rs`:

```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p muniani-gui branding`
Expected: FAIL — `cannot find function resolver in this scope`.

- [ ] **Step 3: Rewrite the module body**

Replace everything above the new `mod tests` in `gui/src/commands/branding.rs` with:

```rust
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
```

- [ ] **Step 4: Fix the three call sites**

`gui/src/commands/start_scan.rs` — `tier_from_env` now matches on a `String`:

```rust
fn tier_from_env() -> Tier {
    match super::branding::tier().as_str() {
        "oiv"          => Tier::Oiv,
        "unclassified" => Tier::Unclassified,
        _              => Tier::Pse,
    }
}
```

In the same file, replace the `red: Default::default(),` line in the `ScanConfig` literal with the configured sweep settings, and rename the function to match what it now does:

```rust
    let (config_ti, _) = muniani_core::config::Config::load();

    let config = ScanConfig {
        institution_name: super::branding::institution(),
        tier:        tier_resuelto(),
        scope:       Scope::Local,
        red:         config_ti.red,
        progress_cb: Some(Box::new(progress_cb)),
        log_cb:      Some(Box::new(log_cb)),
    };
```

Rename `tier_from_env` to `tier_resuelto` at its definition too. Add the `RedConfig` import if the compiler asks for it; `ScanConfig.red` takes `muniani_core::config::RedConfig`, which `Config::load()` already yields.

`gui/src/assistant.rs` — in `spawn_backend`, replace the `institution_override` block with:

```rust
    if std::env::var_os("MUNIGPT_MUNICIPIO").is_none() {
        if let Some(institution) = crate::commands::branding::institucion_forzada() {
            cmd.env("MUNIGPT_MUNICIPIO", institution);
        }
    }
```

`cli/src/main.rs:26` — the clap default must stop naming a real client:

```rust
    #[arg(long, default_value = muniani_core::config::DEFAULT_INSTITUTION, help = "Nombre de la institución")]
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS across `core`, `cli` and `gui`. The `historico` and `report_builder` tests that mention Providencia pass their own literal strings and are unaffected.

- [ ] **Step 6: Verify the un-branded default by hand**

Run: `cargo run -q -p muniani-cli -- --help`
Expected: the `--institution` line shows `[default: Organismo del Estado]`, not Providencia.

- [ ] **Step 7: Commit and push**

```bash
git add gui/src/commands/branding.rs gui/src/commands/start_scan.rs gui/src/assistant.rs cli/src/main.rs
git commit -m "feat(identidad): el host resuelve institucion y tier en tiempo de ejecucion

El encabezado, el informe y el municipio que recibe el sidecar dejan de leer
solo el valor compilado y pasan por munianci.config.json. El Asistente se fuerza
unicamente cuando hay identidad explicita, para no pisar el config.json propio
del backend en un build sin marca.

De paso, el escaneo de la GUI deja de descartar la seccion red del archivo:
usaba los valores por defecto aunque TI hubiera configurado otra cosa."
git push origin main
```

---

### Task 5: Tauri commands for the panel

**Files:**
- Create: `gui/src/commands/ajustes.rs`
- Modify: `gui/src/commands/mod.rs`
- Modify: `gui/src/lib.rs:12-23`
- Test: `gui/src/commands/ajustes.rs` (`mod tests`)

**Interfaces:**
- Consumes: `muniani_core::ti::*` (Task 3), `Config::guardar` and `ruta_escritura` (Task 2), `branding::institucion_forzada` (Task 4).
- Produces the commands the frontend calls: `ti_estado`, `ti_desbloquear`, `ti_leer`, `ti_guardar`, `ti_cambiar_password`, `ti_restaurar_defectos`, `ti_abrir_archivo`, `asistente_reiniciar`. Also `AjustesState`, registered with `.manage()`.

- [ ] **Step 1: Write the failing tests**

Create `gui/src/commands/ajustes.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use muniani_core::config::Config;

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
        assert!(s.espera_pendiente() <= 30, "el techo son 30 segundos");
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
        assert_eq!(restaurada.poam, muniani_core::config::PoamConfig::default());
        assert_eq!(restaurada.red.arp_pps, 0, "la otra seccion se conserva");
    }

    #[test]
    fn restaurar_una_seccion_desconocida_no_cambia_nada() {
        let mut c = Config::default();
        c.poam.plazo_dias_critica = 45;
        assert_eq!(restaurar(&c, "inexistente"), c);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Add `pub mod ajustes;` to `gui/src/commands/mod.rs`, then run:

Run: `cargo test -p muniani-gui ajustes`
Expected: FAIL — `cannot find type AjustesState in this scope`.

- [ ] **Step 3: Implement state and pure helpers**

Above `mod tests` in `gui/src/commands/ajustes.rs`:

```rust
use muniani_core::config::{
    Config, HistoricoConfig, IdentidadConfig, InformeConfig, MonitoreoConfig, PoamConfig, RedConfig,
};
use muniani_core::ti;
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
        let castigo = 1u64 << (f.cuenta - FALLOS_ANTES_DE_ESPERAR).min(6);
        let castigo = castigo.min(ESPERA_MAXIMA_S);
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
```

Note the `espera_pendiente` shift is capped at 6 before the `min`, so `1 << n` cannot overflow after many failures.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p muniani-gui ajustes`
Expected: PASS, 7 tests.

- [ ] **Step 5: Add the commands**

Append to `gui/src/commands/ajustes.rs`, above `mod tests`:

```rust
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
        ruta: muniani_core::config::ruta_escritura().map(|p| p.display().to_string()),
    }
}

#[tauri::command]
pub fn ti_desbloquear(password: String, state: tauri::State<'_, AjustesState>) -> Result<bool, String> {
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
pub fn ti_definir_password(password: String, state: tauri::State<'_, AjustesState>) -> Result<(), String> {
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
    let ruta = muniani_core::config::ruta_escritura()
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
    let ruta = muniani_core::config::ruta_escritura()
        .ok_or("No se pudo determinar donde esta la configuracion.")?;
    if !ruta.exists() {
        Config::load().0.guardar(&ruta).map_err(|e| e.to_string())?;
    }
    app.shell()
        .open(ruta.display().to_string(), None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn asistente_reiniciar(app: tauri::AppHandle, state: tauri::State<'_, AjustesState>) -> Result<(), String> {
    exigir_sesion(&state)?;
    crate::assistant::shutdown(&app);
    crate::assistant::start(&app);
    Ok(())
}
```

`Config` must be deserializable from the frontend payload, which it already is (`Deserialize` is derived). It must also serialize back, which it does.

- [ ] **Step 6: Register state and commands**

In `gui/src/lib.rs`, add to the builder chain after the existing `.manage(...)`:

```rust
        .manage(commands::ajustes::AjustesState::default())
```

and add these to `tauri::generate_handler![...]`:

```rust
            commands::ajustes::ti_estado,
            commands::ajustes::ti_desbloquear,
            commands::ajustes::ti_bloquear,
            commands::ajustes::ti_definir_password,
            commands::ajustes::ti_cambiar_password,
            commands::ajustes::ti_leer,
            commands::ajustes::ti_guardar,
            commands::ajustes::ti_restaurar_defectos,
            commands::ajustes::ti_abrir_archivo,
            commands::ajustes::asistente_reiniciar,
```

`assistant::shutdown` and `assistant::start` are already `pub` in `gui/src/assistant.rs`; no visibility change needed.

- [ ] **Step 7: Build and run the tests**

Run: `cargo test -p muniani-gui`
Expected: PASS. If `tauri_plugin_shell::ShellExt` fails to resolve, confirm `tauri-plugin-shell` is in `gui/Cargo.toml` — `lib.rs` already initialises the plugin, so it is.

- [ ] **Step 8: Commit and push**

```bash
git add gui/src/commands/ajustes.rs gui/src/commands/mod.rs gui/src/lib.rs
git commit -m "feat(ajustes): comandos del panel de TI

Desbloqueo con espera creciente tras tres fallos, lectura y escritura de
munianci.config.json, rotacion de contrasena, restauracion por seccion, apertura
del archivo en el editor del sistema y reinicio del Asistente.

La sesion vive en el host, no en el webview: la contrasena no sale del proceso
de Rust mas alla de las teclas que se escriben en el campo."
git push origin main
```

---

### Task 6: cog button, dropdown shell and lock screen

**Files:**
- Create: `gui/frontend/src/components/AjustesTI.tsx`
- Modify: `gui/frontend/src/types.ts`
- Modify: `gui/frontend/src/App.tsx:87-142`
- Modify: `gui/frontend/src/app.css`

**Interfaces:**
- Consumes: the Tauri commands from Task 5.
- Produces: `<AjustesTI onGuardado={(r: ResultadoGuardar) => void} />`, and the types `ConfigTI`, `EstadoTI`, `ResultadoGuardar` exported from `types.ts`.

- [ ] **Step 1: Add the types**

Append to `gui/frontend/src/types.ts`:

```ts
export type IdentidadTI = { institucion?: string; tier?: string };
export type PoamTI = { plazo_dias_critica: number; plazo_dias_alta: number; plazo_dias_media: number };
export type InformeTI = {
  tamano_papel_tecnico: "oficio" | "carta" | "a4";
  tamano_papel_ejecutivo: "oficio" | "carta" | "a4";
  color_primario: string;
  color_alerta: string;
  color_texto: string;
  color_apagado: string;
};
export type HistoricoTI = { habilitado: boolean; desglose_por_activo: boolean; retencion_meses: number };
export type RedTI = {
  arp: boolean; icmp: boolean; tcp: boolean;
  arp_pps: number; icmp_timeout_ms: number; tcp_timeout_ms: number; hilos: number;
};
export type MonitoreoTI = {
  habilitado: boolean; intervalo_semanas: number;
  dia_semana: string; hora: string; aviso_vencido_dias: number;
};

export type ConfigTI = {
  identidad: IdentidadTI;
  poam: PoamTI;
  informe: InformeTI;
  historico: HistoricoTI;
  red: RedTI;
  monitoreo: MonitoreoTI;
};

export type EstadoTI = {
  conCandado: boolean;
  passwordConfigurada: boolean;
  desbloqueado: boolean;
  esperaS: number;
  origen: string;
  ruta: string | null;
};

export type ResultadoGuardar = {
  requiereReinicioAsistente: boolean;
  afectaInforme: boolean;
  ruta: string;
};
```

- [ ] **Step 2: Write the component shell with the lock screen**

Create `gui/frontend/src/components/AjustesTI.tsx`:

```tsx
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ConfigTI, EstadoTI, ResultadoGuardar } from "../types";

type Props = { onGuardado: (r: ResultadoGuardar) => void };

export function AjustesTI({ onGuardado }: Props) {
  const [abierto, setAbierto] = useState(false);
  const [estado, setEstado] = useState<EstadoTI | null>(null);
  const [config, setConfig] = useState<ConfigTI | null>(null);
  const [password, setPassword] = useState("");
  const [password2, setPassword2] = useState("");
  const [aviso, setAviso] = useState<string | null>(null);
  const contenedor = useRef<HTMLDivElement>(null);

  const refrescarEstado = useCallback(async () => {
    const e = await invoke<EstadoTI>("ti_estado");
    setEstado(e);
    if (e.desbloqueado) setConfig(await invoke<ConfigTI>("ti_leer"));
  }, []);

  useEffect(() => {
    if (abierto) {
      setAviso(null);
      refrescarEstado().catch((e) => setAviso(String(e)));
    }
  }, [abierto, refrescarEstado]);

  useEffect(() => {
    if (!abierto) return;
    const enfocables = () =>
      Array.from(
        contenedor.current?.querySelectorAll<HTMLElement>(
          '.ajustes__panel button, .ajustes__panel input, .ajustes__panel select'
        ) ?? []
      ).filter((el) => !el.hasAttribute("disabled"));

    enfocables()[0]?.focus();

    const alTeclear = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") { setAbierto(false); return; }
      if (ev.key !== "Tab") return;
      const items = enfocables();
      if (items.length === 0) return;
      const primero = items[0];
      const ultimo = items[items.length - 1];
      const activo = document.activeElement as HTMLElement | null;
      if (ev.shiftKey && (activo === primero || !contenedor.current?.contains(activo))) {
        ev.preventDefault();
        ultimo.focus();
      } else if (!ev.shiftKey && activo === ultimo) {
        ev.preventDefault();
        primero.focus();
      }
    };
    const alClicar = (ev: MouseEvent) => {
      if (contenedor.current && !contenedor.current.contains(ev.target as Node)) setAbierto(false);
    };
    document.addEventListener("keydown", alTeclear);
    document.addEventListener("mousedown", alClicar);
    return () => {
      document.removeEventListener("keydown", alTeclear);
      document.removeEventListener("mousedown", alClicar);
    };
  }, [abierto, estado?.desbloqueado]);

  const desbloquear = async () => {
    try {
      const ok = await invoke<boolean>("ti_desbloquear", { password });
      if (!ok) { setAviso("Contrasena incorrecta."); await refrescarEstado(); return; }
      setPassword("");
      setAviso(null);
      await refrescarEstado();
    } catch (e) { setAviso(String(e)); await refrescarEstado(); }
  };

  const definirPassword = async () => {
    if (password !== password2) { setAviso("Las dos contrasenas no coinciden."); return; }
    try {
      await invoke("ti_definir_password", { password });
      setPassword(""); setPassword2(""); setAviso(null);
      await refrescarEstado();
    } catch (e) { setAviso(String(e)); }
  };

  return (
    <div className="ajustes" ref={contenedor}>
      <button
        className="ajustes__cog"
        aria-label="Ajustes de TI"
        aria-expanded={abierto}
        onClick={() => setAbierto((v) => !v)}
      >
        &#9881;
      </button>

      {abierto && (
        <div className="ajustes__panel" role="dialog" aria-label="Ajustes de TI">
          <div className="ajustes__titulo">Ajustes de TI</div>

          {!estado?.conCandado && (
            <div className="ajustes__nota">Sin contrasena: build de desarrollo.</div>
          )}

          {aviso && <div className="ajustes__error" role="alert">{aviso}</div>}

          {estado && !estado.desbloqueado && !estado.passwordConfigurada && (
            <div className="ajustes__bloqueo">
              <p>Este equipo aun no tiene contrasena de TI. Defina una para continuar.</p>
              <input type="password" placeholder="Nueva contrasena" value={password}
                     onChange={(e) => setPassword(e.target.value)} />
              <input type="password" placeholder="Repita la contrasena" value={password2}
                     onChange={(e) => setPassword2(e.target.value)} />
              <button className="btn btn--primary" onClick={definirPassword}>Definir contrasena</button>
            </div>
          )}

          {estado && !estado.desbloqueado && estado.passwordConfigurada && (
            <div className="ajustes__bloqueo">
              <input type="password" placeholder="Contrasena de TI" value={password}
                     onChange={(e) => setPassword(e.target.value)}
                     onKeyDown={(e) => { if (e.key === "Enter") desbloquear(); }} />
              <button className="btn btn--primary" disabled={estado.esperaS > 0} onClick={desbloquear}>
                {estado.esperaS > 0 ? `Espere ${estado.esperaS} s` : "Desbloquear"}
              </button>
            </div>
          )}

          {estado?.desbloqueado && config && (
            <Secciones
              config={config}
              setConfig={setConfig}
              estado={estado}
              setAviso={setAviso}
              onGuardado={onGuardado}
              cerrar={() => setAbierto(false)}
            />
          )}
        </div>
      )}
    </div>
  );
}
```

`Secciones` is written in Task 7. Add a placeholder for now so this task compiles on its own:

```tsx
function Secciones(_: {
  config: ConfigTI;
  setConfig: (c: ConfigTI) => void;
  estado: EstadoTI;
  setAviso: (s: string | null) => void;
  onGuardado: (r: ResultadoGuardar) => void;
  cerrar: () => void;
}) {
  return null;
}
```

- [ ] **Step 3: Mount it in the header**

In `gui/frontend/src/App.tsx`, add the import:

```tsx
import { AjustesTI } from "./components/AjustesTI";
```

and place the component as the last child of `<header className="app-header">`, after the `<nav className="app-tabs">` block:

```tsx
        <AjustesTI onGuardado={() => {}} />
```

The real `onGuardado` handler arrives in Task 8.

- [ ] **Step 4: Add the styles**

Append to `gui/frontend/src/app.css`:

```css
.ajustes { position: relative; margin-left: auto; }

.ajustes__cog {
  background: none;
  border: none;
  color: inherit;
  font-size: 1.35rem;
  line-height: 1;
  cursor: pointer;
  padding: 0.25rem 0.5rem;
  opacity: 0.75;
}
.ajustes__cog:hover { opacity: 1; }

.ajustes__panel {
  position: absolute;
  top: 100%;
  right: 0;
  z-index: 40;
  width: 26rem;
  max-height: 32rem;
  overflow-y: auto;
  padding: 1rem;
  border: 1px solid rgba(168, 183, 199, 0.5);
  border-radius: 6px;
  background: #ffffff;
  color: #0a132d;
  box-shadow: 0 12px 28px rgba(10, 19, 45, 0.22);
  text-align: left;
}

.ajustes__titulo { font-weight: 600; margin-bottom: 0.75rem; }

.ajustes__nota,
.ajustes__error {
  font-size: 0.8rem;
  padding: 0.5rem 0.6rem;
  border-radius: 4px;
  margin-bottom: 0.75rem;
}
.ajustes__nota  { background: #eef4fa; color: #0a132d; }
.ajustes__error { background: #fdecec; color: #8a1f1f; }

.ajustes__bloqueo { display: flex; flex-direction: column; gap: 0.5rem; }
.ajustes__bloqueo input { padding: 0.45rem 0.55rem; border: 1px solid #a8b7c7; border-radius: 4px; }
```

- [ ] **Step 5: Build the frontend**

Run: `cd gui/frontend; npm run build`
Expected: build succeeds with no TypeScript errors.

- [ ] **Step 6: See it in the running app**

Run: `cargo run -p muniani-gui`
Expected: the cog appears at the right of the header on every tab. Clicking it opens the panel showing "Sin contrasena: build de desarrollo." (debug build). Escape and an outside click both close it.

- [ ] **Step 7: Commit and push**

```bash
git add gui/frontend/src/components/AjustesTI.tsx gui/frontend/src/types.ts gui/frontend/src/App.tsx gui/frontend/src/app.css
git commit -m "feat(gui): cog de ajustes de TI con panel desplegable y candado

El engranaje vive en el encabezado y esta a la vista en todas las pestanas: TI
no tiene que saber en cual esta parado para encontrar lo suyo, y el candado ya
impide que un funcionario municipal pase de ahi.

El panel se cierra con Escape y con un clic afuera. En un build de depuracion se
abre sin contrasena y lo dice en pantalla, para que ese estado no se confunda
nunca con lo que recibe un cliente."
git push origin main
```

---

### Task 7: the four editable sections

**Files:**
- Modify: `gui/frontend/src/components/AjustesTI.tsx`
- Modify: `gui/frontend/src/app.css`

**Interfaces:**
- Consumes: `ConfigTI`, `EstadoTI`, `ResultadoGuardar` (Task 6), commands `ti_guardar`, `ti_restaurar_defectos`, `ti_abrir_archivo`, `ti_cambiar_password`, `asistente_reiniciar` (Task 5).
- Produces: the working `Secciones` component.

- [ ] **Step 1: Replace the `Secciones` placeholder**

```tsx
type SeccionesProps = {
  config: ConfigTI;
  setConfig: (c: ConfigTI) => void;
  estado: EstadoTI;
  setAviso: (s: string | null) => void;
  onGuardado: (r: ResultadoGuardar) => void;
  cerrar: () => void;
};

const SECCIONES = [
  { id: "identidad", titulo: "Identidad" },
  { id: "poam", titulo: "Plazos e historico" },
  { id: "red", titulo: "Red y monitoreo" },
  { id: "informe", titulo: "Informe" },
] as const;

function Secciones({ config, setConfig, estado, setAviso, onGuardado, cerrar }: SeccionesProps) {
  const [abierta, setAbierta] = useState<string | null>("identidad");
  const [guardando, setGuardando] = useState(false);

  const set = <K extends keyof ConfigTI>(clave: K, valor: ConfigTI[K]) =>
    setConfig({ ...config, [clave]: valor });

  const guardar = async () => {
    setGuardando(true);
    try {
      const r = await invoke<ResultadoGuardar>("ti_guardar", { nueva: config });
      if (r.requiereReinicioAsistente) {
        const seguir = window.confirm(
          "Cambiar la institucion reinicia el Asistente. Se pierde la conversacion abierta " +
            "y el backend puede tardar hasta tres minutos en volver a estar listo. Continuar?"
        );
        if (seguir) await invoke("asistente_reiniciar");
      }
      setAviso(null);
      onGuardado(r);
      cerrar();
    } catch (e) {
      setAviso(String(e));
    } finally {
      setGuardando(false);
    }
  };

  const restaurar = async (seccion: string) => {
    try {
      setConfig(await invoke<ConfigTI>("ti_restaurar_defectos", { seccion }));
    } catch (e) {
      setAviso(String(e));
    }
  };

  return (
    <>
      {SECCIONES.map((s) => (
        <div className="ajustes__seccion" key={s.id}>
          <button
            className="ajustes__cabecera"
            aria-expanded={abierta === s.id}
            onClick={() => setAbierta(abierta === s.id ? null : s.id)}
          >
            {s.titulo}
          </button>
          {abierta === s.id && (
            <div className="ajustes__campos">
              {s.id === "identidad" && (
                <>
                  <label>
                    Institucion
                    <input
                      type="text"
                      value={config.identidad.institucion ?? ""}
                      onChange={(e) => set("identidad", { ...config.identidad, institucion: e.target.value })}
                    />
                  </label>
                  <label>
                    Clasificacion
                    <select
                      value={config.identidad.tier ?? "pse"}
                      onChange={(e) => set("identidad", { ...config.identidad, tier: e.target.value })}
                    >
                      <option value="pse">Prestador de servicios esenciales</option>
                      <option value="oiv">Operador de importancia vital</option>
                      <option value="unclassified">Sin clasificar</option>
                    </select>
                  </label>
                  <p className="ajustes__ayuda">
                    Operador de importancia vital corresponde solo a quien la Agencia haya calificado
                    como tal por resolucion fundada. Sin clasificar apaga el deber de reporte al CSIRT
                    en todo el informe.
                  </p>
                </>
              )}

              {s.id === "poam" && (
                <>
                  <label>
                    Plazo brecha critica (dias)
                    <input type="number" min={1} value={config.poam.plazo_dias_critica}
                      onChange={(e) => set("poam", { ...config.poam, plazo_dias_critica: Number(e.target.value) })} />
                  </label>
                  <label>
                    Plazo brecha alta (dias)
                    <input type="number" min={1} value={config.poam.plazo_dias_alta}
                      onChange={(e) => set("poam", { ...config.poam, plazo_dias_alta: Number(e.target.value) })} />
                  </label>
                  <label>
                    Plazo brecha media (dias)
                    <input type="number" min={1} value={config.poam.plazo_dias_media}
                      onChange={(e) => set("poam", { ...config.poam, plazo_dias_media: Number(e.target.value) })} />
                  </label>
                  <p className="ajustes__ayuda">
                    No son plazos legales. El unico plazo perentorio de la Ley 21.663 es el reporte al
                    CSIRT del Art. 9 (3 horas), que el informe trata aparte.
                  </p>
                  <label className="ajustes__check">
                    <input type="checkbox" checked={config.historico.habilitado}
                      onChange={(e) => set("historico", { ...config.historico, habilitado: e.target.checked })} />
                    Llevar historico de evaluaciones
                  </label>
                  <label className="ajustes__check">
                    <input type="checkbox" checked={config.historico.desglose_por_activo}
                      onChange={(e) => set("historico", { ...config.historico, desglose_por_activo: e.target.checked })} />
                    Guardar que activo arrastra cada brecha
                  </label>
                  <label>
                    Retencion (meses, 0 = nunca purgar)
                    <input type="number" min={0} value={config.historico.retencion_meses}
                      onChange={(e) => set("historico", { ...config.historico, retencion_meses: Number(e.target.value) })} />
                  </label>
                  <button className="ajustes__restaurar" onClick={() => restaurar("poam")}>
                    Restaurar plazos por defecto
                  </button>
                </>
              )}

              {s.id === "red" && (
                <>
                  <label className="ajustes__check">
                    <input type="checkbox" checked={config.red.arp}
                      onChange={(e) => set("red", { ...config.red, arp: e.target.checked })} />
                    ARP
                  </label>
                  <label className="ajustes__check">
                    <input type="checkbox" checked={config.red.icmp}
                      onChange={(e) => set("red", { ...config.red, icmp: e.target.checked })} />
                    ICMP
                  </label>
                  <label className="ajustes__check">
                    <input type="checkbox" checked={config.red.tcp}
                      onChange={(e) => set("red", { ...config.red, tcp: e.target.checked })} />
                    TCP
                  </label>
                  <label>
                    Sondas ARP por segundo (0 = sin limite)
                    <input type="number" min={0} value={config.red.arp_pps}
                      onChange={(e) => set("red", { ...config.red, arp_pps: Number(e.target.value) })} />
                  </label>
                  <p className="ajustes__advertencia">
                    Si la red usa Dynamic ARP Inspection, subir este valor puede dejar el puerto en
                    err-disable, o sea este equipo se queda sin red hasta que el area de redes lo
                    rehabilite. Coordine el primer barrido de LAN completa con esa area.
                  </p>
                  <label>
                    Espera ICMP (ms)
                    <input type="number" min={1} value={config.red.icmp_timeout_ms}
                      onChange={(e) => set("red", { ...config.red, icmp_timeout_ms: Number(e.target.value) })} />
                  </label>
                  <label>
                    Espera TCP (ms)
                    <input type="number" min={1} value={config.red.tcp_timeout_ms}
                      onChange={(e) => set("red", { ...config.red, tcp_timeout_ms: Number(e.target.value) })} />
                  </label>
                  <label>
                    Hilos (0 = automatico)
                    <input type="number" min={0} value={config.red.hilos}
                      onChange={(e) => set("red", { ...config.red, hilos: Number(e.target.value) })} />
                  </label>
                  <label className="ajustes__check">
                    <input type="checkbox" checked={config.monitoreo.habilitado}
                      onChange={(e) => set("monitoreo", { ...config.monitoreo, habilitado: e.target.checked })} />
                    Reescaneo programado
                  </label>
                  <label>
                    Dia
                    <select value={config.monitoreo.dia_semana}
                      onChange={(e) => set("monitoreo", { ...config.monitoreo, dia_semana: e.target.value })}>
                      {["lunes", "martes", "miercoles", "jueves", "viernes", "sabado", "domingo"].map((d) => (
                        <option key={d} value={d}>{d}</option>
                      ))}
                    </select>
                  </label>
                  <label>
                    Hora
                    <input type="time" value={config.monitoreo.hora}
                      onChange={(e) => set("monitoreo", { ...config.monitoreo, hora: e.target.value })} />
                  </label>
                  <label>
                    Avisar medicion vencida a los (dias)
                    <input type="number" min={1} value={config.monitoreo.aviso_vencido_dias}
                      onChange={(e) => set("monitoreo", { ...config.monitoreo, aviso_vencido_dias: Number(e.target.value) })} />
                  </label>
                  <button className="ajustes__restaurar" onClick={() => restaurar("red")}>
                    Restaurar red por defecto
                  </button>
                </>
              )}

              {s.id === "informe" && (
                <>
                  <label>
                    Papel del informe tecnico
                    <select value={config.informe.tamano_papel_tecnico}
                      onChange={(e) => set("informe", { ...config.informe, tamano_papel_tecnico: e.target.value as "oficio" | "carta" | "a4" })}>
                      <option value="oficio">Oficio (21,6 x 33 cm)</option>
                      <option value="carta">Carta (21,6 x 27,9 cm)</option>
                      <option value="a4">A4 (21,0 x 29,7 cm)</option>
                    </select>
                  </label>
                  <label>
                    Papel del informe ejecutivo
                    <select value={config.informe.tamano_papel_ejecutivo}
                      onChange={(e) => set("informe", { ...config.informe, tamano_papel_ejecutivo: e.target.value as "oficio" | "carta" | "a4" })}>
                      <option value="oficio">Oficio (21,6 x 33 cm)</option>
                      <option value="carta">Carta (21,6 x 27,9 cm)</option>
                      <option value="a4">A4 (21,0 x 29,7 cm)</option>
                    </select>
                  </label>
                  <label>
                    Color primario
                    <input type="color" value={config.informe.color_primario}
                      onChange={(e) => set("informe", { ...config.informe, color_primario: e.target.value })} />
                  </label>
                  <label>
                    Color de alerta
                    <input type="color" value={config.informe.color_alerta}
                      onChange={(e) => set("informe", { ...config.informe, color_alerta: e.target.value })} />
                  </label>
                  <button className="ajustes__restaurar" onClick={() => restaurar("informe")}>
                    Restaurar informe por defecto
                  </button>
                </>
              )}
            </div>
          )}
        </div>
      ))}

      <div className="ajustes__pie">
        <button className="btn btn--primary" disabled={guardando} onClick={guardar}>
          {guardando ? "Guardando..." : "Guardar"}
        </button>
        <button className="btn" onClick={cerrar}>Cancelar</button>
      </div>

      <div className="ajustes__extras">
        <button onClick={() => invoke("ti_abrir_archivo").catch((e) => setAviso(String(e)))}>
          Abrir el archivo de configuracion
        </button>
        <CambiarPassword setAviso={setAviso} />
        <p className="ajustes__origen">Configuracion leida de: {estado.origen}</p>
      </div>
    </>
  );
}

function CambiarPassword({ setAviso }: { setAviso: (s: string | null) => void }) {
  const [visible, setVisible] = useState(false);
  const [actual, setActual] = useState("");
  const [nueva, setNueva] = useState("");

  const cambiar = async () => {
    try {
      await invoke("ti_cambiar_password", { actual, nueva });
      setActual(""); setNueva(""); setVisible(false);
      setAviso(null);
    } catch (e) { setAviso(String(e)); }
  };

  if (!visible) {
    return <button onClick={() => setVisible(true)}>Cambiar contrasena</button>;
  }
  return (
    <div className="ajustes__password">
      <input type="password" placeholder="Contrasena actual" value={actual}
        onChange={(e) => setActual(e.target.value)} />
      <input type="password" placeholder="Contrasena nueva" value={nueva}
        onChange={(e) => setNueva(e.target.value)} />
      <button className="btn btn--sm btn--primary" onClick={cambiar}>Guardar contrasena</button>
      <button className="btn btn--sm" onClick={() => setVisible(false)}>Cancelar</button>
    </div>
  );
}
```

- [ ] **Step 2: Add the section styles**

Append to `gui/frontend/src/app.css`:

```css
.ajustes__seccion { border-top: 1px solid #e3e9ef; }

.ajustes__cabecera {
  width: 100%;
  text-align: left;
  background: none;
  border: none;
  padding: 0.6rem 0;
  font-weight: 600;
  font-size: 0.9rem;
  cursor: pointer;
  color: inherit;
}

.ajustes__campos { display: flex; flex-direction: column; gap: 0.55rem; padding-bottom: 0.75rem; }
.ajustes__campos label { display: flex; flex-direction: column; gap: 0.2rem; font-size: 0.82rem; }
.ajustes__campos input[type="text"],
.ajustes__campos input[type="number"],
.ajustes__campos input[type="time"],
.ajustes__campos select {
  padding: 0.35rem 0.45rem;
  border: 1px solid #a8b7c7;
  border-radius: 4px;
  font-size: 0.85rem;
}
.ajustes__check { flex-direction: row !important; align-items: center; gap: 0.4rem !important; }

.ajustes__ayuda,
.ajustes__advertencia { font-size: 0.75rem; line-height: 1.35; margin: 0; }
.ajustes__ayuda { color: #5b6b7d; }
.ajustes__advertencia {
  color: #8a1f1f;
  background: #fdecec;
  padding: 0.5rem 0.6rem;
  border-radius: 4px;
}

.ajustes__restaurar {
  align-self: flex-start;
  background: none;
  border: none;
  padding: 0;
  font-size: 0.78rem;
  text-decoration: underline;
  cursor: pointer;
  color: #006fb3;
}

.ajustes__pie { display: flex; gap: 0.5rem; padding: 0.85rem 0 0.5rem; border-top: 1px solid #e3e9ef; }

.ajustes__extras { display: flex; flex-direction: column; align-items: flex-start; gap: 0.4rem; }
.ajustes__extras > button {
  background: none; border: none; padding: 0;
  font-size: 0.78rem; text-decoration: underline; cursor: pointer; color: #006fb3;
}
.ajustes__password { display: flex; flex-direction: column; gap: 0.35rem; width: 100%; }
.ajustes__password input { padding: 0.35rem 0.45rem; border: 1px solid #a8b7c7; border-radius: 4px; }
.ajustes__origen { font-size: 0.72rem; color: #5b6b7d; margin: 0.25rem 0 0; word-break: break-all; }
```

- [ ] **Step 3: Build the frontend**

Run: `cd gui/frontend; npm run build`
Expected: build succeeds with no TypeScript errors.

- [ ] **Step 4: Exercise it end to end**

Run: `cargo run -p muniani-gui`

Check by hand, and record what you see:
1. Open the cog. All four sections are listed; opening one closes the previous.
2. Change the critical deadline to 45 and press Guardar.
3. The panel closes. Reopen it: the field still reads 45.
4. Confirm the file on disk: `Get-Content (Join-Path (Split-Path (Get-Process muniani-gui).Path) munianci.config.json)` — or read the path the panel shows under "Configuracion leida de". `poam.plazo_dias_critica` must be 45 and the `_ayuda` block must still be present.
5. Change the institution and press Guardar. The confirm dialog about restarting the Asistente appears.

- [ ] **Step 5: Commit and push**

```bash
git add gui/frontend/src/components/AjustesTI.tsx gui/frontend/src/app.css
git commit -m "feat(gui): secciones editables del panel de TI

Identidad, plazos e historico, red y monitoreo, e informe, en un acordeon que
abre una a la vez. La advertencia de Dynamic ARP Inspection va al lado del campo
que la provoca y no en un tooltip, porque es la unica opcion del panel que puede
dejar al equipo sin red.

Cambiar la institucion pide confirmacion antes de reiniciar el Asistente: se
pierde la conversacion abierta y el backend tarda en volver."
git push origin main
```

---

### Task 8: stale-scan banner

**Files:**
- Modify: `gui/frontend/src/App.tsx`
- Modify: `gui/frontend/src/app.css`

**Interfaces:**
- Consumes: `ResultadoGuardar.afectaInforme` from Task 5, `<AjustesTI onGuardado>` from Task 6.
- Produces: nothing downstream.

- [ ] **Step 1: Track staleness in `App.tsx`**

Add the state next to the other `useState` calls:

```tsx
  const [configVencida, setConfigVencida] = useState(false);
```

Replace the placeholder mount from Task 6 with:

```tsx
        <AjustesTI
          onGuardado={(r) => {
            if (r.afectaInforme && result) setConfigVencida(true);
            revisarMonitoreo();
          }}
        />
```

Clear the flag when a new scan starts. Inside `startScan`, next to the other resets:

```tsx
    setConfigVencida(false);
```

- [ ] **Step 2: Render the banner**

Directly after the existing `{monitoreo?.vencido && (...)}` block and before `<header className="app-header">`:

```tsx
      {configVencida && (
        <div className="aviso-config" role="status">
          <strong>La configuracion cambio despues de este escaneo.</strong>{" "}
          El resultado en pantalla ya no corresponde a los ajustes vigentes.{" "}
          <button
            className="btn btn--primary btn--sm"
            onClick={startScan}
            disabled={scanState === "scanning"}
          >
            Escanear de nuevo
          </button>
        </div>
      )}
```

- [ ] **Step 3: Style it**

Append to `gui/frontend/src/app.css`:

```css
.aviso-config {
  padding: 0.65rem 1rem;
  background: #fff6e5;
  color: #7a4b00;
  border-bottom: 1px solid #f0d9ae;
  font-size: 0.85rem;
}
```

- [ ] **Step 4: Build and verify by hand**

Run: `cd gui/frontend; npm run build`
Expected: build succeeds.

Run: `cargo run -p muniani-gui`
1. Run a scan and let it finish.
2. Open the cog, change the critical deadline, Guardar. The amber banner appears.
3. Open the cog, change only the primary colour, Guardar. No new banner (dismiss the first with a rescan before testing this).
4. Press "Escanear de nuevo". The banner clears when the scan starts.

- [ ] **Step 5: Commit and push**

```bash
git add gui/frontend/src/App.tsx gui/frontend/src/app.css
git commit -m "feat(gui): avisar cuando la configuracion cambio despues del escaneo

El resultado en pantalla se conserva, pero deja de presentarse como vigente.
Sin esto se puede exportar un PDF cuyos plazos contradicen la seccion de
configuracion de ese mismo PDF.

Solo lo marcan identidad, plazos y red. Un cambio de color o de papel no invalida
una medicion."
git push origin main
```

---

### Task 9: docs, ADRs, roadmap and changelog

**Files:**
- Create: `docs/adr/00XX-identidad-configurable-en-ejecucion.md`
- Create: `docs/adr/00XX-candado-de-ti-argon2id.md`
- Create: `docs/adr/00XX-institucion-por-defecto-neutra-y-tier-pse.md`
- Modify: `docs/adr/README.md`
- Modify: `ROADMAP.md`
- Modify: `CHANGELOG.md`
- Modify: `README.md`
- Modify: `CLAUDE.md`

**Interfaces:**
- Consumes: everything above.
- Produces: nothing downstream.

- [ ] **Step 1: Read the ADR index and take the next numbers**

Run: `cat docs/adr/README.md`
Use the next three free numbers in sequence. Do not renumber anything.

- [ ] **Step 2: Write the three ADRs**

Each one is MADR-lite with exactly these headings: `Status`, `Date`, `Deciders: Felipe Carvajal Brown`, `Context`, `Decision`, `Consequences`, `Alternatives considered`. `Date` is `2026-08-03`. `Status` is `Aceptado`.

The content comes from the decisions table in `docs/superpowers/specs/2026-08-03-panel-ajustes-ti-design.md` and from the alternatives recorded there — do not invent new reasoning, and do not add decisions Felipe did not make. The alternatives to record:

- *Identidad configurable*: separate file next to the config; a file under `%LOCALAPPDATA%` away from the install dir.
- *Candado*: immutable build password; installer NSIS prompt; first-run only; signing the config so hand edits are rejected.
- *Institucion por defecto y tier*: keeping a real municipality as the fallback; empty with a first-run prompt; defaulting the tier to `unclassified` or `oiv`.

The tier ADR must cite Art. 1 inc. 2 and Art. 4 inc. 2 of Ley 21.663 against the local PDF at `docs/Ley-21663_08-ABR-2024.pdf`, and must state that OIV is conferred only by resolución fundada under Arts. 5 and 6. It must not assert a classification for any specific institution.

- [ ] **Step 3: Update the ADR index**

Add the three rows to the `# | Title | Status` table in `docs/adr/README.md`.

- [ ] **Step 4: Correct `CLAUDE.md`**

The per-client branding section currently ends with "Lo compilado por build (`MUNIANI_INSTITUTION`, `MUNIANI_TIER`) sigue siendo compilado: la identidad del cliente no es configuración de TI." That is now false. Replace that sentence with:

```
Desde 0.8.0 la identidad tambien es configuracion de ejecucion: la seccion
`identidad` de `munianci.config.json` gana sobre `MUNIANI_INSTITUTION` y
`MUNIANI_TIER`, y el panel de ajustes (el engranaje del encabezado, tras la
contrasena de TI) es la via prevista para editarla. Lo compilado pasa a ser el
valor de fabrica de cada cliente, no un valor inamovible.
```

- [ ] **Step 5: Add the two roadmap milestones**

In `ROADMAP.md`, add two phases with status `No iniciado`, following the file's existing format:

- **Desmunicipalizar la interfaz y el informe** — "Vista Municipal" pasa a un nombre institucional neutro, y "municipalidad" deja de aparecer en la interfaz, el informe y el prompt del Asistente. El producto sirve a municipalidades y a otros organismos del Estado, y hoy el texto solo nombra a las primeras.
- **Enrutamiento al CSIRT de la Defensa Nacional** — bajo el Reglamento de Ciberseguridad de la Defensa Nacional, un organismo del sector Defensa reporta al CSIRT-DN, que a su vez informa a la Agencia. El informe y el JSON para la ANCI hoy se dirigen al CSIRT Nacional.

Do **not** state the Decreto N° 2 number or its publication date in `ROADMAP.md` yet. That identifier is not verified against the primary source; Task 10 verifies it, and only then does it get written down.

- [ ] **Step 6: Update `CHANGELOG.md`**

Under `[Unreleased]`, add to `Añadido` and `Cambiado` in the existing Keep a Changelog format:

- Añadido: panel de ajustes de TI tras el engranaje del encabezado, con contraseña; sección `identidad` en `munianci.config.json`; aviso cuando la configuración cambia después de un escaneo.
- Cambiado: la institución y el tier se resuelven en ejecución y ya no solo en compilación; la institución por defecto pasa a un marcador neutro; el escaneo de la GUI pasa a respetar la sección `red` del archivo, que antes descartaba.

- [ ] **Step 7: Document the panel for IT in `README.md`**

Add a subsection under the existing configuration material covering: where the cog is, that the password ships set per client and can be rotated from the panel, that deleting `%LOCALAPPDATA%\MuniANCI\ti-password.hash` restores the build password, and — stated plainly — that the password is a guard against accidental changes and not a security boundary, because `munianci.config.json` remains editable by hand on purpose.

- [ ] **Step 8: Verify and commit**

Run: `cargo test`
Expected: PASS.

```bash
git add docs/adr CHANGELOG.md ROADMAP.md README.md CLAUDE.md
git commit -m "docs: ADR, roadmap y manual del panel de ajustes de TI

Tres decisiones quedan registradas: la identidad pasa a ser configuracion de
ejecucion, el candado del panel usa Argon2id con rotacion acotada al build, y la
institucion por defecto deja de ser un cliente real mientras el tier se mantiene
en pse.

Se corrige la regla de CLAUDE.md que decia que la identidad del cliente no es
configuracion de TI: este hito la revierte a proposito.

Dos hitos nuevos en el roadmap: desmunicipalizar el texto, y el enrutamiento al
CSIRT de la Defensa Nacional."
git push origin main
```

---

### Task 10: demo corpus documents

**Files:**
- Create: several PDFs under `docs/`
- Modify: `ROADMAP.md` (only to add the verified Decreto identifier)

**Interfaces:**
- Consumes: nothing.
- Produces: primary sources for the Fuerza Aérea and Ejército demos.

This task is independent of Tasks 1-9 and can run in any order relative to them.

- [ ] **Step 1: Confirm `.gitattributes` already protects PDFs**

Run: `grep pdf .gitattributes`
Expected: `*.pdf   binary`. It is already there; do not add `* text=auto`.

- [ ] **Step 2: Download the four document sets**

Save into `docs/`, keeping the publisher's own filename where it is meaningful:

1. Reglamento de Ciberseguridad de la Defensa Nacional (Decreto del Ministerio de Defensa Nacional). Start from the Diario Oficial edition of 31-DIC-2025 referenced at `https://www.doe.cl/alerta/31122025/2748664`.
2. `https://www.ssffaa.cl/wp-content/uploads/2025/10/Politica-General-de-Seguridad-de-la-Informacion_web.pdf`
3. `https://www.defensa.cl/wp-content/uploads/2023/06/POL%C3%8DTICA-DE-DEFENSA-NACIONAL-DE-CHILE-2020.pdf` and the Política Nacional de Ciberdefensa from the Diario Oficial of 09-MAR-2018 at `https://www.diariooficial.interior.gob.cl/publicaciones/2018/03/09/42003/01/1363153.pdf`
4. `https://fach.mil.cl/gob_transp/marco_normativo/fach/reglamentos/d06.pdf` and `https://fach.mil.cl/gob_transp/marco_normativo/fach/reglamentos/c55d.pdf`

If a download fails or returns an HTML error page rather than a PDF, do not substitute a secondary source and do not guess at the content. Report the failure and ask Felipe for the file, exactly as the CLAUDE.md rule about BCN LeyChile prescribes.

- [ ] **Step 3: Verify the Decreto identifier against the PDF**

Open the downloaded Reglamento and read its own first page. Record verbatim: the decree number, the issuing ministry, the promulgation date and the publication date.

Secondary sources disagree — two doe.cl pages say the Diario Oficial publication was 31-DIC-2025 while trendTIC says 3-FEB-2026, and one source gave "Decreto N° 2" without a date. Whatever the PDF says wins. Nothing about this decree goes into `ROADMAP.md`, a report, a demo or any document until it is read off the PDF.

- [ ] **Step 4: Prove the binary round trip**

After staging but before pushing, for every PDF added:

```bash
git cat-file -p HEAD:docs/<archivo>.pdf | sha256sum
sha256sum docs/<archivo>.pdf
```

Expected: identical hashes. If they differ, the file was mangled by line-ending normalisation — stop and fix `.gitattributes` before continuing.

- [ ] **Step 5: Add the verified identifier to the roadmap**

Now that it is read off the primary source, complete the CSIRT-DN roadmap entry with the exact decree number and publication date.

- [ ] **Step 6: Commit and push**

```bash
git add docs/ ROADMAP.md
git commit -m "docs: fuentes primarias del sector Defensa

Reglamento de Ciberseguridad de la Defensa Nacional, la politica general de
seguridad de la informacion de la Subsecretaria para las Fuerzas Armadas, la
Politica Nacional de Ciberdefensa y la Politica de Defensa Nacional 2020, y los
reglamentos publicos de la Fuerza Aerea.

El identificador del decreto queda tomado del PDF y no de la prensa: las fuentes
secundarias no coincidian en la fecha de publicacion."
git push origin main
```

---

### Task 11: Asistente declares which corpus it is serving

**Files:**
- Modify: `assistant/backend/main.py` (the `/status` handler at `:434-444`)
- Modify: `gui/frontend/src/components/AsistenteTab.tsx`
- Modify: `gui/frontend/src/assistant.css`

**Interfaces:**
- Consumes: `rag.db_dir()`, already imported at `assistant/backend/main.py:28`.
- Produces: two extra fields on `GET /status` — `corpus: string` (the DB folder name) and `corpusInstitucional: boolean`.

Runs after Task 6. Independent of Tasks 7-10.

The backend already falls back correctly: `rag.db_dir()` (`assistant/backend/rag.py:50-68`) resolves `MUNIGPT_DB_DIR`, then `db_<slug>` if that folder exists, then the national `db`. Nothing about the fallback logic changes. What is missing is that the fallback is invisible, so after a rename the Asistente looks like it has an institutional corpus it was never shipped.

- [ ] **Step 1: Report the corpus from `/status`**

In `assistant/backend/main.py`, replace the body of the `status` handler with:

```python
@app.get("/status")
async def status():
    missing = inference.missing_models()
    corpus = db_dir()
    return {
        "status": "ok",
        "ready": not missing and inference.server_binary_present(),
        "missingModels": missing,
        "license": _current_license_status(),
        "corpus": corpus.name,
        "corpusInstitucional": corpus.name != "db",
        **inference.model_info(),
    }
```

Drop the existing docstring rather than keeping it: the no-comments rule covers docstrings on new and edited code.

- [ ] **Step 2: Verify the endpoint by hand**

Start the backend from the repo:

```powershell
cd assistant\backend
..\.venv\Scripts\python.exe -m uvicorn main:app --port 8010
```

In another shell:

```powershell
Invoke-RestMethod http://127.0.0.1:8010/status | ConvertTo-Json
```

Expected: the response now carries `corpus` and `corpusInstitucional`. On a checkout with only the national `db/`, `corpus` is `db` and `corpusInstitucional` is `false`. Paste the actual output; do not report this step as done without it.

- [ ] **Step 3: Surface it in the tab**

In `gui/frontend/src/components/AsistenteTab.tsx`, the component already polls `/status` for readiness. Extend the parsed shape with `corpus?: string; corpusInstitucional?: boolean`, and render this notice above the chat whenever the backend is ready and `corpusInstitucional` is false:

```tsx
{listo && estado?.corpusInstitucional === false && (
  <div className="asistente__corpus" role="status">
    Este equipo no tiene un corpus propio de la institucion. El Asistente responde
    sobre la normativa nacional.
  </div>
)}
```

Match the existing names in that file for the readiness flag and the parsed status object rather than introducing `listo` and `estado` if it already calls them something else.

- [ ] **Step 4: Style the notice**

Append to `gui/frontend/src/assistant.css`:

```css
.asistente__corpus {
  padding: 0.5rem 0.75rem;
  background: #eef4fa;
  color: #0a132d;
  border-bottom: 1px solid #d6e2ee;
  font-size: 0.8rem;
}
```

- [ ] **Step 5: Verify in the app**

Run: `cd gui/frontend; npm run build` then `cargo run -p muniani-gui`
Expected: the Asistente tab shows the national-corpus notice once the backend is ready, on a checkout without a `db_<slug>` folder.

- [ ] **Step 6: Commit and push**

```bash
git add assistant/backend/main.py gui/frontend/src/components/AsistenteTab.tsx gui/frontend/src/assistant.css
git commit -m "feat(asistente): decir cuando se responde con el corpus nacional

El respaldo al corpus nacional ya existia en rag.db_dir, pero era invisible:
tras cambiar la institucion, el Asistente parecia tener un corpus propio de ese
organismo que nunca se le instalo. Ahora /status declara que corpus abrio y la
pestana lo dice cuando no es el institucional."
git push origin main
```

---

## Verification checklist

Before calling this milestone done, all of these must have been observed, not assumed:

- [ ] `cargo test` passes from the repo root, output pasted.
- [ ] `cd gui/frontend; npm run build` succeeds, output pasted.
- [ ] `cargo run -q -p muniani-cli -- --help` shows `Organismo del Estado` as the institution default.
- [ ] In the running GUI: the cog opens, a save round-trips to disk, and the written `munianci.config.json` still contains its `_ayuda` block.
- [ ] Changing the institution restarts the Asistente and the header updates.
- [ ] A release build (`cargo tauri build`) with no `MUNIANI_ADMIN_HASH` asks for a password on first cog press rather than opening.
- [ ] `MUNIANI_FORCE_LOCK=1 cargo run -p muniani-gui` locks the panel in a debug build.
- [ ] Opening the cog focuses the first control, and Tab cycles inside the panel instead of escaping to the page behind it.
- [ ] `GET /status` returns `corpus` and `corpusInstitucional`, output pasted, and the Asistente tab shows the national-corpus notice when there is no `db_<slug>`.
- [ ] Every PDF added under `docs/` matches its stored blob by sha256.

## Notes for whoever executes this

- `Config::load()` is called fresh at each use site (`export_report.rs:62`, `monitoreo.rs:64` and `:129`, `cli/src/main.rs:82`). Saving the file is enough; there is no cache to invalidate and no restart needed for anything except the Asistente.
- The `red` section change in Task 4 is a real bug fix riding along: `start_scan.rs` passed `Default::default()` for the sweep settings, so the GUI silently ignored whatever IT had configured. It is in scope because the panel would otherwise let IT edit a value the GUI never reads.
- Out of scope and deliberately untouched: `tauri.conf.json:26` pins `connect-src` to `http://127.0.0.1:8000` while `puerto_utilizable` (`assistant.rs:298`) can pick a different port, which the CSP would then block. Pre-existing, unrelated, worth its own fix.
