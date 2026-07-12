"""
eval_harness.py — offline retrieval-quality gate for the Asistente (ROADMAP 0.4.0).

Runs the golden set (eval/golden_set.json) through the real RAG retrieval and scores
whether the right law is surfaced. This is the deterministic core — no LLM, fully
reproducible — that makes every later retrieval change measurable (reranker,
embeddings A/B, chunking). An LLM-judged faithfulness layer (Ragas/DeepEval with a
local judge) is layered on top separately in eval_judge.py.

Metrics per question (retrieved chunks vs the ground-truth source file(s)):
- hit         : a ground-truth source appears in the top-k retrieved chunks (recall).
- first_rank  : 1-based rank of the first ground-truth chunk (drives MRR); 0 if miss.
- precision   : fraction of retrieved chunks that come from a ground-truth source.

Aggregate: recall@k (hit rate), MRR, mean precision. `--min-recall` turns it into a
release gate (non-zero exit if recall@k falls below the threshold).

Run (needs db/ + the embedding model):
    python eval/eval_harness.py
    MUNIGPT_DB_DIR=db python eval/eval_harness.py --min-recall 0.9
"""

import argparse
import asyncio
import json
import sys
from pathlib import Path

# Make the backend importable when run as `python eval/eval_harness.py`.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from rag import retrieve  # noqa: E402

GOLDEN_DEFAULT = Path(__file__).with_name("golden_set.json")


def load_golden(path: Path = GOLDEN_DEFAULT) -> list[dict]:
    """Load the golden-set questions."""
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    return data["questions"]


def scorable(questions: list[dict]) -> list[dict]:
    """Questions the deterministic retrieval gate scores. Abstention cases (the
    corpus deliberately does NOT answer them) are excluded here — they belong to the
    LLM-judge layer, which checks that the model declines; scoring them for retrieval
    would wrongly count as misses."""
    return [q for q in questions if q.get("type") != "abstention"]


def evaluate_one(ground_truth_sources: list[str], retrieved: list[dict]) -> dict:
    """Score one question. `retrieved` is the ordered list of chunks (dicts with a
    'source' key) that retrieval returned. Pure — no I/O, so it is unit-tested."""
    truth = set(ground_truth_sources)
    sources = [c.get("source", "") for c in retrieved]
    first_rank = 0
    for i, s in enumerate(sources, 1):
        if s in truth:
            first_rank = i
            break
    hit = first_rank > 0
    precision = (sum(s in truth for s in sources) / len(sources)) if sources else 0.0
    return {"hit": hit, "first_rank": first_rank, "precision": precision}


def aggregate(results: list[dict]) -> dict:
    """Aggregate per-question results into recall@k, MRR and mean precision."""
    n = len(results)
    if n == 0:
        return {"n": 0, "recall_at_k": 0.0, "mrr": 0.0, "mean_precision": 0.0}
    recall = sum(r["hit"] for r in results) / n
    mrr = sum((1.0 / r["first_rank"]) if r["first_rank"] else 0.0 for r in results) / n
    mean_precision = sum(r["precision"] for r in results) / n
    return {
        "n": n,
        "recall_at_k": round(recall, 4),
        "mrr": round(mrr, 4),
        "mean_precision": round(mean_precision, 4),
    }


async def run(golden_path: Path) -> dict:
    """Run every golden question through retrieval and score it."""
    questions = scorable(load_golden(golden_path))
    per_question = []
    for q in questions:
        _, chunks = await retrieve(q["question"])
        scored = evaluate_one(q["ground_truth_sources"], chunks)
        scored["id"] = q["id"]
        scored["question"] = q["question"]
        per_question.append(scored)
    return {"summary": aggregate(per_question), "questions": per_question}


def main() -> int:
    parser = argparse.ArgumentParser(description="Asistente retrieval-quality eval gate.")
    parser.add_argument("--golden", type=Path, default=GOLDEN_DEFAULT)
    parser.add_argument("--min-recall", type=float, default=None,
                        help="Fail (exit 1) if recall@k is below this threshold.")
    parser.add_argument("--json", action="store_true", help="Emit the full report as JSON.")
    args = parser.parse_args()

    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass

    report = asyncio.run(run(args.golden))
    s = report["summary"]

    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        for q in report["questions"]:
            mark = "OK " if q["hit"] else "MISS"
            rank = q["first_rank"] or "-"
            print(f"  [{mark}] rank={rank} prec={q['precision']:.2f}  {q['question']}")
        print("\n" + "=" * 60)
        print(f"n={s['n']}  recall@k={s['recall_at_k']}  MRR={s['mrr']}  "
              f"mean_precision={s['mean_precision']}")

    if args.min_recall is not None and s["recall_at_k"] < args.min_recall:
        print(f"\n[gate] recall@k {s['recall_at_k']} < {args.min_recall} — FAIL")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
