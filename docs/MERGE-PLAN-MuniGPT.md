# Plan de fusión: MuniGPT dentro de MuniANCI

Documento de planificación de ingeniería. Objetivo: integrar MuniGPT (asistente RAG
legal offline) como un módulo de MuniANCI (escáner de cumplimiento Ley 21.663),
produciendo un **único producto de escritorio**.

Decisiones ya fijadas con el dueño del repo:

- **Profundidad de integración:** una sola app de escritorio, con el shell Tauri de
  MuniANCI como anfitrión. Se descarta Electron.
- **Historia git:** se preserva la historia de MuniGPT vía `git subtree`.
- **Marca:** MuniANCI sigue siendo el producto; MuniGPT pasa a ser su módulo
  "Asistente".

---

## 0. Estado actual (lo que hay que saber antes de empezar)

**MuniANCI** — workspace Rust (`core` / `cli` / `gui`). El `gui` es una app Tauri 2
con frontend React/Vite. El nombre de institución y el `tier` se compilan en el
binario vía variables de entorno (`MUNIANI_INSTITUTION`, `MUNIANI_TIER`). Repo
propio en GitHub.

**MuniGPT** — backend Python (FastAPI + binario llama.cpp incluido + LanceDB),
shell **Electron**, frontend React/Vite, instalador Inno Setup. Todo corre local;
lo único que sale a la red es `/search` (DuckDuckGo), apagado por defecto. Repo
propio en GitHub.

### Bloqueante descubierto: MuniANCI no compila desde el checkout

El `.gitignore` de MuniANCI tiene una regla `*.json` demasiado amplia. Consecuencia:
**ningún archivo JSON está versionado** (`git ls-files '*.json'` no devuelve nada) y
además no están en disco en este checkout:

- `gui/tauri.conf.json` — requerido por `tauri::generate_context!()`. Sin él, el
  `gui` no compila.
- `gui/frontend/package.json` y `tsconfig.json` — sin ellos no se instala ni
  construye el frontend.
- `core/src/data/eol_db.json` — cargado por `include_str!`; sin él, `core` no
  compila. (El directorio `core/src/data/` ni siquiera existe en disco.)

Esto se arregla en la Fase 0. No tiene sentido fusionar sobre algo que no compila.

---

## 1. Arquitectura objetivo

Una sola app Tauri 2 llamada **MuniANCI** con tres vistas de nivel superior:

- **Vista Municipal** (existente) — resumen ejecutivo del escaneo.
- **Vista Técnica (TI)** (existente) — tabla de brechas, log, export PDF/JSON.
- **Asistente** (nueva) — el chat RAG de MuniGPT.

El backend Python de MuniGPT se lanza como **sidecar** del proceso Tauri (no como
app Electron separada). La lógica que hoy vive en `electron/main.js` (levantar
uvicorn, esperar `/status`, matar el árbol de procesos al salir) se porta a Rust,
en el `setup` hook de Tauri. El frontend React de MuniGPT se fusiona dentro de
`gui/frontend`, apuntando su cliente HTTP a `127.0.0.1:8000`.

```
MuniANCI (proceso Tauri, Rust)
├── core scanner            (in-process, Rust)
├── comandos Tauri          start_scan, export_report  (existentes)
│                           + start_assistant / assistant_status (nuevos)
└── sidecar backend Python  uvicorn main:app  ->  llama.cpp + LanceDB
        ▲ SSE /chat, /status, /config, /search
        │
     gui/frontend (una sola app React/Vite)
        ├── Vista Municipal / Vista Técnica  -> invoke() a comandos Rust
        └── Asistente                        -> fetch/SSE a 127.0.0.1:8000
```

### Layout del repo tras la fusión

```
MuniANCI/
├── core/                 # scanner Rust (sin cambios funcionales)
├── cli/                  # CLI Rust (sin cambios)
├── gui/
│   ├── src/              # + arranque/ciclo de vida del sidecar en el setup hook
│   ├── tauri.conf.json   # + bundle de recursos del backend, CSP a 127.0.0.1:8000
│   └── frontend/         # app React unificada (vistas scanner + Asistente)
├── assistant/            # <- subtree de MuniGPT
│   └── backend/          # FastAPI + rag + ingest + corpus_fetcher + inference
│       ├── bin/          # llama-server.exe            (gitignored, va por instalador)
│       ├── models/       # GGUF                        (gitignored, va por instalador)
│       └── corpus/, db/  # corpus legal + índice       (gitignored)
├── docs/
└── ...
```

`assistant/frontend/` y `assistant/electron/` entran por el subtree pero se retiran
después: el frontend se fusiona en `gui/frontend`, y Electron se elimina por
completo.

---

## 2. Fases de ejecución

Cada fase termina en un estado verificable y commiteable. Trabajo directo sobre
`main` (repo de un solo dueño), commit sólo cuando Felipe lo pida.

### Fase 0 — Reparar y establecer línea base

Objetivo: que MuniANCI compile desde el árbol de trabajo antes de tocar nada.

