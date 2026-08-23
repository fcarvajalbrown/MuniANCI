"""
rag.py — retrieves relevant chunks from LanceDB and assembles LLM context.
Embeddings are produced by the embedded llama.cpp model (see inference.py).
Used by main.py. Not a standalone script.
"""

import asyncio
import json
import os
import re
import sys
import unicodedata
from pathlib import Path

import lancedb

import citas
import config_io
import inference
from paths import base_dir, config_path
from sanitize import build_data_block, clean_for_context

TABLE_NAME = "corpus"
TOP_K      = 5
CANDIDATOS = 20
RRF_K      = 60
META_FILE  = "embedding_meta.json"
DEFAULT_DB = "db"

SCHEMA_VERSION = 2
_AVISADAS: set[str] = set()

ARTICULO_SLOTS         = 3
ARTICULO_MAX_FILAS     = 200
MAX_ARTICULOS_CONSULTA = 2

MARGEN_RELEVANCIA = 0.11

TOPE_PADRE      = 4000
PADRE_MAX_FILAS = 400
SOLAPE_BUSCADO  = 120


def _municipio_slug(name: str) -> str:
    """"Organismo de Ejemplo" -> "organismo-de-ejemplo" (for the db_<slug> folder)."""
    ascii_name = unicodedata.normalize("NFKD", name).encode("ascii", "ignore").decode()
    ascii_name = ascii_name.lower()
    for token in ("municipalidad de ", "municipalidad ", "ilustre ", "i. "):
        ascii_name = ascii_name.replace(token, "")
    return re.sub(r"[^a-z0-9]+", "-", ascii_name).strip("-")


def _config_municipio() -> str | None:
    """The municipio driving DB selection: MUNIGPT_MUNICIPIO env (set by the
    MuniGPT host from the compiled institution) first, else config.json's
    municipio (one level above the assets — see paths.py)."""
    env = os.environ.get("MUNIGPT_MUNICIPIO")
    if env and env.strip():
        return env.strip()
    name = config_io.leer_config(config_path()).get("municipio")
    return name if isinstance(name, str) and name.strip() else None


def db_dir() -> Path:
    """
    Resolve which municipality's DB to open, enabling swappable per-comuna DBs:
      1. MUNIGPT_DB_DIR env var (explicit; set by the desktop host per client/demo)
      2. db_<slug-of-config.municipio> if that folder exists
      3. "db" (the national-law template, shared baseline)

    Cases 2 and 3 resolve against the assets directory, not the current working
    directory, so the packaged sidecar finds them wherever it was launched from.
    """
    env = os.environ.get("MUNIGPT_DB_DIR")
    if env:
        return Path(env)
    muni = _config_municipio()
    if muni:
        candidate = base_dir() / f"{DEFAULT_DB}_{_municipio_slug(muni)}"
        if candidate.exists():
            return candidate
    return base_dir() / DEFAULT_DB


def _assert_embedding_meta(db_path: Path):
    """
    Fails loudly if the shipped/prebuilt DB was embedded with a different model
    than the one running now — otherwise retrieval silently returns garbage.
    """
    meta_path = db_path / META_FILE
    if not meta_path.exists():
        return  # DB predates metadata; ingest.py writes it going forward.
    meta = json.loads(meta_path.read_text(encoding="utf-8"))
    expected = inference.embedding_model_name()
    if meta.get("embedding_model") != expected:
        raise RuntimeError(
            f"DB was built with embedding model '{meta.get('embedding_model')}' "
            f"but the live model is '{expected}'. Re-run: python ingest.py --reset"
        )
    _avisar_esquema(db_path, meta.get("schema_version", 1))


def _avisar_esquema(db_path: Path, version: int):
    if version >= SCHEMA_VERSION:
        return
    clave = str(db_path)
    if clave in _AVISADAS:
        return
    _AVISADAS.add(clave)
    print(
        f"[aviso] La base {db_path} es de esquema v{version} y el codigo espera "
        f"v{SCHEMA_VERSION}: responde sin metadatos de articulo ni encabezado por "
        f"fragmento. Para reconstruirla: python ingest.py --reset --db-dir {db_path}",
        file=sys.stderr,
    )


