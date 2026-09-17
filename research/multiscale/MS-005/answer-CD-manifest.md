# answer.py C/D manifest — frozen 2026-09-17 (research-only)

Small model: omo `qwen3:4b` via `PHYSIS_SMALL_URL=http://100.116.39.57:11434/v1`
(probe: exact `OK` reply, temperature 0). Large arms (A/B) NOT run — no oracle
endpoint configured; harness reports `not_configured`, not scored.

## Arms

- C = packed context (physis `pack` semantic cascade → now `ollama-embed`,
  nomic-embed-text, kind recorded in results) + small model.
- D = naive context (round-robin file heads, equal char budget per case) +
  small model.
- Equal size enforced: naive budget = len(packed) per case (`answer.py:223`).
  Quality differs, size doesn't.

## Corpus / gate

- Cases: `benchmarks/retrieval/cases.json`, gated subset under
  `physis-core/src` with `must_mention` present (`answer.py:183-187`).
- Gate: literal case-insensitive `must_mention` identifiers, no partial
  credit (`answer.py:163-165`). Same rule as `oracle::cites_evidence`.
- Prompt: ≤3 sentences, exact identifiers, `NOT IN CONTEXT` when absent.
  Reasoning stripped (`<think>` / reasoning field), default mode both tiers.

## Metrics / tolerances

- Per arm: passes/scored, rate = passes/n, errors → `incomplete`, no rate
  (`answer.py:239-253`). Context chars + prompt tokens + seconds per case.
- Thesis deltas: C−D (physis contribution), C−B needs large-model run (absent
  → UNSCORED, no claim). A−B compression loss likewise deferred.
- Determinism: temperature 0; retries only 429/5xx (`answer.py:74-75`).

## Command

PHYSIS_SMALL_URL=http://100.116.39.57:11434/v1 PHYSIS_SMALL_MODEL=qwen3:4b \
  python3 benchmarks/retrieval/answer.py --arms C,D --root physis-core/src
