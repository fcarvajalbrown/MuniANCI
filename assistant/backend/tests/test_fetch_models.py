"""
Unit tests for fetch_models.py — SHA256 verification, offline-pack copy, resumable
download, and orchestration. Network is exercised only against a local throwaway
HTTP server (with real Range support), never the internet.
"""

import hashlib
import http.server
import threading

import pytest

import fetch_models


def _entry(path, data, size=None, sha=None, confirmed=False, url=None):
    """Build a manifest-style entry for `data`, with overridable size/sha/source."""
    return {
        "name": path,
        "filename": path,
        "sha256": sha if sha is not None else hashlib.sha256(data).hexdigest(),
        "sizeBytes": size if size is not None else len(data),
        "source": {"confirmed": confirmed, "url": url},
    }


# ── manifest ─────────────────────────────────────────────────────────────────────

def test_load_real_manifest_has_three_verifiable_models():
    models = fetch_models.load_manifest()
    assert len(models) == 3
    for m in models:
        assert len(m["sha256"]) == 64          # real hex digest
        assert isinstance(m["sizeBytes"], int) and m["sizeBytes"] > 0
        assert m["source"]["confirmed"] is False  # candidates, not yet approved


# ── verification ─────────────────────────────────────────────────────────────────

def test_sha256_file_matches_hashlib(tmp_path):
    p = tmp_path / "m.gguf"
    data = b"gguf-bytes-" * 100
    p.write_bytes(data)
    assert fetch_models.sha256_file(p) == hashlib.sha256(data).hexdigest()


def test_is_valid_true_for_matching_file(tmp_path):
    p = tmp_path / "m.gguf"
    data = b"abc" * 50
    p.write_bytes(data)
    assert fetch_models.is_valid(p, _entry("m.gguf", data)) is True


def test_is_valid_false_on_size_mismatch(tmp_path):
    p = tmp_path / "m.gguf"
    data = b"abc" * 50
    p.write_bytes(data)
    assert fetch_models.is_valid(p, _entry("m.gguf", data, size=999)) is False


def test_is_valid_false_on_sha_mismatch(tmp_path):
    p = tmp_path / "m.gguf"
    p.write_bytes(b"abc" * 50)
    entry = _entry("m.gguf", b"abc" * 50, sha="0" * 64)
    assert fetch_models.is_valid(p, entry) is False


def test_is_valid_false_when_absent(tmp_path):
    assert fetch_models.is_valid(tmp_path / "nope.gguf", _entry("nope.gguf", b"x")) is False


# ── offline pack ─────────────────────────────────────────────────────────────────

def test_install_from_pack_copies_valid_file(tmp_path):
    data = b"model-weights" * 1000
    pack = tmp_path / "pack"; pack.mkdir()
    dest = tmp_path / "models"
    (pack / "m.gguf").write_bytes(data)
    entry = _entry("m.gguf", data)
    assert fetch_models.install_from_pack(entry, pack, dest) is True
    assert (dest / "m.gguf").read_bytes() == data
    assert not (dest / "m.gguf.part").exists()  # temp cleaned up


def test_install_from_pack_rejects_corrupt_pack_file(tmp_path):
    pack = tmp_path / "pack"; pack.mkdir()
    dest = tmp_path / "models"
    (pack / "m.gguf").write_bytes(b"corrupt")
    entry = _entry("m.gguf", b"the-real-bytes", sha=hashlib.sha256(b"the-real-bytes").hexdigest())
    assert fetch_models.install_from_pack(entry, pack, dest) is False
    assert not (dest / "m.gguf").exists()


# ── download gating ──────────────────────────────────────────────────────────────

def test_download_refuses_unconfirmed_source(tmp_path):
    # confirmed=False must never touch the network.
    entry = _entry("m.gguf", b"x", confirmed=False, url="http://example.invalid/m.gguf")
    assert fetch_models.download(entry, tmp_path) is False


# ── resumable HTTP download (local server) ───────────────────────────────────────

class _RangeHandler(http.server.BaseHTTPRequestHandler):
    payload = b""
    honor_range = True

    def do_GET(self):
        data = type(self).payload
        rng = self.headers.get("Range")
        if rng and type(self).honor_range and rng.startswith("bytes="):
            start = int(rng.split("=", 1)[1].split("-", 1)[0])
            chunk = data[start:]
            self.send_response(206)
            self.send_header("Content-Range", f"bytes {start}-{len(data) - 1}/{len(data)}")
            self.send_header("Content-Length", str(len(chunk)))
            self.end_headers()
            self.wfile.write(chunk)
        else:
            self.send_response(200)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

    def log_message(self, *_):
        pass


def _serve(payload, honor_range=True):
    _RangeHandler.payload = payload
    _RangeHandler.honor_range = honor_range
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _RangeHandler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, f"http://127.0.0.1:{server.server_address[1]}/m.gguf"


def test_httpx_fresh_download(tmp_path):
    payload = bytes(range(256)) * 40
    server, url = _serve(payload)
    try:
        dest = tmp_path / "m.gguf"
        fetch_models._download_httpx(url, dest)
        assert dest.read_bytes() == payload
    finally:
        server.shutdown()


def test_httpx_resumes_from_partial(tmp_path):
    payload = bytes(range(256)) * 40
    server, url = _serve(payload)
    try:
        dest = tmp_path / "m.gguf"
        # Simulate an interrupted download: half the bytes already on disk.
        (tmp_path / "m.gguf.part").write_bytes(payload[: len(payload) // 2])
        fetch_models._download_httpx(url, dest)
        assert dest.read_bytes() == payload  # resumed to a complete, correct file
    finally:
        server.shutdown()


def test_httpx_restarts_when_server_ignores_range(tmp_path):
    payload = bytes(range(256)) * 40
    server, url = _serve(payload, honor_range=False)  # always 200, full body
    try:
        dest = tmp_path / "m.gguf"
        (tmp_path / "m.gguf.part").write_bytes(payload[:100])  # stale partial
        fetch_models._download_httpx(url, dest)
        assert dest.read_bytes() == payload  # not corrupted by the stale prefix
    finally:
        server.shutdown()


# ── orchestration ────────────────────────────────────────────────────────────────

def test_ensure_models_reports_present_pack_and_missing(tmp_path):
    dest = tmp_path / "models"; dest.mkdir()
    pack = tmp_path / "pack"; pack.mkdir()

    present_data = b"present" * 100
    (dest / "a.gguf").write_bytes(present_data)
    pack_data = b"packed" * 100
    (pack / "b.gguf").write_bytes(pack_data)

    manifest = [
        _entry("a.gguf", present_data),   # already valid in dest
        _entry("b.gguf", pack_data),      # only in the pack
        _entry("c.gguf", b"cee" * 100),   # nowhere, unconfirmed source
    ]
    status = fetch_models.ensure_models(manifest, dest, pack_dir=pack, allow_download=True)
    assert status == {"a.gguf": "present", "b.gguf": "from_pack", "c.gguf": "missing"}
