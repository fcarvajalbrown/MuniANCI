"""
ingest.py
=========
Chunks all PDFs and TXTs in the corpus directory into LanceDB.
Scans corpus/ recursively - no tier subdirectories required.

Embeddings are produced in-process by the embedded llama.cpp model
(see inference.py) — no Ollama, no network.

Usage:
    python ingest.py                  # uses ./corpus and ./db
    python ingest.py --reset          # wipe and rebuild
    python ingest.py --corpus-dir C:/path/to/corpus

The core is exposed as run_ingest(corpus_dir, db_dir, reset) so both this CLI
and the FastAPI /ingest endpoint share one implementation.
"""

import argparse
import json
import re
import sys
from pathlib import Path

import lancedb
import pyarrow as pa

import inference
from sanitize import sanitize_for_index

try:
    from pypdf import PdfReader
    HAS_PYPDF = True
except ImportError:
    HAS_PYPDF = False
    print("[warn] pypdf not installed. pip install pypdf")

# ── Config ──────────────────────────────────────────────────────────────────────

CHUNK_SIZE    = 500
CHUNK_OVERLAP = 50
TABLE_NAME    = "corpus"
META_FILE     = "embedding_meta.json"
EMBED_BATCH   = 16
FTS_LANGUAGE  = "Spanish"

ESTRUCTURA_SUFFIX = ".estructura.json"
SCHEMA_VERSION    = 2

CAMPOS_ESTRUCTURA = {
    "norma":           "",
    "tipo_parte":      "",
    "numero_articulo": "",
    "id_parte":        "",
    "fecha_version":   "",
    "ruta":            "",
    "derogado":        False,
    "transitorio":     False,
}


# ── Text extraction ──────────────────────────────────────────────────────────────

def extract_text_from_pdf(path: Path) -> str:
    """Extracts plain text from a PDF using pypdf. Warns on likely scanned files."""
    if not HAS_PYPDF:
        print(f"  [skip] {path.name} - pypdf not installed")
        return ""

    reader = PdfReader(str(path))  # type: ignore
    pages_text = []
    empty_pages = 0

    for page in reader.pages:
        text = (page.extract_text() or "").strip()
        if text:
            pages_text.append(text)
        else:
            empty_pages += 1

    total = len(reader.pages)
    if total > 0 and empty_pages > total / 2:
        print(f"  [warn] {path.name} - {empty_pages}/{total} empty pages, may be scanned")

    return "\n".join(pages_text)


def extract_text_from_txt(path: Path) -> str:
    """Reads a plain text file, trying utf-8 then latin-1."""
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return path.read_text(encoding="latin-1")


def extract_text(path: Path) -> str:
    """Dispatches to PDF or TXT extractor based on file extension."""
    suffix = path.suffix.lower()
    if suffix == ".pdf":
        return extract_text_from_pdf(path)
    elif suffix == ".txt":
        return extract_text_from_txt(path)
    else:
        print(f"  [skip] {path.name} - unsupported type")
        return ""


# ── Chunking ─────────────────────────────────────────────────────────────────────

def chunk_text(text: str) -> list[dict]:
    """
    Splits text into overlapping chunks of ~CHUNK_SIZE characters.
    Tries to split at sentence boundaries to avoid cutting mid-sentence.
    Returns list of dicts with: text, chunk_index, char_offset.
    """
    text = re.sub(r"\n{3,}", "\n\n", text)
    text = re.sub(r"[ \t]+", " ", text).strip()

    if not text:
        return []

    chunks = []
    start = 0
    chunk_index = 0

    while start < len(text):
        end = min(start + CHUNK_SIZE, len(text))

        if end < len(text):
            search_start = max(start, end - 100)
            boundary = -1
            for sep in [". ", ".\n", "\n\n", "\n", " "]:
                pos = text.rfind(sep, search_start, end)
                if pos != -1:
                    boundary = pos + len(sep)
                    break
            if boundary > start:
                end = boundary

        chunk = text[start:end].strip()
        if chunk:
            chunks.append({
                "text":        chunk,
                "chunk_index": chunk_index,
                "char_offset": start,
            })
            chunk_index += 1

        next_start = end - CHUNK_OVERLAP
        if next_start <= start:
            next_start = end
        start = next_start
        if start >= len(text):
            break

    return chunks


def encabezado_articulo(parte: dict, norma: str) -> str:
    if parte.get("tipo_parte") != "Artículo":
        return ""
    numero = (parte.get("numero_articulo") or "").strip()
    if not numero:
        return ""
    etiqueta = f"Artículo {numero}"
    if parte.get("transitorio"):
        etiqueta += " transitorio"
    return f"{norma}, {etiqueta}" if norma else etiqueta