1. Corregir el `.gitignore`: reemplazar `*.json` por reglas específicas que sólo
   ignoren la **salida de reportes** (p. ej. `informe_brechas.pdf`,
   `csirt_report.json`, o mejor un patrón acotado como `*_report.json` /
   `*_brechas.json`) y mantener versionados `tauri.conf.json`, `package.json`,
   `tsconfig.json`, `eol_db.json`, `config.example.json`.
2. Recuperar los archivos ausentes: `gui/tauri.conf.json`,
   `gui/frontend/package.json`, `gui/frontend/tsconfig.json`,
   `core/src/data/eol_db.json`. Fuente: el binario 0.2 ya construido, otro clon, o
   reconstruirlos. **Si no existen en ningún lado, hay que decidir cómo regenerarlos
   antes de seguir** (no inventar el contenido de `eol_db.json`).
3. Verificar: `cargo build --release -p muniani-cli`, `cargo tauri dev`,
   `npm install && npm run build` en `gui/frontend`.
4. Verificar MuniGPT por separado (en su repo actual): `pytest`, `acceptance_m1.py`,
   backend levanta y responde `/status`.

Salida: ambos productos compilan/arrancan de forma independiente. Punto de partida
limpio.

### Fase 1 — Unificar el repo (subtree, historia preservada)

1. `git subtree add --prefix assistant https://github.com/fcarvajalbrown/MuniGPT.git main`
   (tras confirmar que `main` es la rama correcta de MuniGPT y que su árbol está
   limpio; hoy tiene un `scaffold.ini` borrado y `NEXT-PROMPT.md` sin trackear).
2. Reconciliar los dos `.gitignore`. El de MuniGPT ya distingue bien
   (`config.example.json` versionado, `config.json` ignorado, binarios/modelos
   fuera). Fusionar sin volver a introducir la regla amplia `*.json`.
3. Mover `assistant/CLAUDE.md` a documentación del módulo; el `CLAUDE.md` raíz del
   repo unificado se actualiza en la Fase 6.

Salida: un repo con la historia de ambos proyectos, MuniGPT bajo `assistant/`.

### Fase 2 — Backend como sidecar (ciclo de vida en Rust)

Portar `electron/main.js` a Rust dentro de `gui/`:

1. En el `setup` hook de Tauri, lanzar el backend (equivalente a `startBackend`),
   sondear `GET /status` hasta `ready` (equivalente a `waitForBackend`, con el mismo
   timeout ~180s y mensajes de "cargando modelo"), y al cerrar matar el árbol de
   procesos (uvicorn genera hijos llama-server): en Windows `taskkill /PID <pid> /T
   /F`, como ya hace Electron.
2. Reusar `tauri-plugin-shell` (ya es dependencia del `gui`) o `std::process` para
   el spawn. Exponer un comando `assistant_status` para que el frontend sepa si el
   Asistente está listo.
3. Config de host/puerto (`127.0.0.1:8000`, override por env como `MUNIGPT_PORT`).
4. Decidir **cómo se empaqueta Python** (ver Decisiones abiertas D1): PyInstaller a
   un `.exe` sidecar, o Python embebido + venv como recurso Tauri.

Salida: al abrir MuniANCI, el escáner sigue funcionando y el backend del Asistente
arranca solo.

### Fase 3 — Fusionar el frontend

1. Añadir la pestaña "Asistente" a la navegación de `gui/frontend/src/App.tsx`
   (hoy alterna `worker` / `it`; pasa a `worker` / `it` / `asistente`).
2. Portar los componentes de MuniGPT (`Chat.tsx`, `Message.tsx`, `SearchToggle.tsx`,
   `ComingSoonPill.tsx`, `api.ts`) a `gui/frontend/src/`, apuntando `api.ts` a
   `127.0.0.1:8000` en vez del `--munigpt-api-base` que inyectaba Electron.
3. Unificar estilos: ambos traen CSS global (`app.css` vs `styles.css`). Acotar por
   scope los estilos del Asistente para evitar colisiones; unificar tokens de tema
   (ambos usan fondo oscuro `#0f1420`, buena señal).
4. Fusionar dependencias npm en un solo `package.json` (React, Vite, TS ya comunes;
   sumar lo que use el chat/SSE).
5. Ajustar la CSP de Tauri para permitir `connect-src` a `http://127.0.0.1:8000`
   (incluido SSE) sin abrir nada más.

Salida: el chat RAG corre dentro de la ventana de MuniANCI, con streaming y citas.

### Fase 4 — Reconciliar configuración y marca por cliente

MuniANCI compila `institución`/`tier` en el binario (env de build). MuniGPT usa
`config.json` para branding por municipio. Unificar: una sola fuente de verdad para
el nombre de la institución que alimente tanto al escáner (env de compilación) como
al `config.json` que lee el backend. Definir de dónde sale `config.json` en la app
empaquetada.

Salida: una sola operación de "compilar para el cliente X" que marca ambos módulos.

### Fase 5 — Empaquetado (un solo instalador)

