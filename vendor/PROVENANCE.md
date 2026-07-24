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
    -r assistant/backend/requirements-dev.txt \
    -r assistant/backend/requirements-eval.txt -d vendor/wheels
```

Nota (eval): `requirements-eval.txt` (ragas + langchain 0.3.x) es pesado y sólo se
necesita para `eval/eval_judge.py`. ragas 0.4.3 exige la familia langchain 0.3.x
(langchain 1.x rompe su import interno); los pines están en ese archivo.

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
URLs de origen se VERIFICARON el 2026-07-12 comparando el SHA256 local contra el `oid`
del puntero git-LFS de cada repo (coincidencia exacta), por lo que `source.confirmed`
= true en `assistant/backend/models.manifest.json`. La licencia se re-verifica contra
el `LICENSE` de cada repo al vendorizar.

| Modelo (archivo local) | Quant | Origen verificado (SHA256 coincide con el oid LFS) | SHA256 | Licencia |
|---|---|---|---|---|
| Qwen3-4B-Instruct-Q4_K_M.gguf | Q4_K_M | huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF (remoto `Qwen3-4B-Instruct-2507-Q4_K_M.gguf`) | `3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597` | Apache-2.0 (verificar) |
| Qwen3-1.7B-Q4_K_M.gguf | Q4_K_M | huggingface.co/bartowski/Qwen_Qwen3-1.7B-GGUF (remoto `Qwen_Qwen3-1.7B-Q4_K_M.gguf`) | `72c5c3cb38fa32d5256e2fe30d03e7a64c6c79e668ad84057e3bd66e250b24fb` | Apache-2.0 (verificar) |
| nomic-embed-text-v2-moe.Q4_K_M.gguf | Q4_K_M | huggingface.co/nomic-ai/nomic-embed-text-v2-moe-GGUF | `b5fb2811647b8ef461519a68a3bf67014a84a66a130c8a2af9413ff9f06d3f22` | Apache-2.0 (verificar) |

El manifiesto (`models.manifest.json`) y el descargador/verificador
(`fetch_models.py`) implementan tanto la descarga con reanudación + SHA256 como el
paquete offline copiable. `aria2c` (si está en `vendor/bin/`) acelera la descarga.

## Datos de vulnerabilidades (`nvd/`, gitignored)

Snapshot NVD para el enriquecimiento CVE offline del hito 0.5.0.

**Cuidado con el SHA256 publicado**: el archivo `CVE-all.meta` sigue el formato heredado
de NVD, donde `sha256` corresponde al **JSON descomprimido**, no al `.xz`. El campo
`xzSize` sí describe al comprimido. Verificar por streaming para no escribir 2,95 GB:

```bash
xz -dc vendor/nvd/CVE-all.json.xz | sha256sum   # debe coincidir con el campo sha256 del .meta
```

| Artefacto | Versión (release) | Origen | SHA256 (del JSON descomprimido) | Términos |
|---|---|---|---|---|
| `CVE-all.json.xz` | `v2026.07.24-000010` (369.933 CVE; 98.414.336 B comprimido, 2.949.731.920 B sin comprimir) | github.com/fkie-cad/nvd-json-data-feeds | `f97ebdcbb7edbcc2195213cf1a2d423f01e1974ea10597656886f6c6280fe196` (verificado 2026-07-24) | ToU de NVD + CVE Program (ver abajo) |

**Redistribución: permitida, con condiciones que el producto debe cumplir.** El repo de
fkie-cad no lleva licencia OSS propia: publica los términos de uso de ambas fuentes en su
carpeta `LICENSES/`.

- **NVD**: pide mostrar de forma prominente el aviso *"This product uses the NVD API but
  is not endorsed or certified by the NVD."*, y prohíbe atribuir a NVD contenido modificado.
- **CVE Program (MITRE)**: otorga licencia perpetua, mundial, no exclusiva, gratuita e
  irrevocable para reproducir y distribuir, **a condición de reproducir el aviso de
  copyright de MITRE y la licencia en cada copia**.

Ambos avisos se emiten en el PDF del informe; las constantes viven en
`core/src/cve/mod.rs` (`NVD_NOTICE`, `CVE_NOTICE`).

### Catálogo KEV de CISA

Vulnerabilidades con explotación observada. Es la única señal del producto basada en
explotación real y no en criticidad teórica, y por eso eleva la brecha a `Critical`.

| Artefacto | Versión | Origen | SHA256 | Términos |
|---|---|---|---|---|
| `known_exploited_vulnerabilities.json` | `catalogVersion 2026.07.24` (1.653 CVE, 332 usadas en ransomware; 1.562.293 B) | cisa.gov/sites/default/files/feeds/ | `036c579ee00120ad6b77a9e391ef96c96bd7ba4ab060214df0d79ddda2e64ce6` (verificado 2026-07-24) | CC0 / dominio público (obra del gobierno de EE.UU.); repo `cisagov/kev-data` |

**Campos descartados a propósito.** El snapshot embebido no conserva `requiredAction`
ni `dueDate`: esos plazos obligan a agencias federales de EE.UU. por la BOD 26-04 y no a
una municipalidad chilena. Reproducirlos en un informe ANCI sugeriría un plazo legal
inexistente.

**Actualización sin rebuild.** La app acepta el JSON tal cual lo publica CISA en
`MUNIANI_KEV_FILE` o junto al ejecutable, y ese archivo gana sobre el embebido. El
informe declara siempre qué catálogo se usó (`ScanResult::kev_provenance`).

### Artefactos derivados

El snapshot no viaja con el producto: se transforma en build time con
`cargo run --release -p nvd-index`.

| Derivado | Cómo se genera | Dónde vive | Tamaño |
|---|---|---|---|
| `cpe-catalog.tsv` | `nvd-index catalog` — los 121.452 pares vendor:product del snapshot | `vendor/nvd/` (gitignored) | ~4 MB |
| `cve_index.json.gz` | `nvd-index build` — filtrado a los productos curados | `core/src/data/` (**versionado**, embebido en el binario) | 1,9 MB, 23.882 CVE |
| `kev.json.gz` | `nvd-index kev` — KEV reducido a los campos que el informe justifica | `core/src/data/` (**versionado**, embebido en el binario) | 37 KB, 1.653 CVE |

`core/src/data/cpe_map.json` es la tabla curada nombre→CPE. Cada entrada guarda el conteo
de CVE observado al extraerla, como evidencia de que se leyó de los datos y no se escribió
de memoria. `nvd-index validate` re-verifica las 50 entradas contra el catálogo y falla si
alguna dejó de existir.
