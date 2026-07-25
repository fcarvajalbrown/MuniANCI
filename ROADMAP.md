# Roadmap MuniANCI — de 0.3.0 a 1.0

Estado actual: **0.3.0** (escáner de cumplimiento Ley 21.663 + módulo Asistente RAG
offline, integrados en una sola app Tauri). Este documento traza el camino a **1.0**
y el horizonte posterior. La asignación de cada hito fue decidida por el dueño del
repo; las opciones se fundamentaron en una investigación de 80 búsquedas sobre
scanners OSS, herramientas de cumplimiento gubernamentales de otros países, productos
comerciales GRC/vuln-management, stacks RAG offline, empaquetado Tauri+Python y
licenciamiento offline. Las fuentes están en la sección **Referencias**.

## Principios

- **Offline-first**: nada institucional sale del equipo. Es el diferenciador frente a
  Vanta/Drata/Qualys (cloud-first) y frente a OpenSCAP (offline pero inusable para un
  funcionario municipal). No comprometerlo por ninguna feature.
- **No inventar**: ninguna norma, artículo, cifra o cita legal se afirma sin fuente.
  Las cifras regulatorias marcadas "verificar" (Apéndice B) se confirman contra fuente
  oficial antes de material client-facing o de codificarlas como reglas.
- **Medible antes que ampliable**: el harness de evaluación (0.4.0) precede a las
  mejoras de calidad del Asistente, para decidir con datos y no por reputación.
- **Un solo binario, licencias limpias**: preferir crates Rust y libs de licencia
  permisiva; verificar la licencia de cada dependencia antes de anclarla (Apéndice A).
- **Vendoring / resiliencia de dependencias**: toda biblioteca OSS adoptada se mantiene
  en un mirror local `vendor/` (ver Apéndice C y `vendor/README.md`), preservando su
  `LICENSE`, para que el build y la distribución no dependan de que el upstream siga
  existiendo (crates.io, PyPI, HuggingFace, GitHub). Refuerza la postura offline/air-gapped.

## HARD RULES
- Ask Felipe via UI before starting a 0.X run.
- If in doubt during the run ASK FELIPE.


## Resumen de hitos

Estado: `Completado` se marca al publicar el release del hito (ver CLAUDE.md).

| Hito | Tema | Workstreams | Estado |
|---|---|---|---|
| **0.3.0** | Base: escáner + módulo Asistente integrado | — | Completado (v0.3.0, 2026-07-12) |
| **0.4.0** | Empaquetado + fundaciones de confianza y medición | A, H, harness (D) | Completado (v0.4.0, 2026-07-12) |
| **0.5.0** | Potencia del escáner + cumplimiento ANCI | F, G | Completado (v0.5.0, 2026-07-24) |
| **0.6.0** | Monitoreo continuo + paquetes de evidencia | I, J | Pendiente |
| **0.7.0** | Escaneo profundo/activos + multi-marco + riesgos | M, K, L | Pendiente |
| **0.8.0** | Asistente avanzado + apoyo operativo ANCI | O, P | Pendiente |
| **0.9.0** | Calidad del Asistente (RAG) | D | Pendiente |
| **1.0.0** | Piloto + endurecimiento + verificación legal + docs | — | Pendiente |
| **Horizonte** | Firma + licenciamiento + fidelidad de citas + integraciones, API, benchmarking, multiusuario, Linux | B, C, E, Q, N | Pendiente |

---

## 0.4.0 — Empaquetado y fundaciones

**Objetivo:** que el producto se instale bien en PCs municipales y que toda mejora
posterior sea auditable y medible.

**Empaquetado (A)** — resuelve las decisiones D1 y D2:
- **D1 = PyInstaller `--onedir` (sin UPX)** para el sidecar Python. Evita los falsos
  positivos de antivirus que dispara `--onefile` (auto-extracción a temp) y que
  romperían la instalación en PCs con AV corporativo.
- **D2 = distribución híbrida de los ~8 GB de modelos GGUF**: descarga en primer
  arranque (SHA256 + reanudación) para equipos con red, o paquete offline copiable a
  `AppData` para equipos air-gapped. Bundlear todo en el instalador está bloqueado:
  NSIS/WiX topan en ~2 GB.
- Embeber el bootstrapper de WebView2 (PCs air-gapped no lo tienen preinstalado).
- Watchdog padre-vivo para que el sidecar Python muera al cerrar la app.

**Fundaciones de confianza y medición (H + harness):**
- SBOM en cada release en **SPDX** (ISO, exigible en sector público) + **CycloneDX**,
  adjunto como artefacto descargable.
- `cargo audit` (+ `cargo-deny`, `cargo-auditable`) y `pip-audit` como **gate de CI**:
  el release se bloquea ante vulnerabilidades conocidas.
- Endurecer las capabilities/permissions de Tauri v2 y fijar una CSP estricta.
- Mitigación de **inyección indirecta de prompts** en el corpus RAG: *spotlighting*
  (marcar el contexto recuperado como datos, no instrucciones) + saneamiento en tiempo
  de indexación. OWASP LLM 2025 es explícito: RAG por sí solo no es defensa.
- **Harness de evaluación offline (Ragas + DeepEval)** con un set dorado de ~30-50
  preguntas legales chilenas reales, corriendo con juez local. Es el gate que habilita
  medir todas las mejoras del Asistente.
- **Establecer el mirror `vendor/`** (Apéndice C): `cargo vendor` para crates,
  wheelhouse para Python, y copias pinneadas de binarios, modelos, plantillas y datos,
  con los `LICENSE` preservados. Toda dependencia nueva se vendoriza al adoptarla, por
  si el upstream la remueve.

**Libs:** hf_transfer, aria2c, cargo-cyclonedx, cargo-sbom, cargo audit/deny/auditable,
pip-audit, Ragas, DeepEval.

**Hecho cuando:** instalador funciona en un PC limpio (con y sin red), SBOM + auditorías
corren en CI y bloquean, y el harness produce un puntaje base reproducible.

---

## 0.5.0 — Potencia del escáner y cumplimiento ANCI

Alcance ajustado tras el pase de investigación de 2026-07-24
(`docs/research/0.5.0-escaner-y-cumplimiento-anci.md`, ~30 búsquedas más lectura íntegra
de la Ley 21.663, la Res. Ex. N°87, las IG N°1 y N°4 y los ToU de NVD/CVE). Las cuatro
decisiones de alcance que siguen fueron tomadas por el dueño del repo el 2026-07-24.