1. Configurar el bundler de Tauri para incluir los recursos del backend:
   `llama-server.exe`, modelos GGUF, corpus, `db/`, y el backend Python (según D1).
2. Retirar el instalador Inno Setup (`assistant/installer/munigpt.iss`) o dejarlo
   sólo como referencia histórica.
3. Estrategia de tamaño (ver D2): el instalador de MuniGPT ya pesa ~8 GB por los
   modelos. Combinado es enorme. Opciones: empacar todo, o descarga de modelos en
   primer arranque. Registrar la decisión.
4. Firma de código pendiente (ya anotada en el README de MuniANCI) — no la resuelve
   este plan.

Salida: un instalador único que despliega escáner + Asistente.

### Fase 6 — Docs, tests, CI

1. Fusionar READMEs y CHANGELOG bajo la narrativa "MuniANCI + módulo Asistente".
2. Reescribir el `CLAUDE.md` raíz combinando las convenciones de ambos (las de
   MuniGPT ya están bien redactadas: español, no inventar normas, no atribución IA).
3. Portar los tests: `pytest` + `acceptance_m1.py` de MuniGPT, tests de `core`, y un
   smoke test del app combinado (arranca, escáner responde, Asistente responde
   `/status` y una consulta real).
4. Verificar con el skill `/verify` o `/run` que la app combinada bootea de verdad.

Salida: producto unificado, documentado y probado.

---

## 3. Riesgos y puntos de fricción

- **Dos stacks (Rust + Python)** en un solo instalador: Python no es un binario
  único; empaquetarlo bien es el trabajo más delicado (Fase 5 / D1).
- **Postura offline** es el argumento de venta de ambos productos. Hay que preservar
  y documentar que nada institucional sale del equipo; sólo `/search` (apagado por
  defecto) toca la red. La CSP de Tauri debe permitir sólo `127.0.0.1:8000`.
- **Tamaño del instalador** (~8 GB por modelos GGUF): decisión de producto (D2).
- **Colisiones de frontend** (CSS global, versiones de deps): acotar por scope.
- **Matar el árbol de procesos** correctamente para no dejar `llama-server.exe`
  huérfano corriendo tras cerrar la app (ya resuelto en Electron; hay que replicarlo
  en Rust).
- **El landmine `*.json`** del `.gitignore` (Fase 0): si no se corrige, el repo
  fusionado seguirá sin versionar sus configs.

---

## 4. Decisiones abiertas (requieren input de Felipe antes de la fase respectiva)

- **D1 — Empaquetado de Python (Fase 2/5):** PyInstaller onefile como sidecar
  `.exe`, o Python embebido + venv como recurso Tauri. PyInstaller es más limpio para
  el modelo de sidecar de Tauri; el embebido es más fácil de depurar. (Recomendación
  tentativa: PyInstaller.)
- **D2 — Estrategia de modelos (Fase 5):** empacar los GGUF en el instalador (~8 GB)
  o descargarlos en el primer arranque desde una fuente confirmada. No inventar URLs
  de descarga; si se elige descarga, Felipe confirma el origen.
- **D3 — Umbral de RAM / hardware objetivo:** RESUELTA (2026-07-11). Se mantiene
  `lowRamThresholdGb = 12`. Las máquinas municipales comunes de 8 GB quedan bajo el
  umbral y usan el modelo liviano (Qwen3-1.7B), fiable sin swap junto a Windows y
  ofimática; los equipos de 16 GB o más usan el 4B. Bajarlo a 8 GB haría que un
  equipo de 8 GB intentara el 4B, con riesgo de OOM/swap. El valor ya es 12 en
  `config.example.json` y en el default de `inference.py`, así que no hubo cambio de
  código. (El `config.json` local del demo fuerza 999 para usar siempre el 1.7B; es
  solo del demo y está gitignored.)
- **D4 — ¿Se mantiene la `cli` de MuniANCI?** RESUELTA (2026-07-11). Sí, se
  conserva. El escáner CLI (`muniani-cli`) es un binario aparte del workspace que
  compila y pasa sus tests, sin dependencia del Asistente; preserva el escaneo
  headless/scripteable a costo de mantención casi nulo. El Asistente sigue viviendo
  solo en la GUI.

---

## 5. Cómo lo ejecutaría (enfoque "superpowers": planificar, luego fanout verificado)

- Este documento es la fuente de verdad. Cada fase se ejecuta y se verifica antes de
  pasar a la siguiente.
- Por fase: un agente `Plan`/`Explore` para el detalle, implementación, y luego el
  skill `/code-review` sobre el diff, más `/verify` o `/run` para confirmar que la
  app combinada bootea de verdad (no sólo que compila).
- Fases 2, 3 y 5 (backend, frontend, empaquetado) son bastante independientes una vez
  hecha la Fase 1; se podrían paralelizar con un Workflow multiagente **si Felipe lo
  pide explícitamente** (los workflows consumen muchos tokens y requieren opt-in).
- Nada se marca "hecho" sin salida real de comando (cargo build, npm build, pytest,
  arranque del app).
