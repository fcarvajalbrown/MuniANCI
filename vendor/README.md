# vendor/ — mirror local de dependencias

Espejo local de **toda** dependencia OSS que adopta MuniGPT, para no depender de que
el upstream siga existiendo. Motivo: el producto es offline/air-gapped y se despliega en
municipios; si crates.io, PyPI, HuggingFace o un repo de GitHub yanquea o borra una
versión, el build y la distribución deben seguir funcionando **sin red**.

Regla: cada vez que se adopta una biblioteca (ver `ROADMAP.md`, Apéndice A), se copia
aquí su artefacto pinneado y se preserva su archivo `LICENSE`. No se depende de descargas
en tiempo de build.

## Estructura

| Subcarpeta | Contenido | Mecanismo | ¿git? |
|---|---|---|---|
| `cargo/` | Crates Rust (pnet, netscan, cargo-*) | `cargo vendor` + `.cargo/config.toml` | versionable / git-lfs |
| `wheels/` | Wheels Python (Ragas, DeepEval, PyNaCl, pip-audit, hf_transfer, FlashRank) | `pip download`; instalar con `--no-index --find-links vendor/wheels` | versionable / git-lfs |
| `bin/` | Binarios externos (Nuclei, aria2c) | release pinneado por versión + SHA256 | gitignored (tamaño) |
| `nuclei-templates/` | Snapshot de plantillas Nuclei | snapshot pinneado | versionable / git-lfs |
| `models/` | Pesos (bge-reranker ONNX, bge-m3, nomic, Qwen GGUF) | archivo pinneado + SHA256 | gitignored; va en el paquete offline (D2) |
| `nvd/` | Snapshot NVD para enriquecimiento CVE offline | snapshot pinneado | gitignored; verificar redistribución |

Los artefactos grandes (`bin/`, `models/`, `nvd/`) son gitignored por tamaño y viajan en
el **paquete offline** de instalación (decisión D2), no por git. Los pequeños y de texto
(`cargo/`, `wheels/`, `nuclei-templates/`) pueden versionarse o ir en git-lfs.

## Procedencia

La procedencia de cada artefacto (nombre, versión exacta, URL de origen, SHA256 y
licencia) se registra en [`PROVENANCE.md`](PROVENANCE.md), con una tabla por tipo. Sin
registro no se ancla la dependencia. Aún nada adoptado: los items se agregan al
integrarse en su hito del roadmap.
