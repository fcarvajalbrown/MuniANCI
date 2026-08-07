# MuniGPT

Asistente de inteligencia artificial local para municipios chilenos. La operación
principal (chat, embeddings y búsqueda vectorial) funciona completamente sin
conexión a internet: ningún dato institucional sale del equipo. La única salida
opcional a la red es el endpoint `/search` (búsqueda web vía DDGS/DuckDuckGo),
desactivada por defecto, donde solo se envía el texto de la consulta.

Desarrollado por **Felipe Carvajal Brown** en el contexto del cumplimiento de la
Ley 21.663 (Marco de Ciberseguridad).

---

## Arquitectura

```
corpus_fetcher.py  ->  ingest.py  ->  LanceDB  ->  main.py (/chat)  ->  respuesta
   (descarga BCN)      (chunk +        (vector +     (RAG + SSE)         citada
                        embed)          BM-25)
```

Toda la inferencia (chat y embeddings) corre localmente mediante un binario de
**llama.cpp** (`backend/bin/llama-server.exe`) que se incluye con el producto. No
se requiere Ollama ni GPU: el binario hace dispatch de instrucciones de CPU en
tiempo de ejecución y corre en cualquier x86-64.

## Requisitos

- Windows 10/11 (64 bits)
- 8 GB de RAM mínimo (16 GB recomendado). Con menos de 12 GB se selecciona
  automáticamente el modelo de chat más liviano (ver "Modelos").
- Espacio en disco suficiente para los modelos GGUF y el corpus.
- Solo para desarrollo: Python 3.12+ y Node.js 20+.

El usuario final no necesita instalar nada de lo anterior: el instalador empaqueta
el binario de llama.cpp, los modelos, el backend y la interfaz de escritorio.

## Componentes

- **backend/** — API FastAPI + RAG (Python). Es lo único que vive en este
  directorio tras la fusión con MuniGPT.

Este backend ya no es una app independiente: corre como **sidecar de Tauri**
dentro de MuniGPT. La interfaz de chat se trasladó a `gui/frontend` (pestaña
"Asistente") y el ciclo de vida del proceso (arranque, espera de `/status` y cierre
del árbol de procesos) lo maneja Rust en `gui/src/assistant.rs`. El antiguo frontend
React independiente (`frontend/`) y el shell Electron (`electron/`) fueron
**eliminados**.

## Desarrollo

### Backend

```powershell
git clone https://github.com/fcarvajalbrown/munigpt
cd munigpt
python -m venv .venv
.venv\Scripts\activate
pip install -r backend/requirements.txt
```

Colocar los modelos GGUF en `backend/models/` (nombres configurables en
`config.json`; ver "Modelos"). Descargar el corpus legal desde la BCN (requiere
internet, una sola vez) y construir el índice:

```powershell
cd backend
python corpus_fetcher.py            # todos los tiers
python ingest.py --reset            # chunk + embed en db/
uvicorn main:app --port 8000
```

### Interfaz y app de escritorio

La interfaz de chat y el arranque del backend viven ahora en MuniGPT, no aquí.
Para desarrollar o ejecutar el Asistente de extremo a extremo se construye y lanza
la GUI de MuniGPT (ver el `README.md` / `CLAUDE.md` de la raíz del repositorio); el
sidecar arranca este backend automáticamente. Para iterar solo sobre el backend,
basta con levantarlo por separado con `uvicorn main:app --port 8000` (ver arriba).

## Endpoints

- `POST /chat` — chat RAG. Responde por SSE: primero un evento `citations`, luego
  eventos `token`, y finalmente `done`.
- `POST /search` — búsqueda web vía DDGS/DuckDuckGo (sin API key; gatillada por
  `webSearchEnabled` en `config.json`, responde 503 si está desactivada). Registra
  cada consulta saliente en un log de auditoría local
  (`backend/logs/search_audit.log`).
- `GET /status` — estado del backend y de los modelos (lo consulta el shell).
- `GET /config` — entrega `config.json` (sin secretos) al frontend.
- `POST /ingest` — reconstruye o actualiza el índice desde `backend/corpus/`.

## Modelos

Definidos en el bloque `models` de `config.json` (ver `config.example.json`), no
en Ollama. Todo corre sobre el binario de llama.cpp incluido:

- **Chat (por defecto):** `Qwen3-4B-Instruct-Q4_K_M.gguf`
- **Chat (equipos con poca RAM):** `Qwen3-1.7B-Q4_K_M.gguf`
- **Embeddings:** `nomic-embed-text-v2-moe.Q4_K_M.gguf`

El modelo de chat se elige automáticamente según la RAM total: bajo el umbral
`lowRamThresholdGb` (12 GB por defecto) se usa el modelo liviano. La búsqueda
vectorial usa **LanceDB** embebido, con índice de texto completo BM-25 (tantivy)
para búsqueda híbrida.

## Corpus legal

El corpus se define en las listas por tier de `backend/corpus_fetcher.py`
(`TIER_0_GENERAL`, `TIER_1_CORE`, `TIER_2_EXTENDED`), donde cada entrada apunta a
un `idNorma` de la BCN (leychile.cl). Para agregar una norma, se añade su
`idNorma`. Los documentos propios del municipio (ordenanzas, reglamentos) se
descubren dinámicamente vía el endpoint CSV de búsqueda de la BCN y se cargan en
la instalación.

## Pruebas

```powershell
cd backend
pip install -r requirements-dev.txt
pytest                       # unidad: dedup/merge de rag y chunking de ingest
python acceptance_m1.py      # 15 consultas de aceptación contra retrieve()
```

## Licencia

Distribuido por Felipe Carvajal Brown. El producto integra software y modelos de
código abierto, cada uno bajo su propia licencia (llama.cpp, LanceDB, FastAPI,
React, Tauri, Vite, y los modelos Qwen y nomic-embed-text); consultar la
licencia de cada proyecto original para los términos aplicables.
