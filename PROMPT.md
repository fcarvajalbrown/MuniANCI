# Brief de traspaso — cerrar el empaquetado del Asistente antes del release

Trabajas en `C:\Projects\MuniANCI`, rama `main`, árbol limpio. Lee `ROADMAP.md`, el
`CLAUDE.md` del repo y el `CLAUDE.md` global antes de tocar nada. Este documento
reemplaza a `brief-0.8.0.md`, que ya se cumplió en su primera parte.

═══════════════════════════════════════════════════════════════════════
ESTADO
═══════════════════════════════════════════════════════════════════════

**El Asistente ya viaja en el instalador y funciona desde una instalación.** Era el
bloqueo que abría el brief anterior: la Fase 5 del `docs/MERGE-PLAN-MuniGPT.md` nunca
se había ejecutado. Se ejecutó el 2026-07-25 y está probada en el equipo del dueño del
repo, no solo en pruebas unitarias.

Las decisiones que el MERGE-PLAN dejaba abiertas las tomó Felipe ese día:

| # | Decisión |
|---|---|
| D1 | PyInstaller `--onedir`, sin UPX. |
| D2 | Se embarca solo el GGUF de embeddings (344 MB); los de chat llegan por descarga reanudable o paquete offline, elegidos desde la app. |
| Datos | Modelos en `%LOCALAPPDATA%\MuniANCI\models` vía `MUNIGPT_MODELS_DIR`; `db/`, `corpus/` y `bin/` como recursos junto al ejecutable. |
| CI | `gui/tauri.conf.json` sigue sin los recursos del Asistente; el build completo usa el overlay `gui/tauri.asistente.conf.json` con `--config`. La CI sigue produciendo un instalador solo-escáner. |
| Obtención | Desde la pestaña Asistente, con las dos vías. |
| Degradación | `assistant_status` distingue "no instalado" de "falta un modelo" y de "se cayó". |
| Alcance | Solo el empaquetado. Los workstreams M, O y P de 0.8.0 **no se tocaron**. |

