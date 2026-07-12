"""
run_server.py — packaged entrypoint for the MuniGPT backend sidecar.

PyInstaller (`--onedir`, per D1) can't run `uvicorn main:app` directly, so the build
targets this script instead: it parses --host/--port and calls uvicorn.run(app).
Importing `main` also starts the parent-alive watchdog (watchdog.py). Works in dev
too, mirroring how the packaged binary is invoked:

    python run_server.py --host 127.0.0.1 --port 8000
"""

import argparse

import uvicorn

from main import app


def main() -> None:
    parser = argparse.ArgumentParser(description="MuniGPT backend sidecar")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8000)
    args = parser.parse_args()
    uvicorn.run(app, host=args.host, port=args.port)


if __name__ == "__main__":
    main()
