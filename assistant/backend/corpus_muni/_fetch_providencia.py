"""
One-off, polite downloader for Providencia municipal documents into the per-comuna
corpus (corpus_muni/providencia/). Rate-limited, identifies itself, and validates
that each response is a real PDF (not an HTML error page). Not part of the main
pipeline — run once to seed the Providencia corpus, then ingest.

    ../.venv/Scripts/python.exe corpus_muni/_fetch_providencia.py
"""
import sys
import time
from pathlib import Path

import httpx

OUT = Path(__file__).resolve().parent / "providencia"
OUT.mkdir(parents=True, exist_ok=True)

UA = "MuniGPT-corpus-fetcher/0.2 (municipal legal RAG; contact fcarvajalbrown@gmail.com)"
DELAY_S = 3.0  # be gentle: one request every few seconds

# Curated, citizen-facing set. Direct PDFs on the providencia.cl CMS host are
# reliable; the transparency-portal ones are attempted best-effort.
DOCS = [
    ("tenencia_responsable_mascotas.pdf",
     "https://providencia.cl/provi/site/docs/20230605/20230605090744/ordenanza_tenencia_responsable_de_macotas.pdf"),
    ("plan_regulador_ordenanza_refundida_2026.pdf",
     "https://providencia.cl/provi/site/docs/20260105/20260105103237/ordenanza_refundida.pdf"),
    ("plan_regulador_prcp2007_ordenanza_local.pdf",
     "https://providencia.cl/provi/site/docs/20191112/20191112162159/prcp_2007_ordenanza_local.pdf"),
    ("plan_regulador_prcp2007_memoria_explicativa.pdf",
     "https://providencia.cl/provi/site/docs/20191112/20191112162159/prcp_2007_memoria_explicativa.pdf"),
    ("plan_regulador_estudio_riesgo_ambiental.pdf",
     "https://providencia.cl/provi/site/docs/20260105/20260105115403/prcp_2007___estudio_riesgo_y_prot_ambiental.pdf"),
    ("plan_regulador_modificacion6_2022.pdf",
     "https://providencia.cl/provi/site/docs/20220506/20220506121021/mod_6_prc_2022.pdf"),
]


def looks_like_pdf(content: bytes) -> bool:
    return content[:5] == b"%PDF-"


def main() -> int:
    ok, failed = [], []
    with httpx.Client(headers={"User-Agent": UA}, timeout=60, follow_redirects=True) as client:
        for i, (name, url) in enumerate(DOCS):
            if i:
                time.sleep(DELAY_S)
            try:
                r = client.get(url)
                if r.status_code != 200:
                    failed.append((name, f"HTTP {r.status_code}"))
                    print(f"[skip] {name}: HTTP {r.status_code}")
                    continue
                if not looks_like_pdf(r.content):
                    failed.append((name, "not a PDF"))
                    print(f"[skip] {name}: response is not a PDF ({len(r.content)} bytes)")
                    continue
                (OUT / name).write_bytes(r.content)
                ok.append((name, len(r.content)))
                print(f"[ok]   {name}: {len(r.content):,} bytes")
            except Exception as e:
                failed.append((name, type(e).__name__))
                print(f"[err]  {name}: {e}")

    print(f"\nDownloaded {len(ok)}/{len(DOCS)} into {OUT}")
    for n, s in ok:
        print(f"  - {n} ({s:,} bytes)")
    if failed:
        print("Failed:")
        for n, why in failed:
            print(f"  - {n}: {why}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
