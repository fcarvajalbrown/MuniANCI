"""
eval_harness.py — offline retrieval-quality gate for the Asistente (ROADMAP 0.4.0).

Runs the golden set (eval/golden_set.json) through the real RAG retrieval and scores
whether the right law is surfaced. This is the deterministic core — no LLM, fully
reproducible — that makes every later retrieval change measurable (reranker,
embeddings A/B, chunking). An LLM-judged faithfulness layer (Ragas/DeepEval with a
local judge) is layered on top separately in eval_judge.py.

Two granularities, because they answer different questions (ROADMAP 0.9.0, Tramo A):

- FILE level (schemaVersion 1): did the right law come back at all? Ground truth is
  `ground_truth_sources`, a list of corpus filenames.
- FRAGMENT level (schemaVersion 2): did the chunk that actually answers come back?
  Ground truth adds `ground_truth_fragments`, literal strings that must appear inside
  a retrieved chunk. A chunk counts only if its source matches AND its text contains
  one of those strings. This exists because file-level scoring reports a hit for the
  known artículo 9 failure: right file, wrong chunk.

Metrics per question:
- hit         : a ground-truth chunk appears in the top-k retrieved chunks (recall).
- first_rank  : 1-based rank of the first ground-truth chunk (drives MRR); 0 if miss.
- precision   : fraction of retrieved chunks that are ground truth.
- ndcg        : binary-gain nDCG@k over the retrieved ordering.

Aggregate: recall@k, MRR, mean precision, nDCG@k — reported for the whole set at file
level, and separately for the subset carrying fragment truth. `--min-recall` and
`--min-ndcg` turn it into a release gate (non-zero exit below the threshold).

Abstention entries are not scored for retrieval (they have no ground truth by design).
They are run to record their retrieval signal — best vector distance and best BM-25
score — next to the same signal on answerable questions. That separation is the input
to calibrating the abstention threshold, which does not exist yet.

Run (needs db/ + the embedding model):
    python eval/eval_harness.py
    MUNIGPT_DB_DIR=db python eval/eval_harness.py --min-recall 0.9
    python eval/eval_harness.py --fragments-only --json
"""

import argparse
import asyncio
import json
import math
import re
import sys
import unicodedata
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


def fold(text: str) -> str:
    stripped = unicodedata.normalize("NFKD", text or "")
    stripped = stripped.encode("ascii", "ignore").decode()
    return re.sub(r"\s+", " ", stripped).strip().lower()


def with_fragments(questions: list[dict]) -> list[dict]:
    return [q for q in questions if q.get("ground_truth_fragments")]


def relevance(retrieved: list[dict], sources: list[str],
              fragments: list[str] | None = None) -> list[bool]:
    truth = set(sources)
    folded = [fold(f) for f in (fragments or []) if f.strip()]
    flags = []
    for c in retrieved:
        ok = c.get("source", "") in truth
        if ok and folded:
            text = fold(c.get("text", ""))
            ok = any(f in text for f in folded)
        flags.append(ok)
    return flags


def ndcg(flags: list[bool]) -> float:
    found = sum(flags)
    if not found:
        return 0.0
    dcg = sum(1.0 / math.log2(i + 1) for i, ok in enumerate(flags, 1) if ok)
    idcg = sum(1.0 / math.log2(i + 1) for i in range(1, found + 1))
    return dcg / idcg if idcg else 0.0


def signal(retrieved: list[dict]) -> dict:
    distances = [c["_distance"] for c in retrieved if c.get("_distance") is not None]
    scores = [c["_score"] for c in retrieved if c.get("_score") is not None]
    return {
        "best_distance": round(min(distances), 4) if distances else None,
        "best_score": round(max(scores), 4) if scores else None,
    }


def mean_of(rows: list[dict], key: str) -> float | None:
    values = [r[key] for r in rows if r.get(key) is not None]
    return round(sum(values) / len(values), 4) if values else None


def separation_line(report: dict) -> str:
    ans = mean_of(report.get("answerable_signal", []), "best_distance")
    abst = mean_of(report.get("abstention_signal", []), "best_distance")
    if ans is None or abst is None:
        return "separacion   (sin distancias registradas)"
    return (f"separacion   distancia media: respondibles={ans}  "
            f"abstencion={abst}  delta={round(abst - ans, 4)}")


def evaluate_one(ground_truth_sources: list[str], retrieved: list[dict],
                 ground_truth_fragments: list[str] | None = None) -> dict:
    flags = relevance(retrieved, ground_truth_sources, ground_truth_fragments)
    first_rank = next((i for i, ok in enumerate(flags, 1) if ok), 0)
    precision = (sum(flags) / len(flags)) if flags else 0.0
    return {
        "hit": first_rank > 0,
        "first_rank": first_rank,
        "precision": precision,
        "ndcg": ndcg(flags),
    }


