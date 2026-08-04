"""
rag.py — retrieves relevant chunks from LanceDB and assembles LLM context.
Embeddings are produced by the embedded llama.cpp model (see inference.py).
Used by main.py. Not a standalone script.
"""

import asyncio
import json
import os
import re
import unicodedata
from pathlib import Path

import lancedb

import config_io
import inference
from paths import base_dir, config_path
from sanitize import build_data_block, clean_for_context

TABLE_NAME = "corpus"
TOP_K      = 5
META_FILE  = "embedding_meta.json"
DEFAULT_DB = "db"


def _municipio_slug(name: str) -> str:
    """"Municipalidad de Providencia" -> "providencia" (for the db_<slug> folder)."""
    ascii_name = unicodedata.normalize("NFKD", name).encode("ascii", "ignore").decode()
    ascii_name = ascii_name.lower()
    for token in ("municipalidad de ", "municipalidad ", "ilustre ", "i. "):
        ascii_name = ascii_name.replace(token, "")
    return re.sub(r"[^a-z0-9]+", "-", ascii_name).strip("-")


def _config_municipio() -> str | None:
    """The municipio driving DB selection: MUNIGPT_MUNICIPIO env (set by the
    MuniANCI host from the compiled institution) first, else config.json's
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


def vector_search(table, embedding: list[float]) -> list[dict]:
    """Returns top-k chunks by vector similarity."""
    return (
        table.search(embedding)
        .limit(TOP_K)
        .select(["text", "source", "chunk_index"])
        .to_list()
    )


def fts_search(table, query: str) -> list[dict]:
    """Returns top-k chunks by BM-25 full-text search."""
    try:
        return (
            table.search(query, query_type="fts")
            .limit(TOP_K)
            .select(["text", "source", "chunk_index"])
            .to_list()
        )
    except Exception:
        # FTS index may not exist if tantivy wasn't installed.
        return []


def buscar_en(tabla, consulta: str, embedding: list[float], limite: int) -> list[dict]:
    vectorial = (
        tabla.search(embedding)
        .limit(limite)
        .select(["text", "source", "chunk_index"])
        .to_list()
    )
    try:
        lexica = (
            tabla.search(consulta, query_type="fts")
            .limit(limite)
            .select(["text", "source", "chunk_index"])
            .to_list()
        )
    except Exception:
        lexica = []
    return deduplicate(vectorial + lexica)


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
    Main entry point. Embeds the query, runs hybrid search, deduplicates,
    and returns (context_string, raw_chunks).

    The embedding call is synchronous (llama.cpp), so it runs in a worker
    thread to avoid blocking the event loop.
    """
    table = get_table()

    embedding   = await asyncio.to_thread(inference.embed_query, query)
    vec_results = vector_search(table, embedding)
    fts_results = fts_search(table, query)

    # Merge: vector results first (higher semantic relevance), then FTS.
    combined = deduplicate(vec_results + fts_results)[:TOP_K]
    context  = build_context(combined)

    return context, combined
