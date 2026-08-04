import os
from pathlib import Path
from typing import NamedTuple

import lancedb

import rag


class Corpus(NamedTuple):
    id: str
    etiqueta: str
    ruta: Path


ETIQUETAS = {
    "institucional": "la normativa propia del organismo",
    "nacional": "la legislacion nacional",
}

_tablas: dict[str, object] = {}


def limpiar_cache() -> None:
    _tablas.clear()


def disponibles() -> list[Corpus]:
    entradas: list[Corpus] = []
    vistas: set[Path] = set()

    forzada = os.environ.get("MUNIGPT_DB_DIR")
    if forzada:
        ruta = Path(forzada).resolve()
        return [Corpus("nacional", ETIQUETAS["nacional"], ruta)]

    municipio = rag._config_municipio()
    if municipio:
        ruta = (rag.base_dir() / f"{rag.DEFAULT_DB}_{rag._municipio_slug(municipio)}").resolve()
        if ruta.exists():
            entradas.append(Corpus("institucional", ETIQUETAS["institucional"], ruta))
            vistas.add(ruta)

    nacional = (rag.base_dir() / rag.DEFAULT_DB).resolve()
    if nacional.exists() and nacional not in vistas:
        entradas.append(Corpus("nacional", ETIQUETAS["nacional"], nacional))

    return entradas


def ids() -> set[str]:
    return {c.id for c in disponibles()}


def abrir(c: Corpus):
    clave = str(c.ruta)
    if clave not in _tablas:
        rag._assert_embedding_meta(c.ruta)
        db = lancedb.connect(clave)
        if rag.TABLE_NAME not in db.table_names():
            raise RuntimeError(f"Tabla '{rag.TABLE_NAME}' no encontrada en {c.ruta}")
        _tablas[clave] = db.open_table(rag.TABLE_NAME)
    return _tablas[clave]