**Potencia del escáner (F):**
- **Enriquecimiento CVE offline:** empaquetar el snapshot de NVD de fkie-cad
  (`CVE-all.json.xz`, 93,86 MB comprimidos, ~370 k CVE) convertido en build time a un
  índice compacto propio, e implementar el matching CPE→CVE **en Rust dentro de `core`**
  (crate `cpe` para parsear, comparación de rangos `versionStart*`/`versionEnd*` propia).
  `cpe2cve` de nvdtools **no se empaqueta**: queda solo como oráculo de pruebas en
  desarrollo, para no introducir un tercer runtime. Redistribución permitida con los
  avisos de NVD y MITRE (ver investigación §1.5).
- **Mapeo nombre→CPE con tabla curada**, extendiendo el `detect_product_slug()` que ya
  existe en `eol_enrichment.rs`. Alta precisión sobre alta cobertura: si un producto no
  está en la tabla no se afirma nada sobre él, y el informe declara qué porcentaje del
  inventario quedó sin evaluar. Motivo: desde abr-2026 NIST dejó de enriquecer buena
  parte de los CVE, y el matching difuso produce falsos positivos entre ecosistemas.
- **CISA KEV** (CC0) para priorizar: distingue "300 CVE" de "4 CVE que se están
  explotando hoy". Alimenta el orden del plan de remediación de G. El catálogo viaja
  embebido y se puede sustituir en caliente con el JSON tal cual lo publica CISA
  (`MUNIANI_KEV_FILE` o junto al ejecutable), porque se actualiza cada pocos días.
- **Filtro por nivel de parches del SO.** Sin él, KEV es contraproducente: en un CPE de
  Microsoft la release va en el nombre del producto y el campo versión es `-`, así que
  cualquier Windows 10 22H2 arrastraba todas las CVE publicadas contra esa release desde
  2021. Medido en un equipo al día: 2.336 CVE y 81 "explotadas activamente", entre ellas
  PrintNightmare, corregida ahí hace años. Se descartan las publicadas antes del último
  acumulativo instalado (los acumulativos de Windows contienen todo lo anterior de su
  rama). Límites declarados en el informe: no cubre parches fuera de banda ni opcionales,
  y una CVE publicada antes del acumulativo pero aún sin corrección se descartaría por
  error. Sin fecha legible no se descarta nada.
- Export **OSCAL** además del PDF y JSON CSIRT. Sustituye a SCAP/XCCDF/OVAL, que queda
  anotado por si aparece un consumidor real: no se identificó ninguno en la cadena ANCI,
  y OVAL exige una definición formal de test por control. OSCAL es JSON nativo y su
  modelo POA&M **es** el entregable de G.
  **Solo se emite el POA&M.** *Assessment Results* se difiere por dos razones, ambas
  verificadas contra la referencia de NIST y el ejemplo oficial (decisión del
  2026-07-24):
  1. Exige `import-ap` con un *href* a un **Assessment Plan**. Eso lo podría aportar TI
     municipal por `munianci.config.json` el día que exista.
  2. Cada `result` exige `reviewed-controls` con **identificadores de control** que
     resuelven contra un catálogo OSCAL. **No existe un catálogo OSCAL de la Ley
     21.663.** Sin él, el AR emitiría IDs que no resuelven contra nada: un documento con
     apariencia de estándar que no lo es.

  Por tanto AR queda condicionado a publicar antes un **catálogo OSCAL propio de la Ley
  21.663** (Arts. 7°, 8° y 9°). Ese catálogo no sería una invención —los artículos están
  verificados y los controles ya existen en el producto— y tiene valor por sí solo, así
  que se decide como ítem aparte y no dentro de este hito.
- **Nuclei se difiere a 0.7.0**, donde está el escaneo de aplicaciones web (workstream M)
  y sus plantillas rinden. Motivo: binario Go grande en PCs municipales con antivirus
  corporativo, el mismo riesgo que la decisión D1 de 0.4.0 evitó deliberadamente.
- **Escaneo de red nativo, híbrido por plataforma.** En Windows, APIs Win32 vía los
  crates `windows`/`windows-sys`: `SendARP` para la MAC (hoy `Host.mac` es siempre
  `None`) e `IcmpSendEcho2` para ping real (hoy `is_alive()` solo prueba TCP 80/445/22 y
  pierde impresoras, cámaras IP y equipos de red). En Linux, `pnet`. **`pnet`/`netscan`
  no se usan en Windows**: exigen Npcap, que no es redistribuible (edición libre limitada
  a 5 instalaciones, OEM de pago), lo que agravaría el problema de licencia que el ítem
  original quería resolver. Nmap nunca estuvo en el código: el escaneo ya era Rust puro.

  **Entregado solo en Windows (decisión del 2026-07-24).** La rama Linux con `pnet`
  quedó fuera del hito: el producto se distribuye como app Tauri para PCs municipales
  Windows, el soporte Linux completo ya está asignado al Horizonte (workstream Q), y
  `pnet` sigue marcado `verificar` en el Apéndice A. Sobre todo, no había forma de
  probarla en terreno, y este proyecto no da por funcionando lo que no vio correr. Linux
  conserva el ladder TCP anterior, sin cambio de comportamiento.

  **Medido el 2026-07-24 en una LAN real, no estimado.** Un /24 con 4 equipos encendidos:
  el descubrimiento anterior veía 1 host remoto y ninguna MAC; el nativo ve los 4, todos
  por ARP y con MAC única. El escaneo completo pasa de 18 s a 81 s. Dos correcciones a
  lo que este documento suponía: el limitador de ritmo del ARP cuesta solo 11 s de esa
  diferencia (con `arp_pps=0` el escaneo tarda 70 s), y **subir los hilos no compra
  nada** (64 → 81 s, 128 → 88 s, 253 → 82 s), porque Windows serializa la resolución de
  vecinos por dentro. El costo es inherente a `SendARP` y no hay optimización que lo
  evite. TI puede volver al comportamiento anterior con `red.arp: false`.
- **Corregir la detección de versión TLS.** Hoy `service_probe.rs` fija `"TLSv1.2"` en
  todo handshake exitoso, de modo que el control "TLS 1.0/1.1/SSLv3 activo" —marcado
  `Critical`— no puede dispararse nunca. Requiere sondeo por versión (`rustls` no sirve:
  no soporta TLS < 1.2).

**Cumplimiento alineado a ANCI (G)** — cifras legales ya verificadas contra fuente
oficial en la investigación; ver Apéndice B para lo que sigue pendiente:
- **Modelo dual de cumplimiento.** Las municipalidades están obligadas por los Arts. 4°,
  7° y 9° y por la IG N°1, pero **no son OIV**: la Res. Ex. N°87 las excluyó del primer
  proceso de calificación y la nómina preliminar de la segunda etapa tampoco las incluye,
  de modo que el Art. 8° y las IG N°3 y N°4 no las obligan hoy. Por tanto: lo exigible
  (Art. 7°, Art. 9°, IG N°1) se evalúa como cumplimiento con consecuencia legal; el
  Art. 8° se mide como **madurez voluntaria**, etiquetado como no exigible a la
  institución. El tier es un dato con fecha, no una constante: el Art. 6° obliga a la
  Agencia a revisar la calificación al menos cada tres años.
