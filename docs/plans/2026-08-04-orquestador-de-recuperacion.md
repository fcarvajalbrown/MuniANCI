# Orquestador de recuperación — plan de implementación

> **SUPERADO EL 2026-08-12. Registro, no instrucción.** Este plan implementa el ADR 0004,
> que el [ADR 0007](../adr/0007-agente-multiherramienta-en-una-pasada.md) superó: 0.9.5 dejó
> de ser un planificador cuya única acción era recuperar y pasó a ser un agente que elige
> entre cuatro herramientas en una sola pasada. Las tareas 1 a 4 se entregaron y siguen
> vigentes; **las tareas 5 a 8 quedan sin efecto** y se rediseñan según el ADR 0007. Dos
> cosas más envejecieron dentro del texto: `rag.buscar_en` ya no es la versión que aquí se
> transcribe —alcanzó la paridad con `retrieve()` el 2026-08-12, fusión RRF y ruta de
> artículo incluidas—, y la línea base de pruebas ya no son 110 sino 195.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** que el Asistente elija por sí mismo de qué corpus y con qué consulta recupera, con dos bases abiertas a la vez, sin que pueda llegar a una respuesta sin haber recuperado nada.

**Architecture:** un módulo `orquestador.py` reemplaza la llamada directa a `rag.retrieve()` en `/chat`. Produce un plan JSON restringido por `json_schema` en un turno de modelo, lo valida contra los corpus realmente instalados, ejecuta la recuperación por cada par (corpus, consulta) y arma el contexto. Cualquier falla degrada al camino fijo de hoy.

**Tech Stack:** Python 3.12, FastAPI, LanceDB, `llama-server` de llama.cpp (OpenAI-compatible), httpx, pytest.

## Global Constraints

- **Sin comentarios en el código.** Ninguno: ni de línea, ni de bloque, ni docstrings, ni `TODO`. Si algo necesita explicación, se renombra o se extrae.
- **Sin emojis** en código, mensajes de commit ni documentación.
- **Sin atribución a IA** en commits, código ni documentación.
- **Nada se da por hecho sin salida de comando real.** Un paso que dice "correr las pruebas" se corre.
- **Commits en castellano, formato Conventional Commits**, directo sobre `main`, empujados a origin a medida que se avanza. Nunca abrir un pull request.
- Módulos del backend **planos** (sin paquete): `import rag`, `import corpus`, tal como hace `main.py`.
- Comando de pruebas, siempre desde `assistant\backend`:
  `..\.venv\Scripts\python.exe -m pytest`
- Línea base a no romper: **110 pruebas en verde**.
- Toda clave nueva de `config.json` lleva valor por defecto, para que un archivo antiguo siga cargando.

## Orden y dependencia con 0.9.0

Las tareas 1 a 4 **no dependen de 0.9.0** y se pueden ejecutar hoy. La tarea 1 corrige un
defecto que afecta a los builds ya instalados.

Las tareas 5 a 8 son el orquestador propiamente tal. **Técnicamente compilan y pasan sus
pruebas sin el Tramo A**: `rag.buscar_en` funciona hoy con la búsqueda vectorial y BM-25 que
ya existen. Lo que las ata al Tramo A es una decisión, no un impedimento: el ADR 0004 fijó
que se implementan después, porque las decisiones de enrutamiento valen lo que valga el
recuperador debajo, y porque la búsqueda determinista por artículo necesita los metadatos de
norma y artículo del chunking consciente de estructura. Mientras eso no exista, el campo
`articulo` del plan se valida y se transporta pero no se ejecuta, y la validación de rango
(tarea 4) queda inactiva por diseño.

Correr 5 a 8 antes del Tramo A es posible y contradice el ADR 0004. Es una decisión del
dueño del repo, no del implementador.

---

### Task 1: Lectura de `config.json` tolerante a BOM

`config.json` se lee hoy con `encoding="utf-8"` en cinco lugares, y `rag._config_municipio`
envuelve su lectura en `except Exception: return None`. El Bloc de notas y PowerShell
escriben UTF-8 con BOM por defecto en Windows, `json.loads` lo rechaza, y el municipio se
pierde en silencio: el Asistente responde desde el corpus nacional en vez del institucional.

**Files:**
- Create: `assistant/backend/config_io.py`
- Create: `assistant/backend/tests/test_config_io.py`
- Modify: `assistant/backend/rag.py` (`_config_municipio`)
- Modify: `assistant/backend/main.py` (líneas 53, 223, 474, 651)
- Modify: `assistant/backend/inference.py` (línea 64)

**Interfaces:**
- Produces: `config_io.leer_config(ruta: Path) -> dict`. Archivo ausente devuelve `{}`. BOM tolerado. JSON inválido avisa por stderr y devuelve `{}`, nunca lanza.

- [ ] **Step 1: Write the failing test**

`assistant/backend/tests/test_config_io.py`:

```python
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_config_io.py -v`
Expected: FAIL con `ModuleNotFoundError: No module named 'config_io'`

- [ ] **Step 3: Write minimal implementation**

`assistant/backend/config_io.py`:

```python
import json
import sys
from pathlib import Path


def leer_config(ruta: Path) -> dict:
    ruta = Path(ruta)
    if not ruta.exists():
        return {}
    try:
        texto = ruta.read_text(encoding="utf-8-sig")
    except OSError as e:
        print(f"[config] no se pudo leer {ruta}: {e}", file=sys.stderr, flush=True)
        return {}
    try:
        datos = json.loads(texto)
    except json.JSONDecodeError as e:
        print(f"[config] {ruta} no es JSON valido: {e}", file=sys.stderr, flush=True)
        return {}
    return datos if isinstance(datos, dict) else {}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_config_io.py -v`
Expected: PASS, 4 pruebas

- [ ] **Step 5: Reemplazar las lecturas existentes**

En `rag.py`, `_config_municipio` pasa a:

