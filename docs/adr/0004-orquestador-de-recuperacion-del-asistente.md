# 0004. El Asistente decide qué recuperar con un plan restringido, y sin framework de agentes

**Status:** Superseded by 0007
**Date:** 2026-08-04
**Deciders:** Felipe Carvajal Brown

## Context

`/chat` es hoy una cadena invariable. `rag.retrieve()` se llama siempre y consulta **una
sola** tabla, porque `rag.db_dir()` se resuelve una vez al arrancar el proceso. De ahí
salen tres consecuencias que ya se pagaron:

- El Asistente no puede contrastar en una misma respuesta lo que exige la ley nacional con
  lo que agrega la normativa propia del organismo.
- Por eso, al armar los corpus del sector Defensa el 2026-08-03, hubo que **duplicar** los
  documentos nacionales dentro de cada base institucional. Es la forma cara de resolver lo
  que debería ser una segunda consulta.
- El modelo no decide nada: ni el corpus, ni la consulta, ni puede pedir un artículo
  concreto.

El `ROADMAP.md` asigna este workstream (O) al hito 0.8.5 y exige para él un ADR propio y un
pase de investigación antes de escribir código. La investigación previa de 0.9.0
(`docs/research/0.9.0-calidad-del-asistente-rag.md`) midió además que la búsqueda híbrida
descarta hoy el 100% del lado BM-25 y que el articulado se parte de modo que el fragmento
que responde por el artículo 9 no contiene la cadena "artículo 9". Un orquestador montado
sobre ese recuperador heredaría cada uno de esos fallos y los volvería más difíciles de
atribuir.

## Decision

**El orquestador se diseña ahora y se implementa después del Tramo A de 0.9.0.** Consume
tres entregables de ese tramo: la fusión RRF real, el índice BM-25 en español y el chunking
consciente de estructura, del que salen los metadatos de norma y artículo.

**El modelo elige, dentro de límites que no puede mover:**

1. **Elige corpus y consulta**, con dos corpus abiertos a la vez: `institucional`
   (`db_<slug>`) y `nacional` (`db`).
2. **La recuperación es obligatoria.** No existe camino de código que llegue a una
   respuesta con cero fragmentos recuperados: si el plan no deja consulta usable, corre la
   consulta por defecto.
3. **Dos turnos de modelo**, más un presupuesto de reloj, ambos configurables. Reformular y
   reintentar queda fuera.
4. **La decisión se expresa como un plan JSON restringido por `json_schema`**, no por la API
   de `tools`.
5. **Sin framework de agentes.** Un bucle propio sobre el `llama-server` que el producto ya
   empaqueta.

La búsqueda web como herramienta del modelo queda **diferida, no descartada**: el registro
de herramientas se deja abierto para que agregarla sea una inscripción y no un rediseño, y
hoy no se inscribe.

El diseño desarrollado vive en `docs/design/2026-08-04-orquestador-de-recuperacion.md`.

### Por qué sin framework

Verificado contra los metadatos de PyPI, no contra resúmenes de buscador:

- **strands-agents 1.50.2** (Apache-2.0) tiene proveedor de primera clase para llama.cpp,
  pero sus dependencias **requeridas** son 13 e incluyen `boto3`, `botocore` (14,79 MiB solo
  esa rueda), `mcp`, `opentelemetry-api`, `opentelemetry-sdk` y `watchdog`. Nada de eso sale
  a la red por su cuenta, pero este producto publica un SBOM a TI municipal y su promesa
  central es que nada institucional sale del equipo. Un SDK de AWS y una pila de telemetría
  dentro de ese SBOM es una conversación que habría que dar en cada despliegue. El Apéndice
  C del `ROADMAP.md` obliga además a espejar en `vendor/` cada biblioteca adoptada.
- **langgraph 1.2.10** exige `langchain-core >=1.4.7,<2`, y `requirements-eval.txt` fija la
  familia langchain en 0.3.x porque ragas 0.4.3 se rompe con 1.x. Mismo entorno, choque
  directo.
- **pydantic-ai-slim 2.23.0** (MIT) es el más liviano de los tres y `pydantic` ya está en el
  producto, pero aun así suma `opentelemetry-api`, `pydantic-graph`, `genai-prices` y
  `griffelib`.

Y el prerrequisito ya está puesto: `inference.py:201` levanta el servidor de chat con
`--jinja`, y `httpx` ya es dependencia. Para tres herramientas, un framework envolvería un
bucle que cabe en una página.

### Por qué un plan restringido y no la API de `tools`

La documentación de function calling de llama.cpp enumera las plantillas con manejo nativo
de llamadas a herramientas y **Qwen3 no aparece en esa lista**, de modo que esa ruta podría
caer al camino genérico, que la misma documentación describe como más caro en tokens y menos
eficiente. Un plan restringido por `json_schema` no depende de eso, no puede volver
malformado, y queda registrado antes de que se ejecute nada: en un producto cuyo argumento
de venta es poder mostrar por qué dijo lo que dijo, que la decisión del modelo sea auditable
no es un detalle.