- **Preguntas nuevas para lo que sí obliga a un municipio**: deber general del Art. 7°, y
  la operativa del Art. 9° e IG N°1 (encargado de reportar designado con formación o
  experiencia técnica, casilla institucional registrada, Clave Única con segundo factor,
  nombramiento acreditado por firma electrónica avanzada).
- **Scoring de madurez 0-3 por dominio** (forma tomada del Essential Eight, CC BY 4.0,
  con atribución en el informe; los dominios se derivan de los Arts. 7° y 8°, no se
  copian los ocho controles del ASD, que son otra cosa).
- **Plan de remediación priorizado** (POA&M) derivado del gap report: cada brecha →
  acción, responsable, plazo. Orden de prioridad: (1) CVE en KEV, (2) calificación legal
  del incumplimiento según Art. 39°, (3) severidad, (4) CVSS. Los plazos sugeridos son
  criterio operativo, no legal, y los ajusta TI municipal en `munianci.config.json`; el
  único plazo perentorio del régimen es el reporte del Art. 9°.
- Puntaje numérico agregado exportable en el JSON CSIRT: mecánica SPRS (base fija menos
  deducciones ponderadas) pero **con los pesos anclados en el Art. 39°** —gravísima −5,
  grave −3, leve −1— en vez de una ponderación inventada. Los controles técnicos sin
  correlato en el Art. 39° usan una tabla propia, documentada como criterio técnico y no
  presentada como exigencia legal.
- Doble PDF: técnico por dominio + ejecutivo de una página (patrón CLARA).
- Cada pregunta mapeada a su artículo con ejemplo de evidencia, corrigiendo los anclajes
  excedidos actuales (la IG N°4 está citada en controles aplicables a todos los tiers,
  pero obliga solo a OIV) y las severidades que no coinciden con el Art. 39°.
- **Diferida a 0.6.0: la taxonomía del JSON CSIRT según la Res. Ex. N°7/2025.** El JSON
  sigue usando categorías propias. Motivo (decisión del 2026-07-24): el texto de la
  resolución no está en el repositorio ni se verificó contra fuente oficial, y el
  principio "No inventar" de este documento prohíbe codificar categorías legales sin la
  fuente primaria a la vista. Codificar una taxonomía aproximada sería peor que no tener
  ninguna: el JSON va al CSIRT Nacional con apariencia de estar alineado a la norma.
  Entra en 0.6.0 tras un pase de verificación contra la resolución publicada.
- Histórico de evaluaciones para mostrar evolución entre escaneos (patrón INÉS),
  reutilizando el slug `db_<comuna>` que ya usa el backend del Asistente como clave.
  **En SQLite embebido** (`rusqlite` con `bundled`), no en JSON: con `--scope lan` el
  escáner recorre el /24 completo, y un barrido semanal de ~250 equipos acumula decenas
  de miles de filas al año. Verificado el 2026-07-24 contra fuente chilena: ninguna norma
  obliga a un motor de base de datos. TI controla desde `munianci.config.json` si se
  guarda el desglose por activo y cuántos meses se retiene.
- **Diferido: export del histórico al formato de interoperabilidad del Estado.** El
  **Decreto 12 / Norma Técnica de Interoperabilidad** (Ley 21.180) regula cómo los
  órganos del Estado intercambian datos —nodo de interoperabilidad, protocolo MPGA-1—,
  no cómo los almacenan por dentro. Por eso no condiciona el motor elegido, pero sí
  condicionaría el **formato de salida** el día que este histórico se comparta con otro
  órgano. Entra cuando exista ese caso de uso concreto y tras estudiar el protocolo; hoy
  no hay destinatario identificado (decisión del 2026-07-24).

**Libs:** snapshot NVD (fkie-cad), crate `cpe`, `windows`/`windows-sys`, CISA KEV,
`pnet` (solo Linux), OSCAL (esquemas, no lib). Descartadas para este hito: Nuclei
(→ 0.7.0), `netscan`, `cpe2cve` empaquetado.

**Hecho cuando:** un escaneo produce CVEs reales offline, un puntaje de madurez 0-3 y
un plan de remediación, exportables en PDF (técnico + ejecutivo) y JSON.

---

## 0.6.0 — Monitoreo continuo y evidencia

Alcance ajustado tras el pase de investigación de 2026-07-24
(`docs/research/0.6.0-monitoreo-continuo-y-evidencia.md`, 94 búsquedas en dos pasadas más
la lectura íntegra de la Res. Ex. N°7/2025, la Res. Ex. N°187/2026 y las referencias del
Programador de tareas de Windows). Las cuatro decisiones de alcance las tomó el dueño del
repo el 2026-07-24.

**Monitoreo continuo y deriva (I):**
- **Deriva por control**, no solo el delta agregado que ya existe: cada control se clasifica
  contra la medición anterior como nueva / persistente / resuelta / **reaparecida**. Esa
  última es la que hoy no se puede expresar, y es la que habla del proceso municipal y no
  del parque de equipos. Sale de las tablas que `historico.rs` ya tiene, **sin cambio de
  esquema**, así que los históricos generados en 0.5.0 producen deriva desde el segundo
  escaneo. Coincide con la métrica "Vulnerability Reopen Rate" de la industria.
- **Reescaneo programado sin privilegios de administrador**: tarea por usuario del
  Programador de tareas, registrada **por XML** y no con banderas. Tres valores por defecto
  la romperían en silencio en un PC municipal y ninguno se puede fijar por línea de
  comandos: no corre con el equipo a batería (`DisallowStartIfOnBatteries`), se detiene si
  lo desenchufan (`StopIfGoingOnBatteries`), y el escaneo que se saltó por tener el equipo
  apagado no se recupera (`StartWhenAvailable`). Tampoco se usa `/sd`: interpreta la fecha
  según la configuración regional, la misma clase de error que la regresión de WMI ya
  documentada en `patch_level.rs`. Intervalo por defecto semanal, criterio operativo
  apoyado en la cadencia de CISA Cyber Hygiene, no un plazo legal.
- **Advertir antes de crear la tarea.** Crear una tarea programada es la sub-técnica
  T1053.005 de MITRE ATT&CK y queda en el evento 4698 de Windows: en una municipalidad con
  EDR el producto puede levantar una alerta de persistencia. Se avisa en la UI y en la
  documentación para que TI pueda coordinarlo. Mismo criterio que la decisión D1 de 0.4.0.
- **Aviso de escaneo vencido en la GUI**, que además es la red de seguridad cuando una GPO
  impide crear la tarea. Descartadas las notificaciones toast (nadie las ve en un PC
  compartido) y el correo SMTP (rompe offline-first).

