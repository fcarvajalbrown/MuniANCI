# 0006. Lo declarativo se responde en la aplicación, y lo no respondido deja de afirmarse como incumplido

**Status:** Aceptado
**Date:** 2026-08-04
**Deciders:** Felipe Carvajal Brown

## Context

El catálogo declarativo existe y funciona: 28 preguntas en `core/src/questionnaire.rs`,
con su modelo de respuesta, su exigibilidad por tier y su traducción a brechas. La CLI lo
recorre de forma interactiva en `cli/src/main.rs:414`, e incluso ofrece omitirlo con una
bandera que declara lo que hace, "asume todo no cumplido".

**La GUI no lo pregunta nunca.** `gui/src/commands/start_scan.rs:80` arma cada escaneo con
`QuestionnaireResponse::default()`, es decir sin ninguna respuesta. Y
`questionnaire.rs:518` resuelve la ausencia de respuesta como brecha:

```
let non_compliant = answer.map(|a| !a.compliant).unwrap_or(true); // unanswered = gap
```

El hallazgo que sale de ahí dice, textualmente, `Control declarativo no cumplido: <pregunta>`.
La línea de evidencia sí matiza —"No respondido o declarado no cumplido"— pero el titular no.

De modo que **todo informe generado desde la GUI afirma hasta 28 incumplimientos
declarativos que nunca se preguntaron**, y la GUI es lo que usa un funcionario municipal;
la CLI es la vía técnica. El producto está afirmando algo que no sabe, que es exactamente lo
que el principio "No inventar" del `ROADMAP.md` prohíbe y lo que el producto promete no
hacer.

Se descubrió el 2026-08-04, al fijar el alcance del hito 0.8.3: el bloque de preguntas
nuevas del Decreto 2 (Arts. 5°, 6° y 10°) no tenía dónde aterrizar, porque no hay superficie
donde responder ninguna.

## Decision

**1. Aparece un tercer estado: no respondido, distinto de no cumplido.**

Sigue contando como brecha. No se oculta, no se puntúa mejor y no cambia la madurez: una
institución que no responde nada no obtiene un informe limpio. Lo que cambia es lo que el
producto afirma. El hallazgo, el PDF y el JSON dicen que la pregunta no fue respondida, en
vez de declarar un incumplimiento que nadie constató.

**2. El cuestionario se responde en una cuarta pestaña, sin contraseña.**

Junto a Vista Municipal, Vista Técnica y Asistente. Son declaraciones institucionales —si el
encargado de reportar está designado, si la política fue aprobada por acto administrativo, si
hay análisis de riesgos anual—, y quien las sabe es el jefe de servicio o el delegado de
ciberseguridad, no el área de TI. El panel de ajustes está detrás del candado Argon2id del
[ADR 0002](0002-candado-de-ti-argon2id.md), así que alojarlo ahí dejaría fuera justamente a
quien puede responder. Una pestaña además se ve, y no verse es el problema que se está
corrigiendo.

**3. Las respuestas viven en una sección nueva de `munianci.config.json`.**

El archivo ya está probado sobreviviendo a una reinstalación, ya tiene resuelta la trampa del
BOM con `config::sin_bom`, TI puede leerlo con el Bloc de notas, y es donde vive el resto de
la configuración de ejecución. La sección se agrega con `#[serde(default)]`, como todas, para
que un archivo anterior siga cargando.

## Consequences

- **Los informes cambian de contenido, y hacia abajo en gravedad aparente.** Un informe
  generado desde la GUI hoy trae hasta 28 "no cumplido" que pasarán a leerse como "no
  respondido". No es una mejora cosmética: es dejar de afirmar lo que no se sabe.
- **El conteo de brechas no baja.** La cuenta de la portada y el puntaje de madurez se
  mantienen, porque un control no respondido sigue siendo una brecha. Quien compare dos
  informes verá cambiar el texto y no el número.
- **El JSON gana un valor nuevo donde antes había dos.** Es un cambio de contenido, no de
  nombres de campo, así que no toca el límite que el [ADR 0005](0005-destinatario-del-informe-configurable.md)
  fijó. Un consumidor que asumiera dos estados verá uno tercero.
- **La CLI y la GUI convergen.** El mismo catálogo, el mismo modelo de respuesta y el mismo
  archivo de persistencia. La bandera de la CLI que asume todo no cumplido pasa a significar
  lo que dice: no respondido.
- **El hito 0.8.3 recupera su bloque de cuestionario.** Las preguntas de los Arts. 5°, 6° y
  10° del Decreto 2 tienen dónde responderse. Sin esta decisión, agregarlas habría sido
  ampliar un catálogo que nadie puede contestar.
- **TI puede editar declaraciones a mano.** Es la misma propiedad que ya tiene la identidad
  desde el [ADR 0001](0001-identidad-configurable-en-ejecucion.md), y la misma advertencia:
  el archivo es editable a propósito, y esto es un seguro contra el cambio accidental, no un
  control de seguridad.
- **Nada de esto convierte al informe en evidencia de cumplimiento.** Sigue siendo una
  autoevaluación declarada por la propia institución, y el pie de página ya lo dice.

## Alternatives considered

- **Dejar el modelo binario y limitarse a que la GUI permita responder.** Es el cambio más
  chico, no toca el formato del informe ni el JSON, y la línea de evidencia ya matizaba. Se
  descartó porque deja el titular falso en pie para cualquiera que se salte una pregunta, que
  serán casi todos la primera vez, y porque la afirmación equivocada es el defecto, no la
  ausencia de formulario.
- **Que lo no respondido deje de ser brecha.** Es la lectura más literal de "no sabemos". Se
  descartó porque una institución que no responde nada obtendría un informe limpio, y el
  silencio pasaría por cumplimiento. Es peor que el problema que se corrige.
- **Alojar el cuestionario en el panel de ajustes de TI.** Coherente con todo lo que escribe
  en `munianci.config.json` y protegido contra ediciones casuales. Se descartó porque supone
  que TI sabe si el alcalde firmó la política, y porque esconde las declaraciones justamente
  de quien las hace.
- **Ponerlo como paso previo dentro de Vista Municipal.** Ata el responder al momento en que
  importa, sin agregar pestaña. Se descartó porque pone 28 preguntas delante de alguien que
  abrió la aplicación a escanear, y no deja una vía natural para volver a corregir una sola
  respuesta después.
- **Guardar las respuestas en un archivo aparte.** Separa lo que TI ajusta de lo que la
  institución declara, que es una distinción real. Se descartó por costo sin beneficio
  proporcional: un segundo archivo que instalar, respaldar y mantener sincronizado.
- **Guardarlas en el histórico SQLite, fechadas por escaneo.** Es lo más cercano a evidencia:
  deja rastro de qué se declaró y cuándo. Se descartó para esta entrega porque exige cambio de
  esquema y una noción aparte de "respuestas vigentes" para precargar el formulario. Si el
  piloto pide trazabilidad de declaraciones, es un ADR nuevo y no una ampliación de éste.
