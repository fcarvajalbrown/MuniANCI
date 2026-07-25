# Changelog

All notable changes to MuniANCI will be documented here.
Format: [Semantic Versioning](https://semver.org).

---

## [0.7.0] — 2026-07-25 — segundo marco normativo y seguimiento de riesgos

Hasta ahora el producto medía contra un solo cuerpo legal y volvía a empezar en cada
escaneo. Esta versión agrega el **Decreto 7 de 2023**, la Norma Técnica de Seguridad de
la Información y Ciberseguridad de la Ley 21.180, que obliga a toda municipalidad; y un
**registro de riesgos** que sigue cada hallazgo hasta cerrarlo en vez de regenerar el
plan desde cero.

El alcance salió de un pase de investigación de 20 búsquedas más la lectura íntegra de
tres fuentes primarias —el Decreto 7, el DFL N°1 de 2020 y la guía técnica de la
Secretaría de Gobierno Digital, las tres versionadas en `docs/`—. Está en
`docs/research/0.7.0-escaneo-profundo-multimarco-y-riesgos.md`.

**El escaneo profundo (escaneo con credenciales, osquery, mapa de segmentos, Nuclei) se
difiere a 0.8.0.** Es la parte con más superficie de falla en una red municipal real, y
se prefirió una versión probada antes que una más grande.

### Added
- **Decreto 7 de 2023 como segundo marco.** Diez controles declarativos: el diagnóstico
  inicial del Art. 4°, la Política aprobada por acto administrativo del Jefe Superior de
  Servicio y sus cuatro contenidos del Art. 5°, los dos responsables designados y la
  prohibición de externalizar esas funciones, y las cinco funciones del Título Tercero.
- **Perfil de madurez por marco.** Las cinco funciones del decreto —identificación,
  protección, detección, respuesta y recuperación— resultaron ser las cinco del **NIST
  CSF**: el decreto chileno tomó esa estructura, con gobernanza como categoría dentro de
  identificación, tal como la tenía el CSF 1.1. Así que un solo eje sirve para los dos
  marcos y el mapeo al CSF es una etiqueta, no una ingesta aparte.
- **Fase de la Ley 21.180 por comuna.** Con el nombre de la institución, el informe dice
  en qué fase debería ir este año, tomado del Art. 5° del DFL N°1 (que nombra a las
  municipalidades una por una en los Grupos B y C) y de la tabla del Art. 7°. Providencia
  es Grupo B: le corresponden las fases 4 y 5 en 2026.
- **Registro de riesgos.** Cada hallazgo se sigue con estado, responsable, plazo y nota,
  y el estado sobrevive entre escaneos. Se opera desde la Vista Técnica y se emite en el
  `risk/status` del POA&M, de modo que lo que anota la municipalidad es lo que dice el
  documento que entrega.
- **Inventario de nombres fuera de `gob.cl`.** El Art. 8° inciso final del DS N°293
  obliga a informar a la Agencia todo FQDN fuera de ese dominio expuesto a internet.
  Ahora el escáner produce la lista de candidatos de la que sale esa declaración.
- **La deriva por control, visible en la aplicación de escritorio.** Era lo principal que
  trajo 0.6.0 y solo se veía en el PDF y en la CLI.

### Fixed
- **La aplicación mostraba multas por controles que no acarrean multa.** La Vista
  Municipal calculaba la cifra desde la severidad técnica, que es criterio de este
  producto, y no desde la clasificación de infracción, que es como el Art. 40° construye
  la escala. El resultado era una cifra en pesos por controles no exigibles, o sea por
  incumplimientos que no existen. Ahora la multa aparece solo cuando la brecha es
  exigible y la ley clasifica la infracción, y en los demás casos se dice por qué no hay.
- **Los PDF de las normas estaban corruptos en el repositorio.** `core.autocrlf` venía
  reescribiendo bytes de los PDF de `docs/` como si fueran texto, así que quien clonaba
  el repo recibía dañados los textos de la Ley 21.663 y la Ley 21.459. Se agrega
  `.gitattributes` y se reponen los archivos.
- **Cifras en UTM sin separador de miles.** "50000 UTM" en un documento que lee un jefe
  de servicio se cuenta con el dedo en la pantalla.
- **Tildes en las etiquetas del informe.** Desde que la fuente va embebida el PDF puede
  escribir en castellano, pero las cadenas tipeadas a mano seguían sin tilde.

### Changed
- **Ningún control del Decreto 7 afirma incumplimiento, en ningún tier.** Su guía técnica
  dice de sí misma que "no crea obligaciones adicionales" y admite desarrollar la
  Política gradualmente, así que la norma trae su propio modelo de madurez progresiva.
  Se suma que traducir su Art. 13° a un deber de seguridad exigible es interpretación
  jurídica, y este producto no la hace.
- **El promedio de madurez sigue siendo el de la Ley 21.663.** Mezclarlo con el Decreto 7
  daría un número sin significado, y ese valor es el que el histórico viene guardando
  desde 0.5.0: incluir otro marco habría mostrado un salto que ninguna institución
  provocó.
- **Providencia es la institución por defecto** en un build sin marca, en la GUI, la CLI
  y el Asistente. Antes cada uno decía algo distinto.
- **Procedencia de las fuentes IBM Plex registrada** en `vendor/PROVENANCE.md`, con su
  SHA256 y la licencia SIL OFL 1.1, como exige el Apéndice C.

### Limitación conocida — el Asistente no viaja en el instalador

El módulo Asistente **no funciona desde una instalación**: el instalador lleva solo el
ejecutable de la aplicación, y el backend lo busca en el árbol del repositorio. Hoy corre
únicamente desde el código (`cargo tauri dev`, o el ejecutable en `target/release`).

No es una regresión de esta versión: el empaquetado del sidecar es la **Fase 5** de
`docs/MERGE-PLAN-MuniGPT.md` y nunca se ejecutó. Tiene dos decisiones abiertas por
delante: cómo se empaqueta el runtime de Python (D1) y qué se hace con los **4 GB de
modelos GGUF** (D2), que no caben en un instalador NSIS o WiX, con su techo cercano a los
2 GB. Queda asignado a 0.8.0.

El escáner —que es lo que esta versión mejora— funciona completo desde el instalador.

### Descartado, con su motivo
- **CIS Benchmarks y CIS Controls.** Los primeros son CC BY-NC-SA y los segundos
  CC BY-NC-**ND**: no comercial, y el ND prohíbe derivados, así que ni reexpresarlos como
  preguntas es salida.
- **El texto del Anexo A de ISO/IEC 27001.** Se vende y su licencia no se pudo verificar.
  No se embarca en ninguna forma.

---

## [0.6.5] — 2026-07-25 — deberes de la Red de Conectividad Segura del Estado

Versión de cumplimiento legal, sin funcionalidad nueva. Sale del hallazgo de que el
**DS N°293 de 2024**, el reglamento de la Red de Conectividad Segura del Estado, obliga
directamente a las municipalidades —su Art. 4° las nombra— y el producto no lo cubría en
absoluto. Se leyó el reglamento completo del Diario Oficial (N° 44.123 del 11-04-2025,
CVE 2631206) antes de codificar nada.

### Fixed
- **El delegado de ciberseguridad sí le es exigible a una municipalidad.** El informe lo
  marcaba "madurez voluntaria, no exigible", que es correcto por el camino del Art. 8°
  lit. i) de la ley, exigible solo a los OIV. Pero el **Art. 5° inciso 2 del DS N°293**
  se lo exige además a todo órgano de la Administración del Estado que integre la RCSE.
  El deber llegaba por dos instrumentos y el producto solo miraba uno.