**Paquetes de evidencia y auditoría (J):** carpeta fechada con los dos PDF, el JSON CSIRT,
el POA&M, el resumen del histórico, un `MANIFIESTO.sha256` en formato estándar y un
`COMO-VERIFICAR.txt` en castellano llano. Se verifica con `certutil -hashfile` y
`Get-FileHash`, que Windows ya trae, sin nuestro binario.
- **Sin par de claves, y no es un descuido.** Una clave privada generada en el PC municipal
  vive junto a la evidencia que firma: quien pueda alterar el informe puede volver a
  firmarlo. Agrega gestión de claves sin agregar seguridad.
- **Sin sellado de tiempo RFC 3161**: toda TSA es una llamada de red. La fecha del paquete
  es la del reloj del equipo, y el documento lo dice.
- **Es verificación de integridad, no firma electrónica.** Bajo la Ley 19.799 solo una FEA
  de un prestador acreditado da a un documento de un órgano del Estado la calidad de
  instrumento público. El paquete lo declara con esas palabras.

**Taxonomía de la Res. Ex. N°7/2025 (diferida desde 0.5.0):** entra como **catálogo
verificado, sin mapeo automático**. El Art. segundo clasifica "el hecho acaecido" y una
brecha detectada no lo es, así que asignarle categoría automáticamente afirmaría ante el
CSIRT Nacional un incidente que no ocurrió. El JSON lleva procedencia y conteos y deja
`clasificacion_incidente` en `null` hasta que una persona la complete.
**Completado (2026-07-24, `core/src/taxonomia.rs`).**

**Libs:** `sha2` (RustCrypto, MIT/Apache). Sin dependencias nuevas para la programación:
`std::process::Command` sobre `schtasks.exe`.

**Hecho cuando:** un reescaneo programado corre solo, marca la deriva respecto al
anterior, y emite un paquete de evidencia fechado y verificable con las herramientas que
el propio Windows trae.

---

## 0.7.0 — Escaneo profundo, multi-marco y riesgos

**Escaneo profundo y de activos (M):**
- Escaneo autenticado/credencial (config audit profundo tipo CLARA con privilegios de
  admin).
- Inventario de activos gestionado vía **osquery** (registro, BitLocker, certificados,
  servicios) en Windows y Linux.
- Mapa de topología de red (patrón CSET network architecture tool).
- Escaneo de aplicaciones web (OWASP Top Ten, patrón Cyber Hygiene), con **Nuclei**
  (diferido desde 0.5.0: es aquí, con sus plantillas web, donde su costo se justifica).
  Al adoptarlo hay que resolver el riesgo de falsos positivos de antivirus que la
  decisión D1 de 0.4.0 identificó para binarios grandes en PCs municipales.

**Multi-marco y cuestionarios preconfigurados (K):** soportar, además de Ley 21.663,
otros marcos (ISO 27001, NIST CSF, CIS Benchmarks) con cuestionarios preconfigurados
(patrón Continuum GRC/Onspring/CyberStrong). Amplía el mercado más allá de ANCI.

**Gestión de riesgos y remediación de ciclo completo (L):** registro de riesgos +
seguimiento de remediación con responsable/plazo/estado (no solo generar el POA&M,
gestionarlo hasta el cierre), con tablero (patrón PILAR).

**Libs:** osquery, Nuclei (+ plantillas pinneadas en `vendor/nuclei-templates/`).

**Hecho cuando:** un escaneo autenticado inventaría activos y dibuja la red, y el
municipio puede evaluarse contra al menos un marco adicional y seguir sus riesgos hasta
el cierre.

---

## 0.8.0 — Asistente avanzado y apoyo operativo ANCI

**Asistente avanzado (O):** subir ordenanzas propias del municipio por la UI (ingesta
sin CLI), historial de conversación persistente/exportable, navegación estructurada por
ley/artículo, grafo de citas legales (cross-references), y feedback loop.

**Apoyo operativo ANCI (P):** playbooks de respuesta a incidentes (IG N°4, contención),
flujo de designación del Delegado de Ciberseguridad, plantillas de SGSI/plan de
continuidad, y módulo de capacitación (Art. 8 lit. h). Verificar plazos/obligaciones
oficiales antes de codificarlos.

**Hecho cuando:** un funcionario puede cargar sus ordenanzas y consultarlas, y el
módulo guía la designación del Delegado y la respuesta a un incidente con plantillas.

---

## 0.9.0 — Calidad del Asistente (RAG)

Última parada antes del piloto 1.0: mejorar la calidad de recuperación/respuesta del
Asistente con el harness de 0.4.0 como métrica, no por reputación (principio
"Medible antes que ampliable"). Se separó de firma/licenciamiento (Horizonte, B/C)
porque la calidad del RAG no depende de esas decisiones comerciales y sí debe estar
lista para el piloto.

**Calidad base (D):**
- **Reranker CPU** (bge-reranker-v2-m3 ONNX; FlashRank/rerankers como alternativa
  liviana): recuperar top-20-30 híbrido y reordenar a top-5 antes del LLM. Hoy no
  existe etapa de reranking; es la palanca de calidad más directa.
- Verificar que la fusión híbrida use **RRF (k≈60)**, no promedio ponderado — LanceDB
  ya trae un `RRFReranker()` incorporado para hybrid search; evaluar si alcanza en vez
  de fusión hecha a mano.
- A/B de embeddings (nomic-v2-moe actual vs bge-m3) usando el harness de 0.4.0.
- Chunking recursivo consciente de estructura legal (artículo/inciso) + metadata.

**Ingesta y recuperación (investigación 2026-07-24, ~24 búsquedas — ver Referencias):**
- **Ingesta estructurada vía el servicio XML de LeyChile/BCN** (`obtxml?opt=7&idNorma=`):
  entrega los límites de artículo ya segmentados por BCN para leyes nacionales, en vez
  de inferirlos desde el layout del PDF. No cubre incisos/numerales (siguen en texto
  plano dentro de cada artículo) ni ordenanzas municipales (sin equivalente XML).
- **Indexación consciente de vigencia**: agregar `vigente_desde`/`vigente_hasta` (o un
  flag `derogado`) al schema de LanceDB — hoy no hay forma de distinguir texto vigente
  de un artículo derogado/reemplazado al reingestar una ley modificada. LeyChile ya
  expone esta fecha por norma.
- **Recuperación parent-document (small-to-big)**: buscar sobre el chunk pequeño
  existente, pero inyectar el artículo completo (el "padre") como contexto al LLM.
  Casi gratis una vez que exista chunking consciente de estructura.
- **Stemming en español para el índice BM25/tantivy**: hoy el lado disperso del hybrid
  search corre sin normalización morfológica sobre texto en español (tantivy trae un
  `Language::Spanish` / crate `tantivy-stemmers`).
