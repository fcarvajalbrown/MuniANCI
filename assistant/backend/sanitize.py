"""
sanitize.py — defensas contra inyección indirecta de prompts en la ruta RAG.

Dos capas, alineadas con OWASP Top 10 for LLM Applications (2025), que es explícito
en que RAG por sí solo no es una defensa contra la inyección indirecta:

1. Saneamiento en tiempo de indexación (`sanitize_for_index`, usado por ingest.py):
   el corpus incluye ordenanzas municipales y PDFs Tier-3 que TI puede dejar caer vía
   el endpoint /ingest, así que el texto indexado es sólo semiconfiable. Antes de
   embeber/almacenar cada documento se eliminan los vectores clásicos para ocultar
   instrucciones (caracteres de ancho cero, override bidireccional Unicode, controles)
   y se neutralizan marcadores de rol y frases de override dirigidas al asistente.

2. Spotlighting en tiempo de prompt (`build_data_block`, usado por rag.build_context):
   el contexto recuperado se envuelve en delimitadores explícitos y se marca como
   DATOS, no instrucciones. El system prompt (main.py) instruye al modelo a ignorar
   cualquier instrucción contenida en ese bloque. Los propios delimitadores se
   eliminan de los chunks para que un chunk malicioso no pueda "cerrar" el bloque de
   datos y escapar (delimiter injection).
"""

import re
import unicodedata

# Delimitadores del bloque de datos recuperado (spotlighting). Poco probables en
# texto legal; de todos modos se eliminan de los chunks para evitar el escape.
SPOTLIGHT_OPEN = "<<<DOCUMENTO_RECUPERADO>>>"
SPOTLIGHT_CLOSE = "<<<FIN_DOCUMENTO_RECUPERADO>>>"

# Caracteres invisibles usados para ocultar instrucciones: ancho cero, joiners, BOM,
# y overrides bidireccionales (LRE..RLO, LRI..PDI). Se listan explícitos (por code
# point) para dejar constancia de los vectores cubiertos; el barrido por categoría
# Unicode (abajo) los cubre igual, pero esta lista documenta la intención.
_ZERO_WIDTH = "​‌‍⁠﻿"
_BIDI = "‪‫‬‭‮⁦⁧⁨⁩"
_HIDDEN_CHARS = _ZERO_WIDTH + _BIDI

# Marcadores de rol al inicio de línea (un chunk que se hace pasar por un turno de
# sistema/asistente). Se neutraliza el separador ":", no se borra la línea, porque
# podría ser texto legal legítimo ("Artículo 1:", etc. no gatilla — sólo roles).
_ROLE_MARKER = re.compile(
    r"(?im)^([ \t>*-]*)(system|assistant|user|sistema|asistente|usuario)\s*:",
)

# Frases de override dirigidas al modelo. Conservador: se neutraliza la frase gatillo,
# no se elimina el resto del contenido del chunk.
_OVERRIDE_PHRASES = re.compile(
    r"(?i)("
    r"ignora(?:r)?\s+(?:las\s+|todas\s+las\s+)?instrucciones\s+(?:anteriores|previas)"
    r"|olvida(?:r)?\s+(?:las\s+)?instrucciones\s+(?:anteriores|previas)"
    r"|ignore\s+(?:all\s+)?(?:the\s+)?(?:previous|prior|above)\s+instructions"
    r"|disregard\s+(?:all\s+)?(?:the\s+)?(?:previous|prior|above)\s+instructions"
    r"|new\s+instructions?\s*:"
    r"|nuevas\s+instrucciones\s*:"
    r")"
)

_NEUTRALIZED = "[contenido neutralizado]"


def strip_hidden_chars(text: str) -> str:
    """Elimina caracteres invisibles/bidi y de control (salvo salto de línea y tab)."""
    if not text:
        return text
    text = text.translate({ord(c): None for c in _HIDDEN_CHARS})
    return "".join(
        c for c in text
        if c in "\n\t" or not unicodedata.category(c).startswith("C")
    )


def neutralize_injection(text: str) -> str:
    """Neutraliza marcadores de rol y frases de override dirigidas al asistente."""
    if not text:
        return text
    text = _ROLE_MARKER.sub(r"\1\2 ", text)  # "system:" -> "system " (deja de ser rol)
    text = _OVERRIDE_PHRASES.sub(_NEUTRALIZED, text)
    return text


def sanitize_for_index(text: str) -> str:
    """Capa 1: saneamiento en tiempo de indexación (ingest.py)."""
    return neutralize_injection(strip_hidden_chars(text))


def clean_for_context(text: str) -> str:
    """Prepara un fragmento para el bloque spotlighted: quita ocultos y delimitadores.

    Defiende contra delimiter injection (un chunk que contiene el delimitador de
    cierre para escapar del bloque de datos) incluso en DBs construidas antes de que
    existiera el saneamiento en tiempo de indexación.
    """
    text = strip_hidden_chars(text or "")
    return text.replace(SPOTLIGHT_OPEN, "").replace(SPOTLIGHT_CLOSE, "")


def build_data_block(body: str) -> str:
    """Envuelve `body` (ya formateado con [Fuente: ...]) entre los delimitadores."""
    return f"{SPOTLIGHT_OPEN}\n{body}\n{SPOTLIGHT_CLOSE}"
