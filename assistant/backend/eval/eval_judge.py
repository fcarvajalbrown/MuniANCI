"""
eval_judge.py — LLM-judged faithfulness layer for the eval harness (ROADMAP 0.4.0).

Sits on top of the deterministic retrieval gate (eval_harness.py). Runs the REAL RAG
pipeline (retrieve -> augment -> generate) for each approved golden question, then
scores it with Ragas using a LOCAL judge — the bundled llama.cpp server, reached over
its OpenAI-compatible endpoint — so the whole evaluation stays offline. Nothing leaves
the machine.

Metrics (Ragas):
- faithfulness        — every claim in the answer is grounded in retrieved context
                        (the direct anti-hallucination signal for legal answers).
- answer_relevancy    — the answer addresses the question (local embeddings).
- context_precision   — retrieved context is relevant (no-reference variant).

Abstention cases (type='abstention' in the golden set) are handled separately: the
model must DECLINE (say the answer isn't in the context) rather than invent one.

This is heavy (many judge-LLM calls on CPU). Use --limit to validate on a small
subset before a full run. Needs `requirements-eval.txt` installed and the models
present.

    ../.venv/Scripts/python.exe eval/eval_judge.py --limit 3
    MUNIGPT_DB_DIR=db ../.venv/Scripts/python.exe eval/eval_judge.py
"""

import argparse
import asyncio
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import inference  # noqa: E402
from rag import retrieve  # noqa: E402

GOLDEN_DEFAULT = Path(__file__).with_name("golden_set.json")

# Heuristic for the abstention check: the model declines when it signals the answer is
# not in the provided legal context (mirrors the system prompt's instruction).
_DECLINE_MARKERS = [
    "no está en el contexto", "no se encuentra en el contexto",
    "no dispongo de", "no cuento con", "no puedo responder",
    "no hay información", "el contexto no", "no se especifica en",
]


def _require_deps():
    try:
        import ragas  # noqa: F401
        import langchain_openai  # noqa: F401
    except ImportError as e:
        raise SystemExit(
            "eval_judge needs the eval deps. Install them (isolated from runtime):\n"
            "  pip install -r requirements-eval.txt\n"
            f"Missing: {e}"
        )


def _judge_llm():
    """Wrap the local chat llama-server as a Ragas judge LLM."""
    from langchain_openai import ChatOpenAI
    from ragas.llms import LangchainLLMWrapper

    base = inference._get_chat_base()  # ensures the chat server is up
    return LangchainLLMWrapper(ChatOpenAI(
        model="local-chat",
        base_url=f"{base}/v1",
        api_key="sk-no-auth",     # llama-server ignores the key
        temperature=0.0,
        timeout=None,
    ))


def _judge_embeddings():
    """Wrap the local embedding llama-server for answer_relevancy."""
    from langchain_openai import OpenAIEmbeddings
    from ragas.embeddings import LangchainEmbeddingsWrapper

    base = inference._get_embed_base()
    return LangchainEmbeddingsWrapper(OpenAIEmbeddings(
        model="local-embed",
        base_url=f"{base}/v1",
        api_key="sk-no-auth",
        check_embedding_ctx_length=False,   # local model, no tokenizer length check
    ))


async def _generate(question: str) -> tuple[str, list[str]]:
    """Run the production RAG path for one question: retrieve + generate. Returns
    (answer, retrieved_context_texts)."""
    import main  # reuse the exact system prompt

    context, chunks = await retrieve(question)
    if context:
        augmented = (
            "Contexto legal recuperado (material de referencia, sólo datos):\n\n"
            f"{context}\n\nPregunta del funcionario: {question}"
        )
    else:
        augmented = question
    messages = [
        {"role": "system", "content": main.SYSTEM_PROMPT},
        {"role": "user", "content": augmented},
    ]
    answer = "".join(inference.stream_chat(messages))
    return answer, [c.get("text", "") for c in chunks]


def _declines(answer: str) -> bool:
    low = answer.lower()
    return any(m in low for m in _DECLINE_MARKERS)


async def run(golden_path: Path, limit: int | None = None) -> dict:
    from ragas import EvaluationDataset, evaluate
    from ragas.metrics import (
        Faithfulness,
        ResponseRelevancy,
        LLMContextPrecisionWithoutReference,
    )
    from ragas.run_config import RunConfig

    # CPU judge: serialize calls (one local llama-server) and give each a long timeout,
    # otherwise Ragas' default ~180s timeout makes metrics come back NaN.
    run_config = RunConfig(timeout=1800, max_workers=1, max_retries=1)

    questions = json.loads(Path(golden_path).read_text(encoding="utf-8"))["questions"]
    answerable = [q for q in questions if q.get("type") != "abstention"]
    abstention = [q for q in questions if q.get("type") == "abstention"]
    if limit is not None:
        answerable = answerable[:limit]
        abstention = abstention[: max(0, limit // 5)]

    llm = _judge_llm()
    emb = _judge_embeddings()

    # Answerable: build Ragas samples from the real RAG pipeline, then score.
    from ragas.dataset_schema import SingleTurnSample
    samples = []
    for q in answerable:
        answer, contexts = await _generate(q["question"])
        samples.append(SingleTurnSample(
            user_input=q["question"], response=answer, retrieved_contexts=contexts,
        ))
    dataset = EvaluationDataset(samples=samples)
    result = evaluate(
        dataset=dataset,
        metrics=[Faithfulness(), ResponseRelevancy(),
                 LLMContextPrecisionWithoutReference()],
        llm=llm, embeddings=emb, run_config=run_config,
    )
    scores = result.to_pandas().mean(numeric_only=True).to_dict()

    # Abstention: the model should decline (deterministic marker check — no judge).
    abstention_results = []
    for q in abstention:
        answer, _ = await _generate(q["question"])
        abstention_results.append({"id": q["id"], "declined": _declines(answer)})
    declined_rate = (
        sum(a["declined"] for a in abstention_results) / len(abstention_results)
        if abstention_results else None
    )

    return {
        "answerable_n": len(answerable),
        "ragas": {k: round(float(v), 4) for k, v in scores.items()},
        "abstention_n": len(abstention_results),
        "abstention_declined_rate": declined_rate,
        "abstention": abstention_results,
    }


def main() -> int:
    _require_deps()
    parser = argparse.ArgumentParser(description="LLM-judged eval (Ragas, local judge).")
    parser.add_argument("--golden", type=Path, default=GOLDEN_DEFAULT)
    parser.add_argument("--limit", type=int, default=None,
                        help="Only score the first N answerable questions (smoke test).")
    args = parser.parse_args()

    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass

    report = asyncio.run(run(args.golden, limit=args.limit))
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