- **Truncamiento Matryoshka** sobre el embedding que gane el A/B (nomic-v2-moe y
  bge-m3 soportan MRL): recorte a 256-512 dims para bajar footprint de LanceDB/CPU;
  validar con el harness, no asumir las cifras genéricas de MTEB.
- **Docling** para el parsing offline de ordenanzas municipales (ruta PDF que no tiene
  equivalente XML de BCN, a diferencia de las leyes nacionales) — candidato si la
  calidad de extracción de tablas/layout resulta un problema real.
- **HyPE o Summary-Augmented Chunking** como alternativa barata (sin llamada LLM en
  tiempo de consulta) a "contextual retrieval" completo: preguntas sintéticas por
  chunk (HyPE) o un resumen a nivel de documento antepuesto a cada chunk (SAC),
  apuntando al riesgo real de confundir ordenanzas municipales casi idénticas entre sí.
  Vale una prueba acotada antes de comprometerse — falta leer el paper de SAC completo.

**Descartado (con evidencia, no por omisión):** chunking semántico (peor costo/beneficio
que el chunking consciente de estructura ya planeado, según comparativas 2026); late
chunking completo (el modelo de embeddings actual tiene ventana de 512 tokens, muy
corta para que rinda); ColBERT/multi-vector (el reranker ya planeado cubre gran parte
del mismo beneficio para un corpus de este tamaño); buscar un embedding específico
"legal en español" (no se encontró ninguno con evidencia de retrieval mejor que
bge-m3/nomic — fine-tunear el ganador del A/B sobre pares sintéticos de este corpus es
el camino con más evidencia si se necesita más ganancia).

**Libs:** bge-reranker-v2-m3 (ONNX), FlashRank/rerankers, bge-m3, tantivy-stemmers,
Docling.

**Hecho cuando:** el harness muestra mejora medible de recall/faithfulness/citación con
el reranker y las mejoras de ingesta activas, listo para servir de base al piloto 1.0.

---

## 1.0.0 — Release de producción

- **Piloto en 1-2 municipios reales** + corrección de los bugs que salgan en terreno.
  El instalador del piloto va **sin firmar** (la firma de código pasa a Horizonte, ver
  más abajo); aceptable para una distribución acotada y de confianza directa, no para
  una descarga pública masiva (advertencia de SmartScreen esperable).
- **Endurecimiento técnico:** auditoría de seguridad del app combinado (Tauri + LLM) y
  el harness de evaluación como gate de release.
- **Verificación legal:** confirmar todas las cifras/plazos ANCI marcados "verificar"
  (Apéndice B) contra fuente oficial (Res. Ex. N°87, Reglamento de Reporte de
  Incidentes) antes de cualquier afirmación client-facing.
- **Documentación de despliegue** y manual de operador.

**Hecho cuando:** el piloto valida el flujo completo, la auditoría no deja hallazgos
críticos, y las afirmaciones legales están verificadas y documentadas.

---

## Horizonte (post-1.0)

**Catálogo OSCAL de la Ley 21.663, y con él Assessment Results:**
- Publicar los controles del producto como un **catálogo OSCAL** propio (Arts. 7°, 8° y
  9°, IG N°1). No es una invención: los artículos están verificados y los controles ya
  existen en `compliance_engine`; lo que falta es expresarlos en el modelo `catalog` con
  identificadores estables. Tiene valor por sí solo, más allá de OSCAL: es la lista de
  controles de la ley, citable.
- Recién con ese catálogo tiene sentido emitir **Assessment Results**, porque su campo
  `reviewed-controls` exige IDs de control que resuelvan contra un catálogo real. El
  `import-ap` (href al *Assessment Plan*) lo aportaría TI municipal desde
  `munianci.config.json`. Diferido desde 0.5.0 el 2026-07-24 por esta dependencia.

**Firma de código y auto-update (B):**
- Certificado **OV** (no EV: desde marzo 2024 SmartScreen ya no da bypass instantáneo
  con EV, así que EV no justifica su costo). Verificar elegibilidad de Azure Trusted
  Signing para org chilena; si no aplica, OV de Sectigo/Certum.
- Updater de Tauri v2 solo para la app (instalador chico), con la clave privada del
  updater resguardada. Los modelos se gestionan aparte (checksum + reanudación).

**Licenciamiento offline (C):**
- Token de licencia **firmado con Ed25519** (envelope JSON tipo Keygen, firmado y
  verificado con **PyNaCl**), verificado offline con la clave pública embebida. Claims:
  comuna, institución, `exp`, `features[]`, `fingerprint`, y un `kid` para rotación de
  clave desde el día 1. La clave privada nunca sale del entorno de emisión.
- Node-locking con **fingerprint tolerante**: hash de 2-3 componentes estables; exigir
  que coincida un subconjunto, no todos, para no romper licencias al cambiar un disco.
  Mostrar el fingerprint en la UI para el flujo de emisión manual (USB/email).
- TTL + grace period como "revocación" pragmática. **No sobre-ingeniar**: sin cifrado
  AES del payload, sin anti-debug, sin servidor on-prem. La firma evita falsificación;
  el enforcement del cliente es best-effort, adecuado al bajo riesgo municipal.

**Fidelidad de citas / anti-alucinación (E):**
- **Verificación de cita textual (quote-in-source):** toda referencia legal (art. N°,
  nombre de norma) debe existir literalmente en un chunk recuperado; si no, se marca o
  suprime. Defensa directa y de bajo costo contra artículos inventados.
- **Abstención por contexto insuficiente:** umbral de recuperación + instrucción de
  "responde solo si el contexto lo respalda". Incluir casos negativos en el eval para
  verificar que sí se abstiene.

**Libs (B, C):** PyNaCl.

- **Q — Integraciones y escala:** integraciones (ticketing/SIEM), API para automatización
  del reporte al CSIRT, benchmarking anónimo entre comunas, y soporte Linux/multiplataforma
  completo.
- **N — Multiusuario y roles:** control de acceso por rol (jefe de servicio vs TI), vistas
  diferenciadas y notificaciones. Sin asignar a un hito pre-1.0; se puede adelantar si el
  piloto lo pide.

---

## En vigilancia — estándares básicos obligatorios de la ANCI

**No es un hito. Es algo que hay que mirar, y que puede reordenar varios.** Descubierto en
la segunda pasada de investigación de 0.6.0 (2026-07-24); detalle y fuentes en
`docs/research/0.6.0-monitoreo-continuo-y-evidencia.md` §8.1.

La **Res. Ex. N°140 de 2026** de la ANCI (D.O. 30-05-2026) convocó a consulta pública para
establecer como **obligatorias seis de las nueve** recomendaciones del documento "Los 9
básicos de ciberseguridad", **para todos los sujetos obligados por la Ley 21.663**. La
consulta corrió 30 días corridos y cerró el 29-06-2026. El texto final aún no se publica.

