# 0002. El panel de ajustes se protege con Argon2id, con rotación acotada al build

**Status:** Aceptado
**Date:** 2026-08-03
**Deciders:** Felipe Carvajal Brown

## Context

El panel del engranaje deja editar cosas que cambian lo que dice el informe y lo que hace
el escáner en la red del organismo: el nombre de la institución que emite el documento,
los plazos del POA&M y la tasa de paquetes del barrido ARP, que es el único ajuste del
panel capaz de sacar al equipo de la red si el switch tiene Dynamic ARP Inspection.

Ese es el motivo del candado. No es que el archivo sea secreto: `munianci.config.json`
está pensado para editarse con el Bloc de notas y esa promesa se mantiene. Es que la
aplicación la usa personal municipal que no es de TI, y un panel sin resistencia invita a
tocar valores que después nadie sabe que cambiaron.

Nada más en el workspace hashea contraseñas, así que la decisión incluye adoptar una
dependencia nueva.

## Decision

Se protege el panel con **Argon2id**, mediante el crate `argon2` de RustCrypto (Rust
puro, licencia MIT/Apache), que es el algoritmo que la RFC 9106 recomienda para
contraseñas. El módulo vive en `core/src/ti.rs`.

El hash viaja **compilado por cliente** en `MUNIANI_ADMIN_HASH`, como cadena PHC, junto a
`MUNIANI_INSTITUTION` y `MUNIANI_TIER`.

TI puede rotarlo desde el propio panel. La rotación escribe un override en
`%LOCALAPPDATA%\MuniANCI\ti-password.hash`, que gana sobre el hash compilado. Borrar ese
archivo devuelve la contraseña del build, y eso queda documentado en el README como la
vía de recuperación.

El override guarda la **huella del hash compilado contra el que se creó y se ignora
cuando esa huella no calza con el build en ejecución**. Sin esa regla, un equipo donde
alguna vez se fijó una contraseña de desarrollo abriría después, en silencio, cualquier
build de cliente instalado en la misma máquina, con la contraseña del desarrollador en
lugar de la del cliente, y la contraseña efectivamente entregada a ese cliente no se
ejercitaría nunca. El archivo sobrevive a las reinstalaciones, así que el problema no es
teórico.

Si no hay hash compilado ni override, el primer uso del engranaje pide fijar una
contraseña, que pasa a ser el override. **No se distribuye ninguna contraseña por
defecto** y el README no trae ninguna que copiar.

Los builds de depuración no ponen candado y muestran un aviso visible dentro del panel
que dice que están sin contraseña, para que ese estado no se confunda con lo que recibe
un cliente. `MUNIANI_FORCE_LOCK=1` repone el comportamiento real cuando se quiere
ejercitar el camino de desbloqueo sin cortar un release. Un build de release nunca se
salta el candado, haya o no hash compilado.

Los intentos fallidos esperan cada vez más, en memoria: 1 s, 2 s, 4 s, con techo de 30 s,
se reinician al acertar y al reiniciar la aplicación. No se persisten.

La sesión desbloqueada vive en el estado administrado de Rust, no como un token que
guarde el webview: la contraseña no sale del proceso del host más allá de las teclas que
se escriben en el campo.

## Consequences

- **Modelo de amenaza declarado, y declarado también dentro del producto.** Esto es un
  seguro contra accidentes. Frena a quien se mete a los ajustes sin saber lo que toca. No
  frena a nadie con acceso al sistema de archivos, porque el archivo de configuración
  sigue siendo editable a mano a propósito. El panel y el README lo dicen con esas
  palabras, para que nadie lo tome por un control de seguridad.
- Cada build de cliente necesita su propio `MUNIANI_ADMIN_HASH` al empaquetar. Es un paso
  más en el proceso de release.
- Perder la contraseña rotada no deja a TI fuera de su propia configuración: se borra el
  override, o se edita el archivo con el Bloc de notas.
- Una dependencia nueva en el workspace, con su licencia que verificar y su lugar en el
  mirror local de `vendor/`.

## Alternatives considered

- **Contraseña de build inmutable, sin rotación.** Más simple, y elimina el archivo de
  override y toda la regla de huella. Se descartó porque obliga a un rebuild y una
  reinstalación para cambiar una contraseña que se filtró, que es exactamente el problema
  que este hito vino a resolver para el nombre de la institución.
- **Pedirla en el instalador NSIS.** Aprovecha un momento en que TI ya está frente al
  equipo, pero deja la contraseña fuera del producto y dentro del instalador, obliga a
  reinstalar para rotarla y no cubre a quien copia el ejecutable sin instalador.
- **Fijarla solo en el primer arranque.** Evita compilar nada por cliente, pero deja el
  panel abierto para quien llegue primero al equipo, que en una municipalidad no es
  necesariamente TI.
- **Firmar la configuración para que se rechacen las ediciones a mano.** Convertiría el
  candado en un control real. Se descartó por dos razones: rompe la promesa vigente de que
  el archivo se edita con el Bloc de notas, que es lo que hace mantenible el producto en un
  área de TI pequeña, y convierte una contraseña perdida en un bloqueo de TI respecto de su
  propia configuración.
