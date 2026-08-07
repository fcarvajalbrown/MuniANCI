"""
paths.py — dónde viven los datos en tiempo de ejecución.

Una sola regla, y vale para los dos modos de arranque: **todo activo del backend
—`bin/`, `models/`, `db/`, `corpus/`, el manifiesto— vive junto al ejecutable**, y
`config.json` un nivel más arriba. En el árbol de desarrollo eso es
`assistant/backend/` con `assistant/config.json`; en la app empaquetada es la carpeta
`--onedir` de PyInstaller con `config.json` junto a `munigpt-gui.exe`.

Existe porque `Path(__file__)` no sirve para eso cuando el backend viaja congelado:
PyInstaller resuelve el `__file__` de un módulo empaquetado **dentro de `_internal`**,
así que `Path(__file__).parent / "models"` apuntaría a un subdirectorio del propio
bundle en vez de a la carpeta que el instalador llenó. `sys.executable` sí apunta al
ejecutable real, y es lo único estable en ambos modos.
"""

from __future__ import annotations

import sys
from pathlib import Path


def base_dir() -> Path:
    """La carpeta que contiene los activos: la del ejecutable congelado, o
    `assistant/backend/` en desarrollo."""
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parent


def config_path() -> Path:
    """`config.json` del Asistente: un nivel sobre los activos, en los dos modos."""
    return base_dir().parent / "config.json"
