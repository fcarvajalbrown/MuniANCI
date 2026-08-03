# 0001. La identidad de la institución pasa a ser configuración de ejecución

**Status:** Aceptado
**Date:** 2026-08-03
**Deciders:** Felipe Carvajal Brown

## Context

Todo lo que el área de TI de una institución puede ajustar vive en
`munianci.config.json` (`core/src/config.rs`): plazos del POA&M, retención del
histórico, barrido de red, monitoreo y colores del informe. La identidad no. El nombre
del organismo y su tier se compilan en el binario mediante `MUNIANI_INSTITUTION` y
`MUNIANI_TIER`, de modo que un nombre equivocado no se corrige editando un archivo:
cuesta un build nuevo y una reinstalación en el equipo del cliente.

Eso convierte un error de tipeo, un cambio de nombre o una demostración ante otro
organismo en un problema de ingeniería. El detonante concreto fueron dos reuniones el
mismo día con la Fuerza Aérea de Chile y el Ejército de Chile, con un producto cuyo
valor por defecto era el nombre de una municipalidad real.

El archivo de configuración ya resolvía este tipo de problema para todo lo demás, y ya
tenía las convenciones necesarias: cada área aporta su sección con `#[serde(default)]`,
un archivo antiguo sigue cargando, un archivo ilegible avisa por stderr y cae a los
valores por defecto, y el informe declara de dónde salió la configuración.

## Decision

La identidad se agrega a `munianci.config.json` como una sección `identidad` con dos
campos opcionales, `institucion` y `tier`, y el host la resuelve en tiempo de ejecución
con este orden, primer valor no vacío gana:

1. `munianci.config.json` -> `identidad.institucion` / `identidad.tier`
2. el valor compilado en `MUNIANI_INSTITUTION` / `MUNIANI_TIER`
3. `DEFAULT_INSTITUTION` / `DEFAULT_TIER`

Los campos son `Option<String>` y no `String`. Ausente y vacío no significan lo mismo:
ausente cae al valor compilado, y vacío es una edición que se rechaza al guardar en vez
de dejar el informe sin emisor.

`Config` gana un escritor `guardar` que serializa a un temporal y renombra encima, para
que un corte a mitad de escritura no deje a TI con un archivo truncado, y que repone la
cabecera `_ayuda` cuando el archivo no la traía y la respeta cuando sí.

Lo compilado no desaparece: pasa a ser el valor de fábrica con que sale cada build de
cliente, no un valor inamovible.

## Consequences

- Un nombre equivocado se corrige en el equipo, sin instalador y sin rebuild.
- El cambio alcanza a los dos módulos. `branding.rs` deja de ser la única fuente y el
  host resuelve la identidad para el encabezado, para el informe y para el `MUNIGPT_MUNICIPIO`
  con que se levanta el sidecar del Asistente, que a su vez decide la personalización del
  prompt y la base `db_<slug>`. Antes el Asistente leía únicamente el valor compilado, de
  modo que un cambio de nombre no llegaba a él.
- Renombrar la institución obliga a reiniciar el Asistente, y con eso se pierde el
  historial de chat de esa pestaña. El panel lo advierte antes de guardar.
- Si no existe una `db_<slug>` para el nombre nuevo, el backend responde con el corpus
  nacional. Ese respaldo ya existía y era invisible; declararlo es trabajo aparte.
- No hay estado que invalidar. Cada consumidor llama `Config::load()` en su punto de uso,
  así que basta con escribir el archivo.
- La regla de `CLAUDE.md` que decía que la identidad del cliente no es configuración de
  TI queda revertida a propósito, y se corrige en el mismo hito.

## Alternatives considered

- **Un archivo aparte junto a la configuración.** Separar la identidad en su propio
  archivo la habría dejado fuera del formato que TI ya conoce, con su propia carga, su
  propio manejo de BOM y su propia documentación. Se descartó por duplicar una superficie
  que ya está resuelta y probada.
- **Un archivo bajo `%LOCALAPPDATA%`, fuera del directorio de instalación.** Sobrevive a
  la reinstalación, que es su ventaja, pero esconde la identidad del organismo en el
  perfil de un usuario: deja de estar donde el resto de la configuración y deja de ser
  evidente para quien audita el equipo. Se descartó para la identidad, y se usa solo para
  el override de contraseña, donde esa misma persistencia sí se quiere (ver
  [0002](0002-candado-de-ti-argon2id.md)).