### Added
- **Cinco deberes de la RCSE en el cuestionario**, cada uno con su artículo a la vista:
  integrar la Red (Art. 4°); **informar cada seis meses todos los contratos vigentes** de
  telecomunicaciones, transmisión de datos, acceso a internet, infraestructura digital,
  servicios digitales, TI y almacenamiento, y las modificaciones dentro de 15 días
  corridos (Art. 6°); permitir el monitoreo de tráfico de la Agencia (Art. 7°); usar
  subdominio **.gob.cl** con el .cl redirigiendo, cuyo plazo venció el **11-04-2026**
  según la disposición transitoria cuarta (Art. 8°); e informar todo FQDN fuera de
  gob.cl expuesto a internet (Art. 8°, inciso final).
- **La IG N°2 en el anclaje del segundo factor.** Autoriza medios de autenticación
  alternativos para el encargado que no pueda acceder a Clave Única. Citar solo la IG
  N°1, que exige Clave Única, dejaba a esa municipalidad sin salida aparente.

### Changed
- **Ninguno de los deberes de la RCSE afirma una clasificación de infracción.** El
  decreto no fija una escala propia y el producto no la inventa: se afirma el deber y su
  artículo, no su sanción. Hay una prueba que lo impide.
- **El alcance de esos deberes es toda institución, no solo OIV y PSE.** La RCSE obliga a
  todo órgano de la Administración del Estado, incluido el municipio que todavía no ha
  sido clasificado. Límite conocido y anotado: `Tier` no distingue un PSE estatal de uno
  privado, y a uno privado el decreto no lo alcanza.
