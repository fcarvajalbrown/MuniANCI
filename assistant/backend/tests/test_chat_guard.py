"""Prueba de integracion del guard de citas sobre el propio endpoint /chat.

Las pruebas de citas.py verifican la regla; esta verifica el cableado, que es
donde un guard correcto se queda sin efecto. No toca el modelo ni la base
vectorial: sustituye retrieve() y stream_chat() por dobles.
"""

import asyncio
import json

import main


def _dobles(monkeypatch, tokens, chunks):
    async def falso_retrieve(_consulta):
        return "contexto", chunks

    def falso_stream(_mensajes):
        yield from tokens

    monkeypatch.setattr(main, "retrieve", falso_retrieve)
    monkeypatch.setattr(main.inference, "stream_chat", falso_stream)


def _respuesta(mensaje):
    async def correr():
        resp = await main.chat(main.ChatRequest(message=mensaje))
        crudo = []
        async for trozo in resp.body_iterator:
            crudo.append(trozo if isinstance(trozo, str) else trozo.decode("utf-8"))
        return "".join(crudo)

    bruto = asyncio.run(correr())
    partes = []
    for linea in bruto.splitlines():
        linea = linea.strip()
        if not linea.startswith("data:"):
            continue
        d = json.loads(linea[5:].strip())
        if d.get("type") == "token":
            partes.append(d["content"])
    return "".join(partes)


CHUNK_17 = {
    "text": "Articulo 17.- Obligacion de reporte. Deberan reportar al CSIRT-DN.",
    "source": "decreto2.pdf",
    "chunk_index": 1,
}


def test_el_endpoint_bloquea_un_articulo_inventado(monkeypatch):
    _dobles(
        monkeypatch,
        ["El ", "articulo ", "32 ", "del ", "Reglamento ", "lo ", "establece."],
        [CHUNK_17],
    )
    salida = _respuesta("que dice el reglamento sobre el clima")
    assert "32" in salida
    assert "no aparece en los documentos recuperados" in salida
    assert "lo establece" not in salida


def test_el_endpoint_entrega_una_cita_respaldada(monkeypatch):
    _dobles(
        monkeypatch,
        ["Debe ", "reportar ", "al ", "CSIRT-DN ", "segun ", "el ", "articulo ", "17."],
        [CHUNK_17],
    )
    salida = _respuesta("a quien se reporta un incidente")
    assert salida.strip() == "Debe reportar al CSIRT-DN segun el articulo 17."


def test_una_respuesta_sin_citas_pasa_intacta(monkeypatch):
    _dobles(monkeypatch, ["No ", "tengo ", "esa ", "informacion."], [CHUNK_17])
    assert _respuesta("a quien se reporta un incidente").strip() == "No tengo esa informacion."
