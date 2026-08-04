# Diseño — Orquestador de recuperación del Asistente (workstream O)

Fecha: 2026-08-04. Estado: **diseño aprobado, previo a implementación.**

Decisiones tomadas por el dueño del repo el 2026-08-04, en sesión, una por una. Este
documento las registra y las desarrolla; no agrega ninguna que no se haya preguntado.

`ROADMAP.md` exige para este workstream su propio ADR y su propio pase de investigación
antes de escribir código. Este documento cubre el diseño; el ADR se redacta aparte, con las
decisiones de §2 como contenido.

---

## 1. El problema

Hoy `/chat` es una cadena invariable. `rag.retrieve()` se llama siempre, contra **una sola**
tabla, porque `rag.db_dir()` se resuelve una vez al arrancar el proceso. De ahí salen tres
limitaciones concretas:

- **El Asistente no puede contrastar dos cuerpos normativos en una misma respuesta.** No
  puede decir qué exige la ley nacional y qué agrega el reglamento propio del organismo,
  porque solo tiene una tabla abierta.
- **Por eso los corpus institucionales duplican documentos.** Al armar las bases del sector
  Defensa el 2026-08-03 hubo que copiar los documentos nacionales dentro de cada base
  institucional, que es la forma cara de resolver lo que debería ser una segunda consulta.
- **El modelo no decide nada.** No elige corpus, no elige consulta, no puede pedir un
  artículo concreto.

## 2. Decisiones tomadas

| # | Decisión | Alternativas descartadas |
|---|---|---|
| 1 | **Se diseña ahora y se implementa después del Tramo A de 0.9.0.** | Mantener el orden actual (0.8.5 antes que 0.9.0), plegarlo dentro de 0.9.0, o decidir la ubicación después. |
| 2 | **El orquestador elige corpus y le da herramientas al modelo.** Reformular y reintentar queda fuera. | Saltarse la recuperación; enrutador puramente determinista. |
| 3 | **Sin framework de agentes.** Un bucle propio sobre la API del `llama-server` que el producto ya empaqueta. | Strands Agents, Pydantic AI, LangGraph. |
| 4 | **El modelo elige corpus y consulta, pero la recuperación es obligatoria.** No existe camino a una respuesta con cero fragmentos recuperados. | Autonomía total, incluida la de no recuperar; herramienta de búsqueda web; devolverle al modelo cada llamada rechazada para que la corrija, que costaría un turno más. |
| 5 | **Corpus institucional y nacional, ambos abiertos.** | Descubrir N corpus en disco; una sola tabla con columna `corpus`. |
| 6 | **Dos turnos de modelo, más un presupuesto de reloj, ambos configurables.** | Cuatro turnos con reintento; sin tope de turnos; dejar el tope abierto. |
| 7 | **Plan JSON restringido por gramática**, no la API de `tools`. | API de `tools` de OpenAI; enrutador determinista; las dos con un interruptor. |

**Diferido, no descartado:** la búsqueda web como herramienta del modelo. El dueño la quiere
más adelante y está evaluando de qué servicio. El diseño deja el registro de herramientas
abierto para que agregarla sea una inscripción y no un rediseño; hoy no se inscribe.

### Por qué sin framework

Verificado contra los metadatos de PyPI, no contra resúmenes:

- **strands-agents 1.50.2**, Apache-2.0, tiene proveedor de primera clase para llama.cpp.
  Pero sus dependencias **requeridas** son 13, e incluyen `boto3`, `botocore` (14,79 MiB
  solo esa rueda), `mcp`, `opentelemetry-api`, `opentelemetry-sdk` y `watchdog`. Nada de eso
  sale a la red por su cuenta, pero el producto publica un SBOM a TI municipal y su promesa
  central es que nada institucional sale del equipo: un SDK de AWS y una pila de telemetría
  en ese SBOM es una conversación que hay que dar en cada despliegue. El Apéndice C del
  `ROADMAP.md` además obliga a espejar en `vendor/` cada biblioteca adoptada.
- **langgraph 1.2.10** exige `langchain-core >=1.4.7,<2`, y `requirements-eval.txt` fija la
  familia langchain en 0.3.x porque ragas 0.4.3 se rompe con 1.x. Mismo entorno, choque
  directo.
- **pydantic-ai-slim 2.23.0**, MIT, es el más liviano de los tres y `pydantic` ya está en el
  producto; aun así suma `opentelemetry-api`, `pydantic-graph`, `genai-prices` y `griffelib`.
- **El prerrequisito ya está puesto:** `inference.py:201` ya levanta el servidor de chat con
  `--jinja`, y `httpx` ya es dependencia. Para tres herramientas, un framework envolvería un
  bucle que cabe en una página.

### Por qué plan JSON y no la API de `tools`

La documentación de function calling de llama.cpp enumera las plantillas con manejo nativo
de llamadas a herramientas y **Qwen3 no aparece en esa lista**, de modo que la ruta `tools`
podría caer al camino genérico, que esa misma documentación describe como más caro en tokens
y menos eficiente. El plan restringido por `json_schema` no depende de eso, no puede volver
malformado, y es auditable: queda registrado exactamente qué pidió el modelo antes de que se
ejecute nada.

