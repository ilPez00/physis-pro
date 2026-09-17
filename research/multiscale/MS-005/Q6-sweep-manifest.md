# Q6 allocation sweep — frozen manifest 2026-09-17 (research-only)

Question: at EQUAL token budget and candidate pool, does redundancy-aware
allocation (MMR diversity) beat pure cosine, and does BM25-only beat either
leg on this lexically-disjoint probe set? Spec §10 + MS-005 steps.

## Arms (all over `physis-core/src`, budget 3000, top-k 30, at=5, 7 cases)

| Arm | Command | What varies |
|---|---|---|
| A0 | `run.py --arms semantic` (diversity default 0.3 path via `pack`, semantic cascade) | reference (known 3/7) |
| A1 | `pack --diversity 0` per case, same budget/top-k | MMR off: pure cosine greedy |
| A2 | `pack --diversity 0.6` per case | MMR strong: spread over relevance |
| A3 | `find --mode lexical` per case query | BM25-only leg, no embedder |
| A4 | `find` default hybrid per case | BM25+cosine fused (existing fusion) |

A1/A2 reuse the `pack` CLI (diversity flag exists, `main.rs:106-107`, MMR in
`rag.rs:204-244`). A3/A4 reuse the `find` CLI (`ContentIndex`, BM25+cosine,
`content.rs:42-65,259-367`). No new code: two CLIs + the existing run.py
harness pattern. Utility labels = past hit/miss only, never test answers.

## Controls / tolerances

- Same corpus, budget, top-k, k=5 across arms. Embedder `ollama-embed`
  (cache warm; kind recorded per run). `find` arms score file hits the same
  generous hit@k rule as run.py (any expected basename in top 5).
- Frozen before holdout: diversity values {0, 0.3, 0.6}, budget 3000,
  top-k 30. No tuning on the 7 cases; the sweep grid IS the experiment.
- Metrics per arm: hit@5, ranks, seconds; plus redundancy (mean pairwise
  cosine of selected chunks), source concentration (files/chunks), tokens
  used vs budget. Quality-cost frontier, not quality/token alone.
- Negative/inconclusive valid. MS-005 stays blocked regardless (needs
  MS-002/MS-003 release); this sweep is Q6 evidence, not a gate pass.

## Commands

python3 benchmarks/retrieval/run.py --arms semantic --root physis-core/src
# A1/A2: pack per case with --diversity 0 / 0.6 (script reuses run.py score())
# A3/A4: find per case with --mode lexical / default (same score())
