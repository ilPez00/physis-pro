# Q6 context allocation — frozen manifest (research-only, no production edits)

Date: 2026-09-17. Repos: main `66868d2` (branch `agent/MS-001`), physis-core `d65858b`.
Workspace: pansa `/home/gio/physis_pro`.

## Scope note

MS-005 owns Q6 but is blocked until MS-002/MS-003 report (PLAN.md Phase 27).
This manifest launches NO MS-005 experiment. It freezes the Q6 design inputs
and runs the two cheapest rerunnable baselines (retrieval discrimination +
answer C/D arms) so MS-005 opens with working harnesses, not design debt.
Small model = ollama `qwen3:4b` default in `benchmarks/retrieval/answer.py:58`
— NOT present locally (ollama lists gemma4:e2b, qwen3.8-27b-q4, nomic-embed-text,
lfm2.5). C/D arms need an explicit model decision at MS-005 time.

## Corpus / splits

- Corpus: `benchmarks/retrieval/cases.json`, 10 cases, lexically disjoint
  queries vs answers by construction (`cases.json:2`).
- Arm A (run.py): all 10 cases under root `physis-core/src`; expect-files
  outside the root are skipped and counted (`run.py:93-104`).
- Arms C/D (answer.py): only gated cases (must_mention present) under root
  (`answer.py:183-187`); ungated aborts the harness.
- Split: none — corpus is a fixed probe set, not train/test. Any Q6 selection
  policy (lambda, diagonal treatment, relevance scaling) MUST be frozen on a
  separate training set before touching these cases (spec §10).

## Baselines (frozen arms)

1. `run.py --arms rp,semantic` — TokenFixedRetriever hit@5, equal budget/top-k.
   Discrimination gate: semantic − rp > 0, else no retrieval claim (`run.py:134-140`).
2. `answer.py --arms C,D` — packed vs naive context at EQUAL char budget
   (`answer.py:223`), small model, must_mention gate (`answer.py:163-165`).
   Notably `pack_context` uses the semantic cascade, `naive_context` is
   round-robin file heads — equal size, quality differs.
3. Q6 candidate (NOT run here): greedy utility/token + MMR (`rag.rs:168-256`,
   `diversity` = MMR weight, skip-and-continue packing) vs BM25
   (`Bm25Index`, `rag.rs:317`), RRF fusion (`fuse_rrf`, `rag.rs:437`),
   `rank_hybrid` (cosine+BM25). Sigma choice (covariance vs overlap vs
   similarity penalty) frozen before holdout per spec §10.

## Tolerances / metrics

- hit@k exact per case; rates as hits/n with n reported (10 max).
- Durations: wall seconds per case, reported not gated.
- answer.py: case reaching no model = `incomplete`, no rate (`answer.py:239-253`).
- Q6 proper adds: coverage, redundancy, source concentration, tokens, latency,
  memory, quality-cost frontier (spec §10) — declared here, not measured here.

## Seeds / determinism

- `fuse_rrf` ties break by ascending doc id (`rag.rs:447-451`).
- `bm25_terms` deterministic lowercase runs (`rag.rs:298-313`).
- `TokenFixedRetriever::retrieve` sorts by score desc, greedy MMR O(top_k·n)
  (`rag.rs:178-244`). No RNG in any arm.
- Embedder cascade order BGE → BGE-M3 → CLIP → MiniLM → random-projection;
  `answer.py:127-137` records actual kind into results.

## Infrastructure gaps (blockers for full Q6, not for this manifest)

- No release binary (`target/release/physis-pro` absent); debug binary present.
  Harness needs `cargo build --release --features mcp,cli --bin physis-pro`.
- No dev licence (`.physis-bench/license.key` absent); needs physis-dev-license run.
- No `qwen3:4b` locally; C/D arms need model selection + endpoint.
- MS-002 oracle (mutation replay) absent → independent changed-output labels
  for Q1/Q7 unavailable; Q6 utility labels must come from past answer.py-style
  gates only, never test answers.
