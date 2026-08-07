<p align="center">
  <img src="assets/logo.svg" alt="Logo de MuniGPT: escáner de ciberseguridad y asistente legal para municipios de Chile" width="120" height="120">
</p>

<h1 align="center">MuniGPT</h1>

<p align="center">
  <strong>Software de ciberseguridad para municipios y organismos del Estado de Chile: escáner de cumplimiento de la Ley 21.663 (ANCI) más un asistente legal RAG que funciona 100% offline.</strong>
</p>

<p align="center">
  <img alt="Versión 0.8.1" src="https://img.shields.io/badge/versi%C3%B3n-0.8.1-3b82f6">
  <img alt="Licencia MIT" src="https://img.shields.io/badge/licencia-MIT-22c55e">
  <img alt="Plataforma Windows 10 y 11" src="https://img.shields.io/badge/plataforma-Windows%2010%2F11-334155">
  <img alt="Construido con Rust y Tauri 2" src="https://img.shields.io/badge/Rust-Tauri%202-dea584">
  <img alt="Funciona offline y air-gapped" src="https://img.shields.io/badge/offline-air--gapped-7dd3fc">
</p>

MuniGPT es una herramienta de escritorio de **ciberseguridad municipal** para organismos del Estado chileno, alineada a la **Ley 21.663 (Marco de Ciberseguridad)** y a las Instrucciones Generales de la **ANCI** (Agencia Nacional de Ciberseguridad). Reúne dos módulos en una sola aplicación Tauri, sin que ningún dato institucional salga del equipo:

- **Escáner de cumplimiento** — escaneo activo de red más un cuestionario declarativo de autoevaluación. Produce **dos PDF** (técnico por dominio y ejecutivo de una plana), un **reporte JSON listo para el CSIRT Nacional de Chile** y un **plan de remediación priorizado en OSCAL POA&M**. Detecta shares SMB anónimos, firewall desactivado, protocolos en claro, TLS débil, software y sistemas operativos en fin de vida (EOL), y **vulnerabilidades conocidas (CVE) con una base de NVD embebida**, priorizadas por el catálogo **KEV de CISA**: lo que se está explotando hoy va primero.
- **Asistente legal offline (RAG)** — asistente con IA local (antes producto propio, **MuniGPT**) que responde consultas sobre normativa municipal chilena citando el corpus legal, con toda la inferencia corriendo en el equipo vía **llama.cpp** + **LanceDB**. Vive en `assistant/` y corre como *sidecar* del proceso Tauri. Ver [assistant/README.md](assistant/README.md).

**Palabras clave:** ciberseguridad municipal, Ley 21.663, ANCI, cumplimiento normativo, escáner de vulnerabilidades, autoevaluación CSIRT, asistente legal con IA offline, RAG, Rust, Tauri, Chile.

## Índice

