"""
Pruebas de /models/fetch, /models/pack y /models/status.

Ninguna toca la red ni un GGUF real: se sustituye el manifiesto por dos entradas
falsas de pocos bytes y se apunta MUNIGPT_MODELS_DIR a un tmp_path. Lo que se prueba
es la superficie que la UI va a consumir —avance por archivo, un solo trabajo a la
vez, y que una carpeta de paquete inexistente sea un 400 y no un hilo que muere
solo—, no la lógica de fetch_models, que ya tiene sus propias pruebas.
"""

import asyncio
import time

import main


ENTRADA_CHAT = {
    "name": "chatDefault",
    "filename": "modelo-de-chat.gguf",
    "sha256": "0" * 64,
    "sizeBytes": 1000,
    "source": {"confirmed": True, "url": "https://example.invalid/chat.gguf"},
}
ENTRADA_EMBED = {
    "name": "embedding",
    "filename": "modelo-de-embeddings.gguf",
    "sha256": "1" * 64,
    "sizeBytes": 500,
    "source": {"confirmed": False},
}


def _preparar(tmp_path, monkeypatch):
    """Manifiesto falso, directorio de modelos en tmp, y tarea en reposo."""
    monkeypatch.setenv("MUNIGPT_MODELS_DIR", str(tmp_path))
    monkeypatch.setattr(main, "_manifiesto_necesario",
                        lambda: [ENTRADA_CHAT, ENTRADA_EMBED])
    main._modelos_tarea.update(estado="inactivo", accion=None, resultado=None,
                               error=None)
    return tmp_path


def _esperar_al_hilo() -> None:
    """El trabajo corre en un hilo daemon; dos segundos son de sobra para un doble."""
    for _ in range(200):
        if main._modelos_tarea["estado"] != "corriendo":
            return
        time.sleep(0.01)


def test_status_informa_cero_cuando_no_hay_nada(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch)
    r = asyncio.run(main.models_status())

    assert r["tarea"]["estado"] == "inactivo"
    assert [m["bytes"] for m in r["modelos"]] == [0, 0]
    assert [m["presente"] for m in r["modelos"]] == [False, False]
    # La UI necesita saber cuál se puede bajar y cuál solo viene por paquete offline.
    assert [m["descargable"] for m in r["modelos"]] == [True, False]


def test_status_cuenta_los_bytes_de_una_descarga_a_medias(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch)
    (tmp_path / "modelo-de-chat.gguf.part").write_bytes(b"x" * 400)

    r = asyncio.run(main.models_status())
    chat = r["modelos"][0]
    assert chat["bytes"] == 400
    assert chat["bytesTotal"] == 1000
    assert chat["presente"] is False


def test_status_marca_presente_el_archivo_completo(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch)
    (tmp_path / "modelo-de-chat.gguf").write_bytes(b"x" * 1000)

    chat = asyncio.run(main.models_status())["modelos"][0]
    assert chat["presente"] is True
    assert chat["bytes"] == 1000


def test_pack_con_carpeta_inexistente_es_400(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch)

    try:
        asyncio.run(main.models_pack(main.PackRequest(dir=str(tmp_path / "no-existe"))))
        assert False, "se esperaba HTTPException"
    except main.HTTPException as e:
        assert e.status_code == 400
    assert main._modelos_tarea["estado"] == "inactivo"


def test_un_solo_trabajo_a_la_vez(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch)
    main._modelos_tarea.update(estado="corriendo", accion="descarga")

    try:
        asyncio.run(main.models_fetch())
        assert False, "se esperaba HTTPException"
    except main.HTTPException as e:
        assert e.status_code == 409
    finally:
        main._modelos_tarea.update(estado="inactivo", accion=None)


