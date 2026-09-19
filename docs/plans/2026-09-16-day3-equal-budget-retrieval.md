# Day 3 — equal-budget retrieval: the shipped pack loses to BM25 on evidence spans (2026-09-16)

Continues `docs/plans/2026-09-16-day2-adapter-parity.md`. Day 3 of
`docs/plans/2026-09-15-core-pro-development.md` asks for one thing: **"repair
retrieval using an equal-budget experiment"**, with the gate "all citations
resolve and budgets hold … a quality improvement needs evidence against the
strongest simple baseline, not only against the current implementation. If
inconclusive, keep the baseline and record the misses."

The experiment was built and run. The shipped packer is **behind a plain BM25
baseline at the same token budget** on the frozen held-out set, so nothing was
promoted. The repair is now a specific, evidence-backed change with a named
mechanism rather than a direction.

## 1. What was wrong with the existing harness

`benchmarks/retrieval/run.py` scores hit@k over **file names**, at a budget it
never reports, with no notion of *which lines* answer the query. It cannot
support the Day 3 claim, because the claim is about evidence per token and a
file-name hit at an unreported budget is not comparable. Its recorded result
(2026-09-12, `semantic` 4/7 vs `rp` 1/7) still stands as the discrimination
check it was written to be; it is simply a different measurement.

## 2. The harness (`benchmarks/retrieval/equal_budget.py`)

Four arms, one budget, one chunk grid, one tokenizer, one scoring rule.

| arm | what it is |
|---|---|
| `file_heads` | first 40-line window of each file in path order, blind to the query — the context a naive retriever pulls in |
| `bm25` | Okapi BM25 over the same 40-line chunk grid, greedy fill — the strongest simple baseline |
| `current_pack` | the shipped `physis-pro pack` at its shipped default (`--diversity 0.3`) |
| `candidate_pack` | the same binary at the diversity chosen on the dev split |

Design points that make the comparison mean something:

- **One tokenizer.** The Python counter mirrors
  `physis-core/src/rag.rs::count_tokens`. It is checked against the shipped
  binary's own per-chunk counts before any arm is scored
  (`--check-tokenizer`): **23/23 chunks agree** across both corpora. A mirror
  that disagreed would make every budget claim in this note worthless, so a
  mismatch is a hard failure, not a warning.
- **One chunk grid.** `CHUNK_LINES = 40`, `MAX_FILE_BYTES = 512 KiB`, the same
  skip list and the same NUL-byte text test as `src/pack.rs`. The baselines and
  the arm they are compared against must see the same candidates.
- **Evidence spans, not file names.** Each case names the exact line range that
  answers it, read out of the code. Coverage is the *union* of the selected
  windows: a span that straddles a 40-line boundary is covered when both halves
  are selected, which is the honest reading of "the evidence is in the
  context". `line_recall` is reported alongside so "missed by one line" is
  distinguishable from "missed entirely".
- **Budgets and citations are asserted.** Every arm's selected tokens must be
  `<= budget`, and every cited window must exist in the corpus revision.
- **The answer arm is `NOT MEASURED`.** No answer model was configured in this
  run, and the plan says a missing model arm is not measured rather than
  quietly omitted.

Frozen cases: `benchmarks/retrieval/cases_day3.json`, 16 cases, 7 dev / 9
held-out, classed as symbol, paraphrase, italian, negation, dup-header,
changed-file, cross-border. Ground truth was established by reading the code,
not by running the retriever.

## 3. Result

Corpora, frozen at revision `96a98c41b97be7c262a550caad4691a513d04b87`
(39 dirty files, all pre-existing Day 1/2 work):

| group | root | chunks | files | content hash |
|---|---|---|---|---|
| `core` | `physis-core/src` | 605 | 46 | `6bee2ac4876f677f` |
| `pro` | `src` | 2939 | 240 | `73c9dbc040220fb9` |

Budget 3000 tokens, `--top-k 50`, identical for every arm.

**Held-out (9 cases, scored once after the candidate was frozen):**