def aggregate(results: list[dict]) -> dict:
    n = len(results)
    if n == 0:
        return {"n": 0, "recall_at_k": 0.0, "mrr": 0.0,
                "mean_precision": 0.0, "ndcg_at_k": 0.0}
    recall = sum(r["hit"] for r in results) / n
    mrr = sum((1.0 / r["first_rank"]) if r["first_rank"] else 0.0 for r in results) / n
    mean_precision = sum(r["precision"] for r in results) / n
    mean_ndcg = sum(r.get("ndcg", 0.0) for r in results) / n
    return {
        "n": n,
        "recall_at_k": round(recall, 4),
        "mrr": round(mrr, 4),
        "mean_precision": round(mean_precision, 4),
        "ndcg_at_k": round(mean_ndcg, 4),
    }


async def run(golden_path: Path, fragments_only: bool = False) -> dict:
    all_questions = load_golden(golden_path)
    questions = scorable(all_questions)
    if fragments_only:
        questions = with_fragments(questions)

    per_question = []
    fragment_scored = []
    for q in questions:
        _, chunks = await retrieve(q["question"])
        scored = evaluate_one(q["ground_truth_sources"], chunks)
        scored["id"] = q["id"]
        scored["question"] = q["question"]
        scored.update(signal(chunks))

        fragments = q.get("ground_truth_fragments")
        if fragments:
            frag = evaluate_one(q["ground_truth_sources"], chunks, fragments)
            scored["fragment"] = frag
            fragment_scored.append({**frag, "id": q["id"], "question": q["question"]})

        per_question.append(scored)

    abstentions = []
    for q in all_questions:
        if q.get("type") != "abstention":
            continue
        _, chunks = await retrieve(q["question"])
        abstentions.append({"id": q["id"], "question": q["question"], **signal(chunks)})

    return {
        "summary": aggregate(per_question),
        "fragment_summary": aggregate(fragment_scored),
        "questions": per_question,
        "abstention_signal": abstentions,
        "answerable_signal": [
            {"id": r["id"], "best_distance": r.get("best_distance"),
             "best_score": r.get("best_score")} for r in per_question
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Asistente retrieval-quality eval gate.")
    parser.add_argument("--golden", type=Path, default=GOLDEN_DEFAULT)
    parser.add_argument("--min-recall", type=float, default=None,
                        help="Fail (exit 1) if recall@k is below this threshold.")
    parser.add_argument("--min-ndcg", type=float, default=None,
                        help="Fail (exit 1) if fragment-level nDCG@k is below this.")
    parser.add_argument("--fragments-only", action="store_true",
                        help="Score only the questions carrying fragment ground truth.")
    parser.add_argument("--json", action="store_true", help="Emit the full report as JSON.")
    args = parser.parse_args()

    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass

    report = asyncio.run(run(args.golden, fragments_only=args.fragments_only))
    s = report["summary"]
    f = report["fragment_summary"]

    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        for q in report["questions"]:
            mark = "OK " if q["hit"] else "MISS"
            rank = q["first_rank"] or "-"
            frag = q.get("fragment")
            tail = ""
            if frag:
                fmark = "OK " if frag["hit"] else "MISS"
                tail = f"  frag=[{fmark}] rank={frag['first_rank'] or '-'}"
            print(f"  [{mark}] rank={rank} prec={q['precision']:.2f}"
                  f" ndcg={q['ndcg']:.2f}{tail}  {q['question']}")
        print("\n" + "=" * 60)
        print(f"archivo    n={s['n']}  recall@k={s['recall_at_k']}  MRR={s['mrr']}  "
              f"prec={s['mean_precision']}  nDCG@k={s['ndcg_at_k']}")
        print(f"fragmento  n={f['n']}  recall@k={f['recall_at_k']}  MRR={f['mrr']}  "
              f"prec={f['mean_precision']}  nDCG@k={f['ndcg_at_k']}")
        print(separation_line(report))

    failed = False
    if args.min_recall is not None and s["recall_at_k"] < args.min_recall:
        print(f"\n[gate] recall@k {s['recall_at_k']} < {args.min_recall} — FAIL")
        failed = True
    if args.min_ndcg is not None and f["ndcg_at_k"] < args.min_ndcg:
        print(f"[gate] fragment nDCG@k {f['ndcg_at_k']} < {args.min_ndcg} — FAIL")
        failed = True
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
