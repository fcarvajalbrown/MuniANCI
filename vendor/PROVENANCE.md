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

SHA256 y tamaño son REALES (medidos del archivo local); son el gate de descarga. Las
URLs de origen son candidatas y aún NO confirmadas por el dueño (`source.confirmed`
= false en `assistant/backend/models.manifest.json`); confirmar repo + revisión antes
de habilitar la descarga. La licencia se re-verifica contra el `LICENSE` de cada repo.

| Modelo (archivo) | Quant | Origen candidato (sin confirmar) | SHA256 | Licencia |
|---|---|---|---|---|
| Qwen3-4B-Instruct-Q4_K_M.gguf | Q4_K_M | huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF (archivo remoto `-2507`) | `3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597` | Apache-2.0 (verificar) |
| Qwen3-1.7B-Q4_K_M.gguf | Q4_K_M | huggingface.co/Qwen/Qwen3-1.7B-GGUF | `72c5c3cb38fa32d5256e2fe30d03e7a64c6c79e668ad84057e3bd66e250b24fb` | Apache-2.0 (verificar) |
| nomic-embed-text-v2-moe.Q4_K_M.gguf | Q4_K_M | huggingface.co/nomic-ai/nomic-embed-text-v2-moe-GGUF | `b5fb2811647b8ef461519a68a3bf67014a84a66a130c8a2af9413ff9f06d3f22` | Apache-2.0 (verificar) |

El manifiesto (`models.manifest.json`) y el descargador/verificador
(`fetch_models.py`) implementan tanto la descarga con reanudación + SHA256 como el
paquete offline copiable. `aria2c` (si está en `vendor/bin/`) acelera la descarga.