```python
def _config_municipio() -> str | None:
    env = os.environ.get("MUNIGPT_MUNICIPIO")
    if env and env.strip():
        return env.strip()
    name = config_io.leer_config(config_path()).get("municipio")
    return name if isinstance(name, str) and name.strip() else None
```

con `import config_io` junto a los demás imports, y sin el `try/except Exception`.

En `main.py`, las cuatro lecturas (`CONFIG_PATH.read_text(...)` en las líneas 53, 223, 474 y
651) pasan a `config_io.leer_config(CONFIG_PATH)`. La de la línea 651 es la que hoy lee sin
`encoding`, o sea con la codificación por defecto de la plataforma.

En `inference.py`, la lectura de la línea 64 pasa a `config_io.leer_config(path)`.

- [ ] **Step 6: Run the full suite**

Run: `..\.venv\Scripts\python.exe -m pytest`
Expected: PASS, 114 pruebas (110 previas + 4 nuevas)

- [ ] **Step 7: Commit**

```bash
git add assistant/backend/config_io.py assistant/backend/tests/test_config_io.py assistant/backend/rag.py assistant/backend/main.py assistant/backend/inference.py
git commit -m "fix(asistente): una config guardada con el Bloc de notas dejaba de elegir el corpus del organismo"
git push origin main
```

---

### Task 2: Descubrimiento y apertura de los corpus instalados

Hoy `rag.db_dir()` resuelve una carpeta al arrancar y el Asistente abre una sola tabla. Esta
tarea expone los corpus instalados sin cambiar todavía quién los consulta.

**Files:**
- Create: `assistant/backend/corpus.py`
- Create: `assistant/backend/tests/test_corpus.py`

**Interfaces:**
- Consumes: `rag.db_dir()`, `rag._municipio_slug()`, `rag._config_municipio()`, `rag.TABLE_NAME`, `rag._assert_embedding_meta()`.
- Produces:
  - `corpus.Corpus` — `NamedTuple(id: str, etiqueta: str, ruta: Path)`
  - `corpus.disponibles() -> list[Corpus]`
  - `corpus.abrir(c: Corpus)` — devuelve la tabla LanceDB, cacheada por ruta
  - `corpus.ids() -> set[str]`

- [ ] **Step 1: Write the failing test**

`assistant/backend/tests/test_corpus.py`:

```python
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
    _preparar(tmp_path, monkeypatch, "Organismo de Ejemplo", ["db", "db_organismo-de-ejemplo"])
    ids = [c.id for c in corpus.disponibles()]
    assert ids == ["institucional", "nacional"]


def test_sin_base_institucional_queda_solo_la_nacional(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch, "Organismo de Ejemplo", ["db"])
    ids = [c.id for c in corpus.disponibles()]
    assert ids == ["nacional"]


def test_build_sin_marca_colapsa_en_una_sola_entrada(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch, None, ["db"])
    entradas = corpus.disponibles()
    assert [c.id for c in entradas] == ["nacional"]
    assert len({c.ruta for c in entradas}) == 1


def test_ids_devuelve_el_conjunto(tmp_path, monkeypatch):
    _preparar(tmp_path, monkeypatch, "Organismo de Ejemplo", ["db", "db_organismo-de-ejemplo"])
    assert corpus.ids() == {"institucional", "nacional"}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_corpus.py -v`
Expected: FAIL con `ModuleNotFoundError: No module named 'corpus'`

- [ ] **Step 3: Write minimal implementation**

`assistant/backend/corpus.py`:

```python
import os
from pathlib import Path
from typing import NamedTuple

import lancedb

import rag


class Corpus(NamedTuple):
    id: str
    etiqueta: str
    ruta: Path


ETIQUETAS = {
    "institucional": "la normativa propia del organismo",
    "nacional": "la legislacion nacional",
}

_tablas: dict[str, object] = {}


def limpiar_cache() -> None:
    _tablas.clear()


def disponibles() -> list[Corpus]:
    entradas: list[Corpus] = []
    vistas: set[Path] = set()

    forzada = os.environ.get("MUNIGPT_DB_DIR")
    if forzada:
        ruta = Path(forzada).resolve()
        return [Corpus("nacional", ETIQUETAS["nacional"], ruta)]

    municipio = rag._config_municipio()
    if municipio:
        ruta = (rag.base_dir() / f"{rag.DEFAULT_DB}_{rag._municipio_slug(municipio)}").resolve()
        if ruta.exists():
            entradas.append(Corpus("institucional", ETIQUETAS["institucional"], ruta))
            vistas.add(ruta)

    nacional = (rag.base_dir() / rag.DEFAULT_DB).resolve()
    if nacional.exists() and nacional not in vistas:
        entradas.append(Corpus("nacional", ETIQUETAS["nacional"], nacional))

    return entradas


def ids() -> set[str]:
    return {c.id for c in disponibles()}


def abrir(c: Corpus):
    clave = str(c.ruta)
    if clave not in _tablas:
        rag._assert_embedding_meta(c.ruta)
        db = lancedb.connect(clave)
        if rag.TABLE_NAME not in db.table_names():
            raise RuntimeError(f"Tabla '{rag.TABLE_NAME}' no encontrada en {c.ruta}")
        _tablas[clave] = db.open_table(rag.TABLE_NAME)
    return _tablas[clave]
```

`rag.py` necesita exponer `base_dir` para que el monkeypatch de las pruebas funcione sobre un
solo lugar: agregar `from paths import base_dir, config_path` ya está en `rag.py`, así que
`rag.base_dir` existe como atributo del módulo.

