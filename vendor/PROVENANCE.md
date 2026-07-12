# vendor/ — registro de procedencia

Cada artefacto vendorizado se registra aquí al adoptarse (ver `README.md` y
`ROADMAP.md` Apéndice C). Campos: nombre, versión exacta, origen (URL), SHA256 del
artefacto y licencia. Sin registro no se ancla la dependencia.

Los artefactos grandes de `bin/`, `models/` y `nvd/` son gitignored (viajan en el
paquete offline de instalación, D2); su procedencia igual se registra aquí para que el
paquete sea reproducible y auditable.

## Wheels Python (`wheels/`)

Wheelhouse offline. Reconstruible con:

```
py -3.12 -m pip download -r assistant/backend/requirements.txt \
    -r assistant/backend/requirements-dev.txt -d vendor/wheels
```

Instalación air-gapped: `pip install --no-index --find-links vendor/wheels -r ...`.

| Paquete | Versión | Origen | SHA256 | Licencia |
|---|---|---|---|---|
| _(se completa al vendorizar cada wheel en su hito)_ | | | | |

## Crates Rust (`cargo/`)

Mirror de crates del workspace. Reconstruible con `cargo vendor vendor/cargo`, que
imprime el fragmento de `.cargo/config.toml` a fijar. Las herramientas de auditoría/SBOM
(cargo-audit, cargo-deny, cargo-cyclonedx, cargo-sbom, cargo-auditable) son binarios
`cargo install`, no dependencias del workspace: se pinnean por versión en CI, no en
`Cargo.lock`.

| Crate | Versión | Origen | SHA256 | Licencia |
|---|---|---|---|---|
| _(se completa al ejecutar `cargo vendor` en su hito)_ | | | | |

## Binarios externos (`bin/`, gitignored)

| Binario | Versión | Origen | SHA256 | Licencia |
|---|---|---|---|---|
| _(aria2c u otros, pinneados por release + SHA256)_ | | | | |

## Modelos (`models/`, gitignored — paquete offline D2)

| Modelo | Versión/quant | Origen | SHA256 | Licencia |
|---|---|---|---|---|
| _(GGUF: Qwen3-4B, Qwen3-1.7B, nomic-embed-v2-moe; se registran al fijar el origen)_ | | | | |
