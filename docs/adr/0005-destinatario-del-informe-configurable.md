# 0005. El destinatario del informe es configuración de presentación, y el JSON al CSIRT no se mueve por él

**Status:** Aceptado
**Date:** 2026-08-04
**Deciders:** Felipe Carvajal Brown

## Context

El informe en PDF y la CLI decían `CSIRT Nacional` como literal en el código
(`report_builder.rs` y `cli/src/main.rs`), en la línea que avisa que una brecha hay que
reportarla en máximo 3 horas por el Art. 9°.

Eso es correcto para un organismo civil y equivocado para uno del sector Defensa. Los
**Arts. 29° a 31° de la Ley 21.663** crean el **CSIRT de la Defensa Nacional**,
dependiente del Estado Mayor Conjunto, y lo ponen como coordinador y enlace entre la
Agencia y los CSIRT Institucionales de la Defensa Nacional. Un organismo de ese sector que
recibiera el informe leería que debe reportar por una vía que no es la suya, en el
documento que se entrega a la autoridad.

El hito **0.8.3** es el que construye esa cadena, y su condición de entrada es leer
completas las nueve páginas del
`docs/Decreto-2_31-DIC-2025_Reglamento-Ciberseguridad-Defensa-Nacional.pdf` antes de
escribir código. Mientras tanto quedaba en pie un literal equivocado que no necesitaba
esperar a ese hito para corregirse.

## Decision

**El destinatario pasa a `informe.destinatario_csirt` en `munianci.config.json`**, con
`CSIRT Nacional` por defecto (`config::DEFAULT_CSIRT`). Un archivo escrito antes de este
cambio se comporta igual que siempre, y un valor en blanco o solo con espacios cae al
valor por defecto en vez de dejar el informe sin destinatario
(`InformeConfig::destinatario_csirt_o`).

**Va en `InformeConfig` y no en `ScanMeta`, a propósito.** El JSON que llega al CSIRT
tiene nombres de campo que son parte del contrato, y no se tocan por una cadena de
presentación. Hoy ese JSON no nombra a su destinatario: `ScanMeta` lleva
`institution_name`, `tier` y `scope`, y a quién va dirigido es implícito en dónde se
entrega.

**Este cambio altera a quién se dirige el informe y nada más.** No cambia ningún cálculo,
ninguna exigibilidad y ningún plazo. El procedimiento del reglamento sectorial sigue
íntegro en 0.8.3.

Nada de esto es asesoría legal. Que un organismo concreto deba reportar por el CSIRT-DN y
no por el CSIRT Nacional es una calificación que valida un abogado; el producto se limita
a poner el nombre que su configuración le indica.

## Consequences

- **Un build para el sector Defensa dirige su informe donde corresponde sin recompilar.**
  Basta una línea en `munianci.config.json`, editable con el Bloc de notas, como el resto
  de la superficie de configuración de TI.
- **Queda abierto lo que le toca a 0.8.3, y este ADR no lo prejuzga.** Si el JSON al CSIRT
  debe declarar por sí mismo el enrutamiento, y con qué campos, se decide en ese hito y
  después de leer el Decreto 2 completo. Si esa lectura obliga a mover el contrato del
  JSON, el ADR de 0.8.3 supera a este.
- **El campo no está en el panel de ajustes.** Se edita a mano en el archivo. El panel
  reúne identidad, plazos e histórico, red y monitoreo, e informe, así que tiene lugar
  natural donde ir; se dejó fuera para no fijar una decisión de interfaz sobre un campo
  que 0.8.3 puede reformular.
- **`munianci --escribir-config` documenta el campo**, con su valor por defecto, la vía
  del Art. 9°, la del sector Defensa por los Arts. 29° a 31°, y la advertencia explícita
  de que el procedimiento del reglamento sectorial todavía no está implementado. Nadie
  configura lo que no sabe que existe.
- **Verificado con las 431 pruebas del workspace en verde**, incluidas cuatro que fijan el
  valor por defecto, la configuración de un organismo de Defensa, el valor en blanco y la
  compatibilidad de un archivo anterior, más una que comprueba que el destinatario
  configurado llega a la página del PDF y no solo al struct.

## Alternatives considered

- **Dejar el literal `CSIRT Nacional` hasta 0.8.3.** Es el camino ordenado: una sola
  decisión, tomada con el reglamento leído. Se descartó porque el literal ya era
  equivocado para un destinatario que el producto atiende hoy, aparece en el PDF que se
  entrega a la autoridad, y corregirlo costaba un campo de configuración con valor por
  defecto. Esperar habría sido mantener a sabiendas una afirmación errónea por prolijidad
  de proceso.
- **Ponerlo en `ScanMeta`, para que el JSON al CSIRT también lo lleve.** Es la opción que
  deja el enrutamiento declarado en el artefacto que efectivamente se envía, y no solo en
  el PDF que se lee. Se descartó porque los nombres de campo de ese JSON son contrato con
  el receptor, y porque no hay todavía fuente primaria leída que diga qué debe declarar
  ese archivo en el régimen sectorial. Agregar un campo al contrato para arreglar una
  cadena de presentación es la clase de cambio que después no se puede retirar.
- **Deducirlo del tier o del nombre de la institución.** Evitaría configurar nada: el
  producto elegiría solo. Se descartó porque ni el tier ni el nombre dicen si un organismo
  pertenece al sector Defensa, de modo que deducirlo sería afirmar una clasificación que
  el producto no puede verificar. Es el mismo criterio ya adoptado en
  `core/src/ley21180.rs`, donde el Grupo A nunca se infiere y se informa como no
  identificado.
