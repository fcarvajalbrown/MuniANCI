from pathlib import Path

import corpus


def _preparar(tmp_path, monkeypatch, municipio, carpetas):
    for nombre in carpetas:
        (tmp_path / nombre).mkdir()
    monkeypatch.setattr(corpus.rag, "base_dir", lambda: tmp_path)
    monkeypatch.setattr(corpus.rag, "_config_municipio", lambda: municipio)
    monkeypatch.delenv("MUNIGPT_DB_DIR", raising=False)
    corpus.limpiar_cache()


def test_encuentra_institucional_y_nacional(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch, "Municipalidad de Providencia", ["db", "db_providencia"])
    ids = [c.id for c in corpus.disponibles()]
    assert ids == ["institucional", "nacional"]


def test_sin_base_institucional_queda_solo_la_nacional(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch, "Municipalidad de Providencia", ["db"])
    ids = [c.id for c in corpus.disponibles()]
    assert ids == ["nacional"]


def test_build_sin_marca_colapsa_en_una_sola_entrada(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch, None, ["db"])
    entradas = corpus.disponibles()
    assert [c.id for c in entradas] == ["nacional"]
    assert len({c.ruta for c in entradas}) == 1


def test_ids_devuelve_el_conjunto(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch, "Municipalidad de Providencia", ["db", "db_providencia"])
    assert corpus.ids() == {"institucional", "nacional"}