Los nueve, según `anci.gob.cl/9basicos/`: actualizar periódicamente; capacitar
periódicamente; minimizar privilegios; respaldar periódicamente la información; asegurar
redes; asegurar equipos; monitorear en tiempo real; usar MFA; usar gestor de contraseñas.

**Por qué importa.** "Todos los sujetos obligados" incluye a los prestadores de servicios
esenciales, y una municipalidad lo es por el Art. 4°. El hallazgo central de la
investigación de 0.5.0 fue que el Art. 8° obliga solo a los OIV, y que por eso casi todo el
cuestionario declarativo quedaba inaplicable al cliente objetivo del producto. Esta
normativa es lo que llena ese vacío: sería el **primer cuerpo de controles efectivamente
exigible a una municipalidad**, y el escáner ya mide varios de los nueve de forma técnica.

**Cuáles seis son los elegidos no es público.** Ninguna fuente identifica el subconjunto.
No se codifica nada hasta leer la resolución final completa, igual que se hizo con la
Res. Ex. N°7/2025. Cuando se publique, se decide si es un hito propio y qué le pasa a la
parte multi-marco de 0.7.0.

---

## Apéndice A — Bibliotecas OSS candidatas

Libs seleccionadas para adopción. "Licencia" es lo hallado en la investigación;
`verificar` = confirmar el archivo `LICENSE`/términos antes de anclar la dependencia.

| Biblioteca | Licencia | Hito | Qué aporta |
|---|---|---|---|
| snapshot NVD (fkie-cad/nvd-json-data-feeds) | ToU de NVD + CVE, redistribución **verificada OK** con avisos | 0.5.0 | Base CVE offline empaquetable (93,86 MB xz) |
| crate `cpe` | verificar | 0.5.0 | Parseo CPE 2.3 WFN / 2.2 URI para el matcher Rust |
| `windows` | MIT/Apache-2.0 | 0.5.0 | `SendARP` e `IcmpSendEcho2` sin dependencia pcap. **Adoptada en 0.5.0** (features `Win32_NetworkManagement_IpHelper` y `Win32_System_IO`); `windows-sys` no hizo falta |
| pnet (crates Rust) | verificar por crate | Horizonte (Q) | Escaneo de red nativo **solo en Linux**. Diferido desde 0.5.0: no se pudo probar en terreno |
| CISA KEV | CC0 | 0.5.0 | Priorización por explotación observada |
| OSCAL (esquemas NIST) | publicación NIST, dominio público | 0.5.0 | Formato de Assessment Results y POA&M |
| nvdtools (`cpe2cve`) | Apache-2.0 **verificada** | — | Solo oráculo de pruebas en desarrollo; no se empaqueta |
| cpe-guesser | BSD-2-Clause **verificada** | — | Ayuda de build para poblar la tabla curada slug→CPE |
| Nuclei | MIT **verificada** (motor y plantillas) | 0.7.0 | Motor de detección por plantillas YAML (diferido desde 0.5.0) |
| osquery | Apache-2.0 / GPLv2 (dual) | 0.7.0 | Inventario del host vía SQL |
| bge-reranker-v2-m3 (ONNX) | Apache-2.0 | 0.9.0 | Reranking cross-encoder en CPU |
| FlashRank / rerankers | MIT/Apache, verificar | 0.9.0 | Reranking ONNX ultraliviano (alternativa) |
| bge-m3 (GGUF) | BAAI, verificar | 0.9.0 | Embeddings multilingües para A/B |
| tantivy-stemmers | verificar | 0.9.0 | Stemming español para el índice BM25/FTS |
| Docling | Apache-2.0 | 0.9.0 | Parsing offline de PDFs de ordenanzas (tablas/layout) |
| Ragas | Apache-2.0, verificar | 0.4.0 | Evaluación RAG offline (faithfulness, citación) |
| DeepEval | Apache-2.0, verificar | 0.4.0 | Evals estilo Pytest para gates de CI |
| PyNaCl | Apache-2.0 | Horizonte | Firma/verificación Ed25519 del token de licencia |
| hf_transfer | verificar | 0.4.0 | Descarga resumible de modelos (chunks + SHA256) |
| aria2c | GPLv2 (binario externo, no enlazado) | 0.4.0 | Descarga resumible alternativa |
| cargo-cyclonedx | verificar | 0.4.0 | SBOM CycloneDX (Rust) |
| cargo-sbom | verificar | 0.4.0 | SBOM SPDX + CycloneDX |
| cargo audit / deny / auditable | verificar | 0.4.0 | Auditoría de dependencias Rust (gate CI) |
| pip-audit | Apache-2.0, verificar | 0.4.0 | Auditoría del sidecar Python (gate CI) |

Se mantienen sin cambio (sin razón técnica para migrar): **LanceDB** (Apache-2.0),
**llama.cpp** (MIT), **cryptography** (PyCA, ya en dependencias). Evitar embeber por
licencia: **Nmap** (NPSL), **Npcap** (no redistribuible: edición libre limitada a 5
instalaciones, OEM de pago — arrastrado por `pnet`/`netscan` en Windows) y **Steampipe**
(marca comercial de Turbot).

**Todas** las libs de esta tabla (más llama.cpp, LanceDB y las actuales) se mantienen
en el mirror `vendor/` (Apéndice C), por si el upstream las remueve.

## Apéndice B — Banderas a verificar

Antes de codificar como reglas o usar en material client-facing.

### Verificado el 2026-07-24 contra fuente primaria

Detalle y citas exactas en `docs/research/0.5.0-escaner-y-cumplimiento-anci.md` §1.

- **Plazos de reporte de incidentes** — confirmados en el **Art. 9° de la ley misma**, no
  en fuente secundaria: alerta temprana 3 h, actualización 72 h, informe final 15 días
  corridos. Aparecen además dos reglas que este apéndice omitía: la actualización baja a
  **24 h** cuando el afectado es OIV y sus servicios esenciales están comprometidos, y el
  OIV debe adoptar un **plan de acción en 7 días corridos**. Reglamento aplicable:
  DS N°295 de 2024 (D.O. 01-03-2025). Taxonomía a usar: Res. Ex. N°7/2025 de ANCI.
- **Estatus de las municipalidades** — **no son OIV**. Están obligadas por el Art. 4°
  (sus servicios son esenciales por definición legal, vía Art. 1°), el Art. 7° y el
  Art. 9°, más la IG N°1. Pero la Res. Ex. N°87, sección VII numeral 3, las excluyó
  expresamente del primer proceso de calificación, y la nómina preliminar de la segunda
  etapa (24-04-2026) tampoco las incluye. Por tanto el Art. 8° y las IG N°3 y N°4 —que se
  dirigen a OIV— no las obligan hoy. Revisable: el Art. 6° obliga a recalificar al menos
  cada tres años.
