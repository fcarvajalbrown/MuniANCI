"""
eval_judge.py — LLM-judged faithfulness layer for the eval harness (SCAFFOLD).

Status: DEFERRED (ROADMAP 0.4.0). The deterministic retrieval gate (eval_harness.py)
is the working release gate. This module is the planned layer on top: it scores
faithfulness and answer-relevancy with a LOCAL judge (the bundled llama.cpp server,
OpenAI-compatible) so the whole evaluation stays offline — no cloud judge, nothing
leaves the machine. Kept as a scaffold, not wired into any gate, until:
  1. the golden set (golden_set.json) is approved, and
  2. the Ragas/DeepEval wheels are vendored (vendor/wheels, ROADMAP Apendice C).

Intended design (do not enable a heavy CPU run without owner sign-off):
- Judge LLM: point Ragas/DeepEval at the local llama-server's OpenAI-compatible
  endpoint (http://127.0.0.1:<chat_port>/v1) via a LangChain ChatOpenAI wrapper with a
  dummy api_key; embeddings via the same local embedding server. Discover the ports
  through inference.py rather than hardcoding.
- For each approved golden question: run the real /chat pipeline (retrieve -> augment
  -> generate), then score:
    * faithfulness      — is every claim in the answer grounded in retrieved context?
      (the direct anti-hallucination metric for legal answers)
    * answer_relevancy  — does the answer address the question?
    * context_precision / context_recall — retrieval quality (overlaps the
      deterministic gate; kept for cross-check).
- Add negative / abstention cases to the golden set (questions the corpus does NOT
  answer) and assert the model abstains — this is the metric that belongs here, not in
  the deterministic retrieval gate.
- Emit a JSON report and a per-metric threshold gate, same shape as eval_harness.

Run (once enabled): python eval/eval_judge.py --golden eval/golden_set.json
"""

import sys


def _require_deps():
    """Fail with actionable guidance until the layer is enabled."""
    try:
        import ragas  # noqa: F401
        import deepeval  # noqa: F401
    except ImportError as e:
        raise SystemExit(
            "eval_judge is a deferred scaffold. To enable the LLM-judged layer: "
            "vendor + install ragas and deepeval (see vendor/wheels and ROADMAP "
            "Apendice C), approve golden_set.json, then implement run() per the "
            "module docstring. Missing dependency: " + str(e)
        )


def main() -> int:
    _require_deps()
    # Intentionally unimplemented until the golden set is approved and the wheels are
    # vendored — see the module docstring for the intended design.
    raise SystemExit("eval_judge: not yet implemented (deferred layer).")


if __name__ == "__main__":
    sys.exit(main())
