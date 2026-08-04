from typing import Callable, NamedTuple, Optional

PLAN_SCHEMA = {
    "type": "object",
    "properties": {
        "corpus": {
            "type": "array",
            "items": {"type": "string", "enum": ["institucional", "nacional"]},
        },
        "consultas": {
            "type": "array",
            "items": {"type": "string"},
        },
        "articulo": {
            "type": ["object", "null"],
            "properties": {
                "norma": {"type": "string"},
                "numero": {"type": "integer"},
            },
            "required": ["norma", "numero"],
        },
    },
    "required": ["corpus", "consultas"],
}


class Plan(NamedTuple):
    corpus: list[str]
    consultas: list[str]
    articulo: Optional[dict]


def _existe(verificador, norma: str, numero: int) -> bool:
    try:
        return bool(verificador(norma, numero))
    except Exception:
        return False


def validar(
    bruto,
    corpus_ids: set[str],
    maximo_consultas: int,
    articulo_existe: Optional[Callable[[str, int], bool]] = None,
) -> Plan:
    if not isinstance(bruto, dict):
        return Plan([], [], None)

    crudo_corpus = bruto.get("corpus")
    corpus = [c for c in crudo_corpus if c in corpus_ids] if isinstance(crudo_corpus, list) else []

    crudo_consultas = bruto.get("consultas")
    consultas = []
    if isinstance(crudo_consultas, list):
        for c in crudo_consultas:
            if isinstance(c, str) and c.strip():
                consultas.append(c.strip())
    consultas = consultas[:maximo_consultas]

    articulo = bruto.get("articulo")
    if not isinstance(articulo, dict):
        articulo = None
    else:
        norma = articulo.get("norma")
        numero = articulo.get("numero")
        if not isinstance(norma, str) or isinstance(numero, bool) or not isinstance(numero, int):
            articulo = None
        elif articulo_existe is not None and not _existe(articulo_existe, norma, numero):
            articulo = None
        else:
            articulo = {"norma": norma, "numero": numero}

    return Plan(corpus, consultas, articulo)


def vacio(p: Plan) -> bool:
    return not p.consultas and p.articulo is None


def instruccion(corpus_disponibles) -> str:
    lineas = [f"- {c.id}: {c.etiqueta}" for c in corpus_disponibles]
    catalogo = "\n".join(lineas)
    return (
        "Eres el planificador de busqueda de un asistente legal chileno. "
        "Devuelve SOLO un objeto JSON que indique en que corpus buscar y con que consultas. "
        "No respondas la pregunta.\n\n"
        f"Corpus instalados:\n{catalogo}\n\n"
        "Reglas:\n"
        "- corpus: uno o los dos, segun de donde pueda venir la respuesta.\n"
        "- consultas: frases de busqueda en espanol, sin signos de pregunta.\n"
        "- articulo: solo si la pregunta nombra un articulo concreto de una norma concreta; "
        "si no, null."
    )