- **Verificado el alcance de las cuatro Instrucciones Generales** contra el Diario
  Oficial. Las IG N°1 y N°2 se dirigen a los servicios esenciales del Art. 4°, o sea
  alcanzan a una municipalidad; las IG N°3 y N°4, a los operadores de importancia vital
  del Art. 6°, o sea no. Confirma la conclusión de 0.5.0 sin cambios en los anclajes.

---

## [0.6.0] — 2026-07-25 — monitoreo continuo y evidencia

Hasta ahora el producto era un diagnóstico puntual: alguien se acordaba de escanear, salía
un PDF, y lo único que el informe podía decir del pasado era cuánto se había movido el
puntaje. Esta versión lo convierte en cumplimiento sostenido. El escaneo se repite solo,
el informe dice **qué** control cambió y no solo cuántos, y la municipalidad puede entregar
una carpeta fechada que quien la reciba verifica con herramientas que Windows ya trae.

El alcance salió de un pase de investigación de 134 búsquedas en tres pasadas, más la
lectura íntegra de la Res. Ex. N°7/2025, la Res. Ex. N°187/2026 y la documentación del
Programador de tareas de Windows (`docs/research/0.6.0-monitoreo-continuo-y-evidencia.md`).
La investigación cambió tres decisiones de diseño antes de escribir una línea de código, y
descubrió una normativa en camino que afecta al producto completo (ROADMAP, "En vigilancia").

### Added
- **Deriva de cumplimiento control por control** — el informe ya no dice solo que hay
  dieciséis brechas: dice cuál es nueva, cuál sigue abierta, cuál se resolvió y cuál se
  había resuelto y **volvió**. Esa última es la que importa: un control corregido que se
  cae de nuevo habla del proceso de la municipalidad, no de sus equipos, y el informe
  muestra la fecha en que estuvo cerrado para que TI pueda ir a mirar qué pasó entremedio.
  Sale de las tablas que el histórico ya tenía, así que las mediciones guardadas por 0.5.0
  producen deriva desde el segundo escaneo.
- **Estado "sin verificar", para no afirmar correcciones que nadie hizo** — un control
  técnico desaparece de los resultados tanto cuando se corrigió como cuando el escaneo no
  llegó a mirarlo. Si el alcance pasó de LAN a local, darlo por resuelto sería afirmarle a
  la ANCI una corrección inexistente. Ahora cada escaneo registra su alcance y solo se
  informa una resolución cuando esta medición cubrió al menos lo que cubría la anterior.
  Los controles declarativos son la excepción y se resuelven siempre: una pregunta sin
  responder sigue figurando como brecha, así que si desaparece es porque alguien declaró
  que se cumple.
- **Paquete de evidencia fechado y sellado por hash** — una carpeta con los dos informes,
  el JSON del CSIRT, el plan de remediación y el resumen del histórico, más un manifiesto
  SHA-256. Se verifica con `certutil -hashfile` o `Get-FileHash`, que vienen de fábrica en
  Windows: un sello que solo puede comprobar la herramienta que lo puso no sirve de nada.
  Trae un `COMO-VERIFICAR.txt` en castellano llano que declara con todas sus letras que
  esto es **verificación de integridad y no una firma electrónica** —bajo la Ley 19.799
  solo una firma avanzada de prestador acreditado da calidad de instrumento público—, que
  la fecha es la del reloj del equipo, y que las huellas las calculó la misma herramienta
  que produjo los informes.
- **Reescaneo programado sin privilegios de administrador** — `munianci --programar`
  registra la tarea en el Programador de tareas de Windows para la cuenta del propio
  usuario. Viene apagado de fábrica y avisa antes de crear nada: crear una tarea programada
  es la técnica T1053.005 de MITRE ATT&CK y queda en el evento 4698, así que en una
  municipalidad con EDR el producto puede levantar una alerta de persistencia y el área que
  lo opera merece saberlo. Si una política de grupo lo impide, no se cae: lo dice y remite
  al aviso de medición vencida.
- **Aviso de medición vencida en la aplicación** — cuántos días lleva la medición sin
  renovarse, sobre el encabezado y no dentro de una pestaña. Es la red de seguridad del
  reescaneo programado: si la tarea no se pudo crear, esto es lo único que se lo va a
  recordar a la municipalidad.
- **Botón del paquete de evidencia en la Vista Técnica** — pide una carpeta y no un
  archivo, porque el manifiesto no vale nada separado de lo que sella, y al terminar la
  abre en el Explorador.
