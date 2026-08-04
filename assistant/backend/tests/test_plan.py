import plan

IDS = {"institucional", "nacional"}


def test_descarta_un_corpus_no_instalado():
    p = plan.validar(
        {"corpus": ["institucional", "inventado"], "consultas": ["deber de reportar"]},
        IDS, maximo_consultas=2,
    )
    assert p.corpus == ["institucional"]


def test_sin_corpus_valido_queda_vacio_en_ese_campo():
    p = plan.validar(
        {"corpus": ["inventado"], "consultas": ["deber de reportar"]},
        IDS, maximo_consultas=2,
    )
    assert p.corpus == []


def test_corta_las_consultas_al_maximo():
    p = plan.validar(
        {"corpus": ["nacional"], "consultas": ["una", "dos", "tres"]},
        IDS, maximo_consultas=2,
    )
    assert p.consultas == ["una", "dos"]


def test_descarta_consultas_en_blanco():
    p = plan.validar(
        {"corpus": ["nacional"], "consultas": ["  ", "deber de reportar"]},
        IDS, maximo_consultas=2,
    )
    assert p.consultas == ["deber de reportar"]


def test_descarta_un_articulo_que_no_existe():
    p = plan.validar(
        {"corpus": ["nacional"], "consultas": ["x"], "articulo": {"norma": "Ley 21.663", "numero": 400}},
        IDS, maximo_consultas=2,
        articulo_existe=lambda norma, numero: numero <= 27,
    )
    assert p.articulo is None


def test_conserva_un_articulo_que_existe():
    p = plan.validar(
        {"corpus": ["nacional"], "consultas": ["x"], "articulo": {"norma": "Ley 21.663", "numero": 9}},
        IDS, maximo_consultas=2,
        articulo_existe=lambda norma, numero: numero <= 27,
    )
    assert p.articulo == {"norma": "Ley 21.663", "numero": 9}


def test_sin_verificador_el_articulo_pasa_tal_cual():
    p = plan.validar(
        {"corpus": ["nacional"], "consultas": ["x"], "articulo": {"norma": "Ley 21.663", "numero": 9}},
        IDS, maximo_consultas=2,
    )
    assert p.articulo == {"norma": "Ley 21.663", "numero": 9}


def test_un_plan_sin_consultas_y_sin_articulo_esta_vacio():
    p = plan.validar({"corpus": ["nacional"], "consultas": []}, IDS, maximo_consultas=2)
    assert plan.vacio(p) is True


def test_un_plan_con_articulo_no_esta_vacio_aunque_no_traiga_consultas():
    p = plan.validar(
        {"corpus": ["nacional"], "consultas": [], "articulo": {"norma": "Ley 21.663", "numero": 9}},
        IDS, maximo_consultas=2,
    )
    assert plan.vacio(p) is False


def test_entrada_que_no_es_diccionario_no_lanza():
    assert plan.vacio(plan.validar(None, IDS, maximo_consultas=2)) is True


def test_descarta_un_numero_booleano():
    p = plan.validar(
        {"corpus": ["nacional"], "consultas": ["x"], "articulo": {"norma": "Ley 21.663", "numero": True}},
        IDS, maximo_consultas=2,
    )
    assert p.articulo is None


def test_un_verificador_que_lanza_descarta_el_articulo_y_no_propaga():
    def _revienta(norma, numero):
        raise RuntimeError("metadata no disponible")

    p = plan.validar(
        {"corpus": ["nacional"], "consultas": ["x"], "articulo": {"norma": "Ley 21.663", "numero": 9}},
        IDS, maximo_consultas=2, articulo_existe=_revienta,
    )
    assert p.articulo is None
