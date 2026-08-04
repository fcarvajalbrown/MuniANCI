import citas


def chunk(texto):
    return {"text": texto, "source": "x.pdf", "chunk_index": 0}


def test_detecta_articulo_inventado_del_caso_real():
    respuesta = (
        "El clima en el contexto legal proporcionado es el ambiente de trabajo y "
        "operacion de las Fuerzas Armadas, establecido en el articulo 32 del "
        "Reglamento de Ciberseguridad de la Defensa Nacional."
    )
    contexto = [chunk("Articulo 17.- Obligacion de reporte de ciberataques e incidentes.")]
    assert citas.sin_respaldo(respuesta, contexto) == {32}


def test_deja_pasar_el_articulo_que_si_esta_en_el_contexto():
    respuesta = (
        "Debe reportar al CSIRT-DN, segun el articulo 17 del Reglamento de "
        "Ciberseguridad de la Defensa Nacional."
    )
    contexto = [
        chunk(
            "Articulo 17.- Obligacion de reporte de ciberataques e incidentes. Todos "
            "los organismos e instituciones del sector Defensa deberan reportar al CSIRT-DN."
        )
    ]
    assert citas.sin_respaldo(respuesta, contexto) == set()


def test_las_tildes_no_esconden_una_cita():
    contexto = [chunk("Artículo 9°.- Deber de reportar.")]
    assert citas.sin_respaldo("Lo obliga el artículo 9 de la ley.", contexto) == set()


def test_reconoce_la_forma_abreviada():
    assert citas.articulos("Lo dispone el art. 27 de la ley N° 21.663.") == {27}


def test_reconoce_el_ordinal_con_signo():
    assert citas.articulos("Conforme al artículo N° 8 del reglamento.") == {8}


def test_una_respuesta_sin_articulos_nunca_se_bloquea():
    assert citas.sin_respaldo("No tengo informacion sobre eso.", []) == set()


def test_sin_contexto_toda_cita_queda_sin_respaldo():
    assert citas.sin_respaldo("Segun el articulo 5 de la ley.", []) == {5}


def test_el_mensaje_de_rechazo_nombra_los_articulos():
    m = citas.mensaje_de_rechazo({32})
    assert "32" in m and "el articulo" in m
    m2 = citas.mensaje_de_rechazo({7, 32})
    assert "7, 32" in m2 and "los articulos" in m2
