import json
from pathlib import Path

import config_io


def test_lee_json_normal(tmp_path):
    ruta = tmp_path / "config.json"
    ruta.write_text(json.dumps({"municipio": "Providencia"}), encoding="utf-8")
    assert config_io.leer_config(ruta) == {"municipio": "Providencia"}


def test_tolera_bom_de_windows(tmp_path):
    ruta = tmp_path / "config.json"
    ruta.write_text(json.dumps({"municipio": "Providencia"}), encoding="utf-8-sig")
    assert config_io.leer_config(ruta) == {"municipio": "Providencia"}


def test_archivo_ausente_devuelve_vacio(tmp_path):
    assert config_io.leer_config(tmp_path / "no-existe.json") == {}


def test_json_invalido_avisa_por_stderr_y_no_lanza(tmp_path, capsys):
    ruta = tmp_path / "config.json"
    ruta.write_text("{ esto no es json", encoding="utf-8")
    assert config_io.leer_config(ruta) == {}
    assert "config.json" in capsys.readouterr().err