- [ ] **Step 4: Run test to verify it passes**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_corpus.py -v`
Expected: PASS, 4 pruebas

- [ ] **Step 5: Run the full suite**

Run: `..\.venv\Scripts\python.exe -m pytest`
Expected: PASS, 118 pruebas

- [ ] **Step 6: Commit**

```bash
git add assistant/backend/corpus.py assistant/backend/tests/test_corpus.py
git commit -m "feat(asistente): los corpus instalados dejan de ser uno solo resuelto al arrancar"
git push origin main
```

---

### Task 3: Búsqueda contra un corpus dado y fusión entre corpus

`rag.retrieve()` se mantiene intacto como camino fijo. Esta tarea agrega la capacidad de
buscar en una tabla concreta y de fusionar resultados de varios corpus arrastrando de cuál
vinieron.

**Files:**
- Modify: `assistant/backend/rag.py`
- Modify: `assistant/backend/tests/test_rag.py`

**Interfaces:**
- Consumes: `corpus.Corpus`, `corpus.abrir()`.
- Produces:
  - `rag.buscar_en(tabla, consulta: str, embedding: list[float], limite: int) -> list[dict]`
  - `rag.fusionar(por_corpus: list[tuple[str, list[dict]]], limite: int) -> list[dict]` — cada fragmento sale con la clave `corpus`, deduplicado por `(corpus, source, chunk_index)`
  - `rag.build_context()` incluye el corpus en la etiqueta cuando el fragmento lo trae

- [ ] **Step 1: Write the failing test**

Añadir a `assistant/backend/tests/test_rag.py`:

```python
def _chunk_c(corpus_id, source, idx, text="texto"):
    return {"corpus": corpus_id, "source": source, "chunk_index": idx, "text": text}


def test_fusionar_dedup_por_corpus_fuente_e_indice():
    a = [_chunk_c("institucional", "reglamento.pdf", 0)]
    b = [_chunk_c("nacional", "reglamento.pdf", 0)]
    out = rag.fusionar([("institucional", a), ("nacional", b)], limite=5)
    assert len(out) == 2


def test_fusionar_descarta_el_repetido_del_mismo_corpus():
    a = [_chunk_c("nacional", "ley.txt", 3), _chunk_c("nacional", "ley.txt", 3)]
    out = rag.fusionar([("nacional", a)], limite=5)
    assert len(out) == 1


def test_fusionar_intercala_los_corpus_antes_de_cortar():
    a = [_chunk_c("institucional", "i.pdf", i) for i in range(5)]
    b = [_chunk_c("nacional", "n.txt", i) for i in range(5)]
    out = rag.fusionar([("institucional", a), ("nacional", b)], limite=4)
    assert len(out) == 4
    assert {c["corpus"] for c in out} == {"institucional", "nacional"}


def test_build_context_declara_el_corpus_cuando_viene():
    ctx = rag.build_context([_chunk_c("institucional", "reglamento.pdf", 0, "uno")])
    assert "[Fuente: reglamento.pdf - corpus institucional]" in ctx


def test_build_context_sin_corpus_mantiene_la_etiqueta_antigua():
    ctx = rag.build_context([_chunk("a.txt", 0, "uno")])
    assert "[Fuente: a.txt]" in ctx
```

- [ ] **Step 2: Run test to verify it fails**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_rag.py -v`
Expected: FAIL con `AttributeError: module 'rag' has no attribute 'fusionar'`

- [ ] **Step 3: Write minimal implementation**

En `rag.py`:

```python
def buscar_en(tabla, consulta: str, embedding: list[float], limite: int) -> list[dict]:
    vectorial = (
        tabla.search(embedding)
        .limit(limite)
        .select(["text", "source", "chunk_index"])
        .to_list()
    )
    try:
        lexica = (
            tabla.search(consulta, query_type="fts")
            .limit(limite)
            .select(["text", "source", "chunk_index"])
            .to_list()
        )
    except Exception:
        lexica = []
    return deduplicate(vectorial + lexica)


def fusionar(por_corpus: list[tuple[str, list[dict]]], limite: int) -> list[dict]:
    listas = []
    for corpus_id, fragmentos in por_corpus:
        marcados = []
        vistos = set()
        for f in fragmentos:
            clave = (corpus_id, f.get("source"), f.get("chunk_index"))
            if clave in vistos:
                continue
            vistos.add(clave)
            marcados.append({**f, "corpus": corpus_id})
        listas.append(marcados)

    salida: list[dict] = []
    vistos_global: set[tuple] = set()
    for posicion in range(max((len(l) for l in listas), default=0)):
        for lista in listas:
            if posicion >= len(lista):
                continue
            f = lista[posicion]
            clave = (f["corpus"], f.get("source"), f.get("chunk_index"))
            if clave in vistos_global:
                continue
            vistos_global.add(clave)
            salida.append(f)
            if len(salida) >= limite:
                return salida
    return salida
```

y en `build_context`, la línea que arma la etiqueta pasa a:

```python
        source = clean_for_context(c.get("source", "desconocido"))
        corpus_id = c.get("corpus")
        etiqueta = f"{source} - corpus {corpus_id}" if corpus_id else source
        text = clean_for_context(c.get("text", ""))
        parts.append(f"[Fuente: {etiqueta}]\n{text}")
```

- [ ] **Step 4: Run test to verify it passes**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_rag.py -v`
Expected: PASS, todas las de `test_rag.py`, incluidas las cinco nuevas

- [ ] **Step 5: Run the full suite**

Run: `..\.venv\Scripts\python.exe -m pytest`
Expected: PASS, 123 pruebas

- [ ] **Step 6: Commit**

```bash
git add assistant/backend/rag.py assistant/backend/tests/test_rag.py
git commit -m "feat(asistente): la recuperacion puede abarcar dos corpus y la cita dice de cual salio"
git push origin main
```

---

### Task 4: Esquema y validación del plan

El plan es lo único que el modelo produce en el primer turno. La validación descarta lo que
no se sostiene contra los corpus realmente instalados, sin turno de reparación.

**Files:**
- Create: `assistant/backend/plan.py`
- Create: `assistant/backend/tests/test_plan.py`

**Interfaces:**
- Produces:
  - `plan.PLAN_SCHEMA: dict`
  - `plan.Plan` — `NamedTuple(corpus: list[str], consultas: list[str], articulo: dict | None)`
  - `plan.validar(bruto: dict, corpus_ids: set[str], maximo_consultas: int, articulo_existe=None) -> Plan`
  - `plan.vacio(p: Plan) -> bool`
  - `plan.instruccion(corpus_disponibles: list) -> str`

`articulo_existe` es `Callable[[str, int], bool] | None`. Mientras 0.9.0 no entregue los
metadatos de norma y artículo, se pasa `None` y la validación de rango no corre.

- [ ] **Step 1: Write the failing test**

`assistant/backend/tests/test_plan.py`:

```python
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_plan.py -v`
Expected: FAIL con `ModuleNotFoundError: No module named 'plan'`

- [ ] **Step 3: Write minimal implementation**

`assistant/backend/plan.py`:

```python
from typing import Callable, NamedTuple, Optional

