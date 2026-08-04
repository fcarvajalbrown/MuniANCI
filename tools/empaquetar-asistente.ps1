# empaquetar-asistente.ps1 — arma la carpeta del sidecar del Asistente (Fase 5, D1/D2).
#
#   powershell -ExecutionPolicy Bypass -File tools\empaquetar-asistente.ps1
#
# Deja en assistant\backend\dist\munigpt-backend\ el binario congelado de PyInstaller
# (--onedir, sin UPX) junto a todos los activos que el backend resuelve al lado del
# ejecutable. Esa carpeta es lo que el bundler de Tauri embarca como recurso; ver
# gui\tauri.asistente.conf.json.
#
# Los activos grandes se copian acá y no pasan por el análisis de PyInstaller, que sería
# lento y no aportaría nada. De los modelos viaja SOLO el de embeddings (344 MB): es
# obligatorio -sin él no hay vector de consulta, así que no hay Asistente- y cabe. Los
# GGUF de chat pesan 1,3 y 2,5 GB contra un techo de ~2 GB en NSIS y WiX, así que llegan
# al equipo por descarga reanudable o por paquete offline (D2), desde la propia app.
#
# Requiere: pip install -r assistant\backend\requirements-build.txt

$ErrorActionPreference = "Stop"

$raiz     = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$backend  = Join-Path $raiz "assistant\backend"
$python   = Join-Path $raiz "assistant\.venv\Scripts\python.exe"
$destino  = Join-Path $backend "dist\munigpt-backend"

if (-not (Test-Path $python)) {
    throw "No existe el entorno virtual en $python. Cree assistant\.venv e instale requirements-build.txt."
}

# ── 1. Binario congelado ─────────────────────────────────────────────────────────
Write-Host "== PyInstaller (--onedir) ==" -ForegroundColor Cyan
Push-Location $backend
try {
    & $python -m PyInstaller --noconfirm --distpath dist --workpath build munigpt-backend.spec
    if ($LASTEXITCODE -ne 0) { throw "PyInstaller fallo con codigo $LASTEXITCODE" }
} finally {
    Pop-Location
}

# ── 2. Activos junto al ejecutable ───────────────────────────────────────────────
# La regla del backend (paths.py) es que todo activo vive junto al ejecutable, así que
# esta lista replica el árbol de desarrollo. Un activo ausente es un aviso y no un
# error: db_providencia solo existe donde se armó la demo, y corpus_muni tampoco es
# obligatorio para que el Asistente responda.
$carpetas = @(
    "bin", "db", "db_providencia", "db_ejercito-de-chile", "db_fuerza-aerea-de-chile",
    "corpus", "corpus_muni", "corpus_defensa"
)
foreach ($carpeta in $carpetas) {
    $origen = Join-Path $backend $carpeta
    if (-not (Test-Path $origen)) {
        Write-Host "  (falta $carpeta, se omite)" -ForegroundColor DarkYellow
        continue
    }
    Write-Host "  copiando $carpeta" -ForegroundColor Gray
    Copy-Item -Path $origen -Destination $destino -Recurse -Force
}

Copy-Item -Path (Join-Path $backend "models.manifest.json") -Destination $destino -Force

# ── 3. Solo el modelo de embeddings ──────────────────────────────────────────────
# El nombre sale del manifiesto, no de una constante escrita a mano: si el modelo de
# embeddings cambia, el manifiesto es la fuente y este script lo sigue.
$manifiesto = Get-Content (Join-Path $backend "models.manifest.json") -Raw | ConvertFrom-Json
$embedding  = $manifiesto.models | Where-Object { $_.name -eq "embedding" }
if (-not $embedding) { throw "El manifiesto no declara un modelo 'embedding'." }

$modelosDestino = Join-Path $destino "models"
New-Item -ItemType Directory -Force -Path $modelosDestino | Out-Null
$origenModelo = Join-Path $backend "models\$($embedding.filename)"
if (Test-Path $origenModelo) {
    Write-Host "  copiando $($embedding.filename)" -ForegroundColor Gray
    Copy-Item -Path $origenModelo -Destination $modelosDestino -Force
} else {
    Write-Host "  FALTA el modelo de embeddings ($($embedding.filename)):" -ForegroundColor Yellow
    Write-Host "  el instalador quedara sin el, y el equipo tendra que bajarlo." -ForegroundColor Yellow
}

# ── 4. Cifras reales, no estimadas ───────────────────────────────────────────────
$bytes = (Get-ChildItem $destino -Recurse -File | Measure-Object -Property Length -Sum).Sum
$mb    = [math]::Round($bytes / 1MB, 1)
Write-Host ""
Write-Host "Carpeta lista: $destino" -ForegroundColor Green
Write-Host "Tamano sin comprimir: $mb MB" -ForegroundColor Green
Write-Host "El techo de NSIS y WiX esta cerca de los 2 GB (tauri-apps/tauri#7372)."