- **Taxonomía de incidentes de la Res. Ex. N°7/2025** — cuatro áreas de impacto, once
  efectos observables y cuarenta categorías, transcritas del Diario Oficial. Se difirió
  desde 0.5.0 justamente por no tener la fuente primaria a la vista. El JSON del CSIRT
  declara su procedencia y deja la clasificación del incidente vacía: el Art. segundo
  clasifica "el hecho acaecido", y una brecha detectada no es un hecho acaecido, así que
  asignarle una categoría automáticamente sería afirmar ante el CSIRT Nacional un incidente
  que no ocurrió.
- **`monitoreo` en `munianci.config.json`** — intervalo, día y hora del reescaneo, y a los
  cuántos días avisar que la medición venció. La cadencia semanal por defecto no es un
  plazo legal: ninguna norma chilena fija uno.

### Changed
- **El PDF escribe en castellano.** Desde marzo de 2026 el generador aplanaba las tildes,
  porque las fuentes estándar de PDF no pueden representar la ñ: "Contraseñas por defecto"
  salía impreso "Contrasenas por defecto" y "Art. 9°" perdía el grado. Ahora se embebe IBM
  Plex, la misma familia que usa la interfaz, bajo SIL Open Font License. El informe pasa de
  unos 20 kB a unos 270 kB; es lo que cuesta que el documento diga "Ñuñoa".
- **El histórico registra el alcance de cada escaneo.** Se agrega con una migración
  guardada, así que una base escrita por 0.5.0 se abre igual; sus mediciones quedan con
  alcance desconocido, que se lee como cobertura insuficiente y no afirma resoluciones.

### Fixed
- Nada que corregir de 0.5.0: no se reportaron defectos entre ambas versiones.

---

## [0.5.0] — 2026-07-24 — potencia del escáner y cumplimiento ANCI

El escáner deja de listar problemas y empieza a decir cuáles importan: qué CVE se están
explotando hoy, cuáles ya corrigió el último acumulativo de Windows, qué equipos hay
realmente en la red, y qué hacer primero. Del lado legal, separa lo que la Ley 21.663
exige a una municipalidad de lo que solo es buena práctica. Las municipalidades no son OIV,
y eso cambia qué se les puede exigir. El alcance salió de un pase de investigación que
incluyó la lectura íntegra de la ley, la Res. Ex. N°87 y las Instrucciones Generales N°1 y
N°4 (ROADMAP 0.5.0, `docs/research/0.5.0-escaner-y-cumplimiento-anci.md`).

### Added
- **Enriquecimiento CVE offline** — snapshot de NVD convertido en tiempo de build a un
  índice compacto propio, con el matching CPE→CVE implementado en Rust dentro de `core`.
  El mapeo nombre→CPE usa una tabla curada: si un producto no está en la tabla no se
  afirma nada sobre él, y el informe declara qué porcentaje del inventario quedó sin
  evaluar. Alta precisión antes que alta cobertura, porque desde abril de 2026 el NIST
  dejó de enriquecer buena parte de los CVE y el matching difuso produce falsos positivos
  entre ecosistemas.
- **Catálogo KEV de CISA** — distingue "300 CVE" de "4 CVE que se están explotando hoy", y
  ordena el plan de remediación. Viaja embebido y se sustituye en caliente con el JSON tal
  cual lo publica CISA (`MUNIANI_KEV_FILE` o junto al ejecutable), porque se actualiza cada
  pocos días. El informe declara contra qué catálogo, y de qué fecha, se hizo la afirmación.
- **Descubrimiento de red nativo en Windows** — `SendARP` e `IcmpSendEcho2` vía las APIs
  Win32 de IP Helper, sin exigir privilegios de administrador ni Npcap. El sondeo escala de
  la evidencia más fuerte a la más débil: ARP en capa 2, que el firewall del equipo no
  filtra y es lo único que entrega la dirección MAC; después ICMP; y TCP como último
  recurso. Medido en un /24 real con 4 equipos encendidos: el descubrimiento anterior veía
  1 host remoto y ninguna MAC, el nativo ve los 4 con MAC única. Los tres que aparecen no
  exponen ninguno de los puertos que el escáner probaba antes, que es justamente la
  condición en que suelen estar las impresoras, las cámaras IP y los equipos de red.
- **Modelo dual de cumplimiento** — lo exigible a una municipalidad (Art. 7°, Art. 9° e
  Instrucción General N°1) se evalúa como incumplimiento con consecuencia legal; el Art. 8°
  se mide como madurez voluntaria y se etiqueta como no exigible. Las municipalidades están
  obligadas por los Arts. 4°, 7° y 9°, pero la Res. Ex. N°87 las excluyó expresamente del
  primer proceso de calificación de OIV, así que el Art. 8° y las IG N°3 y N°4 no las
  obligan hoy. El tier es un dato con fecha: el Art. 6° obliga a recalificar al menos cada
  tres años.