PLAN_SCHEMA = {
    "type": "object",
    "properties": {
        "corpus": {
            "type": "array",
            "items": {"type": "string", "enum": ["institucional", "nacional"]},
        },
        "consultas": {
            "type": "array",
            "items": {"type": "string"},
        },
        "articulo": {
            "type": ["object", "null"],
            "properties": {
                "norma": {"type": "string"},
                "numero": {"type": "integer"},
            },
            "required": ["norma", "numero"],
        },
    },
    "required": ["corpus", "consultas"],
}


class Plan(NamedTuple):
    corpus: list[str]
    consultas: list[str]
    articulo: Optional[dict]


def validar(
    bruto,
    corpus_ids: set[str],
    maximo_consultas: int,
    articulo_existe: Optional[Callable[[str, int], bool]] = None,
) -> Plan:
    if not isinstance(bruto, dict):
        return Plan([], [], None)

    crudo_corpus = bruto.get("corpus")
    corpus = [c for c in crudo_corpus if c in corpus_ids] if isinstance(crudo_corpus, list) else []

    crudo_consultas = bruto.get("consultas")
    consultas = []
    if isinstance(crudo_consultas, list):
        for c in crudo_consultas:
            if isinstance(c, str) and c.strip():
                consultas.append(c.strip())
    consultas = consultas[:maximo_consultas]

    articulo = bruto.get("articulo")
    if not isinstance(articulo, dict):
        articulo = None
    else:
        norma = articulo.get("norma")
        numero = articulo.get("numero")
        if not isinstance(norma, str) or not isinstance(numero, int):
            articulo = None
        elif articulo_existe is not None and not articulo_existe(norma, numero):
            articulo = None
        else:
            articulo = {"norma": norma, "numero": numero}

    return Plan(corpus, consultas, articulo)


def vacio(p: Plan) -> bool:
    return not p.consultas and p.articulo is None


def instruccion(corpus_disponibles) -> str:
    lineas = [f"- {c.id}: {c.etiqueta}" for c in corpus_disponibles]
    catalogo = "\n".join(lineas)
    return (
        "Eres el planificador de busqueda de un asistente legal chileno. "
        "Devuelve SOLO un objeto JSON que indique en que corpus buscar y con que consultas. "
        "No respondas la pregunta.\n\n"
        f"Corpus instalados:\n{catalogo}\n\n"
        "Reglas:\n"
        "- corpus: uno o los dos, segun de donde pueda venir la respuesta.\n"
        "- consultas: frases de busqueda en espanol, sin signos de pregunta.\n"
        "- articulo: solo si la pregunta nombra un articulo concreto de una norma concreta; "
        "si no, null."
    )
```

- [ ] **Step 4: Run test to verify it passes**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_plan.py -v`
Expected: PASS, 10 pruebas

- [ ] **Step 5: Run the full suite**

Run: `..\.venv\Scripts\python.exe -m pytest`
Expected: PASS, 133 pruebas

- [ ] **Step 6: Commit**

```bash
git add assistant/backend/plan.py assistant/backend/tests/test_plan.py
git commit -m "feat(asistente): el plan de busqueda se valida contra los corpus que estan instalados"
git push origin main
```

---

### Task 5: El turno restringido en `inference.py`

**Files:**
- Modify: `assistant/backend/inference.py`
- Create: `assistant/backend/tests/test_inference_json.py`

**Interfaces:**
- Produces: `inference.completar_json(messages: list[dict], schema: dict, *, max_tokens: int = 256, timeout: float) -> dict`. Lanza `httpx.HTTPError` o `TimeoutError` hacia arriba; el que decide qué hacer con la falla es el orquestador.

- [ ] **Step 1: Write the failing test**

`assistant/backend/tests/test_inference_json.py`:

```python
import json

import httpx

import inference


class _Respuesta:
    def __init__(self, contenido):
        self._contenido = contenido

    def raise_for_status(self):
        return None

    def json(self):
        return {"choices": [{"message": {"content": self._contenido}}]}


def test_devuelve_el_objeto_del_contenido(monkeypatch):
    capturado = {}

    def _post(url, json=None, timeout=None):
        capturado["payload"] = json
        return _Respuesta('{"corpus": ["nacional"], "consultas": ["deber de reportar"]}')

    monkeypatch.setattr(inference, "_get_chat_base", lambda: "http://127.0.0.1:9")
    monkeypatch.setattr(inference.httpx, "post", _post)

    salida = inference.completar_json([{"role": "user", "content": "x"}], {"type": "object"}, timeout=5.0)
    assert salida == {"corpus": ["nacional"], "consultas": ["deber de reportar"]}


def test_envia_el_esquema_en_response_format(monkeypatch):
    capturado = {}

    def _post(url, json=None, timeout=None):
        capturado["payload"] = json
        return _Respuesta("{}")

    monkeypatch.setattr(inference, "_get_chat_base", lambda: "http://127.0.0.1:9")
    monkeypatch.setattr(inference.httpx, "post", _post)

    inference.completar_json([{"role": "user", "content": "x"}], {"type": "object"}, timeout=5.0)
    formato = capturado["payload"]["response_format"]
    assert formato["type"] == "json_schema"
    assert formato["json_schema"]["schema"] == {"type": "object"}
    assert capturado["payload"]["temperature"] == 0
    assert capturado["payload"]["stream"] is False


def test_contenido_no_json_lanza(monkeypatch):
    monkeypatch.setattr(inference, "_get_chat_base", lambda: "http://127.0.0.1:9")
    monkeypatch.setattr(inference.httpx, "post", lambda url, json=None, timeout=None: _Respuesta("no soy json"))
    try:
        inference.completar_json([{"role": "user", "content": "x"}], {"type": "object"}, timeout=5.0)
    except json.JSONDecodeError:
        return
    raise AssertionError("debia lanzar JSONDecodeError")
```

