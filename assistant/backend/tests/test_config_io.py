import json
from pathlib import Path

import config_io


def test_lee_json_normal(tmp_path):
    ruta = tmp_path / "config.json"
    ruta.write_text(json.dumps({"municipio": "Organismo de Ejemplo"}), encoding="utf-8")
    assert config_io.leer_config(ruta) == {"municipio": "Organismo de Ejemplo"}


def test_tolera_bom_de_windows(tmp_path):
    ruta = tmp_path / "config.json"
    ruta.write_text(json.dumps({"municipio": "Organismo de Ejemplo"}), encoding="utf-8-sig")
    assert config_io.leer_config(ruta) == {"municipio": "Organismo de Ejemplo"}


def test_archivo_ausente_devuelve_vacio(tmp_path):
    assert config_io.leer_config(tmp_path / "no-existe.json") == {}


def test_json_invalido_avisa_por_stderr_y_no_lanza(tmp_path, capsys):
    ruta = tmp_path / "config.json"
    ruta.write_text("{ esto no es json", encoding="utf-8")
    assert config_io.leer_config(ruta) == {}
    assert "config.json" in capsys.readouterr().err


def test_config_corrupta_cae_al_ejemplo_y_no_a_los_defaults(tmp_path, monkeypatch):
    import inference

    roto = tmp_path / "config.json"
    roto.write_text("{ roto", encoding="utf-8")
    ejemplo = tmp_path / "config.example.json"
    ejemplo.write_text('{"models": {"nCtx": 777}}', encoding="utf-8")

    monkeypatch.setattr(inference, "_CONFIG_PATH", roto)
    monkeypatch.setattr(inference, "_CONFIG_EXAMPLE", ejemplo)

    assert inference._load_models_config()["nCtx"] == 777