def test_pack_corre_ensure_models_sin_permitir_descarga(tmp_path, monkeypatch):
    """Un paquete offline no debe abrir la red por ningún camino."""
    _preparar(tmp_path, monkeypatch)
    pack = tmp_path / "paquete"
    pack.mkdir()
    visto = {}

    def falso_ensure(manifest, dest, pack_dir=None, allow_download=True):
        visto.update(dest=dest, pack_dir=pack_dir, allow_download=allow_download)
        return {"chatDefault": "from_pack"}

    monkeypatch.setattr(main.fetch_models, "ensure_models", falso_ensure)

    asyncio.run(main.models_pack(main.PackRequest(dir=str(pack))))
    _esperar_al_hilo()

    assert visto["allow_download"] is False
    assert visto["pack_dir"] == pack
    assert visto["dest"] == tmp_path
    assert main._modelos_tarea["estado"] == "listo"
    assert main._modelos_tarea["resultado"] == {"chatDefault": "from_pack"}


def _solo_este_directorio(monkeypatch, directorio):
    """Acota la ruta de búsqueda al tmp: en esta máquina de desarrollo `models/`
    junto a los activos tiene los cuatro GGUF reales, y sin esto find_model los ve."""
    import fetch_models

    monkeypatch.setattr(fetch_models, "models_search_path", lambda: [directorio])


def test_el_chat_se_satisface_con_cualquiera_de_los_dos_modelos(monkeypatch, tmp_path):
    """La regresión que reportó el dueño del repo: un equipo de 16 GB con solo el
    modelo liviano instalado se declaraba NO listo y exigía bajar 2,3 GB, teniendo a
    mano un modelo que funciona. Y al revés, un PC municipal de 8 GB no tiene por qué
    descargar un modelo que no va a poder correr."""
    import inference

    _solo_este_directorio(monkeypatch, tmp_path)
    preferido, alternativa = inference.chat_model_names()
    (tmp_path / inference.embedding_model_name()).write_bytes(b"")

    # Nada de chat: falta, y se nombra el que corresponde a la RAM de este equipo.
    assert inference.missing_models() == [preferido]

    # Solo el que NO corresponde a la RAM: alcanza, y es el que se va a usar.
    (tmp_path / alternativa).write_bytes(b"")
    assert inference.missing_models() == []
    assert inference.select_chat_model_name() == alternativa

    # Con los dos, vuelve a mandar la preferencia por RAM.
    (tmp_path / preferido).write_bytes(b"")
    assert inference.select_chat_model_name() == preferido


def test_el_embedding_sigue_siendo_obligatorio(monkeypatch, tmp_path):
    """Sin vector de consulta no hay recuperación, así que no hay Asistente."""
    import inference

    _solo_este_directorio(monkeypatch, tmp_path)
    preferido, _ = inference.chat_model_names()
    (tmp_path / preferido).write_bytes(b"")

    assert inference.missing_models() == [inference.embedding_model_name()]


def test_fetch_de_un_archivo_concreto_no_arrastra_el_otro(monkeypatch, tmp_path):
    """Pedir el modelo liviano no debe bajar tambien el grande: son alternativas."""
    _preparar(tmp_path, monkeypatch)
    import fetch_models

    monkeypatch.setattr(fetch_models, "models_search_path", lambda: [tmp_path])

    pedido = main._faltantes(ENTRADA_CHAT["filename"])
    assert [e["filename"] for e in pedido] == [ENTRADA_CHAT["filename"]]


def test_una_falla_del_hilo_queda_en_el_estado(tmp_path, monkeypatch):
    """Si ensure_models revienta, la UI tiene que poder decirlo en vez de esperar."""
    _preparar(tmp_path, monkeypatch)

    def revienta(*a, **k):
        raise OSError("disco lleno")

    monkeypatch.setattr(main.fetch_models, "ensure_models", revienta)

    asyncio.run(main.models_fetch())
    _esperar_al_hilo()

    assert main._modelos_tarea["estado"] == "error"
    assert "disco lleno" in main._modelos_tarea["error"]
    main._modelos_tarea.update(estado="inactivo", error=None)
