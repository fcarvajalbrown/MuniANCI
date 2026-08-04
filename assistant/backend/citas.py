"""citas.py — verificacion de cita textual (quote-in-source).

El modelo puede redactar una respuesta plausible citando un articulo que no
existe. Ocurrio el 2026-08-03 con una pregunta fuera de dominio ("cual es el
clima"): respondio citando el "articulo 32 del Reglamento de Ciberseguridad de
la Defensa Nacional", que tiene 23 articulos.

La regla es deterministica y no depende del modelo: todo numero de articulo que
aparezca en la respuesta tiene que aparecer tambien en alguno de los fragmentos
recuperados. Si no, la respuesta no se entrega.

Limite conocido y deliberado: se comparan numeros de articulo, no la norma a la
que pertenecen, asi que un articulo 9 de otra ley presente en el contexto
respalda una cita al articulo 9. Es una cota inferior de seguridad, no una
verificacion completa; el objetivo es cortar el numero inventado.
"""

import re
import unicodedata

_ARTICULO = re.compile(r"art(?:iculos?|\.)\s*(?:n\s*[°º]?\s*)?(\d{1,3})")


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


def mensaje_de_rechazo(faltantes: set[int]) -> str:
    numeros = ", ".join(str(n) for n in sorted(faltantes))
    plural = "los articulos" if len(faltantes) > 1 else "el articulo"
    return (
        "No puedo entregar esa respuesta. La redaccion generada citaba "
        f"{plural} {numeros}, que no aparece en los documentos recuperados para "
        "esta consulta, y este asistente no afirma referencias legales que no "
        "pueda respaldar con el texto a la vista.\n\n"
        "Reformule la pregunta o consulte la norma directamente."
    )