- **Madurez 0 a 3 por dominio** — dice *dónde* está el problema, que es lo que un puntaje
  agregado no puede decir: un 82 de 100 puede ser cinco dominios sanos y uno roto. La forma
  se tomó del Essential Eight australiano, con su atribución CC BY en el informe; los
  dominios se derivan de los Arts. 7° y 8° y no copian los ocho controles del ASD.
- **Plan de remediación priorizado en OSCAL POA&M 1.2.2** — cada brecha con su acción,
  responsable y plazo, ordenadas por CVE en el catálogo KEV, luego calificación legal del
  incumplimiento según el Art. 39°, luego severidad. Los plazos sugeridos son criterio
  operativo y no legal: el único plazo perentorio del régimen es el reporte del Art. 9°.
- **Superficie de configuración para TI municipal** — `munianci.config.json` junto al
  ejecutable, editable con el Bloc de notas, sin rebuild ni instalador. Cubre los plazos del
  plan de remediación, el tamaño de papel y los colores del informe, el histórico, y el
  barrido de red. `munianci --escribir-config <ruta>` genera un archivo de ejemplo con todos
  los valores por defecto y una explicación de cada campo, porque nadie configura lo que no
  sabe que existe.
- **Informe ejecutivo de una plana**, aparte del técnico: responde tres preguntas (dónde
  estamos, qué arriesgamos, qué hacer primero) para quien firma, no para quien parchea.
  Papel chileno —oficio para el técnico, carta para el ejecutivo— y la paleta del Kit
  Gobierno de Chile, usada con moderación para no gastar tóner de color.
- **Histórico de evaluaciones por comuna en SQLite embebido**, con el delta respecto de la
  medición anterior en ambos informes. En SQLite y no en JSON porque un barrido semanal de
  un /24 acumula decenas de miles de filas al año. TI controla desde la configuración si se
  guarda el desglose por activo y cuántos meses se retiene.
- **Puntaje agregado anclado en la escala legal** — mecánica SPRS (base fija menos
  deducciones ponderadas), pero con los pesos tomados del Art. 39°: gravísima −5, grave −3,
  leve −1, en vez de una ponderación inventada. Los controles técnicos sin correlato en el
  Art. 39° usan una tabla propia, documentada como criterio técnico y no presentada como
  exigencia legal.
- **`tools/notas-release`** — genera el cuerpo del release desde el CHANGELOG. GitHub
  renderiza los release con saltos de línea duros, así que una sección envuelta a 85
  columnas se publica a media página; le pasó al 0.4.0.

### Changed
- **Las CVE del sistema operativo se filtran por el nivel de parches instalado.** Sin esto
  el catálogo KEV era contraproducente: en un CPE de Microsoft la release va en el nombre
  del producto y el campo versión es `-`, así que cualquier Windows 10 22H2 arrastraba todas
  las CVE publicadas contra esa release desde 2021. Medido en un equipo al día: 2.336 CVE y
  81 marcadas como explotadas activamente, entre ellas PrintNightmare, corregida ahí hace
  años. Ahora se descartan las publicadas antes del último acumulativo instalado, porque los
  acumulativos de Windows contienen todo lo anterior de su rama. Los límites van declarados
  en el informe: no cubre parches fuera de banda ni opcionales, y una CVE publicada antes
  del acumulativo pero aún sin corrección se descartaría por error. Sin fecha legible no se
  descarta nada.
- **La detección de versión TLS ahora funciona.** El sondeo fijaba `TLSv1.2` en todo
  handshake exitoso, de modo que el control "TLS 1.0/1.1/SSLv3 activo" —marcado como
  crítico— no podía dispararse nunca. Se reemplazó por un `ClientHello` construido a mano
  por versión, que detecta las versiones *habilitadas* y no la negociada. `rustls` no servía:
  no soporta TLS anterior a 1.2.
- **Cada pregunta va anclada a su artículo** con un ejemplo de evidencia, corrigiendo los
  anclajes excedidos: la IG N°4 estaba citada en controles aplicables a todos los tiers pero
  obliga solo a OIV.
- **El inventario declara con qué evidencia vio cada host** (`discovered_by` en el JSON):
  ARP prueba presencia física en el segmento, ICMP prueba que la pila IP responde, TCP solo
  prueba que un puerto acepta conexión. Sin esto, un activo sin MAC se lee como error del
  escáner, cuando lo más probable es que sea un equipo con el firewall filtrando el ping.
