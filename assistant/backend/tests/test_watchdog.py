"""
Unit tests for watchdog.py — parent-alive detection and start guard.

The polling loop and os._exit path are not exercised (they would terminate the test
process); the pure detection logic and the env-driven start guard are.
"""

import os

import watchdog


def test_current_process_is_not_gone():
    # Our own PID exists; with no create_time guard it must read as alive.
    assert watchdog._parent_gone(os.getpid(), None) is False


def test_unused_pid_is_gone():
    # A PID that (almost certainly) does not exist reads as gone.
    assert watchdog._parent_gone(2_000_000_000, None) is True


def test_create_time_mismatch_means_reused_pid():
    # Same PID, but a create_time that can't match -> treated as gone (PID reuse).
    assert watchdog._parent_gone(os.getpid(), -1.0) is True


def test_start_is_noop_without_env(monkeypatch):
    monkeypatch.delenv("MUNIGPT_PARENT_PID", raising=False)
    assert watchdog.start_parent_watchdog() is False


def test_start_ignores_non_integer_env(monkeypatch):
    monkeypatch.setenv("MUNIGPT_PARENT_PID", "not-a-pid")
    assert watchdog.start_parent_watchdog() is False


def test_start_returns_true_with_valid_parent(monkeypatch):
    # Point at our own PID: the watchdog thread starts (daemon; never trips here).
    monkeypatch.setenv("MUNIGPT_PARENT_PID", str(os.getpid()))
    assert watchdog.start_parent_watchdog(interval=3600) is True
