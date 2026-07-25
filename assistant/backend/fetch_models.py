"""
fetch_models.py — obtain the GGUF models for the Asistente (D2).

Two paths, both gated by the REAL SHA256 in models.manifest.json:

1. Offline pack (air-gapped): copy verified models from a local pack directory
   (a USB drive, a network share) into the models directory. No network, no URL.
       python fetch_models.py --pack D:/muniani-models

2. Download on first run (networked machines): fetch each missing model over HTTP
   with resume (HTTP Range) and SHA256 verification. Only runs for entries whose
   source has been confirmed by the repo owner (source.confirmed = true), so an
   unconfirmed candidate URL is never fetched silently.
       python fetch_models.py

In both cases a file is accepted only if its SHA256 matches the manifest, so a
truncated, tampered or wrong-repo file is rejected. If the `aria2c` binary is on
PATH (or vendored at vendor/bin), it is used for the download for speed; otherwise a
built-in httpx resumable download is used. No model is ever fetched from a source the
owner has not confirmed.

Model directory resolution: MUNIGPT_MODELS_DIR env, else backend/models/.
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Optional

from paths import base_dir

MANIFEST_DEFAULT = base_dir() / "models.manifest.json"
_CHUNK = 1024 * 1024  # 1 MiB


# ── manifest ─────────────────────────────────────────────────────────────────────

def load_manifest(path: Path = MANIFEST_DEFAULT) -> list[dict]:
    """Parse the manifest and return its model entries."""
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    models = data.get("models", [])
    if not isinstance(models, list):
        raise ValueError("manifest 'models' must be a list")
    return models


def models_dir() -> Path:
    """Where models live. MUNIGPT_MODELS_DIR env, else models/ next to the assets.

    Single source of truth: inference.py imports this instead of resolving the
    directory a second time, so the env override applies to serving models and not
    only to fetching them.
    """
    env = os.environ.get("MUNIGPT_MODELS_DIR")
    if env and env.strip():
        return Path(env.strip())
    return base_dir() / "models"


# ── verification ─────────────────────────────────────────────────────────────────

def sha256_file(path: Path) -> str:
    """Streaming SHA256 of a file (never loads the whole GGUF into memory)."""
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for block in iter(lambda: fh.read(_CHUNK), b""):
            h.update(block)
    return h.hexdigest()


def is_valid(path: Path, entry: dict) -> bool:
    """True if `path` exists, matches the manifest size, and matches the SHA256.

    Size is checked first as a cheap gate so a wrong-size file skips the expensive
    hash. An entry with no sha256 is treated as unverifiable -> not valid.
    """
    expected_sha = entry.get("sha256")
    if not expected_sha or not path.is_file():
        return False
    expected_size = entry.get("sizeBytes")
    if expected_size is not None and path.stat().st_size != expected_size:
        return False
    return sha256_file(path) == expected_sha


# ── offline pack ─────────────────────────────────────────────────────────────────

def install_from_pack(entry: dict, pack_dir: Path, dest_dir: Path) -> bool:
    """Copy one model from an offline pack into dest_dir, if the pack has a valid
    copy. Returns True if the model is valid in dest_dir afterward."""
    fname = entry["filename"]
    dest = dest_dir / fname
    if is_valid(dest, entry):
        return True  # already there and valid
    src = pack_dir / fname
    if not is_valid(src, entry):
        return False
    dest_dir.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(dest.suffix + ".part")
    shutil.copyfile(src, tmp)
    if sha256_file(tmp) != entry["sha256"]:
        tmp.unlink(missing_ok=True)
        return False
    tmp.replace(dest)
    return True


# ── download (resumable) ─────────────────────────────────────────────────────────

def _aria2c_bin() -> Optional[str]:
    """aria2c on PATH or vendored at vendor/bin, else None."""
    exe = "aria2c.exe" if os.name == "nt" else "aria2c"
    vendored = Path(__file__).resolve().parents[2] / "vendor" / "bin" / exe
    if vendored.is_file():
        return str(vendored)
    return shutil.which("aria2c")


def _download_aria2c(url: str, dest: Path, aria2c: str) -> None:
    """Resumable download via aria2c (-c continues a partial file)."""
    dest.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [aria2c, "-c", "-x", "8", "-s", "8", "--dir", str(dest.parent),
         "--out", dest.name, url],
        check=True,
    )


def _download_httpx(url: str, dest: Path) -> None:
    """Resumable download via httpx: continue a .part file with an HTTP Range."""
    import httpx

    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(dest.suffix + ".part")
    have = tmp.stat().st_size if tmp.exists() else 0
    headers = {"Range": f"bytes={have}-"} if have else {}
    with httpx.stream("GET", url, headers=headers, follow_redirects=True,
                      timeout=None) as resp:
        if have and resp.status_code == 200:
            # Server ignored the Range (sent the whole file) — restart cleanly.
            have = 0
            mode = "wb"
        elif have and resp.status_code == 206:
            mode = "ab"
        else:
            resp.raise_for_status()
            mode = "wb"
        with open(tmp, mode) as fh:
            for block in resp.iter_bytes(_CHUNK):
                fh.write(block)
    tmp.replace(dest)


def download(entry: dict, dest_dir: Path) -> bool:
    """Download one model (if its source is confirmed), then verify. Returns True
    only if the model is valid in dest_dir afterward. Never fetches an unconfirmed
    source."""
    source = entry.get("source") or {}
    if not source.get("confirmed"):
        return False
    url = source.get("url")
    if not url:
        return False
    dest = dest_dir / entry["filename"]
    if is_valid(dest, entry):
        return True
    aria2c = _aria2c_bin()
    if aria2c:
        _download_aria2c(url, dest, aria2c)
    else:
        _download_httpx(url, dest)
    return is_valid(dest, entry)


# ── orchestration ────────────────────────────────────────────────────────────────

def ensure_models(
    manifest: list[dict],
    dest_dir: Path,
    pack_dir: Optional[Path] = None,
    allow_download: bool = True,
) -> dict:
    """Ensure every model is present and valid in dest_dir.

    Order per model: already valid -> copy from pack -> download (if allowed and
    source confirmed). Returns {model_name: status} where status is one of
    'present', 'from_pack', 'downloaded', 'missing'.
    """
    result: dict[str, str] = {}
    for entry in manifest:
        name = entry.get("name", entry.get("filename", "?"))
        dest = dest_dir / entry["filename"]
        if is_valid(dest, entry):
            result[name] = "present"
            continue
        if pack_dir and install_from_pack(entry, pack_dir, dest_dir):
            result[name] = "from_pack"
            continue
        if allow_download and download(entry, dest_dir):
            result[name] = "downloaded"
            continue
        result[name] = "missing"
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description="Fetch/verify Asistente GGUF models.")
    parser.add_argument("--manifest", type=Path, default=MANIFEST_DEFAULT)
    parser.add_argument("--dest", type=Path, default=None,
                        help="Models directory (default: MUNIGPT_MODELS_DIR or backend/models).")
    parser.add_argument("--pack", type=Path, default=None,
                        help="Offline pack directory to copy verified models from.")
    parser.add_argument("--offline", action="store_true",
                        help="Never download; use only the offline pack / present files.")
    args = parser.parse_args()

    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass

    manifest = load_manifest(args.manifest)
    dest = args.dest or models_dir()
    status = ensure_models(
        manifest, dest, pack_dir=args.pack, allow_download=not args.offline
    )
    for name, st in status.items():
        print(f"  {name}: {st}")
    missing = [n for n, st in status.items() if st == "missing"]
    if missing:
        print(f"\n[error] {len(missing)} model(s) still missing: {', '.join(missing)}")
        print("Provide an offline pack (--pack DIR) or confirm the source URLs in the "
              "manifest (source.confirmed=true) to enable download.")
        return 1
    print(f"\nAll {len(status)} models present and verified in {dest}/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
