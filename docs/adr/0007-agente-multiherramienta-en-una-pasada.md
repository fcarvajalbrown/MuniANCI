# 0007. El Asistente elige entre cuatro herramientas en una sola pasada, sin bucle

**Status:** Aceptado. Supersedes 0004
**Date:** 2026-08-12
**Deciders:** Felipe Carvajal Brown

## Context

El ADR 0004 fijó un planificador de consultas: el modelo elegía corpus y consulta, y la única
acción disponible era recuperar. Al revisarlo el 2026-08-12, antes de implementarlo, quedó claro
que un orquestador con una sola acción no es un orquestador. Su valor real era arquitectónico —
que las bases institucionales dejaran de duplicar la ley nacional — y no agéntico: con dos
corpus, elegir entre dos no necesita un modelo.

La decisión fue entonces construir un agente con herramientas de verdad, y se eligieron cuatro:
catálogo de normas, artículo por norma y número, búsqueda web, y estado de cumplimiento del
propio organismo.

La investigación previa (`docs/research/0.9.5-agente-multiherramienta.md`) midió lo que eso
exige y encontró tres cosas que la decisión tiene que absorber:

- **Qwen3-1.7B obtiene 11 puntos en multi-turno y 82,92 en selección de un solo turno** —74,61
  en el ajuste live, que es el difícil— según el Berkeley Function Calling Leaderboard V4,
  verificado el 2026-08-12 leyendo la página renderizada, que es la única vía: el fetch devuelve
  la prosa sin la tabla. Es el modelo que `config.json` fuerza hoy en `chatDefault` y
  `chatLowRam`, por decisión propia, para poder cambiarlo con el Bloc de notas en el lugar de la
  demostración. Un bucle agéntico clásico cae justo en la métrica donde ese modelo rinde 7,5
  veces peor.
- **El modo de invocación casi no cambia el resultado.** En el Qwen3-4B, las filas Function
  Calling y Prompt quedan a 0,16 puntos en general y a 1,7 en el live. Evitar la API nativa de
  `tools` no cuesta prácticamente nada.
- **La búsqueda web es la peor columna de la tabla para estos modelos:** 2,5 en el 1.7B, entre 3
  y 4,5 en el 4B.
- **Cuatro herramientas no degradan la elección.** La pérdida por tamaño de catálogo aparece
  entre 10 y 15 herramientas. Este eje no es un riesgo.
- **El estado de cumplimiento no necesita cableado nuevo entre Rust y el sidecar.** El histórico
  ya es un SQLite escrito junto al ejecutable, y `historico::slug()` usa el mismo criterio que el
  `db_<comuna>` del Asistente. Python lo lee con la biblioteca estándar. Pero la GUI **no
  registra ningún escaneo**: solo la CLI llama a `historico::registrar`.

## Decision

**El modelo elige herramientas una sola vez, en un turno, y puede elegir varias a la vez.** El
código las ejecuta, y un segundo turno responde con lo que volvió. Dos turnos en total, el mismo
presupuesto que fijaba el ADR 0004.

1. **La selección es una pasada, no un bucle.** El modelo compromete sus herramientas y sus
   argumentos antes de ver ningún resultado. Lo que se pierde es reaccionar a un resultado; lo
   que se gana es operar en la métrica de un solo turno, donde el modelo empaquetado rinde 82,92
   en vez de 11.
2. **Cuatro herramientas, más la recuperación, que sigue siendo obligatoria.** No existe camino
   de código que llegue a una respuesta con cero fragmentos recuperados: si la selección no deja
   nada usable, corre la recuperación por defecto.
3. **La selección se expresa como un objeto restringido por `json_schema`**, no por la API de
   `tools`. Las cifras de BFCL citadas son del modo Prompt, así que esta ruta no paga
   penalización por evitar la API nativa, y sigue valiendo que Qwen3 no figura en la lista de
   plantillas con manejo nativo de llamadas a herramientas.
4. **`articulo` no se acepta sin verificador.** `plan.validar` recibe el `articulo_existe` que
   hoy queda sin usar: un número de artículo inventado se descarta antes de recuperar nada. Sin
   ese verificador conectado, la herramienta no entra.
5. **El modelo puede pedir la búsqueda web, nunca habilitarla.** Si `webSearchEnabled` está
   apagada, la petición se rechaza y se anota, igual que un corpus no instalado. Quien autoriza
   que algo salga del equipo sigue siendo el usuario. La medición lo respalda además de la
   promesa comercial: la búsqueda web es la columna donde estos modelos rinden peor de toda la
   tabla, con 2,5 puntos en el 1.7B.