- **Taxonomía de incidentes de la Res. Ex. N°7/2025** — **verificada el 2026-07-24 contra
  el Diario Oficial** (N° 44.088 del 01-03-2025, CVE 2617388, 4 pp.), leída íntegra.
  Cuatro áreas de impacto y once efectos observables (Art. tercero); cuarenta categorías
  (Art. cuarto). Obliga a "las instituciones públicas y privadas que presten servicios
  calificados como esenciales" (Art. primero), o sea también a una municipalidad. Ojo con
  el Art. segundo: clasifica por "los efectos observables del **hecho acaecido**", así que
  no se le puede asignar una categoría a una brecha detectada. Transcrita en
  `core/src/taxonomia.rs`.
- **Nómina OIV de la segunda etapa** — la Res. Ex. N°187/2026 (D.O. 24-07-2026, CVE
  2842835) cerró el primer proceso de calificación y **tampoco incluye municipalidades**.
  Los rubros de esa etapa son combustibles, agua potable y saneamiento, transporte,
  concesionarios de servicios públicos, seguridad social, postal y farmacéutico.
- **Multas del Art. 40°** — leves hasta 5.000 UTM (10.000 OIV), graves 10.000 (20.000
  OIV), gravísimas 20.000 (40.000 OIV). Las constantes de `report_builder.rs` coinciden
  exactamente; no requieren cambio.
- **Redistribución de datos NVD/CVE** — permitida. NVD pide el aviso "This product uses
  the NVD API but is not endorsed or certified by the NVD"; MITRE otorga licencia
  irrevocable a condición de reproducir su aviso de copyright y la licencia en cada copia.
- **Licencias OSS del hito 0.5.0** — Nuclei y sus plantillas MIT; nvdtools Apache-2.0;
  cpe-guesser BSD-2; KEV CC0; Essential Eight CC BY 4.0. **Npcap NO es redistribuible**,
  lo que descarta `pnet`/`netscan` en Windows.

### Pendiente

- **Alcance real de las IG N°2, N°3 y N°4** (D.O. 26-12-2025). La investigación de 0.5.0
  concluyó que la IG N°4 obliga solo a OIV, y sobre esa base se corrigieron anclajes
  legales en `compliance_engine`. La segunda pasada de investigación de 0.6.0 encontró que
  las fuentes secundarias **se contradicen**: el sitio de la ANCI las presenta como
  aplicables de forma amplia bajo la Ley 21.663 y un estudio jurídico las titula "para
  Servicios Esenciales y OIV". Hay que leer los cuatro textos oficiales completos, como se
  hizo con la Res. Ex. N°7. Si el alcance fuera más amplio, varios anclajes quedarían
  subestimados. Enlaces del Diario Oficial en
  `docs/research/0.6.0-monitoreo-continuo-y-evidencia.md` (Referencias).
- **Precios de certificados de firma y de Azure Trusted Signing**, y la elegibilidad
  geográfica de Azure para una org chilena: confirmar con el CA/Microsoft.
- **Licencias OSS** que siguen marcadas `verificar` en el Apéndice A (crate `cpe`,
  `windows`/`windows-sys`, `pnet`, y las de los hitos 0.4.0 y 0.9.0). `deny.toml` ya
  actúa como compuerta automática para las de crates.
- **Licencia de EPSS (FIRST)** antes de anclarlo como fuente de priorización.
- **Cifras de mejora** (NDCG del reranker, ganancias de embeddings): son referenciales
  de fuentes divulgativas; medir con el harness propio antes de afirmarlas.
- **Aplicabilidad de cualquier obligación a un municipio concreto**: nada de lo anterior
  es asesoría legal. Antes de material client-facing, validar con un abogado.

## Apéndice C — Vendoring (mirror local `vendor/`)

Toda dependencia OSS adoptada se copia a `vendor/`, por si el upstream desaparece o
yanquea la versión. Se preserva el `LICENSE` de cada una. Mecanismo por tipo:

| Tipo | Ubicación | Mecanismo | Notas |
|---|---|---|---|
| Crates Rust (pnet, netscan, cargo-*) | `vendor/cargo/` | `cargo vendor` + `.cargo/config.toml` | Texto; versionable o git-lfs |
| Wheels Python (Ragas, DeepEval, PyNaCl, pip-audit, hf_transfer, FlashRank) | `vendor/wheels/` | `pip download` → instalar con `--no-index --find-links` | Wheelhouse offline |
| Binarios externos (Nuclei, aria2c) | `vendor/bin/` | Release pinneado por versión + SHA256 | Como el `llama-server` actual |
| Plantillas / reglas (Nuclei templates) | `vendor/nuclei-templates/` | Snapshot pinneado | Evita `-update-templates` en runtime |
| Modelos (bge-reranker ONNX, bge-m3, nomic, Qwen GGUF) | `vendor/models/` | Archivo pinneado + SHA256 | Grande; es el "paquete offline" de D2 |
| Datos (snapshot NVD, KEV) | `vendor/nvd/` | Snapshot pinneado + SHA256 | Redistribución verificada OK (Apéndice B); incluir los avisos de NVD y MITRE |

Los artefactos grandes (modelos, snapshot NVD, binarios) son gitignored por tamaño y se
distribuyen como el paquete offline (D2), no por git; los pequeños (crates, wheels,
plantillas, config) pueden versionarse o ir en git-lfs. El objetivo es que un build
reproducible funcione **sin red**, coherente con el principio offline-first. La política
vive en `vendor/README.md`.

## Apéndice D — Entorno competitivo

Registro de actores que ya están dentro de la municipalidad chilena. No es análisis de
mercado: es dejar anotado quién tiene la relación comercial y el acceso, porque eso pesa
más que la comparación de funcionalidades.

### SMC — Sistemas Modulares de Computación SpA

Detectado el 2026-07-24 al investigar qué base de datos usan los sistemas municipales
chilenos. Es el hallazgo incómodo de esa búsqueda.

| Dato | Verificado |
|---|---|
| Antigüedad y foco | Se presenta con más de 40 años dando solución a servicios informáticos municipales (smc.cl) |
| Condición | Proveedor del Estado de Chile (registro de proveedores) |
| Municipios contratantes | Contrato publicado por La Cisterna en su portal de transparencia; portal de gestión documental operativo para Renca (`sgd-renca.smc.cl`) |
| Alcance del producto | Suite modular de gestión municipal: bienes, gestión documental, atenciones, y licitaciones que agrupan Salud, Educación y Municipal |

**Por qué importa.** No compite hoy en cumplimiento de la Ley 21.663 —no se verificó que
ofrezca nada de ciberseguridad ni ANCI—, pero ya está adentro: tiene el contrato, el
soporte, el canal y cuarenta años de relación con el área de TI que sería nuestro
usuario. Un módulo suyo de cumplimiento llegaría al municipio sin licitación nueva.