- [Aviso legal](#aviso-legal) · [Despliegue de emergencia](#despliegue-de-emergencia) · [Requisitos](#requisitos) · [Instalación](#instalación) · [Uso CLI](#uso--cli) · [Configuración](#configuración-para-ti-municipal) · [Uso GUI](#uso--gui) · [Controles evaluados](#controles-evaluados) · [Salidas](#salidas-que-produce-un-escaneo) · [Arquitectura](#arquitectura) · [Pruebas](#pruebas) · [Marco normativo](#marco-normativo) · [Licencia](#licencia)
- Hoja de ruta a la versión 1.0: [ROADMAP.md](ROADMAP.md)

---

## Aviso legal

El uso de esta herramienta en redes de organismos del Estado requiere:

1. Inscripción previa en el registro de la ANCI ([portal.anci.gob.cl](https://portal.anci.gob.cl))
2. Notificación previa a la ANCI del acceso
3. Reporte de vulnerabilidades al responsable del sistema y a la ANCI

Cumplidas estas condiciones, el acceso queda amparado por el **safe harbor del Art. 2° Ley 21.459**. Sin ellas, puede constituir acceso ilícito.

---

## Despliegue de emergencia

Procedimiento corto para dejar un instalador marcado en manos de una institución cuando no hay tiempo de leer el resto de este archivo. Los detalles de cada paso están en [Compilación por cliente](#compilación-por-cliente).

**El equipo donde se compila necesita internet. El equipo donde se instala, no.** El empaquetador descarga el instalador offline de WebView2 y su propio herramental de bundling; sin DNS la compilación termina en `failed to bundle project: io: No such host is known (os error 11001)` después de haber compilado todo el Rust, que es la peor hora para enterarse.

```powershell
# 1. Congelar el sidecar del Asistente (deja ~930 MB en assistant\backend\dist\)
powershell -ExecutionPolicy Bypass -File tools\empaquetar-asistente.ps1

# 2. Marcar e instalar, desde gui\
$env:MUNIGPT_INSTITUTION = "Ejército de Chile"
$env:MUNIGPT_TIER        = "pse"
$env:MUNIGPT_ADMIN_HASH  = "<cadena PHC de Argon2id para esta institución>"
cargo tauri build --config tauri.asistente.conf.json

# 3. Sellar lo que se entrega
Get-FileHash target\release\bundle\nsis\*.exe -Algorithm SHA256
```

**Paso 4, que no es opcional: probarlo desconectado antes de entregarlo.** Instalar el `.exe` en un equipo con el adaptador de red deshabilitado, abrirlo y confirmar que las tres vistas cargan, que un escaneo `local` termina y que el Asistente responde una consulta citando el corpus. Un instalador que se arma sin errores puede fallar igual en el equipo de destino, y ahí ya está entregado.

Si el Asistente debe viajar en el instalador, el overlay `--config tauri.asistente.conf.json` es obligatorio: sin él sale un paquete solo-escáner que instala sin quejarse y no trae el módulo.

---

## Requisitos

- Rust 1.78+ (`rustup update stable`)
- Node.js 20+ y npm 10+ (solo para compilar la GUI)
- Windows 10/11 o Linux (Ubuntu 22.04+)
- WebView2 Runtime (Windows): requerido para compilar/ejecutar desde el código; el instalador de release lo incluye en su versión offline, así que los equipos municipales air-gapped no necesitan instalarlo aparte (en Win11 ya viene preinstalado)
- Privilegios de administrador local recomendados (BitLocker, WMI)

---

## Instalación

```powershell
git clone https://github.com/fcarvajalbrown/MuniGPT
cd MuniGPT
cargo build --release -p munigpt-cli
```

El binario CLI queda en `target\release\munigpt-cli.exe`.

Para compilar la GUI ver sección **Compilación por cliente** más abajo.

---

## Uso — CLI

```powershell
# Escaneo local con cuestionario interactivo
munigpt-cli --name "Municipalidad de X" --tier pse --scope local

# Escaneo de toda la LAN, sin cuestionario
munigpt-cli --name "Municipalidad de X" --tier pse --scope lan --no-questionnaire

# Salida personalizada
munigpt-cli --name "..." --pdf informe.pdf --json csirt.json --poam plan.json

# Generar el archivo de configuración de ejemplo y salir
munigpt-cli --escribir-config munigpt.config.json

# Escaneo que además deja un paquete de evidencia listo para presentar
munigpt-cli --name "Municipalidad de X" --evidencia C:\evidencia

# Programar el reescaneo periódico (no requiere ser administrador)
munigpt-cli --programar --name "Municipalidad de X" --scope local
munigpt-cli --desprogramar
```

Con `--no-questionnaire` los controles declarativos quedan **sin evaluar**, no reprobados: el plan de remediación los lista como "primero hay que verificarlo". Es la diferencia entre no cumplir y no haber mirado.

### Flags CLI

| Flag | Default | Descripción |
|------|---------|-------------|
| `--name` | `"Organismo del Estado"` | Nombre de la institución |
| `--tier` | `pse` | Clasificación: `oiv`, `pse`, `unclassified` |
| `--scope` | `local` | Alcance: `local`, `lan` |
| `--pdf` | `informe_brechas.pdf` | Ruta del informe técnico. El ejecutivo se deriva de esta ruta (`informe_brechas_ejecutivo.pdf`) |
| `--json` | `csirt_report.json` | Ruta del reporte JSON para el CSIRT |
| `--poam` | `poam.json` | Ruta del plan de remediación en OSCAL POA&M |
| `--no-questionnaire` | — | Omite el cuestionario declarativo |
| `--evidencia` | — | Genera un paquete de evidencia fechado y sellado por hash en la carpeta indicada |
| `--programar` | — | Registra el reescaneo periódico en el Programador de tareas y sale |
| `--desprogramar` | — | Quita el reescaneo periódico y sale |
| `--programado` | — | Modo no interactivo, para las corridas del reescaneo programado |
| `--escribir-config` | — | Escribe un `munigpt.config.json` de ejemplo comentado y sale |
| `--version` | — | Versión del binario |

> **Antes del primer `--scope lan`, coordine con el área de redes.** El barrido recorre el /24 completo con ARP, que es una firma de reconocimiento y va a generar alerta en el IDS. Si la red usa Dynamic ARP Inspection, el ritmo se controla desde la configuración (ver abajo); el valor de fábrica ya es conservador.

---

## Configuración para TI municipal

Lo que el área de TI de cada municipalidad puede ajustar vive en un JSON junto al ejecutable, editable con el Bloc de notas, **sin recompilar ni reinstalar nada**. Se busca en `MUNIGPT_CONFIG` y, si no está, como `munigpt.config.json` junto al binario. Si no existe, rigen los valores por defecto.

`munigpt-cli --escribir-config <ruta>` genera un ejemplo con todos los valores y una explicación de cada campo. El informe declara de dónde salió la configuración que usó.

| Sección | Qué controla |
|---------|--------------|
| `identidad` | Nombre del organismo que emite el informe y su tier (`oiv`, `pse`, `unclassified`). Gana sobre lo compilado en el build |
| `poam` | Plazos sugeridos de corrección por severidad. **No son plazos legales**: el único perentorio del régimen es el reporte del Art. 9° |
| `informe` | Tamaño de papel de cada PDF (`oficio`, `carta`, `a4`) y los cuatro colores de la paleta |
| `historico` | Si se lleva histórico, si se guarda el desglose por activo, y cuántos meses se retiene |
| `red` | Métodos del barrido de LAN (`arp`, `icmp`, `tcp`), ritmo del ARP, timeouts e hilos |
| `monitoreo` | Intervalo, día y hora del reescaneo programado, y a los cuántos días avisar que la medición venció. Viene apagado |

Notas de operación:

- Un archivo ilegible **no** degrada en silencio: avisa por stderr y sigue con los valores por defecto, para que un error de edición no se descubra en un informe con plazos equivocados.
- El Bloc de notas y PowerShell escriben UTF-8 con BOM en Windows; el lector lo descarta, así que editar con cualquiera de los dos funciona.
- Un archivo escrito por una versión anterior sigue cargando: las secciones que falten toman sus valores por defecto.
- **`red.arp_pps`** limita el barrido a 10 sondas ARP por segundo de fábrica. Dynamic ARP Inspection, habitual en switches Cisco, deja el puerto en err-disable al superar su umbral: sin el límite, el escáner puede dejar sin red al equipo desde el que corre. Poner `0` lo desactiva.
- **`monitoreo` viene apagado a propósito.** Registrar una tarea programada es la técnica T1053.005 de MITRE ATT&CK y queda en el evento 4698 de Windows: en una municipalidad con antivirus corporativo o EDR puede leerse como persistencia. `--programar` lo advierte antes de crear nada. Si una política de grupo lo impide, el escaneo manual sigue funcionando y la aplicación avisa cuando la medición envejece.

### El panel de ajustes (el engranaje)

Todo lo anterior también se edita sin salir de la aplicación. El **engranaje del encabezado**, visible en las tres pestañas, abre un panel con las mismas secciones agrupadas en cuatro: identidad, plazos e histórico, red y monitoreo, e informe. Guardar escribe el mismo `munigpt.config.json` de arriba, así que el panel y el Bloc de notas son dos vías al mismo archivo.

**La contraseña.** El panel está tras una contraseña que viaja fijada de fábrica en cada build de cliente. Se puede cambiar desde el propio panel, y ese cambio queda en el equipo, en `%LOCALAPPDATA%\MuniGPT\ti-password.hash`.

**Si se pierde la contraseña**, se borra ese archivo y vuelve a regir la del build. No hay que reinstalar nada. Ese archivo queda amarrado al build con el que se creó, de modo que una contraseña puesta en un equipo no abre el producto de otra institución instalado después en la misma máquina.

**Qué es y qué no es.** La contraseña es un **seguro contra cambios accidentales**, para que quien no es de TI no termine moviendo plazos, el nombre del organismo o el ritmo del barrido de red sin saber lo que toca. **No es un control de seguridad**, y no conviene tomarlo por tal: `munigpt.config.json` sigue siendo un archivo de texto editable con el Bloc de notas por cualquiera que tenga acceso al equipo, y eso es deliberado, porque es lo que permite operar el producto en un área de TI pequeña y recuperarse de una contraseña perdida.

Dos avisos de operación:

- Cambiar el **nombre de la institución** reinicia el Asistente. Se pierde el historial de chat de esa pestaña y el backend puede tardar hasta 180 segundos en volver a estar listo.
- Si se cambia la configuración **después** de un escaneo, la aplicación avisa que el resultado en pantalla quedó viejo y ofrece repetirlo. El resultado anterior no se borra, para que nadie exporte un PDF cuyos plazos contradigan la sección de configuración de ese mismo PDF.

---

## Uso — GUI

La GUI (`munigpt-gui`) es una aplicación de escritorio Tauri 2 con tres vistas:

- **Vista Municipal** — resumen ejecutivo en español formal, escala de multas UTM, aviso de obligación CSIRT
- **Vista Técnica (TI)** — tabla completa de brechas con evidencia, terminal de log en tiempo real, exportación PDF/JSON
- **Asistente** — chat legal RAG offline (módulo `assistant/`), levantado automáticamente como sidecar al abrir la app

### Compilación por cliente

El nombre de la institución y el tier se compilan en el binario como **valor de fábrica** de cada cliente. No son inamovibles: la sección `identidad` de `munigpt.config.json` gana sobre ellos, y el panel de ajustes es la vía prevista para editarla (ver [Configuración](#configuración-para-ti-municipal)). Lo que se compila es con qué nombre sale el producto de fábrica, no el único nombre que puede tener. Un solo valor, `MUNIGPT_INSTITUTION`, marca **ambos** módulos: el escáner lo estampa en el informe y el host lo pasa al Asistente (vía `MUNIGPT_MUNICIPIO`), de modo que el encabezado, el reporte y la personalización del Asistente nombran la misma institución sin editar `config.json`.

```powershell
# 1. Congelar el sidecar del Asistente y dejar sus activos junto al ejecutable
powershell -ExecutionPolicy Bypass -File tools\empaquetar-asistente.ps1

# 2. Instalador completo, desde gui\
$env:MUNIGPT_INSTITUTION = "Municipalidad de Chillán"
$env:MUNIGPT_TIER        = "pse"
$env:MUNIGPT_ADMIN_HASH  = "<cadena PHC de Argon2id para este cliente>"
cargo tauri build --config tauri.asistente.conf.json
```

El instalador queda en `target\release\bundle\`.

> **El overlay `--config tauri.asistente.conf.json` es obligatorio si el Asistente debe viajar en el instalador.** `cargo tauri build` a secas produce un instalador solo-escáner: `gui/tauri.conf.json` no declara los recursos del Asistente a propósito, para que la integración continua siga generando el paquete liviano. Sin el overlay, quien instale el producto no tendrá el módulo Asistente.

#### Variables de entorno de compilación

| Variable | Valores válidos | Descripción |
|----------|----------------|-------------|
| `MUNIGPT_INSTITUTION` | Cualquier string | Nombre de la institución cliente (marca escáner + Asistente) |
| `MUNIGPT_TIER` | `oiv`, `pse`, `unclassified` | Clasificación bajo Ley 21.663 |

| `MUNIGPT_ADMIN_HASH` | Cadena PHC de Argon2id | Contraseña de fábrica del panel de ajustes de TI |

> Si `MUNIGPT_INSTITUTION` no se define, y tampoco hay una `identidad.institucion` en el archivo de configuración, el binario mostrará `"Organismo del Estado"`: un marcador neutro, para que ningún build sin marca nombre a un cliente real. El Asistente, en ese caso, conserva el `municipio` de su propio `config.json`.
> Si `MUNIGPT_TIER` no se define, el binario usará `pse` por defecto. Es lo que corresponde a un organismo de la Administración del Estado sin resolución de la Agencia, por los Arts. 1° inc. 2 y 4° inc. 2 de la Ley 21.663; el razonamiento está en `docs/adr/0003-institucion-por-defecto-neutra-y-tier-pse.md`.
> Si `MUNIGPT_ADMIN_HASH` no se define, el build de release pedirá fijar una contraseña la primera vez que se abra el engranaje. No se distribuye ninguna contraseña por defecto.

### Ejecución en modo desarrollo

```powershell
cd gui\frontend
npm install
cd ..
cargo tauri dev
```

Para que la pestaña Asistente responda de extremo a extremo en desarrollo, el host necesita ubicar el backend Python y su intérprete. Ver las variables `MUNIGPT_BACKEND_DIR` y `MUNIGPT_PYTHON` en [assistant/README.md](assistant/README.md); el sidecar arranca el backend, sondea `GET /status` y reap del árbol de procesos al cerrar.

---

## Controles evaluados

### Exigible frente a madurez voluntaria

Las municipalidades **no son OIV**. Están obligadas por los Arts. 4°, 7° y 9° de la Ley 21.663 y por la Instrucción General N°1, pero la Res. Ex. N°87 de la ANCI las excluyó expresamente del primer proceso de calificación, y la nómina preliminar de la segunda etapa tampoco las incluye. El Art. 8° y las IG N°3 y N°4, que se dirigen a OIV, no las obligan hoy.

El escáner separa las dos cosas: lo exigible se evalúa como incumplimiento con consecuencia legal, y el resto se mide como **madurez voluntaria**, etiquetado en el informe como no exigible a la institución. Es un dato con fecha, no una constante: el Art. 6° obliga a la Agencia a revisar la calificación al menos cada tres años.

### Dos marcos, medidos por separado

Desde 0.7.0 el producto también evalúa el **Decreto 7 de 2023** (MINSEGPRES), la Norma Técnica de Seguridad de la Información y Ciberseguridad de la Ley 21.180, que obliga a todo órgano de la Administración del Estado y cubre las plataformas electrónicas que sustentan procedimientos administrativos.

Sus controles se miden **siempre como madurez y nunca como incumplimiento**: la guía técnica de la Secretaría de Gobierno Digital dice de sí misma que "no crea obligaciones adicionales" y admite que la Política se desarrolle gradualmente. Sus cinco funciones —identificación, protección, detección, respuesta y recuperación— son las mismas del NIST CSF, que se muestra como referencia internacional y no como juicio de cumplimiento.

Los dos marcos se informan con su propio promedio de madurez. Mezclarlos daría un número sin significado, y el de la Ley 21.663 es el que el histórico viene registrando desde 0.5.0.

### Fase de la Ley 21.180

El informe indica en qué fase de la Ley de Transformación Digital debería ir la comuna este año, según el Art. 5° del DFL N°1 de 2020 —que nombra a las municipalidades una por una en los Grupos B y C— y la tabla del Art. 7°. Es un dato informativo de otra norma: no afecta el puntaje de cumplimiento ni la madurez.

### Seguimiento de riesgos

Cada hallazgo se sigue con estado, responsable y plazo desde la Vista Técnica, y el estado sobrevive entre escaneos. Se emite en el `risk/status` del POA&M, de modo que el documento que entrega la municipalidad refleja el trabajo realmente hecho. Un riesgo aceptado se emite como `deviation-approved` y no como cerrado: aceptar no es corregir.

### Objetivos (escaneados automáticamente)

| Control | Severidad | Tier | Fundamento |
|---------|-----------|------|------------|
| Shares anónimos (SMB/NFS/WebDAV) | Crítico | Todos | Art. 7° |
| Admin shares expuestos (C$, ADMIN$, IPC$) | Crítico | Todos | Art. 7° |
| Firewall desactivado | Crítico | Todos | Art. 7°; para OIV además IG N°4 art. sexto |
| Protocolos en claro (Telnet/FTP) | Crítico | Todos | Art. 7°; para OIV además IG N°4 art. cuarto lit. c) |
| TLS 1.0/1.1/SSLv3 activo | Crítico | OIV+PSE | Art. 7°; NIST SP 800-52 rev2 (criterio técnico) |
| Sistema operativo en EOL | Crítico | Todos | Art. 7°; para OIV además Art. 8° lit. a) y d) |
| Vulnerabilidades conocidas (CVE) | según CVSS y KEV | Todos | Art. 7°; NVD/CVE (criterio técnico) |
| Certificado vencido o autofirmado | Alto | OIV+PSE | Art. 7°; buena práctica (criterio técnico) |
| Software en EOL | Alto | Todos | Art. 7°; para OIV además Art. 8° lit. a) y d) |
| BitLocker/LUKS desactivado | Alto | OIV | Art. 8° lit. a); ISO 27001 A.10.1 |
| Cloud sync activo | Alto | OIV | Art. 8° lit. a) |
| Sin agente de backup detectado | Alto | OIV+PSE | Art. 8° lit. c) |

### Declarativos (cuestionario interactivo)

Cubren el deber general del Art. 7°, la operativa del Art. 9° y de la IG N°1 (encargado de reportar con formación técnica, casilla institucional registrada, segundo factor en Clave Única, nombramiento acreditado con firma electrónica avanzada) y, como madurez voluntaria, los deberes del Art. 8° dirigidos a OIV: SGSI, planes de continuidad, Delegado de Ciberseguridad y capacitación continua.

La lista completa, con la severidad, el tier y el ejemplo de evidencia de cada pregunta, vive en `core/src/questionnaire.rs` y es la fuente de verdad; el informe imprime el anclaje legal de cada una.

---

## Salidas que produce un escaneo

| Archivo | Para quién |
|---------|-----------|
| **PDF técnico** | Para quien parchea. Brechas por dominio con su evidencia y anclaje legal, madurez 0 a 3 por dominio, y paginación con las atribuciones de licencia en cada página |
| **PDF ejecutivo** | Para quien firma. Una sola plana: dónde estamos, qué arriesgamos, qué hacer primero |
| **JSON CSIRT** | Reporte completo, incluido el inventario de activos con MAC y el método con que se descubrió cada host |
| **POA&M (OSCAL 1.2.2)** | Plan de remediación priorizado: CVE en KEV primero, después la calificación legal del incumplimiento según el Art. 39°, después severidad |
| **Histórico (SQLite)** | Serie de evaluaciones por comuna. Ambos informes muestran el delta contra la medición anterior, y el técnico agrega la **deriva por control**: cuál brecha es nueva, cuál se resolvió y cuál se había resuelto y volvió |
| **Paquete de evidencia** | Carpeta fechada con todo lo anterior más un manifiesto SHA-256. Se verifica con `certutil -hashfile` o `Get-FileHash`, que Windows ya trae. Es verificación de integridad, **no** una firma electrónica de la Ley 19.799 |

El puntaje agregado usa la mecánica SPRS (base fija menos deducciones ponderadas), con los pesos tomados del **Art. 39°** —gravísima −5, grave −3, leve −1— en vez de una ponderación inventada. Los controles técnicos sin correlato en el Art. 39° usan una tabla propia, declarada en el informe como criterio técnico y no como exigencia legal.

### Bases de datos embebidas

| Base | Origen | Actualización |
|------|--------|---------------|
| **EOL** — 38 productos (Windows, Office, SQL Server, .NET, Python, Node.js, PHP, MySQL, PostgreSQL, Apache, nginx, OpenSSL y más) | [endoflife.date](https://endoflife.date) | Con cada release |
| **CVE** — índice compacto derivado del snapshot de NVD, con matching CPE→CVE en Rust | NVD | Con cada release |
| **KEV** — vulnerabilidades con explotación observada | [CISA](https://www.cisa.gov/known-exploited-vulnerabilities-catalog) (CC0) | Sustituible en caliente: `MUNIGPT_KEV_FILE` o el JSON de CISA junto al ejecutable |

El mapeo nombre de producto → CPE usa una tabla curada, no coincidencia difusa: si un producto no está en la tabla, el informe no afirma nada sobre él y **declara qué porcentaje del inventario quedó sin evaluar**. Las CVE del sistema operativo se filtran además por el último acumulativo instalado, porque los acumulativos de Windows contienen todo lo anterior de su rama; los límites de ese filtro van escritos en el informe.

---

## Arquitectura

```
MuniGPT/
├── core/         # Library crate — escaneo, cumplimiento y generación de informes
│   ├── src/
│   │   ├── probes/           # sondas: descubrimiento de red, servicios, TLS, discos, software
│   │   │   └── net_discovery/  # ARP + ICMP nativos (Win32) y fallback TCP portable
│   │   ├── cve/              # índice NVD, matching CPE->CVE, catálogo KEV
│   │   ├── patch_level.rs    # descarta las CVE que el acumulativo instalado ya corrige
│   │   ├── compliance_engine.rs / questionnaire.rs   # controles y cuestionario
│   │   ├── scoring.rs / maturity.rs                  # puntaje y madurez 0-3
│   │   ├── poam.rs           # plan de remediación en OSCAL POA&M
│   │   ├── historico.rs      # serie de evaluaciones por comuna (SQLite)
│   │   ├── config.rs         # munigpt.config.json — lo que TI ajusta sin recompilar
│   │   ├── report_builder.rs # PDF técnico + ejecutivo, JSON CSIRT
│   │   └── data/             # bases embebidas: EOL, índice CVE, snapshot KEV
│   └── tests/    # pruebas en vivo (marcadas #[ignore]: requieren red)
├── cli/          # Binary crate — interfaz de línea de comandos (solo escáner)
├── gui/          # Binary crate — Tauri 2 desktop app (escáner + Asistente)
│   ├── src/
│   │   ├── assistant.rs         # ciclo de vida del backend Asistente (sidecar)
│   │   └── commands/branding.rs # institución/tier compilados -> ambos módulos
│   └── frontend/ # React/TypeScript/Vite (Vista Municipal / Técnica / Asistente)
├── assistant/    # Módulo Asistente (subtree de MuniGPT)
│   └── backend/  # FastAPI + RAG + llama.cpp + LanceDB (Python, corre como sidecar)
│       └── eval/ # Harness de evaluación offline (set dorado + juez LLM local)
├── tools/        # Utilidades de build y de release, fuera del binario del producto
│   ├── nvd-index/      # convierte el snapshot de NVD en el índice embebido
│   └── notas-release/  # genera el cuerpo del release desde el CHANGELOG
├── installer/    # Gancho NSIS que libera el sidecar antes de copiar, y el .iss heredado
├── vendor/       # Mirror local de dependencias (resiliencia offline, ver vendor/README.md)
├── .github/      # CI: build/tests, gates de auditoría, SBOM
└── docs/         # Investigación por hito, plan de fusión y documentación de ingeniería
```

El Asistente solo vive en la GUI; la CLI del escáner se conserva como binario aparte. Ver [docs/MERGE-PLAN-MuniGPT.md](docs/MERGE-PLAN-MuniGPT.md) para el detalle de la fusión y [CHANGELOG.md](CHANGELOG.md) para el historial de versiones.

---

## Pruebas

```powershell
# Rust (core + cli + gui + tools)
cargo test --workspace

# Pruebas que necesitan red, excluidas del run normal por el principio offline-first
cargo test -p munigpt-core --test tls_probe_live        -- --ignored --nocapture
cargo test -p munigpt-core --test net_discovery_live    -- --ignored --nocapture

# Backend del Asistente (desde assistant\backend, con el venv del módulo)
..\.venv\Scripts\python.exe -m pip install pytest    # una sola vez
..\.venv\Scripts\python.exe -m pytest                # unidad: rag, ingest, audit, licencia, etc.
..\.venv\Scripts\python.exe acceptance_m1.py         # 15 consultas de aceptación contra retrieve()
```

El corpus, los modelos GGUF, el binario de llama.cpp y las bases vectoriales (`db/`, `db_<comuna>/`) están gitignored por tamaño; deben estar presentes en disco para correr `acceptance_m1.py`. `acceptance_m1.py` valida el corpus nacional en `db/`: si el `config.json` local apunta a otra comuna, forzar la base con `MUNIGPT_DB_DIR=db`.

### Integración continua y calidad

Cada push corre **CI** (GitHub Actions, `.github/workflows/ci.yml`): build + tests, gates de auditoría de dependencias que **bloquean** el build (`cargo audit`, `cargo deny`, `pip-audit`) y generación de **SBOM** (SPDX + CycloneDX) como artefacto. El módulo Asistente incluye además un **harness de evaluación offline** (`assistant/backend/eval/`) que mide la calidad de recuperación (recall@k, MRR) contra un set dorado derivado del corpus, con una capa opcional de juez LLM totalmente local.

---

## Nota sobre firma de código

El ejecutable de MuniGPT **no está firmado digitalmente** en esta versión. Windows Defender y soluciones AV empresariales (McAfee, Defender ATP) mostrarán una advertencia al instalar.

La firma con certificado Authenticode (DigiCert/Sectigo, ~USD 200-400/año) está pendiente para la versión de distribución municipal. Hasta entonces, el equipo de TI del cliente debe aprobar manualmente la ejecución.

---

## Marco normativo

| Norma | Descripción |
|-------|-------------|
| Ley 21.663 (DO 08/04/2024) | Ley Marco de Ciberseguridad |
| Ley 21.459 (DO 20/06/2022) | Delitos informáticos / Convenio de Budapest |
| DFL N°1/2024 | Entrada en vigor Art. 5°, 8°, 9° (01/03/2025) |
| DS N°295/2024 | Reglamento de Reporte de Incidentes |
| IG N°1 ANCI (jun 2025) | Inscripción plataforma reporte |
| IG N°2, 3, 4 ANCI (dic 2025) | Reporte complementario, delegado, contención |
| DS N°293/2024 (DO 11/04/2025) | Red de Conectividad Segura del Estado (RCSE) |
| Decreto 7/2023 (DO 17/08/2023) | Norma Técnica de Seguridad de la Información y Ciberseguridad (Ley 21.180) |
| DFL N°1/2020 (DO 06/04/2021) | Gradualidad de la Ley 21.180 por grupos y fases |
| Res. Ex. N°7/2025 ANCI | Taxonomía de incidentes |

---

## Licencia

MIT — Felipe Carvajal Brown