- [ ] **Step 2: Run test to verify it fails**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_inference_json.py -v`
Expected: FAIL con `AttributeError: module 'inference' has no attribute 'completar_json'`

- [ ] **Step 3: Write minimal implementation**

En `inference.py`, junto a `stream_chat`:

```python
def completar_json(messages: list[dict], schema: dict, *, max_tokens: int = 256,
                   timeout: float) -> dict:
    base = _get_chat_base()
    payload = {
        "messages": messages,
        "stream": False,
        "temperature": 0,
        "max_tokens": max_tokens,
        "chat_template_kwargs": {"enable_thinking": False},
        "response_format": {
            "type": "json_schema",
            "json_schema": {"name": "plan", "strict": True, "schema": schema},
        },
    }
    r = httpx.post(f"{base}/v1/chat/completions", json=payload, timeout=timeout)
    r.raise_for_status()
    contenido = r.json()["choices"][0]["message"]["content"]
    return json.loads(contenido)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_inference_json.py -v`
Expected: PASS, 3 pruebas

- [ ] **Step 5: Verificar contra el servidor real**

Las pruebas anteriores son herméticas y no prueban que `llama-server` acepte ese
`response_format`. Con el backend levantado:

```bash
cd assistant/backend
../.venv/Scripts/python.exe -c "import inference, plan, json; print(json.dumps(inference.completar_json([{'role':'system','content':plan.instruccion([])},{'role':'user','content':'que obliga el articulo 9 de la Ley 21.663'}], plan.PLAN_SCHEMA, timeout=60.0), ensure_ascii=False))"
```

Expected: un objeto JSON con `corpus`, `consultas` y `articulo`. Anotar el tiempo que tardó:
es el número que fija `presupuestoMs` en la tarea 7.

Si el servidor rechaza `response_format`, la alternativa documentada de llama.cpp es el campo
de nivel superior `"json_schema": schema` en lugar del bloque `response_format`; se cambia
esa clave en `completar_json` y se repite este paso.

- [ ] **Step 6: Run the full suite**

Run: `..\.venv\Scripts\python.exe -m pytest`
Expected: PASS, 136 pruebas

- [ ] **Step 7: Commit**

```bash
git add assistant/backend/inference.py assistant/backend/tests/test_inference_json.py
git commit -m "feat(asistente): el modelo puede responder un objeto que no puede volver malformado"
git push origin main
```

---

### Task 6: El orquestador

**Files:**
- Create: `assistant/backend/orquestador.py`
- Create: `assistant/backend/tests/test_orquestador.py`

**Interfaces:**
- Consumes: `corpus.disponibles()`, `corpus.abrir()`, `plan.PLAN_SCHEMA`, `plan.validar()`, `plan.vacio()`, `plan.instruccion()`, `inference.completar_json()`, `inference.embed_query()`, `rag.buscar_en()`, `rag.fusionar()`, `rag.build_context()`, `rag.TOP_K`.
- Produces: `orquestador.resolver(consulta_recuperacion: str) -> tuple[str, list[dict]]`, con la misma forma que devuelve `rag.retrieve()`.

- [ ] **Step 1: Write the failing test**

`assistant/backend/tests/test_orquestador.py`:

```python
import asyncio

import orquestador
from corpus import Corpus


DOS_CORPUS = [
    Corpus("institucional", "la normativa propia", "/db_x"),
    Corpus("nacional", "la legislacion nacional", "/db"),
]


def _preparar(monkeypatch, plan_bruto, buscados=None, falla_plan=None):
    registro = {"consultas": []}

    monkeypatch.setattr(orquestador.corpus, "disponibles", lambda: DOS_CORPUS)
    monkeypatch.setattr(orquestador.corpus, "abrir", lambda c: f"tabla:{c.id}")
    monkeypatch.setattr(orquestador.inference, "embed_query", lambda q: [0.0])

    def _completar(messages, schema, *, max_tokens=256, timeout):
        if falla_plan is not None:
            raise falla_plan
        return plan_bruto

    monkeypatch.setattr(orquestador.inference, "completar_json", _completar)

    def _buscar(tabla, consulta, embedding, limite):
        registro["consultas"].append((tabla, consulta))
        return (buscados or {}).get(tabla, [])

    monkeypatch.setattr(orquestador.rag, "buscar_en", _buscar)
    monkeypatch.setattr(orquestador, "_config", lambda: {
        "activo": True, "turnosMaximos": 2, "presupuestoMs": 8000,
        "maxConsultas": 2, "corpusPorDefecto": ["institucional", "nacional"],
    })
    return registro


def test_consulta_los_corpus_que_pidio_el_plan(monkeypatch):
    registro = _preparar(monkeypatch, {"corpus": ["institucional"], "consultas": ["deber de reportar"]})
    asyncio.run(orquestador.resolver("que obliga el articulo 9"))
    assert [t for t, _ in registro["consultas"]] == ["tabla:institucional"]


def test_consulta_los_dos_cuando_el_plan_los_pide(monkeypatch):
    registro = _preparar(monkeypatch, {"corpus": ["institucional", "nacional"], "consultas": ["x"]})
    asyncio.run(orquestador.resolver("pregunta"))
    assert sorted(t for t, _ in registro["consultas"]) == ["tabla:institucional", "tabla:nacional"]