def chunk_estructura(estructura: dict) -> list[dict]:
    norma = (estructura.get("norma") or {}).get("display", "")
    salida: list[dict] = []
    indice = 0

    for parte in estructura.get("partes", []):
        texto = sanitize_for_index(parte.get("texto", ""))
        if not texto.strip():
            continue
        encabezado = encabezado_articulo(parte, norma)
        for trozo in chunk_text(texto):
            cuerpo = f"{encabezado}\n{trozo['text']}" if encabezado else trozo["text"]
            salida.append({
                "text":            cuerpo,
                "chunk_index":     indice,
                "char_offset":     trozo["char_offset"],
                "norma":           norma,
                "tipo_parte":      parte.get("tipo_parte", ""),
                "numero_articulo": parte.get("numero_articulo", ""),
                "id_parte":        parte.get("id_parte", ""),
                "fecha_version":   parte.get("fecha_version", ""),
                "ruta":            parte.get("ruta", ""),
                "derogado":        bool(parte.get("derogado", False)),
                "transitorio":     bool(parte.get("transitorio", False)),
            })
            indice += 1

    return salida


# ── Schema ───────────────────────────────────────────────────────────────────────

def get_schema(embedding_dim: int) -> pa.Schema:
    """PyArrow schema for the LanceDB corpus table."""
    return pa.schema([
        pa.field("text",            pa.string()),
        pa.field("embedding",       pa.list_(pa.float32(), embedding_dim)),
        pa.field("source",          pa.string()),
        pa.field("chunk_index",     pa.int32()),
        pa.field("char_offset",     pa.int64()),
        pa.field("norma",           pa.string()),
        pa.field("tipo_parte",      pa.string()),
        pa.field("numero_articulo", pa.string()),
        pa.field("id_parte",        pa.string()),
        pa.field("fecha_version",   pa.string()),
        pa.field("ruta",            pa.string()),
        pa.field("derogado",        pa.bool_()),
        pa.field("transitorio",     pa.bool_()),
    ])


# ── Main ─────────────────────────────────────────────────────────────────────────

def build_fts_index(table, stem: bool = True):
    """
    BM-25 index tuned for Spanish legal text. LanceDB's defaults are English, so a
    DB built without these arguments applies English stemming to "artículo" and
    "reportar" and removes English stop words while "de", "la" and "que" stay
    indexed and compete for score. with_position enables phrase queries, which a
    bare term search cannot express for an identifier like "artículo 9".
    """
    table.create_fts_index(
        "text",
        replace=True,
        language=FTS_LANGUAGE,
        stem=stem,
        remove_stop_words=True,
        ascii_folding=True,
        with_position=True,
    )


def reindex_fts(db_dir: Path, stem: bool = True) -> dict:
    """
    Rebuilds only the BM-25 index of an existing DB. Separate from run_ingest
    because the embeddings are the slow part and they do not change: this makes the
    Spanish index reachable for the DBs already built and shipped.
    """
    db_dir = Path(db_dir)
    if not db_dir.exists():
        raise FileNotFoundError(f"DB not found at {db_dir}")
    db = lancedb.connect(str(db_dir))
    if TABLE_NAME not in db.table_names():
        raise FileNotFoundError(f"Table '{TABLE_NAME}' not found in {db_dir}")
    table = db.open_table(TABLE_NAME)
    build_fts_index(table, stem=stem)
    return {"db": str(db_dir), "rows": table.count_rows(),
            "language": FTS_LANGUAGE, "stem": stem}


