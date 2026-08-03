# 0003. La institución por defecto es un marcador neutro y el tier por defecto es `pse`

**Status:** Aceptado
**Date:** 2026-08-03
**Deciders:** Felipe Carvajal Brown

## Context

`DEFAULT_INSTITUTION` valía `"Municipalidad de Providencia"`. Ese es el respaldo que usa
cualquier build sin marca: la CLI sin `--name`, un build de desarrollo, y cualquier
equipo cuya identidad no esté ni configurada ni compilada. Es decir, un cliente real
quedaba como valor por defecto de todos los demás, y el nombre aparece en el encabezado
de la aplicación y en el informe en PDF que se entrega a la autoridad.

Al mismo tiempo hay que fijar el tier por defecto, que no es cosmético: de él dependen la
exigibilidad de cada control, la clasificación de infracciones y el deber de reporte al
CSIRT que el informe afirma o niega.

## Decision

**Institución por defecto: `"Organismo del Estado"`.** Un marcador neutro, que no nombra
a nadie y que se lee como lo que es, un valor sin configurar.

**Tier por defecto: `"pse"`**, que se mantiene sin cambio. El fundamento se lee en el
texto de la Ley 21.663 versionado en `docs/Ley-21663_08-ABR-2024.pdf`:

- El **Art. 1° inciso 2** define el universo: "Para efectos de esta ley, la Administración
  del Estado estará constituida por los Ministerios, las Delegaciones Presidenciales
  Regionales y Provinciales, los Gobiernos Regionales, las Municipalidades, las Fuerzas
  Armadas, las Fuerzas de Orden y Seguridad Pública, las empresas públicas creadas por
  ley, y los órganos y servicios públicos creados para el cumplimiento de la función
  administrativa".
- El **Art. 4° inciso 2** dice que "son servicios esenciales aquellos provistos por los
  organismos de la Administración del Estado", sin exigir acto alguno que lo declare.

De ambos se sigue que un organismo del Estado sin resolución de la Agencia es prestador
de servicios esenciales, y que `pse` es el respaldo correcto.

Los otros dos valores posibles serían afirmaciones falsas:

- `unclassified` apagaría el deber de reporte al CSIRT en todo el informe. En el código,
  `apply_significance_filter` (`core/src/compliance_engine.rs:536`) calcula
  `is_reportable_tier` como `matches!(tier, Tier::Oiv | Tier::Pse)`, de modo que
  `Unclassified` deja `requires_csirt_report` en falso para toda brecha. El producto le
  estaría diciendo a un órgano del Estado que no tiene deber de reportar.
- `oiv` afirmaría una calificación que solo confiere la Agencia. El **Art. 5°** entrega
  esa calificación a una resolución del Director o Directora Nacional, y el **Art. 6°**
  la somete a un procedimiento con informes sectoriales, consulta pública respecto de las
  instituciones privadas e informe del Ministerio de Hacienda respecto de las públicas,
  que termina en una "resolución fundada".

Este ADR fija un valor por defecto del producto. **No clasifica a ninguna institución en
particular**, y nada aquí es asesoría legal: si el informe va a afirmar que un organismo
concreto incumple, esa calificación la valida un abogado.

## Consequences

- Ningún build sin marca vuelve a nombrar a un cliente real. Lo que el usuario ve cuando
  no hay identidad configurada es un marcador que se reconoce como tal.
- La CLI cambia su valor por defecto visible: `munianci --help` ahora muestra
  `[default: Organismo del Estado]`.
- El defecto neutro es un marcador, no una respuesta. La vía prevista para poner el
  nombre real es la sección `identidad` del archivo o el panel de ajustes
  ([0001](0001-identidad-configurable-en-ejecucion.md)).
- El tier por defecto vale para un organismo del Estado. Para una institución privada
  alcanzada por el Art. 4°, el defecto no se sostiene solo y hay que configurarlo.
- El hito 0.7.6 se hace más necesario: el producto ya no se llama a sí mismo municipal en
  el valor por defecto, pero el resto del texto sigue diciendo "Vista Municipal" y
  "municipalidad".

## Alternatives considered

- **Dejar una municipalidad real como respaldo.** Es el estado que se venía arrastrando.
  Se descartó porque pone el nombre de un cliente en el informe de otro, y porque el
  siguiente destinatario no era una municipalidad.
- **Dejarlo vacío y pedir el nombre en el primer arranque.** Obliga a decidir a quien
  instala y evita cualquier marcador. Se descartó porque el producto tiene que poder
  arrancar y generar un informe sin diálogos previos, y porque un campo vacío se propaga
  a un encabezado sin emisor en vez de a algo que se lea como no configurado.
- **Tier por defecto `unclassified`.** Es el valor más prudente si se lo mira como
  "todavía no sabemos". Se descartó porque en este producto no es neutro: apaga el deber
  de reporte al CSIRT en todo el informe, lo que es una afirmación, y equivocada para un
  órgano del Estado.
- **Tier por defecto `oiv`.** Sería el más exigente, y por eso parecía el más seguro. Se
  descartó porque afirma una calificación que, según los Arts. 5° y 6°, solo existe por
  resolución fundada de la Agencia.