**Lo que NO se verificó y no debe suponerse:** si tiene o planea un producto de
ciberseguridad, su cobertura real de comunas, sus precios, ni su arquitectura técnica.
Cualquier afirmación sobre eso hay que comprobarla antes de escribirla.

**Consecuencia para el producto.** Refuerza dos decisiones ya tomadas: el binario único
sin servidor ni licencia de base de datos (no exige nada de la infraestructura que SMC ya
administra) y la operación totalmente offline. La diferenciación defendible es la
especialización legal —Ley 21.663, IG de la ANCI, taxonomía de la Res. Ex. N°7/2025— y no
la gestión municipal general, donde el incumbente lleva cuatro décadas.

## Referencias

Marco regulatorio (Chile):
- Ley 21.663, texto oficial — https://www.bcn.cl/leychile/navegar?idNorma=1202434
- ANCI, obligación de reportar — https://anci.gob.cl/noticias/obligacion-de-reportar/
- ANCI, nómina OIV (Res. Ex. N°87) — https://anci.gob.cl/noticias/anci-presenta-nomina-de-oiv-correspondiente-al-primer-procedimiento-de-calificacion/
- ACHM, ciberseguridad municipal (dic-2025) — https://www.achm.cl/wp-content/uploads/2025/12/Ciberseguridad-Municipal-Desafios-y-Estrategias.pdf

Entorno competitivo (Apéndice D):
- SMC, Sistemas Modulares de Computación — https://smc.cl/
- SMC como proveedor del Estado — https://www.todolicitaciones.cl/proveedor/861302008/sistemas-modulares-de-computacion-spa
- Contrato SMC publicado por La Cisterna — http://transparencia.cisterna.cl/archivos/CONTRATOS/INTERNOS_AL_MUNICIPIO/SISTEMA_MODULARES_SMC/CONTRATO_SMC.pdf

Norma técnica chilena aplicable al intercambio de datos entre órganos del Estado:
- Decreto 12 / Norma Técnica de Interoperabilidad (Ley 21.180) — https://www.bcn.cl/leychile/Navegar?idNorma=1195125&idVersion=2023-08-17
- Guía Técnica de Interoperabilidad — https://wikiguias.digital.gob.cl/es/guias/guia-tecnica-interoperabilidad

Herramientas de cumplimiento gubernamentales:
- CSET (CISA) — https://www.cisa.gov/resources-tools/services/cyber-security-evaluation-tool-csetr
- CCN-CERT CLARA (España) — https://www.ccn-cert.cni.es/es/soluciones-seguridad/clara.html
- Essential Eight Maturity Model (Australia) — https://www.cyber.gov.au/business-government/asds-cyber-security-frameworks/essential-eight/essential-eight-maturity-model
- Cyber Essentials (Reino Unido) — https://www.ncsc.gov.uk/cyberessentials/overview
- NIST SP 800-171 assessment methodology — https://www.acq.osd.mil/asda/dpc/cp/cyber/docs/safeguarding/NIST-SP-800-171-Assessment-Methodology-Version-1.2.1-6.24.2020.pdf

Productos comerciales / OSS de escaneo:
- Vanta, automated compliance — https://www.vanta.com/products/automated-compliance
- OpenSCAP — https://www.open-scap.org/
- Nuclei — https://github.com/projectdiscovery/nuclei
- osquery — https://github.com/osquery/osquery
- nvdtools — https://github.com/facebookincubator/nvdtools
- NVD JSON data feeds (fkie-cad) — https://github.com/fkie-cad/nvd-json-data-feeds

RAG offline:
- bge-reranker-v2-m3 (ONNX) — https://huggingface.co/onnx-community/bge-reranker-v2-m3-ONNX
- RAGAS (evaluación offline) — https://www.langchain.com/blog/evaluating-rag-pipelines-with-ragas-langsmith
- Reciprocal Rank Fusion — https://glaforge.dev/posts/2026/02/10/advanced-rag-understanding-reciprocal-rank-fusion-in-hybrid-search/
- nomic-embed-text-v2-moe (GGUF) — https://huggingface.co/nomic-ai/nomic-embed-text-v2-moe-GGUF
- LanceDB hybrid search / `RRFReranker()` — https://docs.lancedb.com/search/hybrid-search
- BCN LeyChile, servicio XML (`obtxml`), ejemplo Ley 21.719 — https://www.leychile.cl/Consulta/obtxml?opt=7&idNorma=1209272
- BCN, acceso a normas desde otros sistemas (esquema XML) — https://www.leychile.cl/esquemas/accesoLeyesChilenas4.pdf
- VersionRAG (retrieval consciente de versión/vigencia) — https://arxiv.org/html/2510.08109v1
- Small-to-big / parent-document retrieval — https://medium.com/data-science/advanced-rag-01-small-to-big-retrieval-172181b396d4
- tantivy, stemmer por idioma (`Language::Spanish`) — https://docs.rs/tantivy/latest/tantivy/tokenizer/enum.Language.html
- Matryoshka embeddings (MRL) — https://sbert.net/examples/sentence_transformer/training/matryoshka/README.html
- Chunking consciente de estructura en texto legal (comparativa) — https://arxiv.org/pdf/2605.19806
- Comparativa de métodos de chunking, costo/beneficio — https://arxiv.org/pdf/2606.00881
- Summary-Augmented Chunking, RAG legal (NLLP 2025) — https://arxiv.org/abs/2510.06999
- Docling (parsing offline de documentos) — https://procycons.com/en/blogs/pdf-data-extraction-benchmark/

Fidelidad de citas / anti-alucinación:
- Stanford RegLab, "Hallucination-Free?" — https://reglab.stanford.edu/publications/hallucination-free-assessing-the-reliability-of-leading-ai-legal-research-tools/
- OWASP Top 10 for LLM Applications (2025) — https://owasp.org/www-project-top-10-for-large-language-model-applications/

Licenciamiento offline:
- Keygen, offline licenses — https://keygen.sh/docs/choosing-a-licensing-model/offline-licenses/
- PyNaCl, signing — https://pynacl.readthedocs.io/en/latest/signing/

Empaquetado y firma (Tauri + Python):
- Tauri v2 sidecar — https://v2.tauri.app/develop/sidecar/
- Tauri v2 updater — https://v2.tauri.app/plugin/updater/
- Tauri v2 security — https://v2.tauri.app/security/
- Límite ~2 GB del instalador NSIS/WiX — https://github.com/tauri-apps/tauri/issues/7372
- Code signing OV vs EV — https://www.ssl.com/faqs/which-code-signing-certificate-do-i-need-ev-ov/
- cargo-cyclonedx — https://github.com/CycloneDX/cyclonedx-rust-cargo
- pip-audit — https://github.com/pypa/pip-audit