- **Se eliminaron un nombre de empresa inventado y las versiones escritas a mano.** El pie
  del PDF, el banner de la CLI y el `publisher` del instalador decían "Felipe Carvajal Brown
  Software", que no existe, y arrastraban un `v0.1` mientras el proyecto iba en 0.4.0.

### Rendimiento
Medido el 2026-07-24 en una LAN real (/24, 4 equipos encendidos, 16 núcleos), no estimado.

| | Antes (solo TCP) | 0.5.0 (ARP → ICMP → TCP) |
|---|---|---|
| Hosts remotos descubiertos | 1 | 4 |
| Con dirección MAC | 0 | 4 |
| Escaneo completo | 18 s | 81 s |

El barrido de LAN es más lento y ve dos veces y media más activos. El costo es inherente a
`SendARP`, no al limitador de ritmo: con `red.arp_pps` en 0 el escaneo baja a 70 s, apenas
11 s menos. Subir los hilos tampoco ayuda (64 → 81 s, 128 → 88 s, 253 → 82 s) porque Windows
serializa la resolución de vecinos por dentro. Una municipalidad que prefiera el
comportamiento anterior pone `red.arp` en `false`.

### Seguridad de red
El barrido ARP sale limitado a **10 sondas por segundo** de fábrica. Dynamic ARP Inspection,
habitual en switches Cisco, limita el ARP en puertos de acceso y al superar el umbral deja
el puerto en err-disable: sin el límite, el escáner puede dejar sin red al equipo desde el
que corre hasta que el área de redes lo rehabilite. El archivo de configuración explica el
riesgo donde nombra el campo. **Coordine el primer escaneo con LAN completa con el área
de redes**: un barrido de un /24 es una firma de reconocimiento y va a generar alerta en el
IDS. El payload del ping se identifica como MuniANCI en vez de imitar a `ping.exe`, porque
un escáner que se declara ante el SOC es más fácil de autorizar que uno que se disfraza.

### Diferido, con la razón escrita en el ROADMAP
- **La rama Linux del descubrimiento con `pnet`.** El producto se distribuye como app Tauri
  para PCs Windows y el soporte Linux completo ya estaba asignado al Horizonte. Sobre todo,
  no había forma de probarla en terreno. Linux conserva el ladder TCP anterior, sin cambio
  de comportamiento.
- **La taxonomía de incidentes de la Res. Ex. N°7/2025 en el JSON CSIRT.** El texto de la
  resolución no está verificado contra fuente oficial, y codificar categorías legales
  aproximadas sería peor que no tener ninguna: el JSON va al CSIRT Nacional con apariencia
  de estar alineado a la norma.
- **OSCAL Assessment Results.** Su campo `reviewed-controls` exige identificadores que
  resuelvan contra un catálogo OSCAL, y no existe un catálogo OSCAL de la Ley 21.663.
  Emitirlo produciría IDs que no resuelven contra nada: un documento con apariencia de
  estándar que no lo es. Queda condicionado a publicar antes ese catálogo.
- **El export del histórico al formato de interoperabilidad del Estado** (Decreto 12,
  Ley 21.180). Regula el intercambio entre órganos, no el almacenamiento, y hoy no hay
  destinatario identificado.
- **Nuclei**, a 0.7.0, donde está el escaneo de aplicaciones web y sus plantillas rinden.

### Anclajes legales verificados contra fuente primaria
- Plazos del Art. 9°: alerta temprana 3 horas, actualización 72 horas, informe final 15 días
  corridos. Reglamento aplicable: DS N°295 de 2024 (D.O. 01-03-2025).
- Multas del Art. 40°: leves hasta 5.000 UTM (10.000 OIV), graves 10.000 (20.000 OIV),
  gravísimas 20.000 (40.000 OIV).
- Estatus de las municipalidades: no son OIV. Res. Ex. N°87, sección VII numeral 3.
- Redistribución de datos NVD y CVE: permitida con los avisos de NVD y MITRE, que el informe
  imprime en todas sus páginas por condición de licencia.

---

## [0.4.0] — 2026-07-12 — empaquetado y fundaciones de confianza y medición

Empaquetado para PCs municipales y fundaciones para que toda mejora posterior sea
auditable y medible (ROADMAP 0.4.0).

### Added
- **CI (GitHub Actions)** — primera CI del repo: build + tests (Windows), y gates
  de auditoría de dependencias que BLOQUEAN (`cargo audit`, `cargo deny` para
  licencias/bans/sources, `pip-audit`), más generación de **SBOM** SPDX + CycloneDX
  (Rust y backend Python) como artefacto descargable.
- **Mitigación de inyección indirecta de prompts** en la ruta RAG (`sanitize.py`):
  saneamiento en tiempo de indexación (quita caracteres ocultos/bidi, neutraliza
  frases de override y marcadores de rol) + *spotlighting* del contexto recuperado
  (delimitadores, marcado como datos) para el modelo. OWASP LLM 2025.
