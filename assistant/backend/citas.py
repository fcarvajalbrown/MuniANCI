import re
import unicodedata

_ARTICULO = re.compile(r"art(?:iculos?|\.)\s*(?:n\s*[°º]?\s*)?(\d{1,3})")

_TLD = "cl|com|org|net|gov|edu|info|int"
_URL = re.compile(
    r"(?:https?://|www\.)[^\s<>\"'()\[\]]+"
    rf"|\b[a-z0-9][a-z0-9-]*(?:\.[a-z0-9-]+)*\.(?:{_TLD})\b(?:/[^\s<>\"'()\[\]]*)?"
)

_BORDES = ".,;:!?\"'»)]}>"


def normalizar(texto: str) -> str:
    descompuesto = unicodedata.normalize("NFKD", texto or "")
    sin_tildes = "".join(c for c in descompuesto if not unicodedata.combining(c))
    return sin_tildes.lower()


def articulos(texto: str) -> set[int]:
    return {int(m.group(1)) for m in _ARTICULO.finditer(normalizar(texto))}


def articulos_respaldados(chunks) -> set[int]:
    respaldo: set[int] = set()
    for c in chunks or []:
        respaldo |= articulos(c.get("text", ""))
    return respaldo


def sin_respaldo(respuesta: str, chunks) -> set[int]:
    return articulos(respuesta) - articulos_respaldados(chunks)


def enlaces(texto: str) -> set[str]:
    return {m.group(0).rstrip(_BORDES) for m in _URL.finditer(normalizar(texto))}


def _contexto(chunks) -> str:
    return normalizar(" ".join((c.get("text") or "") for c in chunks or []))


def enlaces_sin_respaldo(respuesta: str, chunks) -> set[str]:
    contexto = _contexto(chunks)
    return {u for u in enlaces(respuesta) if u not in contexto}


def mensaje_de_rechazo(faltantes: set[int], enlaces_faltantes=frozenset()) -> str:
    citado = []
    if faltantes:
        numeros = ", ".join(str(n) for n in sorted(faltantes))
        citado.append(f"{'los articulos' if len(faltantes) > 1 else 'el articulo'} {numeros}")
    if enlaces_faltantes:
        lista = ", ".join(sorted(enlaces_faltantes))
        citado.append(f"{'los enlaces' if len(enlaces_faltantes) > 1 else 'el enlace'} {lista}")
    referencias = "referencias ni enlaces" if enlaces_faltantes else "referencias legales"
    return (
        "No puedo entregar esa respuesta. La redaccion generada citaba "
        f"{' y '.join(citado)}, que no aparece en los documentos recuperados para "
        f"esta consulta, y este asistente no afirma {referencias} que no "
        "pueda respaldar con el texto a la vista.\n\n"
        "Reformule la pregunta o consulte la norma directamente."
    )
