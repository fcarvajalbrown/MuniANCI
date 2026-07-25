# -*- mode: python ; coding: utf-8 -*-
r"""
Especificación de PyInstaller para el sidecar del Asistente (decisión D1).

    ..\.venv\Scripts\python.exe -m PyInstaller munigpt-backend.spec

`--onedir` y **sin UPX**, y las dos cosas son deliberadas. `--onefile` se
autoextrae a un directorio temporal antes de ejecutar, que es el patrón de un packer y
el disparador de heurística de antivirus más directo; UPX agrava lo mismo sobre
ejecutables de PyInstaller. En un PC municipal con antivirus corporativo eso rompe la
instalación, no la hace lenta. Lo que `--onedir` **no** compra es inmunidad: el
bootloader de PyInstaller se marca igual en algunos casos, y la mitigación con
evidencia es la firma de código, que este proyecto tiene asignada al Horizonte.

Los activos grandes NO entran acá: `bin/`, `db/`, `db_providencia/`, `corpus/`,
`corpus_muni/`, el manifiesto y el GGUF de embeddings los copia
`tools/empaquetar-asistente.ps1` junto al ejecutable, porque hacerlos pasar por el
análisis de PyInstaller es lento y no aporta nada.

`config.json` tampoco viaja acá: el backend lo lee un nivel **sobre** los activos
(`paths.config_path()`), o sea junto a `muniani-gui.exe`, así que lo embarca el
bundler de Tauri y no PyInstaller. Meterlo dentro de esta carpeta lo dejaría en el
lugar donde nadie lo busca.
"""

from PyInstaller.utils.hooks import collect_all

datas = []
binaries = []
hiddenimports = []

# lancedb y pyarrow traen extensiones nativas y datos propios; tantivy es el índice
# BM25; ddgs es la única ruta de red del producto. Se recolectan enteros en vez de
# adivinar nombres de módulos: si el análisis se equivoca acá, el sidecar arranca y
# falla recién al primer /chat, que es el peor momento para descubrirlo.
for paquete in ("lancedb", "pyarrow", "tantivy", "ddgs", "pypdf"):
    d, b, h = collect_all(paquete)
    datas += d
    binaries += b
    hiddenimports += h

# uvicorn carga sus protocolos y su gestor de ciclo de vida por nombre en tiempo de
# ejecución, así que el análisis estático no los ve. Sin esto el binario compila y
# muere al arrancar.
hiddenimports += [
    "uvicorn",
    "uvicorn.logging",
    "uvicorn.loops.auto",
    "uvicorn.loops.asyncio",
    "uvicorn.protocols.http.auto",
    "uvicorn.protocols.http.h11_impl",
    "uvicorn.protocols.http.httptools_impl",
    "uvicorn.protocols.websockets.auto",
    "uvicorn.lifespan.on",
    "uvicorn.lifespan.off",
]

# Módulos propios que solo se alcanzan por import dinámico o desde main.py.
hiddenimports += ["main", "rag", "ingest", "inference", "sanitize", "license",
                  "watchdog", "paths", "fetch_models", "corpus_fetcher"]

a = Analysis(
    ["run_server.py"],
    pathex=["."],
    binaries=binaries,
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    # Lo que no viaja al equipo municipal. Dos grupos:
    #
    # 1. El harness de evaluación: es una actividad manual de desarrollo (ver
    #    assistant/CLAUDE.md), no algo que corra en la municipalidad.
    # 2. Las dependencias opcionales del registro de funciones de embedding de
    #    lancedb. Este producto embebe con llama-server, así que ese registro nunca
    #    se usa: `rag.py` solo llama `table.search(...).to_list()` y `ingest.py`
    #    arma su esquema con pyarrow. Arrastraban scipy, pandas, PIL y el cliente de
    #    HuggingFace, unos 105 MB en el bundle. Si alguna vez se usa un embedding
    #    en proceso, hay que sacar el paquete de esta lista y volver a medir.
    # 3. pymupdf (49 MB), que solo usa `convert.py`: una utilidad manual de
    #    desarrollo, fuera del pipeline (ver assistant/CLAUDE.md). La ingesta del
    #    producto lee PDF con pypdf, que sí viaja.
    excludes=["pytest", "ragas", "deepeval", "langchain", "matplotlib", "tkinter",
              "scipy", "pandas", "PIL", "huggingface_hub", "hf_xet", "pymupdf",
              "fitz"],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="munigpt-backend",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,          # ver el encabezado: UPX empeora los falsos positivos
    console=True,       # el host lo lanza con CREATE_NO_WINDOW, así que no se ve
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=False,
    upx_exclude=[],
    name="munigpt-backend",
)
