import json
import sys
from pathlib import Path


def leer_config(ruta: Path) -> dict:
    ruta = Path(ruta)
    if not ruta.exists():
        return {}
    try:
        texto = ruta.read_text(encoding="utf-8-sig")
    except OSError as e:
        print(f"[config] no se pudo leer {ruta}: {e}", file=sys.stderr, flush=True)
        return {}
    try:
        datos = json.loads(texto)
    except json.JSONDecodeError as e:
        print(f"[config] {ruta} no es JSON valido: {e}", file=sys.stderr, flush=True)
        return {}
    return datos if isinstance(datos, dict) else {}


def escribir_clave(ruta: Path, seccion: str, clave: str, valor) -> dict:
    ruta = Path(ruta)
    datos = leer_config(ruta)
    bloque = datos.get(seccion)
    if not isinstance(bloque, dict):
        bloque = {}
    bloque[clave] = valor
    datos[seccion] = bloque
    ruta.parent.mkdir(parents=True, exist_ok=True)
    temporal = ruta.with_suffix(ruta.suffix + ".tmp")
    temporal.write_text(json.dumps(datos, ensure_ascii=False, indent=2), encoding="utf-8")
    temporal.replace(ruta)
    return datos
