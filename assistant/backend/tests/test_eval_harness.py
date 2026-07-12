"""
Unit tests for eval_harness.py metric functions (pure — no model, no DB).
The end-to-end run() is exercised separately against the real db/.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "eval"))

import eval_harness  # noqa: E402


def _chunks(*sources):
    return [{"source": s, "chunk_index": i} for i, s in enumerate(sources)]


def test_evaluate_one_hit_at_first_rank():
    r = eval_harness.evaluate_one(["ley_a.txt"], _chunks("ley_a.txt", "ley_b.txt"))
    assert r["hit"] is True
    assert r["first_rank"] == 1
    assert r["precision"] == 0.5


def test_evaluate_one_hit_at_later_rank():
    r = eval_harness.evaluate_one(["ley_a.txt"], _chunks("ley_b.txt", "ley_c.txt", "ley_a.txt"))
    assert r["hit"] is True
    assert r["first_rank"] == 3


def test_evaluate_one_miss():
    r = eval_harness.evaluate_one(["ley_a.txt"], _chunks("ley_b.txt", "ley_c.txt"))
    assert r["hit"] is False
    assert r["first_rank"] == 0
    assert r["precision"] == 0.0


def test_evaluate_one_empty_retrieval():
    r = eval_harness.evaluate_one(["ley_a.txt"], [])
    assert r["hit"] is False
    assert r["precision"] == 0.0


def test_evaluate_one_multiple_ground_truths():
    r = eval_harness.evaluate_one(["a.txt", "b.txt"], _chunks("a.txt", "b.txt", "c.txt"))
    assert r["precision"] == 2 / 3
    assert r["first_rank"] == 1


def test_aggregate_mixes_hits_and_misses():
    results = [
        {"hit": True, "first_rank": 1, "precision": 1.0},
        {"hit": True, "first_rank": 2, "precision": 0.5},
        {"hit": False, "first_rank": 0, "precision": 0.0},
    ]
    agg = eval_harness.aggregate(results)
    assert agg["n"] == 3
    assert agg["recall_at_k"] == round(2 / 3, 4)
    assert agg["mrr"] == round((1.0 + 0.5 + 0.0) / 3, 4)
    assert agg["mean_precision"] == round(1.5 / 3, 4)


def test_aggregate_empty():
    assert eval_harness.aggregate([]) == {
        "n": 0, "recall_at_k": 0.0, "mrr": 0.0, "mean_precision": 0.0,
    }


def test_load_golden_references_only_real_corpus_sources():
    # Every ground-truth source must be a real filename present in the corpus dir,
    # so the golden set can never reference an invented file.
    corpus_files = {p.name for p in (Path(eval_harness.__file__).resolve().parents[1]
                                     / "corpus").rglob("*.txt")}
    questions = eval_harness.load_golden()
    assert len(questions) >= 25
    for q in questions:
        for src in q["ground_truth_sources"]:
            assert src in corpus_files, f"{src} not in corpus"