6. **`estado_cumplimiento` lee el histórico en solo lectura**, por `sqlite3` de la biblioteca
   estándar, sin endpoint nuevo y sin dependencia nueva.
7. **Sin framework de agentes.** Un bucle propio sobre el `llama-server` que el producto ya
   empaqueta. Lo verificado en el ADR 0004 contra metadatos de PyPI no cambió.

## Consequences

- **La GUI pasa a registrar el histórico**, como ya hace la CLI. Es requisito de la herramienta
  4, y arregla por sí solo un vacío del producto: hoy el delta y la deriva no funcionan desde la
  interfaz gráfica, que es donde los usa un municipio.
- **`rag.buscar_en` tiene que alcanzar la paridad con `retrieve()`** — fusión RRF y ruta de
  artículo por corpus — antes de medir nada. El plan del 2026-08-04 es anterior al Tramo A de
  0.9.0, y montar el agente sobre la versión vieja de esa función haría que el hito midiera peor
  que el anterior por una razón que no tiene que ver con el agente.
- **Una pregunta normal sigue costando un turno de modelo más**, sobre la línea base medida de
  5,8 a 10,7 s con el 1.7B y 27,1 s con el 4B. El tope de turnos y el presupuesto de reloj
  siguen siendo configuración, no constantes.
- **El agente no puede corregirse.** Si elige mal las herramientas, responde con lo que esa
  elección trajo. La red de seguridad no es un reintento: es que la recuperación siempre corre.
- **Toda falla degrada al camino de hoy, no a un error.** Selección que falla, que excede el
  plazo o que la validación deja vacía caen a una recuperación híbrida y la respuesta normal.
  Ninguna en silencio: cada una escribe una línea de log.
- **`citas.py` no cambia y sigue siendo la última línea.** Corre sobre la respuesta completa
  antes de que salga un token.
- **Las citas declaran su origen**, y ahora eso incluye distinguir una fuente legal de un
  resultado web, que es texto no confiable y tiene que pasar por `clean_for_context`.
- **Aparece un interruptor de corte** para volver al camino fijo sin reinstalar.
- **La demostración necesita un escaneo previo.** En un equipo recién instalado no hay
  mediciones, así que `estado_cumplimiento` responde que todavía no hay ninguna. Es correcto y
  no es lo que se quiere mostrar.

## Alternatives considered

- **Mantener el ADR 0004 tal cual**, con el planificador de una sola acción. Es lo que ya estaba
  diseñado, con plan y tareas escritas. Se descartó porque un orquestador con una herramienta no
  justifica el turno de modelo que cuesta: con dos corpus, buscar en ambos y fusionar da el mismo
  resultado visible sin latencia extra.
- **Multi-corpus determinista, sin turno de modelo.** Buscar siempre en las dos bases y fusionar.
  Entrega el beneficio visible — la ordenanza propia y la ley nacional en una respuesta — con
  cero latencia agregada y ningún modo de falla nuevo. Se descartó porque el objetivo pasó a ser
  que el modelo elija entre herramientas, no solo entre corpus.
- **Bucle agéntico de N turnos con tope.** Estrictamente más capaz: permite consultar el
  catálogo, descubrir que la ordenanza no está y cambiar a búsqueda web. Se descartó por los 11
  puntos de multi-turno del modelo que se demuestra, contra 82,92 en un solo turno, y porque
  cada turno adicional son 9,0 s con el 1.7B o 27,1 s con el 4B, y el peor caso es la pregunta
  que alguien hace en una demostración.
- **Una pasada más un único turno correctivo.** Recupera el comportamiento de bucle más valioso
  —reaccionar a un resultado vacío— acotado a tres turnos en el peor caso. Se descartó por
  latencia: el turno correctivo se paga justo cuando algo ya salió mal, que es cuando menos
  tiempo hay.
- **Cálculo de plazos como quinta herramienta.** Los plazos de reporte son interpretación legal,
  y el producto se vende sobre no afirmar ninguna que no se pueda trazar a un artículo.
- **Exponer el estado de cumplimiento por un endpoint nuevo de Tauri a sidecar.** Más limpio en
  papel. Se descartó porque el histórico ya está en disco en una ruta determinista y con el mismo
  slug en los dos módulos, así que el endpoint sería cableado nuevo para llegar a un archivo que
  Python abre con la biblioteca estándar.