## 3. Alcance

**Dentro:** selección de corpus por el modelo, consulta elegida por el modelo, búsqueda
determinista por artículo, fusión entre corpus, presupuesto de turnos y de reloj, y la
corrección del BOM en `config.json` (§7).

**Fuera:** reformular y reintentar, saltarse la recuperación, búsqueda web como herramienta,
multi-agente, y cualquier cambio al empaquetado por cliente.

## 4. Arquitectura

Un módulo nuevo, `orquestador.py`. `/chat` deja de llamar a `rag.retrieve()` y llama a este.
El módulo hace tres cosas y ninguna más: producir un plan, validarlo, ejecutarlo.

```
/chat
 ├── desambiguación por palabras clave      (sin modelo, sin cambios)
 ├── orquestador.resolver(consulta, historial)
 │     ├── turno 1: plan restringido por json_schema
 │     ├── validación del plan
 │     └── ejecución: búsqueda por artículo + híbrida por (corpus, consulta)
 └── turno 2: respuesta  ->  citas.sin_respaldo()  ->  SSE
```

## 5. Componentes

### 5.1 `corpus_disponibles()`

Reemplaza la resolución única de `db_dir()` en el camino de recuperación. Devuelve los
corpus instalados como `{id, etiqueta, tabla}`:

| id | Origen | Etiqueta para el modelo |
|---|---|---|
| `institucional` | `db_<slug>` | La normativa propia del organismo |
| `nacional` | `db` | La legislación nacional |

Las tablas se abren una vez y quedan en caché, cada una verificada contra su
`embedding_meta.json` igual que hace hoy `rag.get_table()`, para que una base construida con
otro modelo de embeddings falle fuerte y no devuelva basura.

Casos degenerados, resueltos explícitamente y no por accidente:

- No existe `db_<slug>`: queda solo `nacional`, y el plan que pida `institucional` se corrige
  a `nacional` con registro en el log.
- Build sin marca, donde ambos resuelven a la misma carpeta: colapsan en una sola entrada.
- Una tabla ilegible: se sigue con la otra, y la respuesta declara con cuál respondió.

`db_dir()` se mantiene tal cual para `/ingest`, que sigue escribiendo en una sola base.

### 5.2 El plan

```json
{
  "corpus":    ["institucional", "nacional"],
  "consultas": ["deber de reportar incidentes al CSIRT"],
  "articulo":  { "norma": "Ley 21.663", "numero": 9 }
}
```

`articulo` puede ser `null`. `consultas` tiene tope (`maxConsultas`). El turno que lo produce
va con `json_schema`, `temperature` 0 y `max_tokens` chico: su salida es un objeto, no prosa,
así que el costo lo domina el procesamiento del prompt y no la generación.

**La validación no es un trámite.** Se descarta y se registra: un `corpus` que no está
instalado, un `numero` de artículo fuera del rango de esa norma según los metadatos que deja
la ingesta estructurada de 0.9.0, y un `consultas` vacío. **No hay turno de reparación**: el
presupuesto son dos turnos, y un plan que la validación deja vacío cae al camino fijo.

### 5.3 Ejecución

1. Si hay `articulo`, corre primero la búsqueda determinista por metadato — el ítem de 0.9.0
   que convierte "artículo 9 de la Ley 21.663" en un filtro y no en una apuesta de similitud.
2. Cada par `(corpus, consulta)` corre la recuperación híbrida existente, ya con fusión RRF
   real y el índice BM-25 en español del Tramo A.
3. Los candidatos se juntan, se deduplican por `(corpus, source, chunk_index)` y se cortan a
   `TOP_K` — o pasan al reranker, si el spike de 0.9.0 termina adoptando uno.

### 5.4 Armado del contexto

`rag.build_context` mantiene el spotlighting sin cambios. Lo único que cambia es que la
etiqueta de fuente lleva su corpus: `[Fuente: reglamento_x.pdf — corpus institucional]`. Hoy
el producto declara una vez por sesión cuándo cayó al corpus nacional; esto lo vuelve cierto
fragmento por fragmento, y el evento `citations` del SSE lleva el mismo dato.

## 6. Flujo por petición

1. El atajo de desambiguación por palabras clave sigue primero y sigue sin modelo.
2. Turno 1: el plan, con `json_schema`, el tope de tokens y el plazo de reloj.
3. Validación. Si no queda nada usable, camino fijo.
4. Ejecución de la recuperación según el plan.
5. Turno 2: la respuesta, sin cambios respecto de hoy, salvo que el contexto puede abarcar
   dos corpus.
6. `citas.py` corre sobre la respuesta completa antes de que salga un solo token, igual que
   hoy.

**No cambia:** el contrato SSE, la sanitización en tiempo de índice, el spotlighting, las
fichas de desambiguación ni la píldora de búsqueda web.

### Las dos garantías quedan fuera del alcance del modelo

