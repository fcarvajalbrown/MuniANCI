import xml.etree.ElementTree as ET

import corpus_fetcher
import ingest

NS = "http://www.leychile.cl/esquemas"

CUERPO_LARGO = "Artículo 9°. Deber de reportar. " + "Texto largo de relleno. " * 60

XML = f"""<?xml version="1.0" encoding="UTF-8"?>
<Norma xmlns="{NS}">
  <Identificador fechaPromulgacion="2024-03-26" fechaPublicacion="2024-04-08">
    <TiposNumeros><TipoNumero><Tipo>Ley</Tipo><Numero>21663</Numero></TipoNumero></TiposNumeros>
  </Identificador>
  <Metadatos><TituloNorma>LEY MARCO DE CIBERSEGURIDAD</TituloNorma></Metadatos>
  <Encabezado fechaVersion="2024-04-08" derogado="no derogado">
    <Texto>LEY NUM. 21.663</Texto>
  </Encabezado>
  <EstructurasFuncionales>
    <EstructuraFuncional transitorio="no transitorio" tipoParte="Título"
        fechaVersion="2024-04-08" derogado="no derogado" idParte="1">
      <Texto>TÍTULO II Obligaciones</Texto>
      <Metadatos>
        <NombreParte presente="no"></NombreParte>
        <TituloParte presente="si">TÍTULO II Obligaciones</TituloParte>
      </Metadatos>
      <EstructurasFuncionales>
        <EstructuraFuncional transitorio="no transitorio" tipoParte="Artículo"
            fechaVersion="2024-04-08" derogado="no derogado" idParte="2">
          <Texto>{CUERPO_LARGO}</Texto>
          <Metadatos>
            <NombreParte presente="si">9º</NombreParte>
            <TituloParte presente="no"></TituloParte>
          </Metadatos>
        </EstructuraFuncional>
        <EstructuraFuncional transitorio="transitorio" tipoParte="Artículo"
            fechaVersion="2024-04-08" derogado="derogado" idParte="3">
          <Texto>Artículo primero. Disposición transitoria derogada.</Texto>
          <Metadatos>
            <NombreParte presente="si">primero</NombreParte>
            <TituloParte presente="no"></TituloParte>
          </Metadatos>
        </EstructuraFuncional>
      </EstructurasFuncionales>
    </EstructuraFuncional>
  </EstructurasFuncionales>
  <Promulgacion fechaVersion="2024-04-08" derogado="no derogado">
    <Texto>Promulgación y firma.</Texto>
  </Promulgacion>
</Norma>
""".encode("utf-8")


def estructura():
    return corpus_fetcher.build_estructura(XML, "1202434")


def test_norma_display_uses_chilean_thousands_separator():
    assert corpus_fetcher.norma_display("Ley", "21663") == "Ley 21.663"
    assert corpus_fetcher.norma_display("DFL", "1") == "DFL 1"


def test_structured_parse_loses_no_text_versus_the_flat_parse():
    root = ET.fromstring(XML)
    flat = "\n".join(
        p for p in ((el.text or "").strip() for el in root.iter(f"{{{NS}}}Texto")) if p
    )
    structured = "\n".join(p["texto"] for p in corpus_fetcher.extract_partes(root))
    assert structured == flat


def test_norma_identity_comes_from_the_xml():
    norma = estructura()["norma"]
    assert norma["display"] == "Ley 21.663"
    assert norma["titulo"] == "LEY MARCO DE CIBERSEGURIDAD"
    assert norma["id_norma"] == "1202434"
    assert norma["fecha_publicacion"] == "2024-04-08"


def test_partes_carry_the_four_attributes_the_flat_txt_cannot_express():
    partes = estructura()["partes"]

    articulo = next(p for p in partes if p["id_parte"] == "2")
    assert articulo["tipo_parte"] == "Artículo"
    assert articulo["numero_articulo"] == "9°"
    assert articulo["fecha_version"] == "2024-04-08"
    assert articulo["derogado"] is False
    assert articulo["ruta"] == "TÍTULO II Obligaciones"

    transitorio = next(p for p in partes if p["id_parte"] == "3")
    assert transitorio["transitorio"] is True
    assert transitorio["derogado"] is True


def test_masculine_ordinal_is_normalised_to_the_degree_sign():
    articulo = next(p for p in estructura()["partes"] if p["id_parte"] == "2")
    assert articulo["numero_articulo"] == "9°"


def test_every_chunk_of_an_article_names_its_article():
    del_articulo = [c for c in ingest.chunk_estructura(estructura())
                    if c["id_parte"] == "2"]
    assert len(del_articulo) > 1
    for c in del_articulo:
        assert c["text"].startswith("Ley 21.663, Artículo 9°\n")
        assert c["numero_articulo"] == "9°"
        assert c["ruta"] == "TÍTULO II Obligaciones"


def test_transitorio_article_is_labelled_as_transitorio():
    transitorio = next(c for c in ingest.chunk_estructura(estructura())
                       if c["id_parte"] == "3")
    assert transitorio["text"] == (
        "Ley 21.663, Artículo primero transitorio\n"
        "Artículo primero. Disposición transitoria derogada."
    )
    assert transitorio["derogado"] is True


def test_non_article_parts_get_no_header():
    chunks = ingest.chunk_estructura(estructura())
    encabezado = next(c for c in chunks if c["tipo_parte"] == "Encabezado")
    assert encabezado["text"] == "LEY NUM. 21.663"
    assert encabezado["numero_articulo"] == ""

    titulo = next(c for c in chunks if c["tipo_parte"] == "Título")
    assert titulo["text"] == "TÍTULO II Obligaciones"


def test_a_chunk_never_mixes_two_partes():
    for c in ingest.chunk_estructura(estructura()):
        cuerpo = c["text"].split("\n", 1)[-1]
        assert not ("Deber de reportar" in cuerpo and "Promulgación" in cuerpo)
        assert not ("Disposición transitoria" in cuerpo and "Deber de reportar" in cuerpo)


def test_chunk_index_is_unique_and_sequential_across_the_document():
    chunks = ingest.chunk_estructura(estructura())
    assert [c["chunk_index"] for c in chunks] == list(range(len(chunks)))


def test_flat_documents_still_produce_every_structure_column():
    flat = [{**ingest.CAMPOS_ESTRUCTURA, **c}
            for c in ingest.chunk_text("Una ordenanza en PDF sin estructura BCN.")]
    assert flat
    for campo, vacio in ingest.CAMPOS_ESTRUCTURA.items():
        assert flat[0][campo] == vacio


def test_schema_carries_every_structure_column():
    nombres = set(ingest.get_schema(4).names)
    for campo in ingest.CAMPOS_ESTRUCTURA:
        assert campo in nombres