def test_un_plan_vacio_cae_al_camino_fijo(monkeypatch):
    registro = _preparar(monkeypatch, {"corpus": [], "consultas": []})
    asyncio.run(orquestador.resolver("pregunta original"))
    assert [c for _, c in registro["consultas"]] == ["pregunta original", "pregunta original"]


def test_una_falla_del_turno_del_plan_cae_al_camino_fijo(monkeypatch):
    registro = _preparar(monkeypatch, None, falla_plan=RuntimeError("servidor caido"))
    asyncio.run(orquestador.resolver("pregunta original"))
    assert [c for _, c in registro["consultas"]] == ["pregunta original", "pregunta original"]


def test_nunca_devuelve_cero_recuperacion(monkeypatch):
    _preparar(monkeypatch, {"corpus": [], "consultas": []},
              buscados={"tabla:nacional": [{"source": "ley.txt", "chunk_index": 0, "text": "t"}]})
    contexto, fragmentos = asyncio.run(orquestador.resolver("pregunta"))
    assert fragmentos != []
    assert contexto != ""


def test_el_interruptor_apagado_usa_el_camino_fijo(monkeypatch):
    registro = _preparar(monkeypatch, {"corpus": ["institucional"], "consultas": ["deber de reportar"]})
    monkeypatch.setattr(orquestador, "_config", lambda: {
        "activo": False, "turnosMaximos": 2, "presupuestoMs": 8000,
        "maxConsultas": 2, "corpusPorDefecto": ["institucional", "nacional"],
    })
    asyncio.run(orquestador.resolver("pregunta original"))
    assert [c for _, c in registro["consultas"]] == ["pregunta original", "pregunta original"]


def test_los_fragmentos_salen_etiquetados_con_su_corpus(monkeypatch):
    _preparar(monkeypatch, {"corpus": ["nacional"], "consultas": ["x"]},
              buscados={"tabla:nacional": [{"source": "ley.txt", "chunk_index": 0, "text": "t"}]})
    _, fragmentos = asyncio.run(orquestador.resolver("pregunta"))
    assert fragmentos[0]["corpus"] == "nacional"


def test_un_corpus_ilegible_no_impide_responder_con_el_otro(monkeypatch):
    _preparar(monkeypatch, {"corpus": ["institucional", "nacional"], "consultas": ["x"]},
              buscados={"tabla:nacional": [{"source": "ley.txt", "chunk_index": 0, "text": "t"}]})

    def _abrir(c):
        if c.id == "institucional":
            raise RuntimeError("tabla corrupta")
        return f"tabla:{c.id}"

    monkeypatch.setattr(orquestador.corpus, "abrir", _abrir)
    _, fragmentos = asyncio.run(orquestador.resolver("pregunta"))
    assert [f["corpus"] for f in fragmentos] == ["nacional"]


def test_turnos_maximos_bajo_dos_salta_el_turno_del_plan(monkeypatch):
    registro = _preparar(monkeypatch, {"corpus": ["institucional"], "consultas": ["deber de reportar"]})
    monkeypatch.setattr(orquestador, "_config", lambda: {
        "activo": True, "turnosMaximos": 1, "presupuestoMs": 8000,
        "maxConsultas": 2, "corpusPorDefecto": ["institucional", "nacional"],
    })
    asyncio.run(orquestador.resolver("pregunta original"))
    assert [c for _, c in registro["consultas"]] == ["pregunta original", "pregunta original"]
```

- [ ] **Step 2: Run test to verify it fails**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_orquestador.py -v`
Expected: FAIL con `ModuleNotFoundError: No module named 'orquestador'`

- [ ] **Step 3: Write minimal implementation**

`assistant/backend/orquestador.py`:

```python
import asyncio
import sys

import config_io
import corpus
import inference
import plan
import rag
from paths import config_path

POR_DEFECTO = {
    "activo": True,
    "turnosMaximos": 2,
    "presupuestoMs": 8000,
    "maxConsultas": 2,
    "corpusPorDefecto": ["institucional", "nacional"],
}


def _config() -> dict:
    bloque = config_io.leer_config(config_path()).get("orquestador")
    if not isinstance(bloque, dict):
        return dict(POR_DEFECTO)
    return {**POR_DEFECTO, **bloque}


def _aviso(motivo: str) -> None:
    print(f"[orquestador] camino fijo: {motivo}", file=sys.stderr, flush=True)


def _planificar(consulta: str, instalados: list, cfg: dict):
    mensajes = [
        {"role": "system", "content": plan.instruccion(instalados)},
        {"role": "user", "content": consulta},
    ]
    bruto = inference.completar_json(
        mensajes, plan.PLAN_SCHEMA, timeout=cfg["presupuestoMs"] / 1000.0
    )
    return plan.validar(bruto, {c.id for c in instalados}, cfg["maxConsultas"])


def _ejecutar(elegidos: list, consultas: list[str], embedding, limite: int):
    por_corpus = []
    for c in elegidos:
        try:
            tabla = corpus.abrir(c)
        except Exception as e:
            _aviso(f"corpus {c.id} ilegible ({e})")
            continue
        acumulado = []
        for consulta in consultas:
            acumulado += rag.buscar_en(tabla, consulta, embedding, limite)
        por_corpus.append((c.id, acumulado))
    if not por_corpus:
        raise RuntimeError("Ningun corpus instalado pudo abrirse.")
    return rag.fusionar(por_corpus, limite)


async def resolver(consulta_recuperacion: str) -> tuple[str, list[dict]]:
    cfg = _config()
    instalados = corpus.disponibles()
    if not instalados:
        raise RuntimeError("No hay ningun corpus instalado. Ejecute ingest.py.")

    elegido = None
    if cfg["activo"] and cfg["turnosMaximos"] >= 2:
        try:
            elegido = await asyncio.to_thread(_planificar, consulta_recuperacion, instalados, cfg)
        except Exception as e:
            _aviso(f"el turno del plan fallo ({e})")
            elegido = None
        if elegido is not None and plan.vacio(elegido):
            _aviso("el plan quedo vacio tras validar")
            elegido = None

    if elegido is None or not elegido.corpus:
        corpus_ids = set(cfg["corpusPorDefecto"])
        seleccionados = [c for c in instalados if c.id in corpus_ids] or instalados
        consultas = [consulta_recuperacion]
    else:
        seleccionados = [c for c in instalados if c.id in elegido.corpus]
        consultas = elegido.consultas or [consulta_recuperacion]

    embedding = await asyncio.to_thread(inference.embed_query, consultas[0])
    fragmentos = await asyncio.to_thread(
        _ejecutar, seleccionados, consultas, embedding, rag.TOP_K
    )
    return rag.build_context(fragmentos), fragmentos
```