- **Lanzamiento del sidecar empaquetado** (`--onedir` PyInstaller) con fallback a
  `python -m uvicorn` en dev, más **watchdog padre-vivo** (`watchdog.py`): el backend
  se autotermina si el host muere de forma anormal.
- **Distribución de modelos (D2)** — `models.manifest.json` + `fetch_models.py`:
  descarga reanudable con verificación SHA256 y paquete offline copiable para equipos
  air-gapped. Orígenes verificados por coincidencia de SHA256 contra el puntero
  git-LFS de cada repo.
- **Harness de evaluación offline** (`eval/`) — set dorado de 45 preguntas legales
  reales derivadas del corpus (aprobado) + métricas de recuperación deterministas
  (recall@k, MRR, precisión) como gate reproducible. Base: recall@k=0.978. Capa de
  juez LLM (`eval_judge.py`, Ragas con el llama.cpp local como juez, totalmente
  offline) implementada y validada; es una actividad manual pesada, no un gate de CI.
- **Mirror `vendor/`** — estructura, `.gitignore` de artefactos grandes y
  `PROVENANCE.md` (nombre/versión/origen/SHA256/licencia por artefacto).

### Changed
- **CSP estricta** de Tauri v2 (`connect-src` limitado a `127.0.0.1:8000` + IPC;
  sin orígenes externos) y **capability de menor privilegio** (webview reducido a
  `core:default`; diálogo/shell son nativos de Rust, fuera del ACL del webview). CSP de
  desarrollo aparte para no romper el HMR de Vite.
- **WebView2**: instalador offline embebido (`webviewInstallMode = offlineInstaller`)
  para PCs municipales air-gapped.

### Security / dependencias
- Se resolvieron 2 avisos RustSec de severidad alta: `lopdf` 0.34 -> 0.42
  (RUSTSEC-2026-0187, desbordamiento de pila) y `crossbeam-epoch` 0.9.18 -> 0.9.20.
  Dos avisos DoS transitivos de `quick-xml` (fijados por `tauri -> plist`) quedan
  documentados e ignorados con condición de remoción; cualquier otro aviso bloquea.

---

## [0.3.0] — 2026-07-11 — módulo Asistente (fusión MuniGPT)

MuniGPT, antes un producto de escritorio propio (asistente legal RAG offline), se
integró como el módulo **Asistente** de MuniANCI. Un solo producto Tauri, dos
módulos. Plan e historial en `docs/MERGE-PLAN-MuniGPT.md`.

### Added
- Módulo `assistant/` — backend FastAPI + RAG (llama.cpp embebido + LanceDB),
  importado con historia vía `git subtree`. Toda la inferencia corre local; la
  única salida a la red es `/search` (DuckDuckGo), apagada por defecto.
- `gui/src/assistant.rs` — ciclo de vida del backend como *sidecar* del proceso
  Tauri: lo levanta en el `setup` hook, sondea `GET /status` hasta `ready`, y reap
  del árbol de procesos (uvicorn + llama-server) al cerrar. Reemplaza al antiguo
  shell Electron.
- Pestaña **Asistente** en la GUI — el chat RAG portado a `gui/frontend`
  (streaming SSE, citas, chips de desambiguación) apuntando a `127.0.0.1:8000`.
- Bases vectoriales por comuna intercambiables — `rag.db_dir()` resuelve
  `MUNIGPT_DB_DIR` -> `db_<slug-comuna>` -> `db`.
- `app_branding` (comando Tauri) y `gui/src/commands/branding.rs` — exponen la
  institución/tier compilados al frontend para el encabezado.

### Changed
- **Marca unificada por cliente (Fase 4).** `MUNIANI_INSTITUTION` (env de
  compilación) ahora alimenta ambos módulos: el host pasa el valor al backend del
  Asistente como `MUNIGPT_MUNICIPIO`, que gobierna la personalización del prompt y
  la selección de base (`db_<slug>`). El backend resuelve el `municipio` en orden
  `MUNIGPT_MUNICIPIO` -> `config.json`. En builds sin marca, el Asistente conserva
  su `config.json` (no se rompe el demo). El encabezado de la GUI muestra la
  institución en vez del texto fijo "MuniANCI".

### Removed
- Shell Electron (`assistant/electron/`) y frontend standalone
  (`assistant/frontend/`) — superados por el host Tauri y `gui/frontend`.
- `assistant/package.json` / `package-lock.json` (config electron-builder) e
  instalador Inno Setup standalone (`assistant/installer/munigpt.iss`). El
  instalador unificado es una fase posterior (Fase 5, aún no ejecutada).

