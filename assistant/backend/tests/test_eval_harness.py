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
        "n": 0, "recall_at_k": 0.0, "mrr": 0.0, "mean_precision": 0.0, "ndcg_at_k": 0.0,
    }


def test_scorable_excludes_abstention_cases():
    qs = [
        {"id": "q1", "ground_truth_sources": ["a.txt"]},
        {"id": "a1", "type": "abstention", "ground_truth_sources": []},
    ]
    kept = eval_harness.scorable(qs)
    assert [q["id"] for q in kept] == ["q1"]


def test_golden_set_is_approved_and_has_staged_abstentions():
    import json
    data = json.loads((Path(eval_harness.__file__).with_name("golden_set.json")).read_text(encoding="utf-8"))
    assert data["approved"] is True
    abstentions = [q for q in data["questions"] if q.get("type") == "abstention"]
    assert len(abstentions) >= 3
    for q in abstentions:
        assert q["ground_truth_sources"] == []  # out of corpus on purpose


def _frag_chunks(*pairs):
    return [{"source": s, "chunk_index": i, "text": t}
            for i, (s, t) in enumerate(pairs)]


def test_fragment_truth_rejects_right_file_wrong_chunk():
    # The artículo 9 failure in miniature: correct law, chunk that does not answer.
    chunks = _frag_chunks(("ley.txt", "Artículo 8. Deberes de los operadores."))
    file_level = eval_harness.evaluate_one(["ley.txt"], chunks)
    frag_level = eval_harness.evaluate_one(["ley.txt"], chunks,
                                           ["plazo máximo de tres horas"])
    assert file_level["hit"] is True
    assert frag_level["hit"] is False
    assert frag_level["first_rank"] == 0


def test_fragment_truth_accepts_the_answering_chunk():
    chunks = _frag_chunks(
        ("ley.txt", "Artículo 8. Deberes."),
        ("ley.txt", "a) Dentro del plazo máximo de tres horas contado desde que"),
    )
    r = eval_harness.evaluate_one(["ley.txt"], chunks, ["plazo máximo de tres horas"])
    assert r["hit"] is True
    assert r["first_rank"] == 2


def test_fragment_matching_ignores_accents_and_whitespace():
    chunks = _frag_chunks(("ley.txt", "dentro del  PLAZO\nmaximo de tres   horas"))
    r = eval_harness.evaluate_one(["ley.txt"], chunks, ["plazo máximo de tres horas"])
    assert r["hit"] is True


def test_fragment_requires_the_source_to_match_too():
    chunks = _frag_chunks(("otra_ley.txt", "plazo máximo de tres horas"))
    r = eval_harness.evaluate_one(["ley.txt"], chunks, ["plazo máximo de tres horas"])
    assert r["hit"] is False


def test_ndcg_rewards_the_relevant_chunk_being_first():
    top = eval_harness.evaluate_one(["a.txt"], _chunks("a.txt", "b.txt", "c.txt"))
    last = eval_harness.evaluate_one(["a.txt"], _chunks("b.txt", "c.txt", "a.txt"))
    assert top["ndcg"] == 1.0
    assert last["ndcg"] < top["ndcg"]


def test_ndcg_is_zero_without_a_relevant_chunk():
    assert eval_harness.evaluate_one(["a.txt"], _chunks("b.txt"))["ndcg"] == 0.0


def test_signal_reads_best_distance_and_score():
    chunks = [
        {"source": "a.txt", "_distance": 0.8, "_score": 3.0},
        {"source": "b.txt", "_distance": 0.4, "_score": 9.0},
    ]
    assert eval_harness.signal(chunks) == {"best_distance": 0.4, "best_score": 9.0}


def test_signal_tolerates_missing_columns():
    assert eval_harness.signal([{"source": "a.txt"}]) == {
        "best_distance": None, "best_score": None,
    }


def test_with_fragments_selects_only_the_fragment_subset():
    qs = [
        {"id": "q1", "ground_truth_sources": ["a.txt"]},
        {"id": "q2", "ground_truth_sources": ["a.txt"], "ground_truth_fragments": ["x"]},
    ]
    assert [q["id"] for q in eval_harness.with_fragments(qs)] == ["q2"]


def test_golden_set_fragments_exist_verbatim_in_their_source_file():
    # The guard against invented legal content: every fragment must be literally
    # present in the corpus file it claims to come from.
    corpus = (Path(eval_harness.__file__).resolve().parents[1] / "corpus")
    by_name = {p.name: p for p in corpus.rglob("*.txt")}
    checked = 0
    for q in eval_harness.load_golden():
        for frag in q.get("ground_truth_fragments", []):
            assert q["ground_truth_sources"], f"{q['id']} has fragments but no source"
            found = False
            for src in q["ground_truth_sources"]:
                assert src in by_name, f"{src} not in corpus"
                text = eval_harness.fold(by_name[src].read_text(encoding="utf-8"))
                if eval_harness.fold(frag) in text:
                    found = True
                    break
            assert found, f"{q['id']}: fragment not found verbatim: {frag!r}"
            checked += 1
    assert checked >= 5


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