def get_table():
    """Opens the LanceDB corpus table for the active DB. Raises if not found."""
    db_path = db_dir()
    if not db_path.exists():
        raise RuntimeError(f"DB not found at {db_path}. Run ingest.py first.")
    _assert_embedding_meta(db_path)
    db = lancedb.connect(str(db_path))
    if TABLE_NAME not in db.table_names():
        raise RuntimeError(f"Table '{TABLE_NAME}' not found. Run ingest.py first.")
    return db.open_table(TABLE_NAME)


COLUMNS     = ["text", "source", "chunk_index"]
ESTRUCTURA  = ["norma", "tipo_parte", "numero_articulo", "id_parte",
               "fecha_version", "ruta", "derogado", "transitorio"]


def columnas(table) -> list[str]:
    try:
        presentes = set(table.schema.names)
    except Exception:
        return list(COLUMNS)
    return COLUMNS + [c for c in ESTRUCTURA if c in presentes]


def vector_search(table, embedding: list[float]) -> list[dict]:
    """Returns the candidate chunks by vector similarity. `_distance` is selected
    explicitly: LanceDB still auto-projects it but warns that it will stop, and the
    eval harness reads it to calibrate the abstention threshold."""
    return (
        table.search(embedding)
        .limit(CANDIDATOS)
        .select(columnas(table) + ["_distance"])
        .to_list()
    )


def fts_search(table, query: str) -> list[dict]:
    """Returns the candidate chunks by BM-25 full-text search."""
    try:
        return (
            table.search(query, query_type="fts")
            .limit(CANDIDATOS)
            .select(columnas(table) + ["_score"])
            .to_list()
        )
    except Exception:
        # FTS index may not exist if tantivy wasn't installed.
        return []


def rrf(listas: list[list[dict]], k: int = RRF_K, limite: int = TOP_K) -> list[dict]:
    """Reciprocal Rank Fusion: score(d) = suma de 1/(k + rango) sobre cada lista en
    que aparece d. Sustituye al corte `vectorial + lexica` truncado a TOP_K, que
    descartaba el 100% del lado BM-25 siempre que el vectorial devolviera TOP_K filas
    unicas. Los empates conservan el orden de llegada, de modo que la lista vectorial
    sigue mandando cuando dos fragmentos empatan en el mismo rango."""
    puntajes: dict[tuple, float] = {}
    elegidos: dict[tuple, dict] = {}
    for lista in listas:
        for posicion, fragmento in enumerate(lista, 1):
            clave = (fragmento.get("source"), fragmento.get("chunk_index"))
            puntajes[clave] = puntajes.get(clave, 0.0) + 1.0 / (k + posicion)
            elegidos.setdefault(clave, fragmento)
    orden = sorted(elegidos, key=lambda clave: -puntajes[clave])
    return [{**elegidos[clave], "_rrf": round(puntajes[clave], 6)}
            for clave in orden[:limite]]


def _numero_normalizado(valor: str) -> str:
    return (valor or "").strip().rstrip("°º").strip()


def _comillas(valor: str) -> str:
    escapado = valor.replace("'", "''")
    return f"'{escapado}'"


def ruta_articulo(tabla, consulta: str, candidatos: list[dict],
                  limite: int = ARTICULO_SLOTS) -> list[dict]:
    numeros = sorted(citas.articulos(consulta))
    if not numeros or len(numeros) > MAX_ARTICULOS_CONSULTA:
        return []

    if "numero_articulo" not in set(getattr(tabla, "schema").names):
        return []

    orden_fuente: dict[str, int] = {}
    for posicion, c in enumerate(candidatos):
        fuente = c.get("source")
        if fuente and fuente not in orden_fuente:
            orden_fuente[fuente] = posicion
    if not orden_fuente:
        return []
    fuentes = list(orden_fuente)

    variantes = [v for n in numeros for v in (str(n), f"{n}°", f"{n}º")]
    filtro = (f"source IN ({', '.join(_comillas(f) for f in fuentes)}) "
              f"AND numero_articulo IN ({', '.join(_comillas(v) for v in variantes)})")

    try:
        filas = (
            tabla.search()
            .where(filtro)
            .limit(ARTICULO_MAX_FILAS)
            .select(columnas(tabla))
            .to_list()
        )
    except Exception:
        return []

    filas.sort(key=lambda f: (orden_fuente.get(f.get("source", ""), len(orden_fuente)),
                              f.get("chunk_index", 0)))
    return [{**f, "_articulo": _numero_normalizado(f.get("numero_articulo", ""))}
            for f in filas[:limite]]