def run_ingest(corpus_dir: Path, db_dir: Path, reset: bool, stem: bool = True) -> dict:
    """
    Scans corpus_dir recursively for all PDFs and TXTs, chunks them, embeds each
    chunk via the local llama.cpp embedding model, and loads into LanceDB.
    Returns {"docs": n, "chunks": n, "embedding_model": name}.
    """
    corpus_dir = Path(corpus_dir)
    db_dir     = Path(db_dir)

    if not corpus_dir.exists():
        raise FileNotFoundError(f"Corpus directory not found: {corpus_dir}")

    documents = sorted(
        f for ext in ("*.pdf", "*.txt")
        for f in corpus_dir.rglob(ext)
    )
    if not documents:
        raise FileNotFoundError(f"No PDF or TXT files found in {corpus_dir}")

    print(f"Found {len(documents)} documents in {corpus_dir}/")

    # Embedding dimension from the live model (drives the table schema).
    embedding_dim = inference.embedding_dim()
    embed_model   = inference.embedding_model_name()
    print(f"  Embedding model: {embed_model}  (dim {embedding_dim})")

    db_dir.mkdir(parents=True, exist_ok=True)
    db = lancedb.connect(str(db_dir))

    if reset and TABLE_NAME in db.table_names():
        print(f"Dropping table '{TABLE_NAME}'...")
        db.drop_table(TABLE_NAME)

    schema = get_schema(embedding_dim)

    if TABLE_NAME not in db.table_names():
        print(f"Creating table '{TABLE_NAME}'...")
        table = db.create_table(TABLE_NAME, schema=schema)
    else:
        print(f"Appending to table '{TABLE_NAME}'...")
        table = db.open_table(TABLE_NAME)
        presentes = set(table.schema.names)
        faltantes = [f.name for f in schema if f.name not in presentes]
        if faltantes:
            raise RuntimeError(
                f"Table '{TABLE_NAME}' in {db_dir} predates schema v{SCHEMA_VERSION} "
                f"(missing: {', '.join(faltantes)}). Appending would mix two chunk "
                f"layouts in one index. Rebuild it: "
                f"python ingest.py --reset --db-dir {db_dir}"
            )

    total_chunks = 0

    for doc_path in documents:
        print(f"\n  {doc_path.name}")

        est_path = doc_path.with_name(doc_path.stem + ESTRUCTURA_SUFFIX)
        text = ""

        if doc_path.suffix.lower() == ".txt" and est_path.exists():
            estructura = json.loads(est_path.read_text(encoding="utf-8"))
            partes = estructura.get("partes", [])
            articulos = sum(1 for p in partes if p.get("tipo_parte") == "Artículo")
            print(f"    {len(partes)} partes, {articulos} artículos (estructura BCN)")
            chunks = chunk_estructura(estructura)
        else:
            text = extract_text(doc_path)
            # Index-time prompt-injection sanitization (OWASP LLM 2025, layer 1): strip
            # hidden/bidi characters and neutralize instruction-override / role markers
            # before chunking, so a crafted Tier-3 PDF or ordenanza can't smuggle
            # instructions into the retrieved context. Spotlighting (rag.build_context)
            # is layer 2.
            text = sanitize_for_index(text)
            if not text or len(text) < 100:
                print(f"    [skip] No text extracted")
                continue

            print(f"    {len(text):,} chars (sin estructura)")
            chunks = [{**CAMPOS_ESTRUCTURA, **c} for c in chunk_text(text)]

        if not chunks:
            print(f"    [skip] No chunks produced")
            continue

        print(f"    {len(chunks)} chunks - embedding...", flush=True)

        doc_chunks = 0
        for i in range(0, len(chunks), EMBED_BATCH):
            batch = chunks[i:i + EMBED_BATCH]
            try:
                embeddings = inference.embed_documents([c["text"] for c in batch])
            except Exception as e:
                print(f"    [warn] Embedding failed: {e}")
                continue

            table.add([
                {
                    "text":        c["text"],
                    "embedding":   emb,
                    "source":      doc_path.name,
                    "chunk_index": c["chunk_index"],
                    "char_offset": c["char_offset"],
                    **{campo: c[campo] for campo in CAMPOS_ESTRUCTURA},
                }
                for c, emb in zip(batch, embeddings)
            ])
            doc_chunks += len(batch)

        print(f"    {doc_chunks} chunks inserted")
        total_chunks += doc_chunks
        del chunks
        del text

    print(f"\nBuilding FTS index ({FTS_LANGUAGE}, stem={stem})...")
    try:
        build_fts_index(table, stem=stem)
        print("  FTS OK")
    except Exception as e:
        print(f"  [warn] FTS failed: {e} - pip install tantivy")

    # Embedding-model metadata sidecar — rag.get_table() asserts against this so
    # a stale/mismatched DB fails loudly instead of returning garbage retrieval.
    (db_dir / META_FILE).write_text(
        json.dumps({"embedding_model": embed_model, "dim": embedding_dim,
                    "schema_version": SCHEMA_VERSION}),
        encoding="utf-8",
    )

    print(f"\n{'='*50}")
    print(f"Done. {len(documents)} docs, {total_chunks} chunks.")
    print(f"DB: {db_dir}/")

    return {"docs": len(documents), "chunks": total_chunks, "embedding_model": embed_model}


def main():
    parser = argparse.ArgumentParser(description="Ingest MuniGPT corpus into LanceDB.")
    parser.add_argument("--corpus-dir", type=Path, default=Path("corpus"))
    parser.add_argument("--db-dir",     type=Path, default=Path("db"))
    parser.add_argument("--reset",      action="store_true")
    parser.add_argument("--reindex-fts", action="store_true",
                        help="Rebuild only the BM-25 index of --db-dir (no embedding).")
    parser.add_argument("--no-stem", action="store_true",
                        help="Build the BM-25 index without Spanish stemming.")
    args = parser.parse_args()

    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass

    stem = not args.no_stem

    if args.reindex_fts:
        print("MuniGPT -- ingest.py --reindex-fts")
        try:
            info = reindex_fts(args.db_dir, stem=stem)
        except FileNotFoundError as e:
            print(f"[error] {e}")
            sys.exit(1)
        print(f"  {info['db']}: {info['rows']} rows, "
              f"language={info['language']}, stem={info['stem']}")
        return

    print("MuniGPT -- ingest.py")
    print(f"Corpus: {args.corpus_dir}/")
    print(f"DB:     {args.db_dir}/")
    print(f"Reset:  {args.reset}\n")

    try:
        run_ingest(corpus_dir=args.corpus_dir, db_dir=args.db_dir,
                   reset=args.reset, stem=stem)
    except FileNotFoundError as e:
        print(f"[error] {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