---

## [0.2.0] — 2026-03-31

### Added
- `muniani-gui` — Tauri 2 desktop GUI with React/TypeScript/Vite frontend
  - Vista Municipal (worker tab) — plain-Spanish gap summary, UTM fine scale, CSIRT notice
  - Vista Técnica (IT tab) — full gap table with evidence, live technical log terminal, asset detail
  - Progress channel streaming from Rust core to both tabs via `ScanProgress { pct, log }`
  - Native PDF and JSON export with OS save dialog (`tauri-plugin-dialog`)
  - Post-export folder reveal via `tauri-plugin-shell`
  - NIST/NSA design system — IBM Plex Sans + IBM Plex Mono, federal color palette
  - Per-client build via compile-time env vars (`MUNIANI_INSTITUTION`, `MUNIANI_TIER`)
- `eol_enrichment` module — post-normalization EOL patching via bundled static database
  - 38 products covered: Windows, Office, SQL Server, .NET, Python, Node.js, PHP, MySQL,
    PostgreSQL, MariaDB, MongoDB, Redis, Elasticsearch, Apache, nginx, Tomcat, OpenSSL,
    VMware, Veeam, LibreOffice, Firefox, Chrome, and more
  - Source: endoflife.date (March 2026 snapshot), embedded as `core/src/data/eol_db.json`
  - Fixes Office 2016 incorrectly reported as `is_eol: false` in v0.1
- Full WMI COM implementation — `wmi_query`, `wmi_scalar_u32`, `wmi_string_list`
- Real firewall detection via Windows registry (no elevation required)
- TLS certificate chain validation — classifies `Expired`, `SelfSigned`, `ExpiredAndSelfSigned`
- `backup_agent_running: Option<bool>` in `OsInfo` — `None` = WMI failed, `Some(false)` = no agent
- `log_cb` field in `ScanConfig` — separate technical log callback for GUI terminal
- BitLocker gap suppressed for PSE tier (OIV-only control per Art. 8° lit. a)
- PDF encoding fix — `to_pdf_safe()` sanitizes UTF-8 to WinAnsiEncoding (printpdf 0.9.1)
- 28 unit tests passing across all core modules

### Changed
- `ScanConfig` gains `log_cb: Option<Box<dyn Fn(&str) + Send + Sync>>` field —
  CLI sets this to `None`; GUI wires it to the IT terminal channel
- `normalizer::normalize()` renamed/aligned with updated lib.rs scan pipeline
- Workspace `Cargo.toml` adds `gui` as member

### Pending (deferred to v0.3.0)
- CVE enrichment via NVD API (Office 2016 `max_cvss` still `null`)
- Code signing certificate (DigiCert/Sectigo) — required before municipal delivery;
  unsigned `.exe` will be blocked by enterprise AV (McAfee, Defender ATP)
- Inno Setup portable `.exe` packaging
- Tauri 2 GUI security audit

### Legal anchors verified against
- Ley 21.663 full text (DO 08/04/2024, BCN)
- Ley 21.459 full text (DO 20/06/2022, last amended 01/04/2025)
- ANCI Instrucciones Generales N°1–4 (2025)
- DS N°295/2024 Reglamento de Reporte de Incidentes

---

## [0.1.0] — 2026-03-28

### Added
- `muniani-core` library crate with full scan pipeline
- `os_abstraction` layer — Windows (Win32/WMI) and Unix (/proc, dpkg, rpm)
- Probes: `host_discovery`, `drive_enum`, `service_probe`, `sw_inventory`, `os_check`
- `normalizer` — deduplicates raw findings into typed `AssetGraph`
- `compliance_engine` — maps findings to `Vec<Gap>` with Art. 8°/9° anchors
- `questionnaire` — declarative controls for Art. 8° lit. c, h, i and IG N°1–4
- Art. 27° significance filter for correct CSIRT reporting tagging
- `report_builder` — PDF informe de brechas + CSIRT JSON
- UTM fine scale table per Art. 40° Ley 21.663 (OIV and PSE tiers)
- Ley 21.459 Art. 2° safe harbor disclaimer in every PDF report
- `muniani-cli` binary with interactive questionnaire and progress reporting
- Windows + Linux support via `#[cfg]` platform gates
- 18 unit tests across core modules

### Legal anchors verified against
- Ley 21.663 full text (DO 08/04/2024, BCN)
- Ley 21.459 full text (DO 20/06/2022, last amended 01/04/2025)
- ANCI Instrucciones Generales N°1–4 (2025)
- DS N°295/2024 Reglamento de Reporte de Incidentes