- [ ] **Step 4: Run test to verify it passes**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_orquestador.py -v`
Expected: PASS, 9 pruebas

- [ ] **Step 5: Run the full suite**

Run: `..\.venv\Scripts\python.exe -m pytest`
Expected: PASS, 145 pruebas

- [ ] **Step 6: Commit**

```bash
git add assistant/backend/orquestador.py assistant/backend/tests/test_orquestador.py
git commit -m "feat(asistente): el modelo elige el corpus y la consulta, y toda falla vuelve al camino de siempre"
git push origin main
```

---

### Task 7: Conectarlo a `/chat` y a la configuración

**Files:**
- Modify: `assistant/backend/main.py:543-545` (la llamada a `retrieve`) y `main.py:572-575` (las citas)
- Modify: `assistant/config.example.json`
- Modify: `assistant/CLAUDE.md`
- Create: `assistant/backend/tests/test_chat_citas_corpus.py`

**Interfaces:**
- Consumes: `orquestador.resolver()`.

- [ ] **Step 1: Write the failing test**

`assistant/backend/tests/test_chat_citas_corpus.py`:

```python
import main


def test_las_citas_llevan_el_corpus():
    fragmentos = [
        {"corpus": "institucional", "source": "reglamento.pdf", "chunk_index": 2, "text": "t"},
        {"source": "ley.txt", "chunk_index": 0, "text": "t"},
    ]
    citas = main.armar_citas(fragmentos)
    assert citas[0] == {"source": "reglamento.pdf", "chunk_index": 2, "corpus": "institucional"}
    assert citas[1] == {"source": "ley.txt", "chunk_index": 0, "corpus": None}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_chat_citas_corpus.py -v`
Expected: FAIL con `AttributeError: module 'main' has no attribute 'armar_citas'`

- [ ] **Step 3: Write minimal implementation**

En `main.py`, agregar `import orquestador` y la función:

```python
def armar_citas(chunks: list[dict]) -> list[dict]:
    return [
        {
            "source": c.get("source", ""),
            "chunk_index": c.get("chunk_index", 0),
            "corpus": c.get("corpus"),
        }
        for c in chunks
    ]
```

Reemplazar la llamada de las líneas 543-545:

```python
    context, chunks = await orquestador.resolver(
        _retrieval_query(req.message, req.history, category_label)
    )
```

y el armado de citas de las líneas 572-575:

```python
    citations = armar_citas(chunks)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_chat_citas_corpus.py -v`
Expected: PASS, 1 prueba

- [ ] **Step 5: Agregar el bloque de configuración**

En `assistant/config.example.json`, después de `"models"`:

```json
  "orquestador": {
    "activo": true,
    "turnosMaximos": 2,
    "presupuestoMs": 8000,
    "maxConsultas": 2,
    "corpusPorDefecto": ["institucional", "nacional"]
  },
```

`presupuestoMs` se fija con el tiempo medido en el paso 5 de la tarea 5, redondeado hacia
arriba con holgura; el 8000 de arriba es el valor a reemplazar por esa medición.

- [ ] **Step 6: Documentar el módulo nuevo**

En `assistant/CLAUDE.md`, sección Architecture, insertar antes de la entrada de `rag.py`:

```markdown
**`orquestador.py`** — decides what to retrieve before retrieving it. One constrained
model turn produces a plan (`{corpus, consultas, articulo}`) forced by `json_schema`, the
plan is validated against the corpora actually installed (`corpus.py`), and each
`(corpus, consulta)` pair runs `rag.buscar_en`. Any failure - plan turn error, wall-clock
budget exhausted, plan emptied by validation - degrades to the fixed path: one hybrid
retrieval on the original query. There is no code path to an answer with zero retrieved
chunks. Settings live in `config.json`'s `orquestador` block, with `activo` as a kill
switch. See `docs/adr/0004-orquestador-de-recuperacion-del-asistente.md`.
```

y en la entrada de `POST /chat`, reemplazar "calls `rag.retrieve()`" por "calls
`orquestador.resolver()`, which may consult more than one corpus".

- [ ] **Step 7: Run the full suite**

Run: `..\.venv\Scripts\python.exe -m pytest`
Expected: PASS, 146 pruebas

- [ ] **Step 8: Probar la aplicación de punta a punta**

Levantar la GUI y hacer las tres preguntas del corpus del sector Defensa que ya se usaron
como control el 2026-08-03, más una que solo pueda responder el corpus nacional. Verificar en
las citas que cada respuesta declara de qué corpus salió, y anotar el tiempo de respuesta
para compararlo con la línea base de 5,8 a 10,7 s.

- [ ] **Step 9: Commit**

```bash
git add assistant/backend/main.py assistant/backend/tests/test_chat_citas_corpus.py assistant/config.example.json assistant/CLAUDE.md
git commit -m "feat(asistente): el chat pasa por el orquestador y cada cita declara su corpus"
git push origin main
```

---

### Task 8: Medirlo con el harness

Sin esto, que el modelo elija bien el corpus es una afirmación y no un dato.

**Files:**
- Modify: `assistant/backend/eval/golden_set.json`
- Modify: `assistant/backend/eval/eval_harness.py`
- Create: `assistant/backend/tests/test_eval_harness_corpus.py`

**Interfaces:**
- Consumes: `orquestador.resolver()`.
- Produces: `eval_harness.evaluate_one()` acepta una verdad de referencia con corpus.

- [ ] **Step 1: Write the failing test**

`assistant/backend/tests/test_eval_harness_corpus.py`:

```python
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "eval"))