- **Recuperación obligatoria.** Si el plan no deja consulta usable, corre igual la consulta
  por defecto. No existe camino a una respuesta con cero fragmentos.
- **Verificación de citas.** `citas.py` es posterior a la generación, así que no le importa
  cómo se eligió el contexto.

## 7. Manejo de errores

Todo falla hacia el comportamiento de hoy, no hacia un error:

| Falla | Qué hace |
|---|---|
| El turno del plan devuelve error | Camino fijo: una recuperación híbrida y respuesta |
| El turno del plan excede `presupuestoMs` | Se cancela, camino fijo |
| El plan queda vacío tras validar | Camino fijo |
| Un corpus ilegible | Responde con el otro, y lo declara |
| Los dos corpus ilegibles | Se mantiene la falla ruidosa actual de `rag.get_table()` |
| Falla el turno de respuesta | El evento `error` del SSE que ya existe |

Ninguna es silenciosa: cada una escribe una línea de log, con el mismo formato que el
`[citas]` que ya existe.

## 8. Un defecto previo que este diseño corrige

`config.json` se lee con `encoding="utf-8"` en cinco lugares, y `rag._config_municipio`
envuelve su lectura en `except Exception: return None`. Medido:

```
utf-8 con BOM: json.loads FALLA -> JSONDecodeError Unexpected UTF-8 BOM (decode using utf-8-sig)
```

Consecuencia: si alguien edita `config.json` con el Bloc de notas o con PowerShell —que en
Windows escriben UTF-8 con BOM por defecto—, el municipio se resuelve a `None` **en silencio**
y el Asistente responde desde el corpus nacional en vez del institucional. Es exactamente la
falla que la regla del lado Rust (`config::sin_bom`) existe para evitar; el lado Python nunca
la recibió. `main.py:651` está peor: lee sin `encoding`, así que toma la codificación por
defecto de la plataforma.

Corregirlo es precondición de un diseño cuyo tema es elegir el corpus correcto. Todas las
lecturas de `config.json` pasan a `utf-8-sig`, y la captura ancha de `_config_municipio` deja
de tragarse el error sin avisar.

## 9. Configuración

Bloque nuevo `orquestador` en el `config.json` del Asistente, con **todas** las claves con
valor por defecto, para que un archivo antiguo siga cargando:

| Clave | Qué controla |
|---|---|
| `activo` | Interruptor de corte: en `false` vuelve al camino fijo de hoy |
| `turnosMaximos` | Tope de turnos de modelo |
| `presupuestoMs` | Plazo de reloj del turno del plan |
| `maxConsultas` | Tope de consultas por plan |
| `corpusPorDefecto` | Lista de ids de corpus que se consultan cuando el plan no deja ninguno, y en el camino fijo. Por defecto los dos instalados |

`turnosMaximos` queda en 2 por la decisión 6. Los dos valores numéricos restantes
(`presupuestoMs` y `maxConsultas`) **quedan en blanco en este documento** y se fijan con la
medición del spike, no a ojo.

## 10. Pruebas y medición

**Unitarias**, sobre las partes puras, agregadas a las 110 que hoy pasan:

- La validación del plan descarta corpus no instalados y artículos fuera de rango.
- La fusión entre corpus deduplica por `(corpus, source, chunk_index)`.
- `corpus_disponibles()` resuelve las tres formas: los dos corpus, solo el nacional, y la
  carpeta compartida del build sin marca.
- Las lecturas de `config.json` toleran BOM.

**Harness.** Entradas nuevas en el set dorado cuya verdad de referencia vive en el corpus
institucional, de modo que la elección de corpus se **mida** en vez de afirmarse. Más un
control de determinismo: la misma pregunta produce el mismo plan a temperatura 0.

**Spike, antes de escribir el orquestador.** Mide en este hardware el costo del turno del
plan con los dos modelos de chat, y de paso comprueba si Qwen3 produce llamadas a
herramientas fiables por la ruta de plantillas de llama.cpp —lo que decidiría si la decisión
7 puede revisarse algún día—. Los valores por defecto de §9 salen de ahí.

## 11. Lo que este diseño no verificó

1. **El costo real del turno del plan** en CPU. Es el número que decide si esto se puede
   mostrar en vivo, y no está medido.
2. **Si Qwen3 hace llamadas a herramientas fiables** por la ruta nativa de llama.cpp. La
   decisión 7 evita depender de ello, pero la pregunta queda abierta.
3. **Cuánto cuesta tener dos tablas abiertas** en memoria en un PC municipal.
4. **Si el modelo elige bien el corpus.** Es precisamente lo que el harness tiene que medir
   antes de dar el hito por bueno.

## 12. Dependencia con 0.9.0

Este diseño supone entregado el Tramo A de 0.9.0. En concreto necesita: la fusión RRF real,
el índice BM-25 en español, y sobre todo el **chunking consciente de estructura**, porque de
ahí salen los metadatos de norma y artículo sin los cuales no hay búsqueda determinista por
artículo (§5.3) ni validación de rango (§5.2).