| arm | case hits | span recall | line recall | tokens | wall | over budget | unresolved citations |
|---|---|---|---|---|---|---|---|
| `file_heads` | 0/9 | 0.000 | 0.000 | 26964 | 0.0 s | 0 | 0 |
| `bm25` | **5/9** | **0.556** | **0.556** | 26812 | 1.9 s | 0 | 0 |
| `current_pack` | 3/9 | 0.333 | 0.360 | 26992 | 75.8 s | 0 | 0 |
| `candidate_pack` | 3/9 | 0.333 | 0.360 | 26993 | 75.0 s | 0 | 0 |

Dev split (7 cases) chose `diversity = 0.0` for the candidate (3/7 cases, span
recall 0.429, against 3/7 for the shipped 0.3). On held-out it changes nothing,
which is itself the answer to "is the shipped diversity default wrong?" — no.

Grouped by root: `core` bm25 1/3, pack 1/3; `pro` bm25 4/6, pack 2/6.
Grouped by class: bm25 wins `symbol`, `changed-file`, `italian`, `negation`;
the pack wins `paraphrase` (1/2 against bm25's 0/2); `dup-header` 1/2 each;
`cross-border` 1/3 vs 0/3.

**Verdict: `KEEP_BASELINE`.** The gate passed on the mechanical conditions — all
citations resolve, no arm exceeded the budget — and failed on the substantive
one: nothing beat BM25. The honest reading is stronger than "inconclusive": the
shipped semantic pack is *two cases behind* a lexical baseline on this set, at
roughly **40× the latency**.

## 4. Why the pack misses — the mechanism, from the recorded rows

`sym-choose-worker` (query `choose_worker`, required span
`orchestration/mod.rs:145-165`) is the clean case:

- `bm25` selected `orchestration/mod.rs` **twice** — both halves of the
  boundary-straddling window — for `line_recall 1.0`.
- `current_pack` selected `orchestration/mod.rs` **once**, landing
  `line_recall 0.24`, and spent the rest of the budget on 17 files including
  `web/axes.rs` (2 tokens) and `capability.rs` (7 tokens).

The packer's greedy MMR fills the budget with many small, diverse fragments.
When the evidence straddles a chunk boundary, the second half loses to a fresh
2-token fragment from an unrelated file. `cross-border` is the only class where
the pack scores 0/3 while BM25 scores 1/3.

The `italian` class is a second, independent signal: the semantic arm loses
both Italian cases and BM25 wins one. The cascade-selected encoder is
`bge-base`, an English model. That is the Day 9 trigger *"Italian retrieval
fails on Day 3 → representation comparison: installed multilingual encoder vs
current encoder and lexical baseline"*, now with a measured case set behind it.

## 5. The repair, specified but not promoted

The mechanism names the change: **when a selected chunk's neighbour in the same
file fits the remaining budget, take it too** (adjacency expansion), or
equivalently apply a minimum-chunk-size floor so 2–7 token fragments cannot
displace evidence. `RagCorpus` already carries a `bm25: Bm25Index` leg that
`TokenFixedRetriever::retrieve` never consults — a hybrid (lexical ∪ cosine,
rank-fused, filled to the same budget) is the other candidate.

Neither was implemented here, deliberately: the held-out set is now spent, and
scoring a new candidate against it would be tuning on the test set. The plan's
own rule applies — *freeze the final held-out cases before scoring*. A new
candidate needs a **fresh frozen holdout**, and the harness now makes that
cheap: add cases to `cases_day3.json` with `split: "heldout"` and re-run.

## 6. Evidence

| check | command | result |
|---|---|---|
| tokenizer mirror | `python3 benchmarks/retrieval/equal_budget.py --check-tokenizer` | 23/23 chunks agree (12 core, 11 pro) |
| dev sweep + held-out | `python3 benchmarks/retrieval/equal_budget.py` | 6m33s; KEEP_BASELINE; 0 over-budget; 0 unresolved citations |
| result record | `benchmarks/retrieval/day3-results.json` | revision, corpus hashes, per-case rows, summaries, verdict |

Feature set: `--features` default for the harness; the pack arm runs
`target/release/physis-pro` with `PHYSIS_ALLOW_DEV_LICENCE=1` and a warm
`PHYSIS_DATA_DIR=.physis-bench` embed cache (2939 hits, 0 misses on the `pro`
root — the 75 s is cache load and query embedding, not corpus embedding).

## 7. Agent/MCP surface — inspected, one break found

Requested alongside the plan day, so recorded here rather than as a plan item.

- **Pro MCP** (`src/mcp.rs`, 30 `physis_*` tools) is entirely inside
  `#[cfg(feature = "mcp")]`, and `mcp` is **not** in `Cargo.toml`'s default
  features (`default = ["cli", "voice", "web", "tui", "sql", "postgres",
  "email", "embed-onnx"]`). The checked-in `target/release/physis-pro`
  (14 Sep 10:44) therefore answers `error: unrecognized subcommand 'serve'`,
  and `.mcp.json`'s `["serve", "--stdio"]` entry **cannot start in this
  checkout**. `scripts/package.sh` builds `--features full` (which includes
  `mcp`), so released artifacts are fine; a plain `cargo build --release` is
  not. Fix is the documented one: `cargo build --release --features mcp,cli
  --bin physis-pro` (see `docs/AGENT_DOGFOOD.md`, `docs/ORCHESTRATOR_AGENT_INTEGRATION.md`).
- **Core MCP** (`physis-pro system serve`) is a different, working server: 10
  tools (`physis_capabilities/inspect/find/pack/read/history/remember/predict/
  tasks/task`), no licence, no model. Verified live — `initialize` returns
  `{"name":"physis-system","version":"0.1.21"}`, `tools/list` returns 10.
- **Coverage gap:** no integration test launches the 30-tool Pro server.
  `tests/system_mcp.rs` exercises `system serve`; `src/mcp.rs`'s 5 unit tests
  cover JSON argument helpers only. Day 8's "CLI/MCP/UI agree after restart"
  has nothing behind it for the Pro surface yet.
- **Agent** (`src/ai/`): `agent.rs` (`run_agent`/`run_agent_stream`,
  `AgentConfig`, `EventSink`), `tools.rs` (`ToolRegistry`), `provider.rs`
  (cascade + failover), `memory.rs` (`EpisodicMemory` on sled), `session.rs`,
  `harness.rs`; web surface `src/web/agent.rs` (SSE `/api/v1/agent/run`, abort,
  `AgentRunRegistry`). Day 7 ("make agent memory about outcomes") targets this
  and has not started.

## 8. Remaining, explicitly not done

1. **No candidate was promoted.** Adjacency expansion / minimum chunk size and
   the unused `RagCorpus.bm25` hybrid leg are specified above and untested.
2. **The `answer` arm is `NOT_MEASURED`** — no provider configured.
3. **The case set is lexically biased on purpose** in some classes (Italian
   queries share tokens with the evidence), which is why BM25 does well. That is
   the point of a strong baseline, but it means the numbers are not evidence
   about open-domain semantic retrieval.
4. **`file_heads` is weak by construction** (0/9). It is a floor, not a rival.
5. **The pack arm is ~40× slower** than BM25 here; the latency column is part
   of the tradeoff and was not optimized.

**Research verdict: `unsupported`** — the shipped semantic pack is not better
than the strongest simple baseline at equal budget on this set. No promotion.

**Next action:** freeze a fresh holdout and test adjacency expansion against
`bm25` on it, or move to Day 4 (explicit event-time vs knowledge-time queries)
and carry the repair as a Day 9 research follow-up.

**Superseded later the same day** by
[`2026-09-16-day3-retrieval-repair.md`](2026-09-16-day3-retrieval-repair.md):
the hybrid candidate named here was implemented and scored on a fresh holdout
and did not improve, and the measurement moved the repair target from the
ranking leg to the selection policy.

**Actor**: continuation session (OpenCode)
**Date**: 2026-09-16
**Revisions**: Pro `96a98c4` (branch `main`, 39 dirty files), Core `d65858b`
