"""
Resolución de rutas en los dos modos de arranque (dev y congelado).

Importa porque el modo congelado no se puede probar corriendo el binario en CI: aquí
se simula el estado que PyInstaller deja (`sys.frozen` + `sys.executable` apuntando al
ejecutable de la carpeta `--onedir`) y se verifica que los activos se resuelvan junto a
ese ejecutable y no dentro del bundle.
"""

import sys
from pathlib import Path

import paths


def test_base_dir_en_desarrollo_es_la_carpeta_del_backend():
    assert paths.base_dir() == Path(__file__).resolve().parents[1]


def test_config_esta_un_nivel_sobre_los_activos():
    assert paths.config_path() == paths.base_dir().parent / "config.json"


def test_base_dir_congelado_sigue_al_ejecutable(monkeypatch, tmp_path):
    exe = tmp_path / "onedir" / "munigpt-backend.exe"
    exe.parent.mkdir()
    exe.write_bytes(b"")
    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(exe))

    assert paths.base_dir() == exe.parent
    assert paths.config_path() == tmp_path / "config.json"


def test_models_dir_respeta_el_env(monkeypatch, tmp_path):
    """El host apunta MUNIGPT_MODELS_DIR a un directorio escribible del usuario."""
    import fetch_models

    monkeypatch.setenv("MUNIGPT_MODELS_DIR", str(tmp_path / "modelos"))
    assert fetch_models.models_dir() == tmp_path / "modelos"


def test_models_dir_sin_env_cae_junto_a_los_activos(monkeypatch):
    import fetch_models

    monkeypatch.delenv("MUNIGPT_MODELS_DIR", raising=False)
    assert fetch_models.models_dir() == paths.base_dir() / "models"


def test_la_busqueda_mira_el_env_y_despues_los_activos(monkeypatch, tmp_path):
    """Escribir y servir no son el mismo directorio: el modelo de chat aterriza en el
    directorio escribible del usuario, el de embeddings viaja junto a los activos."""
    import fetch_models

    monkeypatch.setenv("MUNIGPT_MODELS_DIR", str(tmp_path / "escribible"))
    rutas = fetch_models.models_search_path()

    assert rutas[0] == tmp_path / "escribible"
    assert rutas[1] == paths.base_dir() / "models"


def test_la_busqueda_no_duplica_cuando_coinciden(monkeypatch):
    """Sin env override los dos candidatos son el mismo directorio."""
    import fetch_models

    monkeypatch.delenv("MUNIGPT_MODELS_DIR", raising=False)
    assert fetch_models.models_search_path() == [paths.base_dir() / "models"]


def test_find_model_prefiere_el_directorio_escribible(monkeypatch, tmp_path):
    import fetch_models

    escribible = tmp_path / "escribible"
    escribible.mkdir()
    (escribible / "m.gguf").write_bytes(b"nuevo")
    monkeypatch.setenv("MUNIGPT_MODELS_DIR", str(escribible))

    assert fetch_models.find_model("m.gguf") == escribible / "m.gguf"
    assert fetch_models.find_model("no-esta.gguf") is None


def test_encuentra_el_modelo_embarcado_aunque_el_env_apunte_a_otro_lado(
    monkeypatch, tmp_path
):
    """El error que este diseño evita: con un solo directorio, apuntar
    MUNIGPT_MODELS_DIR a un directorio escribible vacío escondería el GGUF de
    embeddings que viaja en el instalador, y el Asistente pediría bajar 344 MB que ya
    están en el disco."""
    import fetch_models

    monkeypatch.setenv("MUNIGPT_MODELS_DIR", str(tmp_path / "vacio"))
    junto_a_los_activos = paths.base_dir() / "models"
    embarcado = junto_a_los_activos / "embarcado-de-prueba.gguf"
    junto_a_los_activos.mkdir(exist_ok=True)
    embarcado.write_bytes(b"embarcado")
    try:
        assert fetch_models.find_model("embarcado-de-prueba.gguf") == embarcado
    finally:
        embarcado.unlink()
