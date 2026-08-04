# Registro de decisiones de arquitectura (ADR)

Una decisión significativa por archivo, numerada y en formato MADR reducido: `Status`,
`Date`, `Deciders`, `Context`, `Decision`, `Consequences`, `Alternatives considered`.

Reglas de este directorio:

- Un ADR aceptado es inmutable. No se reescribe, no se amplía y no se le agregan
  secciones.
- Una decisión nueva o un cambio de rumbo se registra en un ADR nuevo, con el número
  siguiente, cuyo `Status` declara a cuál supera.
- La única edición que recibe un ADR ya aceptado es su línea `Status` cuando queda
  superado.

| # | Título | Status |
|---|---|---|
| [0001](0001-identidad-configurable-en-ejecucion.md) | La identidad de la institución pasa a ser configuración de ejecución | Aceptado |
| [0002](0002-candado-de-ti-argon2id.md) | El panel de ajustes se protege con Argon2id, con rotación acotada al build | Aceptado |
| [0003](0003-institucion-por-defecto-neutra-y-tier-pse.md) | La institución por defecto es un marcador neutro y el tier por defecto es `pse` | Aceptado |
| [0004](0004-orquestador-de-recuperacion-del-asistente.md) | El Asistente decide qué recuperar con un plan restringido, y sin framework de agentes | Aceptado |
