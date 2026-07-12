"""
Unit tests for eval_judge.py pure helpers. The Ragas run itself needs the local
llama-server and is a heavy manual activity (see the module docstring), so it is not
exercised here — only the deterministic decline heuristic.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "eval"))

import eval_judge  # noqa: E402


def test_declines_detects_context_absence_phrasing():
    assert eval_judge._declines("La respuesta no está en el contexto proporcionado.")
    assert eval_judge._declines("No dispongo de esa información en el contexto legal.")


def test_declines_false_for_a_substantive_answer():
    answer = "El artículo establece que la patente se paga anualmente ante la Tesorería."
    assert eval_judge._declines(answer) is False


def test_declines_is_accent_and_case_tolerant_on_markers():
    # Marker matching is lowercased; a capitalized decline still counts.
    assert eval_judge._declines("EL CONTEXTO NO contiene esa norma.")
