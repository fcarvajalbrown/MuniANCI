# 0008. La búsqueda web abre el navegador, y el modo lo elige TI

**Status:** Aceptado
**Date:** 2026-08-12
**Deciders:** Felipe Carvajal Brown

## Context

La búsqueda web del Asistente funciona hoy por dentro del producto: la píldora "Búsqueda web"
llama a `POST /search`, que consulta DuckDuckGo con **DDGS**, un cliente no oficial y sin llave.
Está gobernada por la bandera `webSearchEnabled` de `config.json`, apagada por defecto, y cada
consulta saliente deja una línea `{timestamp, query, resultCount}` en
`backend/logs/search_audit.log` por el requisito FR-07.

Ese camino arrastra dos problemas que no son de código. DDGS puede ser estrangulado (202/403) y
los términos de servicio de DuckDuckGo desaconsejan el uso automatizado, de modo que una consulta
puede volver vacía por razones ajenas al producto, incluso delante de una autoridad municipal. Y
un resultado web entra al contexto como texto no confiable: `sanitize.py` protege la ruta RAG con
saneamiento en índice, y un resultado web no pasa por ahí.

**No hay llave de ningún servicio de búsqueda.** Esa es la restricción que ordena esta decisión, y
es temporal por naturaleza: se levanta el día que exista una.

La investigación de 0.9.5 (`docs/research/0.9.5-agente-multiherramienta.md` §3.3) agregó la razón
de medición: la búsqueda web es la peor columna de la tabla agéntica para los modelos que el
producto empaqueta, con 2,5 puntos en el 1.7B y entre 3 y 4,5 en el 4B, contra 82,92 y 87,88 en
selección de un turno. De las cuatro herramientas del ADR 0007, es aquella en la que menos hay que
confiar en el criterio del modelo.

## Decision

1. **La píldora abre DuckDuckGo en el navegador del sistema, con la consulta ya escrita.** Es una
   medida temporal mientras no exista llave de un servicio de búsqueda, y el ADR la declara como
   tal en vez de presentarla como el diseño definitivo.
2. **Los dos modos conviven y el área de TI elige cuál rige.** La búsqueda dentro de la aplicación
   con DDGS no se elimina. El modo es un valor de configuración, junto a `webSearchEnabled` en el
   `config.json` del Asistente, que es donde ya vive la bandera que gobierna toda esta superficie.
3. **La consulta entregada al navegador se registra en `search_audit.log`**, igual que una consulta
   hecha por `POST /search`. Lo que salió del equipo no cambia porque el pedido lo haga el
   navegador en vez de nuestro proceso, y un registro con un vacío silencioso es peor que uno que
   declara que la búsqueda ocurrió en otra parte. La línea del modo navegador no lleva conteo de
   resultados: el producto nunca los ve.
4. **`busqueda_web` sigue en el diseño del ADR 0007 y no se registra como herramienta del modelo**
   mientras no exista esa llave. El agente de 0.9.5 se implementa con tres herramientas
   registradas más la recuperación, que sigue siendo obligatoria.
5. **Este ADR no supera al 0007.** No revierte ninguno de sus puntos: el 5 ya subordinaba
   `busqueda_web` a `webSearchEnabled` y prohibía al modelo encenderla. No registrar la herramienta
   mientras no hay servicio es esa misma regla en su límite. El diseño de cuatro herramientas sigue
   en pie; lo que cambia es cuándo se conecta la cuarta.

## Consequences

- **El modelo deja de ver resultados web por este camino.** Los ve la persona, en su navegador. Con
  eso desaparece por ahora el riesgo de que texto no confiable entre al contexto sin pasar por
  `clean_for_context`, y `citas.py` no se ve afectado: el modo navegador no aporta ninguna cita.
- **Y por lo mismo, el modo navegador no mejora ninguna respuesta.** Es una ayuda de consulta para
  el funcionario, no una fuente del Asistente. La documentación tiene que decirlo con esas
  palabras, porque una píldora dentro del chat sugiere lo contrario.
- **La consulta sigue saliendo del equipo.** El modo navegador no es una medida de privacidad: es
  el usuario quien la despacha y quien la ve. La promesa del producto se mantiene donde siempre
  estuvo, en que nada institucional sale, y la bandera apagada por defecto no se toca.
- **Dos modos son dos superficies que probar y documentar.** El manual de TI tiene que describir
  qué hace cada uno y cuál viene por defecto.
- **El formato de `search_audit.log` gana una línea sin `resultCount`.** Cualquier lector de ese
  archivo tiene que tolerar el campo ausente en vez de suponerlo.
- **Abrir el navegador ocurre en Rust, no en el webview.** `gui/capabilities/default.json` deja
  fuera del ACL del webview las APIs JS de `shell` y `dialog` a propósito, y el guardado con
  diálogo nativo ya vive en `export_report.rs`. La apertura sigue ese mismo camino y no reabre esa
  superficie.
- **Puede fallar, y tiene que decirlo.** Un equipo sin navegador por defecto, o con una política que
  bloquea la apertura, deja la píldora sin efecto. Ese caso avisa en la interfaz en vez de no hacer
  nada.
- **La condición de salida queda nombrada:** el día que exista llave de un servicio de búsqueda,
  vuelve a decidirse esta superficie, y con ella el registro de `busqueda_web` como herramienta.

## Alternatives considered

- **Eliminar `POST /search` y la dependencia DDGS.** Es lo más limpio que se le puede decir a una
  municipalidad: ningún camino de código hace una petición saliente desde dentro del producto. Se
  descartó porque bota una implementación que funciona y que es la referencia para el día que
  `busqueda_web` se registre, y porque le quita la elección a un área de TI que puede preferir los
  resultados dentro de la aplicación.
- **Conservar el endpoint apagado, sin que la interfaz lo llame.** Más barato que un modo
  configurable. Se descartó porque deja código muerto que igual arrastra la dependencia y no le
  sirve a nadie.
- **Contratar ahora un servicio de búsqueda con llave.** Es la salida de fondo y quedará por
  decidir cuando exista. Hoy no hay llave, y elegir proveedor es una decisión de costo antes que de
  arquitectura, así que este ADR no nombra ninguno.
- **No registrar la consulta del modo navegador.** Sostiene que, una vez entregada al navegador, el
  producto no es quien busca y no puede saber si la persona la ejecutó, la editó o cerró la
  pestaña. Se descartó porque FR-07 existe para que la institución pueda mostrar qué salió del
  equipo, y esa pregunta no depende de quién despachó el pedido.
- **Superar el ADR 0007.** Se descartó porque ninguno de sus puntos se revierte y los otros seis
  siguen vigentes; superarlo obligaría además a un 0009 solo para restituir el diseño de cuatro
  herramientas cuando exista la llave.