Cifras medidas, no estimadas: runtime de PyInstaller 431 MB, carpeta del sidecar con
activos 887 MB, instalador NSIS **688 MB**, MSI **752 MB**. El techo de NSIS/WiX está
cerca de los 2 GB (tauri-apps/tauri#7372), así que hay holgura.

La investigación previa está en `docs/research/0.8.0-empaquetado-del-asistente.md`.

═══════════════════════════════════════════════════════════════════════
LO QUE FALTA PARA CERRAR EL RELEASE
═══════════════════════════════════════════════════════════════════════

**1. La versión no está decidida, y hay que preguntarle a Felipe antes de tocar nada.**
El empaquetado se completó, pero 0.8.0 en el `ROADMAP.md` incluye además el escaneo
profundo (M), el Asistente avanzado (O) y el apoyo operativo ANCI (P), que no se
empezaron. Taggear 0.8.0 hoy afirmaría un hito que no está hecho. Las opciones que se
le plantearon y que quedaron **sin responder**: un intermedio tipo 0.7.1 (o renumerar,
porque 0.7.5 ya está asignado a "otros órganos del Estado"), o declarar 0.8.0 como el
hito de empaquetado y correr M/O/P. Es decisión suya, por la UI de opciones.

**2. Documentación, que la convención del repo exige antes de taggear.**
- `README.md` **declara que el Asistente no viaja en el instalador**. Eso ya es falso y
  es lo primero que alguien lee. Lo escribió el commit `777fe80`.
- `CHANGELOG.md` no tiene ni una línea de lo de hoy. Son doce commits.
- `ROADMAP.md`: marcar la fase de empaquetado, y arreglar dos defectos preexistentes
  —la línea 3 dice "Estado actual: **0.6.0**" y el bloque de 0.8.0 de las líneas
  383-412 quedó sin encabezado, antes del `## 0.8.0` de la 416—.
- `assistant/CLAUDE.md`: su nota de *Packaging* dice que el instalador unificado "is
  not built in-repo yet". Ya se construye.
- `docs/MERGE-PLAN-MuniGPT.md`: marcar la Fase 5 y las decisiones D1/D2 como resueltas.

**3. Verificaciones que quedaron abiertas.** Ninguna es teórica; todas se pueden hacer
en media hora con el instalador que ya existe.
- **Una consulta real respondida desde la app instalada.** El backend congelado ya
  respondió con citas y 242 tokens corriendo suelto, y la app instalada llegó a
  `ready:true` con el modelo liviano, pero nadie escribió todavía una pregunta en la
  pestaña y leyó la respuesta.
- **Que el histórico sobreviva a una reinstalación.** Es lo más importante de esta
  lista. `ruta_historico()` en `gui/src/commands/monitoreo.rs:49` escribe
  `historico_<comuna>.db` **junto al ejecutable**, o sea dentro del directorio de
  instalación, y ahí vive la medición acumulada de meses de la municipalidad.
  `munianci.config.json` está en el mismo lugar por diseño documentado. Lo único que
  los protege hoy es que NSIS borra solo lo que instaló. Hay una evidencia a favor: al
  desinstalar, la carpeta `models` sobrevivió. Pero el `.db` no se probó, y es el que
  tiene datos que no se regeneran. **Prueba: instalar, escanear, anotar tamaño y hash
  del `.db`, reinstalar el mismo instalador, verificar.** Si no sobrevive, hay que
  sacarlo del árbol de instalación.
- **El gancho `installer/asistente.nsh`.** Mata `munigpt-backend.exe` y
  `llama-server.exe` y borra `$INSTDIR\backend` antes de copiar, contra el riesgo de
  tauri-apps/tauri#15134. Compiló dentro del instalador, pero nunca corrió: hace falta
  instalar dos veces.
- **El panel de "no instalado".** Se puede forzar renombrando `backend\` en el
  directorio de instalación.

═══════════════════════════════════════════════════════════════════════
CÓMO SE ARMA EL INSTALADOR (probado, no supuesto)
═══════════════════════════════════════════════════════════════════════

```powershell
# 1. Sidecar congelado + activos junto al ejecutable (~1 min de PyInstaller)
powershell -ExecutionPolicy Bypass -File tools\empaquetar-asistente.ps1

# 2. Instalador completo. El overlay es obligatorio: sin el, el Asistente no viaja.
cd gui; cargo tauri build --config tauri.asistente.conf.json
```

**`cargo build --release -p muniani-gui` NO sirve para probar.** No embebe el frontend,
así que la app cae al `devUrl` y muestra la pantalla de error de Edge. Se nota en el
tamaño: 17.005.568 bytes contra 17.538.048 del build correcto. Para un ejecutable
probable sin esperar la compresión LZMA, usar `cargo tauri build --no-bundle`.

Requiere `pip install -r assistant\backend\requirements-build.txt` una vez.

═══════════════════════════════════════════════════════════════════════
TRAMPAS Y HALLAZGOS QUE NO CONVIENE REDESCUBRIR
═══════════════════════════════════════════════════════════════════════

- **La notación de mapa en `bundle.resources` es obligatoria.** Con notación de lista,
  Tauri convierte `../` en `_up_` en el destino, así que el backend aterrizaría en
  `$RESOURCE/_up_/assistant/backend/dist/...` y `packaged_sidecar_bin()` no lo
  encontraría. Está documentado por Tauri y verificado en el layout producido.
- **`externalBin` no sirve para una salida `--onedir`**: espera archivos sueltos y les
  agrega el target triple.
- **Escribir y servir un modelo son directorios distintos.** `models_dir()` es el
  destino de escritura; `models_search_path()` / `find_model()` es la búsqueda, y mira
  primero el directorio escribible y después `models/` junto a los activos. Sin esa
  separación, apuntar `MUNIGPT_MODELS_DIR` a un directorio vacío esconde el GGUF de
  embeddings que viaja en el instalador.
- **El requisito de chat lo satisface cualquiera de los dos modelos.** Antes
  `missing_models()` exigía por nombre el que la RAM prefería: un equipo de 16 GB con
  solo el liviano instalado se declaraba no listo y pedía bajar 2,3 GB. Y un PC
  municipal de 8 GB no puede correr el 4B. La preferencia por RAM sigue, pero como
  preferencia.
- **El informe ejecutivo no era alcanzable desde la GUI.** `write_pdf` emite solo el
  técnico; el ejecutivo lo escribía únicamente el `build_con` de la CLI. Ahora la Vista
  Municipal exporta ese y solo ese, porque el técnico lleva IP y rutas de recursos
  compartidos y el propio `report_builder` dice que conviene tratarlo como reservado.
- **`--onedir` no elimina los falsos positivos de antivirus**, solo quita el disparador
  de `--onefile`. La mitigación con evidencia es la firma de código (Horizonte, B).
- **El `.spec` excluye 162 MB a propósito**: scipy, pandas, PIL y el cliente de
  HuggingFace los arrastraba el registro de embeddings de lancedb, que este producto no
  usa; pymupdf solo lo usa `convert.py`, que está fuera del pipeline. Si algún día se
  usa un embedding en proceso, hay que sacar el paquete de la lista y volver a medir.
- **Disco.** `target/debug` había llegado a 46,74 GB. Con
  `[profile.dev] debug = "line-tables-only"` una reconstrucción completa queda en
  2.775 MB. La regla de no acumular instaladores está en el `CLAUDE.md` del repo.
- **La tarea `MuniGPT-Hibernate` que apareció en el Programador de tareas no es de este
  producto.** Se verificó: no está en el código, no está registrada, no está en disco
  ni en el registro. Lo único que este producto crea es
  `MuniANCI - reescaneo de cumplimiento`, y solo cuando el usuario lo pide.

═══════════════════════════════════════════════════════════════════════
REGLAS DEL REPO QUE APLICAN
═══════════════════════════════════════════════════════════════════════

- **HARD RULE: preguntar a Felipe por la UI de opciones antes de iniciar un run 0.X**, y
  ante cualquier duda durante el run. La versión de este release entra en esa regla.
- **Investigar antes de tocar código, en cada hito**, con writeup y descartes explícitos.
- **Nunca inventar.** Ninguna norma, cifra ni cita sin fuente primaria leída.
- **No es asesoría legal.**
- **Commit y push activos**, en unidades Conventional Commit, directo sobre `main`.
  Nunca abrir un PR salvo que se pida en ese turno. Sin atribución de IA. Sin emojis.
- **Nada está "listo" sin salida real de comando.** Correr el binario, no solo las
  pruebas.
- **Felipe prueba la aplicación construida antes de que se taggee nada**, y confirma
  antes de publicar el release, que es material que sale al mundo.
- **Nunca pegar una sección del CHANGELOG directo en un release de GitHub**: usar
  `cargo run -q -p notas-release -- <version> > notas.md`.
- Se habla con Felipe en inglés; el producto, los commits y los docs en castellano de
  Chile.

═══════════════════════════════════════════════════════════════════════
PRIMER PASO SUGERIDO
═══════════════════════════════════════════════════════════════════════

Preguntarle a Felipe la versión por la UI de opciones, y con eso decidido: barrer
`README.md`, escribir el `CHANGELOG.md`, y recién después correr las cuatro
verificaciones pendientes sobre un instalador recién armado. El histórico es la que
más importa: si esa medición no sobrevive a una actualización, ninguna municipalidad
debería instalar la siguiente versión encima de la anterior hasta que se arregle.
