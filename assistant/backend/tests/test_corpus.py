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


def test_sin_municipio_configurado_queda_solo_la_nacional(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch, None, ["db"])
    assert [c.id for c in corpus.disponibles()] == ["nacional"]


def test_ids_devuelve_el_conjunto(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch, "Municipalidad de Providencia", ["db", "db_providencia"])
    assert corpus.ids() == {"institucional", "nacional"}


class _FalsaDB:
    def table_names(self):
        return [corpus.rag.TABLE_NAME]

    def open_table(self, nombre):
        return f"tabla:{nombre}"


def test_abrir_revalida_la_metadata_en_cada_llamada(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch, None, ["db"])
    llamadas = []
    monkeypatch.setattr(corpus.rag, "_assert_embedding_meta", lambda ruta: llamadas.append(ruta))
    monkeypatch.setattr(corpus.lancedb, "connect", lambda ruta: _FalsaDB())
    c = corpus.disponibles()[0]
    corpus.abrir(c)
    corpus.abrir(c)
    assert len(llamadas) == 2
