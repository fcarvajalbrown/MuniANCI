"""
watchdog.py — parent-alive watchdog for the sidecar backend.

The MuniGPT host (gui/src/assistant.rs) reaps this backend's process tree on a
clean exit via `taskkill /T /F`. But if the host dies abnormally (crash, kill -9,
power of the debugger), that reap never runs and the backend — plus its llama-server
children — would be orphaned. This watchdog closes that gap: the host passes its PID
in `MUNIGPT_PARENT_PID`, and a daemon thread here polls it; when the parent is gone,
it reaps the llama-server children (via inference.shutdown) and exits.

Started from main.py at import, so it runs no matter how the backend was launched
(dev `python -m uvicorn`, or the packaged PyInstaller onedir binary). A no-op when
MUNIGPT_PARENT_PID is unset (e.g. running the backend standalone).
"""

import os
import threading
import time

_DEFAULT_INTERVAL = 5.0


def _parent_gone(pid: int, create_time) -> bool:
    """True if the parent process is gone (or its PID was reused by another one).

    Uses psutil (a runtime dependency). `create_time` guards against PID reuse: if a
    process with the same PID exists but started at a different time, it is not our
    parent, so the real parent is gone. Falls back to os.kill(0) if psutil is absent.
    """
    try:
        import psutil
    except ImportError:
        try:
            os.kill(pid, 0)  # POSIX-only existence probe; not reused-PID aware
            return False
        except OSError:
            return True
    if not psutil.pid_exists(pid):
        return True
    if create_time is not None:
        try:
            return psutil.Process(pid).create_time() != create_time
        except psutil.NoSuchProcess:
            return True
        except Exception:
            return False
    return False


def _terminate() -> None:
    """Reap the llama-server children, then exit the process immediately."""
    try:
        import inference
        inference.shutdown()
    except Exception:
        pass
    os._exit(0)


def start_parent_watchdog(interval: float = _DEFAULT_INTERVAL) -> bool:
    """Start the watchdog if MUNIGPT_PARENT_PID is set. Returns True if started."""
    pid_str = os.environ.get("MUNIGPT_PARENT_PID")
    if not pid_str:
        return False
    try:
        parent_pid = int(pid_str)
    except ValueError:
        return False

    create_time = None
    try:
        import psutil
        create_time = psutil.Process(parent_pid).create_time()
    except Exception:
        # psutil missing, or parent already unreadable — poll on PID existence alone.
        create_time = None

    def _watch() -> None:
        while True:
            time.sleep(interval)
            if _parent_gone(parent_pid, create_time):
                _terminate()

    threading.Thread(target=_watch, daemon=True, name="parent-watchdog").start()
    return True
