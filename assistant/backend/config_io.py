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