import eval_harness


def test_acierta_solo_si_coincide_el_corpus():
    recuperados = [{"corpus": "nacional", "source": "ley_21663_ciberseguridad.txt"}]
    assert eval_harness.evaluate_one(
        ["ley_21663_ciberseguridad.txt"], recuperados, corpus_esperado="nacional"
    )["hit"] is True
    assert eval_harness.evaluate_one(
        ["ley_21663_ciberseguridad.txt"], recuperados, corpus_esperado="institucional"
    )["hit"] is False


def test_sin_corpus_esperado_se_comporta_como_antes():
    recuperados = [{"corpus": "nacional", "source": "ley_21663_ciberseguridad.txt"}]
    assert eval_harness.evaluate_one(["ley_21663_ciberseguridad.txt"], recuperados)["hit"] is True
```

- [ ] **Step 2: Run test to verify it fails**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_eval_harness_corpus.py -v`
Expected: FAIL con `TypeError: evaluate_one() got an unexpected keyword argument 'corpus_esperado'`

- [ ] **Step 3: Write minimal implementation**

En `eval/eval_harness.py`:

```python
def evaluate_one(ground_truth_sources, retrieved, corpus_esperado=None):
    truth = set(ground_truth_sources)
    if corpus_esperado is None:
        sources = [c.get("source", "") for c in retrieved]
    else:
        sources = [
            c.get("source", "") if c.get("corpus") == corpus_esperado else ""
            for c in retrieved
        ]
    first_rank = 0
    for i, s in enumerate(sources, 1):
        if s in truth:
            first_rank = i
            break
    hit = first_rank > 0
    precision = (sum(s in truth for s in sources) / len(sources)) if sources else 0.0
    return {"hit": hit, "first_rank": first_rank, "precision": precision}
```

y en `run()`, pasar la clave nueva del set dorado y usar el orquestador:

```python
    for q in questions:
        _, chunks = await orquestador.resolver(q["question"])
        scored = evaluate_one(
            q["ground_truth_sources"], chunks, q.get("corpus_esperado")
        )
```

con `import orquestador` reemplazando a `from rag import retrieve`.

- [ ] **Step 4: Run test to verify it passes**

Run: `..\.venv\Scripts\python.exe -m pytest tests/test_eval_harness_corpus.py -v`
Expected: PASS, 2 pruebas

- [ ] **Step 5: Agregar preguntas de corpus institucional al set dorado**

En `eval/golden_set.json`, agregar entradas cuya respuesta esté **solo** en el corpus
institucional, con esta forma:

```json
{"id": "c01", "question": "<pregunta de tema>", "corpus_esperado": "institucional", "ground_truth_sources": ["<archivo real de esa base>"]}
```

Y al menos una de control en sentido contrario, para que la métrica no premie elegir siempre
el institucional:

```json
{"id": "c02", "question": "<pregunta que solo responde la ley nacional>", "corpus_esperado": "nacional", "ground_truth_sources": ["ley_21663_ciberseguridad.txt"]}
```

Los nombres de archivo salen de listar la base institucional de este equipo, no de suponerlos:

```bash
cd assistant/backend
ls corpus_defensa corpus_muni/*
```

Se construyen igual que las existentes: la pregunta es de tema y la verdad de referencia es
el archivo real que lo trata, sin afirmar ningún contenido legal nuevo. **Estas entradas
necesitan la aprobación del dueño del repo antes de fijarse**, igual que el set original: el
campo `approved` del archivo lo declara.

- [ ] **Step 6: Correr el harness completo**

Run: `..\.venv\Scripts\python.exe eval/eval_harness.py`
Expected: recall@k, MRR y precisión impresos. Comparar contra la línea base previa al
orquestador y anotar ambas cifras en el commit.

- [ ] **Step 7: Comprobar el determinismo del plan**

Correr dos veces la misma pregunta y verificar que el plan es idéntico:

```bash
cd assistant/backend
for i in 1 2; do ../.venv/Scripts/python.exe -c "import inference, plan, corpus, json; print(json.dumps(inference.completar_json([{'role':'system','content':plan.instruccion(corpus.disponibles())},{'role':'user','content':'que obliga el articulo 9 de la Ley 21.663'}], plan.PLAN_SCHEMA, timeout=60.0), sort_keys=True, ensure_ascii=False))"; done
```

Expected: las dos líneas idénticas.

- [ ] **Step 8: Commit**

```bash
git add assistant/backend/eval/eval_harness.py assistant/backend/eval/golden_set.json assistant/backend/tests/test_eval_harness_corpus.py
git commit -m "test(asistente): la eleccion de corpus se mide con el harness y no se da por buena"
git push origin main
```

---

## Lo que este plan deja fuera a propósito

- **La búsqueda determinista por artículo.** El campo `articulo` del plan se valida y se
  transporta, pero no se ejecuta: depende del chunking consciente de estructura del Tramo A
  de 0.9.0, que es el que deja los metadatos de norma y artículo. Cuando exista, se conecta
  en `_ejecutar` y se le pasa a `plan.validar` el verificador `articulo_existe`.
- **La búsqueda web como herramienta del modelo.** Diferida por decisión del dueño mientras
  evalúa el servicio. El esquema del plan no la incluye.
- **Reformular y reintentar.** Fuera del alcance por la decisión 3 del ADR 0004: el
  presupuesto son dos turnos.
