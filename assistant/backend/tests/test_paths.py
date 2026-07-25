"""
Resolución de rutas en los dos modos de arranque (dev y congelado).

Importa porque el modo congelado no se puede probar corriendo el binario en CI: aquí
se simula el estado que PyInstaller deja (`sys.frozen` + `sys.executable` apuntando al
ejecutable de la carpeta `--onedir`) y se verifica que los activos se resuelvan junto a
ese ejecutable y no dentro del bundle.
"""

import importlib
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


def test_inference_sirve_desde_el_directorio_del_env(monkeypatch, tmp_path):
    """La regresión que este cambio corrige: inference.py resolvía `models/` por su
    cuenta e ignoraba MUNIGPT_MODELS_DIR, así que el host podía descargar un modelo a
    un directorio donde el motor nunca lo iba a buscar."""
    monkeypatch.setenv("MUNIGPT_MODELS_DIR", str(tmp_path / "modelos"))
    import inference

    recargado = importlib.reload(inference)
    try:
        assert recargado.MODELS_DIR == tmp_path / "modelos"
    finally:
        monkeypatch.delenv("MUNIGPT_MODELS_DIR", raising=False)
        importlib.reload(inference)