def podar_por_relevancia(fragmentos: list[dict],
                         margen: float = MARGEN_RELEVANCIA) -> list[dict]:
    distancias = [f["_distance"] for f in fragmentos if f.get("_distance") is not None]
    if not distancias:
        return fragmentos

    corte = min(distancias) + margen
    respaldadas = {f.get("source") for f in fragmentos if f.get("_distance") is not None}

    salida = []
    for f in fragmentos:
        distancia = f.get("_distance")
        if distancia is not None:
            if distancia <= corte:
                salida.append(f)
        elif f.get("_articulo") or f.get("source") in respaldadas:
            salida.append(f)
    return salida


def _clave_padre(fragmento: dict) -> tuple[str, str] | None:
    if (fragmento.get("tipo_parte") or "") != "Artículo":
        return None
    id_parte = (fragmento.get("id_parte") or "").strip()
    fuente = fragmento.get("source") or ""
    if not id_parte or not fuente:
        return None
    return (fuente, id_parte)


def _encabezado_comun(filas: list[dict]) -> str:
    primeras = {f.get("text", "").split("\n", 1)[0] for f in filas}
    if len(primeras) != 1:
        return ""
    linea = primeras.pop()
    return linea if "Artículo" in linea else ""


def _sin_encabezado(texto: str, encabezado: str) -> str:
    if encabezado and texto.startswith(encabezado):
        return texto[len(encabezado):].lstrip("\n")
    return texto


def _unir_solapado(acumulado: str, siguiente: str,
                   tope: int = SOLAPE_BUSCADO) -> str:
    if not acumulado:
        return siguiente
    if not siguiente:
        return acumulado
    maximo = min(len(acumulado), len(siguiente), tope)
    for n in range(maximo, 0, -1):
        if acumulado.endswith(siguiente[:n]):
            return acumulado + siguiente[n:]
    return f"{acumulado} {siguiente}"


def _ventana(cuerpos: list[str], centro: int, tope: int) -> str:
    texto = cuerpos[centro][:tope]
    izquierda, derecha = centro - 1, centro + 1
    while izquierda >= 0 or derecha < len(cuerpos):
        crecio = False
        if derecha < len(cuerpos):
            tentativa = _unir_solapado(texto, cuerpos[derecha])
            if len(tentativa) <= tope:
                texto, derecha, crecio = tentativa, derecha + 1, True
        if izquierda >= 0:
            tentativa = _unir_solapado(cuerpos[izquierda], texto)
            if len(tentativa) <= tope:
                texto, izquierda, crecio = tentativa, izquierda - 1, True
        if not crecio:
            break
    return texto


def filas_de_padres(tabla, claves: list[tuple[str, str]]) -> dict:
    if not claves:
        return {}
    try:
        nombres = set(tabla.schema.names)
    except Exception:
        return {}
    if "id_parte" not in nombres:
        return {}

    fuentes = sorted({c[0] for c in claves})
    partes = sorted({c[1] for c in claves})
    filtro = (f"source IN ({', '.join(_comillas(f) for f in fuentes)}) "
              f"AND id_parte IN ({', '.join(_comillas(p) for p in partes)})")

    try:
        filas = (
            tabla.search()
            .where(filtro)
            .limit(PADRE_MAX_FILAS * len(claves))
            .select(columnas(tabla))
            .to_list()
        )
    except Exception:
        return {}

    buscadas = set(claves)
    agrupadas: dict[tuple[str, str], list[dict]] = {}
    for f in filas:
        clave = (f.get("source") or "", (f.get("id_parte") or "").strip())
        if clave in buscadas:
            agrupadas.setdefault(clave, []).append(f)
    for grupo in agrupadas.values():
        grupo.sort(key=lambda f: f.get("chunk_index", 0))
    return agrupadas


def _armar_padre(fragmento: dict, filas: list[dict], tope: int) -> dict:
    encabezado = _encabezado_comun(filas)
    cuerpos = [_sin_encabezado(f.get("text", ""), encabezado) for f in filas]

    completo = ""
    for cuerpo in cuerpos:
        completo = _unir_solapado(completo, cuerpo)

    prefijo = f"{encabezado}\n" if encabezado else ""
    disponible = max(tope - len(prefijo), 0)

    if len(completo) <= disponible:
        texto, recortado = completo, False
    else:
        centro = next((i for i, f in enumerate(filas)
                       if f.get("chunk_index") == fragmento.get("chunk_index")), 0)
        texto, recortado = _ventana(cuerpos, centro, disponible), True

    return {**fragmento,
            "text": f"{prefijo}{texto}",
            "_padre": True,
            "_padre_trozos": len(filas),
            "_padre_recortado": recortado}