## Consequences

- **Los corpus institucionales dejan de necesitar documentos duplicados.** La base del
  organismo puede contener solo su normativa propia, y la nacional se consulta aparte.
- **Una pregunta normal cuesta un turno de modelo más.** Sobre la línea base medida —5,8 a
  10,7 s con el 1.7B y 27,1 s con el 4B— eso se nota, y por eso el tope de turnos y el
  presupuesto de reloj son configuración y no constantes. Los dos valores numéricos por
  defecto se fijan con la medición del spike, no a ojo.
- **Toda falla degrada al camino de hoy, no a un error**: plan que falla, que excede el
  plazo o que la validación deja vacío caen a una recuperación híbrida y la respuesta
  normal. Ninguna en silencio: cada una escribe una línea de log.
- **`citas.py` no cambia y sigue siendo la última línea.** Corre sobre la respuesta completa
  antes de que salga un token, y no le importa cómo se eligió el contexto.
- **La cita declara su corpus.** El evento `citations` y la etiqueta de fuente pasan a decir
  de qué base salió cada fragmento, en vez de declararlo una vez por sesión.
- **Aparece un interruptor de corte** (`orquestador.activo`), para volver al camino fijo sin
  reinstalar.
- **Hay que corregir antes un defecto medido:** `config.json` se lee como `utf-8` y el BOM
  que escriben el Bloc de notas y PowerShell hace fallar `json.loads` dentro de un `except`
  que devuelve `None`, así que el municipio se pierde **en silencio** y el Asistente
  responde desde el corpus nacional. Es la misma falla que la regla `config::sin_bom` evita
  del lado Rust, y el lado Python nunca la recibió.
- **El `ROADMAP.md` queda inconsistente hasta que se reordene.** Con la numeración actual
  0.8.5 va antes que 0.9.0, o sea el orquestador quedaría montado sobre el recuperador sin
  arreglar, que es justo lo que esta decisión evita.
- **La ruta de `tools` no queda cerrada.** Si el spike muestra que Qwen3 hace llamadas
  fiables por la plantilla nativa, cambiar el productor del plan es sustituir un componente,
  no rehacer el orquestador. Ese cambio, si ocurre, es un ADR nuevo.

## Alternatives considered

- **Mantener el orden del roadmap y construirlo en 0.8.5, antes de 0.9.0.** Es el camino más
  corto a las respuestas multi-corpus, que es lo que duele hoy. Se descartó porque el
  orquestador enruta hacia un recuperador con defectos medidos, y entonces una mala
  respuesta no se puede atribuir al enrutador o al recuperador sin desarmar los dos.
- **Plegar el orquestador dentro de 0.9.0.** Coherente como unidad, y el harness mediría
  ambos a la vez. Se descartó porque 0.9.0 ya quedó con los tres tramos completos y esto lo
  duplicaba.
- **Adoptar Strands Agents.** Es la opción con más funcionalidad por unidad de trabajo y con
  soporte explícito para llama.cpp. Se descartó por sus dependencias requeridas, detalladas
  arriba: el costo no es de megabytes, es de superficie declarada en el SBOM de un producto
  que se vende como offline.
- **Adoptar LangGraph o Pydantic AI.** LangGraph choca con el pin de langchain que necesita
  ragas. Pydantic AI es viable y liviano; se descartó porque para tres herramientas el bucle
  propio es más corto que la integración, y no agrega dependencias a un sidecar que ya pesa
  928,6 MB.
- **Enrutador determinista, con el modelo solo cuando las reglas empatan.** Latencia casi
  nula y totalmente reproducible para el harness. Se descartó porque no cumple lo que se
  buscaba: que el modelo elija corpus y consulta.
- **Autonomía total, incluida la de no recuperar.** Ahorra una incrustación y dos búsquedas
  en un saludo. Se descartó porque con cero fragmentos recuperados una afirmación que no
  lleve número de artículo pasa intacta por `citas.py`, y ese es el primo del incidente del
  2026-08-03.
- **Cuatro turnos, para permitir reformular y reintentar.** Ganancia real en preguntas
  vagas. Se descartó porque el peor caso son cuatro veces la latencia de hoy, y el peor caso
  es exactamente lo que pregunta alguien en una demostración.
- **Descubrir N corpus en disco**, o **una sola tabla con columna `corpus`**. La primera
  obliga al modelo a elegir bien entre nombres que no conoce; la segunda es la recuperación
  más limpia, pero cambia el empaquetado por cliente y haría que cada instalador cargue todos
  los corpus.
- **Devolverle al modelo cada llamada rechazada para que la corrija.** Más seguro sobre el
  papel. Se descartó porque cuesta un turno más, y el presupuesto son dos.