def expandir_a_padres(tabla, fragmentos: list[dict],
                      tope: int = TOPE_PADRE) -> list[dict]:
    claves: list[tuple[str, str]] = []
    for f in fragmentos:
        clave = _clave_padre(f)
        if clave and clave not in claves:
            claves.append(clave)

    agrupadas = filas_de_padres(tabla, claves)

    salida: list[dict] = []
    resueltas: set[tuple[str, str]] = set()
    for f in fragmentos:
        clave = _clave_padre(f)
        if clave is None:
            salida.append(f)
            continue
        if clave in resueltas:
            continue
        resueltas.add(clave)
        filas = agrupadas.get(clave)
        salida.append(_armar_padre(f, filas, tope) if filas else f)
    return salida


def buscar_en(tabla, consulta: str, embedding: list[float],
              limite: int = TOP_K) -> list[dict]:
    vectoriales = vector_search(tabla, embedding)
    lexicos = fts_search(tabla, consulta)

    combinados = rrf([vectoriales, lexicos], limite=limite)

    directos = ruta_articulo(tabla, consulta, vectoriales + lexicos)
    if directos:
        combinados = deduplicate(directos + combinados)[:limite]

    return expandir_a_padres(tabla, podar_por_relevancia(combinados))


def fusionar(por_corpus: list[tuple[str, list[dict]]], limite: int) -> list[dict]:
    listas = []
    for corpus_id, fragmentos in por_corpus:
        marcados = []
        vistos = set()
        for f in fragmentos:
            clave = (corpus_id, f.get("source"), f.get("chunk_index"))
            if clave in vistos:
                continue
            vistos.add(clave)
            marcados.append({**f, "corpus": corpus_id})
        listas.append(marcados)

    salida: list[dict] = []
    vistos_global: set[tuple] = set()
    for posicion in range(max((len(l) for l in listas), default=0)):
        for lista in listas:
            if posicion >= len(lista):
                continue
            f = lista[posicion]
            clave = (f["corpus"], f.get("source"), f.get("chunk_index"))
            if clave in vistos_global:
                continue
            vistos_global.add(clave)
            salida.append(f)
            if len(salida) >= limite:
                return salida
    return salida


def deduplicate(chunks: list[dict]) -> list[dict]:
    """Removes duplicate chunks by (source, chunk_index), preserving order."""
    seen = set()
    unique = []
    for c in chunks:
        key = (c.get("source"), c.get("chunk_index"))
        if key not in seen:
            seen.add(key)
            unique.append(c)
    return unique


def build_context(chunks: list[dict]) -> str:
    """Formats retrieved chunks into a spotlighted data block for the LLM prompt.

    Prompt-injection defense (OWASP LLM 2025): the retrieved corpus is untrusted
    DATA. Each chunk (and its source label) is stripped of hidden/bidi characters and
    of the spotlight delimiters themselves — so a chunk can't close the block and
    escape — then the whole thing is wrapped between the SPOTLIGHT delimiters. The
    system prompt (main.py) tells the model never to follow instructions found
    inside this block. Complements index-time sanitization in ingest.py.
    """
    if not chunks:
        return ""
    parts = []
    for c in chunks:
        source = clean_for_context(c.get("source", "desconocido"))
        corpus_id = c.get("corpus")
        etiqueta = f"{source} - corpus {clean_for_context(str(corpus_id))}" if corpus_id else source
        text   = clean_for_context(c.get("text", ""))
        parts.append(f"[Fuente: {etiqueta}]\n{text}")
    return build_data_block("\n\n---\n\n".join(parts))


async def retrieve(query: str) -> tuple[str, list[dict]]:
    """
    Main entry point. Embeds the query, runs hybrid search over CANDIDATOS per side,
    fuses both rankings with RRF, and returns (context_string, raw_chunks).

    The embedding call is synchronous (llama.cpp), so it runs in a worker
    thread to avoid blocking the event loop.
    """
    table = get_table()

    embedding = await asyncio.to_thread(inference.embed_query, query)
    combined  = buscar_en(table, query, embedding, TOP_K)

    context = build_context(combined)

    return context, combined
