# Day 3 (repair) — the ranking leg is not the binding constraint (2026-09-16)

Continues `docs/plans/2026-09-16-day3-equal-budget-retrieval.md`, which measured
the shipped packer behind a plain BM25 baseline and left the repair as a
specified-but-unpromoted change. This session re-verified Day 2, found the actual
root cause of the retrieval weakness, repaired it, and scored the repair on a
fresh frozen holdout.

The repair is a **negative result with a positive structural change**: the
ranking leg was not the problem, and the packer now has one retriever instead of
two divergent ones.

## 0. Day 2 re-verified first

The Day 2 gate was re-run before touching anything, because this session changes
retrieval and the boundary must be known to hold before the change, not after.

| check | command | result |
|---|---|---|
| boundary gate | `scripts/check-core-boundary.sh` | PASS (4/4: dependency direction, module surface, command surface, removed verb exits 2) |
| migration parity | `PHYSIS_EMBEDDER=random-projection cargo test --test observe_migration` | 4 passed, 0 failed |
| moved routes | `PHYSIS_EMBEDDER=random-projection cargo test --test ported_routes` | 10 passed, 0 failed |
| Core standalone | `tar --exclude=target` to a temp dir, then `cargo check --no-default-features` | exit 0 |

## 1. Root cause: the packer was built on a stale fork

`src/rag.rs` (411 lines) was a fork of `physis-core/src/rag.rs` (734 lines). It
predated the G5 hybrid work and lacked:

- the `bm25: Bm25Index` field on `RagCorpus`,
- `Bm25Index`, `bm25_terms`, `rank_by_cosine`,
- `fuse_rrf`, `rank_hybrid`,
- `HybridVerdict` and `measure_hybrid_vs_cosine`.

`src/pack.rs` imported `crate::rag`. So the packer was the **one retrieval path
built on a copy that could not see the fusion the rest of the codebase already
used** — `fuse_rrf` is called from `src/ported/act.rs:503` and swept as `rrf@60`
in `act_recall.rs`, and `src/ported/studio.rs:1408` already imported
`physis_core::rag` directly and built the BM25 leg. The fork was the bug.

## 2. The change

**De-forked.** `src/rag.rs` is now `pub use physis_core::rag::*;` plus one
helper. 411 lines of divergent duplicate removed; every existing
`crate::rag::…` / `physis_pro::rag::…` caller is unchanged.

The helper exists because the two `VectorEmbed` traits are **not
interchangeable**: Core's is minimal (`embed` / `embed_batch` / `dimension`) and
the product's adds `embed_image`. `RagCorpus::build` takes Core's, so a product
embedder cannot be handed to it. `corpus_from_texts(texts, &dyn
crate::embed::VectorEmbed)` builds the identical corpus — same chunk ids, same
token counter, same BM25 leg — so product callers cannot forget the lexical leg.
Two call sites (`web/search.rs`, `web/ui_chat.rs`) that passed a product
embedder to Core's `RagCorpus::build` now use it; five `RagCorpus` literals
(`cli.rs`, `pack.rs`, `web/search.rs`, `web/agent.rs`, `mcp.rs`) gained the
`bm25` leg.

**Core: a reusable budget fill.** `TokenFixedRetriever::retrieve_with_ranking(&
[(chunk id, score)], corpus)` holds the existing greedy MMR/budget loop and
selects on the scores it is handed; `retrieve(query_emb, corpus)` now delegates
to it over `rank_by_cosine`. This is the plan's permitted Core correction —
reusable without Pro infrastructure — and it is what lets any ranking leg reuse
the token-fixed packing. The score, not the list order, is the criterion; a test
pins that (a hand-made ranking that disagrees with cosine still drives the
selection) and pins the delegation.

**Pro: the mode.** `PackMode { Cosine, Hybrid }`, `Default = Cosine`. `pack`
and `pack_cached` take it; `finish_pack` picks `retrieve` or
`retrieve_with_ranking(&rank_hybrid(...))` — same budget, same `top_k`, same MMR
discount, only the ranking differs. CLI `pack --mode cosine|hybrid`. The MCP
tool, `estimate` and `warm` pass `PackMode::default()`, so the default lives in
exactly one place and flipping it later is a one-line change.

The default stays `Cosine`. A retrieval default is a product decision, and the
repository's own promotion rule is `HybridVerdict::ships_fusion` — which needs a
measurement that says ship.

## 3. The measurement

Fresh frozen holdout, `benchmarks/retrieval/cases_day3b.json`: 12 cases, **all
held out**. No dev split, because the candidate has no tuned parameter — RRF `k`
is fixed inside `physis-core` — so there was nothing to fit. Set #1 was spent
scoring the cosine arm and was not reused. No case is shared with set #1.

Corpora frozen at the working-tree revision, both arms scored in one invocation:

| group | root | chunks | files | content hash |
|---|---|---|---|---|
| `core` | `physis-core/src` | 606 | 46 | `01fd052dcf624e65` |
| `pro` | `src` | 2932 | 240 | `1d082d2349c4c5a1` |

Budget 3000 tokens, `--top-k 50`, identical for every arm. Tokenizer mirror
re-checked against the shipped binary before scoring: **26/26 chunks agree**.

| arm | case hits | span recall | line recall | tokens | wall | over budget | unresolved citations |
|---|---|---|---|---|---|---|---|
| `file_heads` | 1/12 | 0.083 | 0.083 | 35928 | 0.0 s | 0 | 0 |
| `bm25` | **4/12** | **0.333** | **0.509** | 35493 | 2.1 s | 0 | 0 |
| `current_pack` (cosine) | 3/12 | 0.250 | 0.362 | 35904 | 102.3 s | 0 | 0 |
| `candidate_pack` (hybrid) | 3/12 | 0.250 | 0.362 | 35927 | 105.3 s | 0 | 0 |

**Verdict: `KEEP_BASELINE`.** The hybrid does not beat cosine, and neither beats
BM25. Gate conditions hold: 0 over-budget, 0 unresolved citations, answer arm
`NOT_MEASURED` (no provider).

## 4. Why — the ranking leg is not the binding constraint

This is the finding worth keeping. Across all 12 held-out cases the two pack
arms are **identical on `case_hit` and on `line_recall`** — while selecting
materially different files:

| case | cosine selects | hybrid selects |
|---|---|---|
| `sym-fuse-rrf` | `coverage.rs`, `discovery.rs`, `embed_onnx.rs`, `linkage.rs`, `map.rs` … | `claim_identity.rs`, `config_run.rs`, `coverage.rs`, `explanation.rs`, `linkage.rs` … |
| `xborder-retrieve-ranking` | `coverage.rs`, `embed_onnx.rs`, `linkage.rs`, `main.rs`, `map.rs` … | `chain.rs`, `claim_identity.rs`, `coverage.rs`, `linkage.rs`, `main.rs` … |

The legs disagree about *which* twelve chunks to spend 3000 tokens on, and agree
on *whether the answer's window survived*. 3000 tokens is ~0.7% of 2932 chunks,
so the decision that matters is the selection policy — greedy fill, the 40-line
grid, the diversity discount — not the ordering the fill starts from.

The cross-border class makes it concrete. 7 of the 12 cases require a span that
straddles a 40-line boundary, and **no arm covered any of them**. For
`xborder-retrieve-ranking` (required span `rag.rs:181-215`, boundary 200/201):

- cosine selects `rag.rs:161-200` — the first half.
- hybrid selects `rag.rs:161-200`, `121-160`, `1-40`, `601-640` — still not
  `201-240`.
- The span needs both halves; neither arm takes the second one.

A manual probe before the run had suggested the hybrid fixed exactly this
(`choose_worker`: cosine took one half, hybrid took both). It does not
generalize — that was one case, and this set is the control that says so.

The pack arms also cost ~102–105 s per case against BM25's 2.1 s, and the mode
does not change that.

## 5. Set design, recorded as a lesson

Set #2 is **over-weighted to cross-border** (7/12) and is therefore a weak
discriminator between ranking legs: on 10 of 12 cases both arms score zero, so
the set can only detect a change that rescues a boundary-straddling span. That
was deliberate — it was aimed at the observed Day 3 failure mode — but it means
the tie is evidence about *this* set, not a general statement that fusion never
helps. A set with more symbol/dup-header/Italian cases would give the legs more
chances to differ.

`file_heads` scoring 1/12 is not a regression: it is the floor, and it happened
to catch `par-name-collision` because that file sorts early.

## 6. What ships

| change | file | why |
|---|---|---|
| fork deleted | `src/rag.rs` (411 → ~45 lines) | one retriever in the workspace; the packer can see the fusion the rest of the code uses |
| `corpus_from_texts` | `src/rag.rs` | the two `VectorEmbed` traits differ; product callers cannot forget the lexical leg |
| `retrieve_with_ranking` | `physis-core/src/rag.rs` | reusable budget fill for any ranking leg; `retrieve` delegates; 1 new test |
| `PackMode` + `--mode` | `src/pack.rs`, `src/main.rs` | hybrid, lexical and auto reachable and measurable; default unchanged |
| `--generate N` | `benchmarks/retrieval/equal_budget.py`, `gen_cases.py` | cases sampled from the frozen copy, so difficulty is a controlled variable |
| required-span check | `benchmarks/retrieval/equal_budget.py` | a stale generated set fails loudly instead of scoring as honest misses |
| 5 `RagCorpus` literals | `cli.rs`, `pack.rs`, `web/search.rs`, `web/agent.rs`, `mcp.rs` | carry the BM25 leg |
| `PackMode::default()` at call sites | `mcp.rs`, `estimate.rs`, `main.rs` | the default lives in one place |

Nothing was deleted as a feature. `HybridVerdict`/`measure_hybrid_vs_cosine`
remain the promotion instrument.

## 7. Evidence

| check | command | result |
|---|---|---|
| Core retriever tests | `PHYSIS_EMBEDDER=random-projection cargo test -p physis-core --lib rag::` | 10 passed, 0 failed |
| pack tests (incl. expansion) | `PHYSIS_EMBEDDER=random-projection cargo test --lib pack::tests` | 16 passed, 0 failed |
| tokenizer mirror | `python3 benchmarks/retrieval/equal_budget.py --check-tokenizer` | 26/26 chunks agree |
| set #3, expansion | `python3 benchmarks/retrieval/equal_budget.py --cases benchmarks/retrieval/cases_day3c.json --candidate expand:cosine` | EXIT 0; KEEP_BASELINE; 0 over-budget; 0 unresolved citations |
| generated holdout | `python3 benchmarks/retrieval/equal_budget.py --generate 30 --seed 1 --candidates mode:lexical,mode:auto` | 60 cases; lexical 32/60, cosine 14/60, auto 21/60; 0 over-budget; 0 unresolved citations |
| Pro MCP protocol | `PHYSIS_EMBEDDER=random-projection cargo test --test pro_mcp` | 1 passed (30 tools, write round-trip) |
| hybrid reachable | `physis-pro pack src choose_worker --budget 3000 --mode hybrid` | selects both halves of the straddling window |
| equal-budget run | `python3 benchmarks/retrieval/equal_budget.py --cases benchmarks/retrieval/cases_day3b.json --candidate mode:hybrid --json benchmarks/retrieval/day3b-results.json` | KEEP_BASELINE; 0 over-budget; 0 unresolved |
| Pro lint | `cargo clippy --all-targets -- -D warnings` | exit 0 |
| Pro suite | `PHYSIS_EMBEDDER=random-projection cargo test` | **1195 passed, 0 failed, 5 ignored** |
| MCP surface | `PHYSIS_EMBEDDER=random-projection cargo test --features mcp --test system_mcp` | 2 passed, 0 failed |
| Day 2 gate | see §0 | all green |

`cargo test --features full` cannot run on this box: `full` includes `ocr`, which
pulls `tesseract` → `leptonica-sys`, and the system `lept` library is not
installed (`pkg-config` exits 1). That is an environment limit, not a regression
from this change; the default-feature suite plus the `mcp` suite cover every file
this change touches.

`src/rag.rs` and `src/pack.rs` are rustfmt-clean. The repository as a whole is
not (pre-existing diffs in `examples/adaptive_doc_demo.rs`, `src/web/coherence.rs`,
`physis-core/src/rag.rs`), so `cargo fmt` was not run over the tree — only the
two files this change rewrote.

## 8. Second candidate — adjacency expansion (also not promoted)

The finding in §4 named the selection policy as the target, so the next
candidate was adjacency expansion: when a selected window's neighbour in the
same file is worth taking, take it. Scored on a third frozen holdout,
`benchmarks/retrieval/cases_day3c.json` (12 cases, 3 boundary-straddling and 9
across the other classes — set #2 was 7/12 cross-border and could not tell two
ranking legs apart, so this set is deliberately balanced).

**First implementation was a no-op, and verifiably so.** "Take the neighbour if
it fits the remaining budget" added **zero** chunks: the greedy fill saturates
the budget by construction (2999/3000, 3000/3000 on every case), so no neighbour
ever fits. Proof independent of any score: the candidate's token totals were
identical to the baseline's on all 12 cases (35956 == 35956), and a manual
`pack` with and without `--expand` produced a byte-identical chunk list. That is
a specification failure, not a measurement, so the corrected implementation was
scored on the same set — this was its first real test, not a second look.

**Corrected policy:** enlarge the pool with each selected window's neighbours
(at the parent's score) and re-run the *same* budget fill over it. A neighbour
now has to win its place on the parent's score rather than find slack that does
not exist.

Frozen-corpus run (`--frozen`, see §10), budget 3000, `--top-k 50`:

| arm | case hits | span recall | line recall | tokens | wall |
|---|---|---|---|---|---|
| `file_heads` | 0/12 | 0.000 | 0.000 | 35916 | 0.0 s |
| `bm25` | **7/12** | **0.583** | **0.653** | 34957 | 1.0 s |
| `current_pack` (cosine) | 4/12 | 0.333 | 0.399 | 35956 | 75.6 s |
| `candidate_pack` (expand) | 4/12 | 0.333 | **0.413** | 35439 | 64.1 s |

**Verdict: `KEEP_BASELINE`.** Expansion is a **trade**, not a gain:

- cross-border: **1/3 vs 0/3** — it does fix the failure mode it was built for
  (`xborder-typed-edge`, `relation.rs:102-125`, now covered).
- symbol: `sym-mechanism` **lost** (`address.rs:52-80` displaced by a
  neighbour).
- paraphrase: `par-belief-at-time` **lost**.
- Net case hits unchanged; line recall +0.014.

At 3000 tokens over ~2900 chunks the budget holds about twelve windows, so every
window taken for a neighbour displaces one that some other query needed. This is
not a bug to fix with another heuristic; it is what a 0.4%-of-corpus budget
does. Expansion stays available behind `--expand` and off by default.

## 9. The lexical hypothesis — proposed on three holdouts, falsified on the fourth

After sets #1–#3 a plain BM25 baseline over the same 40-line grid, at the same
token budget, was ahead of the shipped cosine pack on every set, at 1–2 s
against 65–105 s. That looked like a dominated shipped default, so the third
candidate was **`PackMode::Lexical`** — rank by the `Bm25Index` that already sits
in `RagCorpus`, falling back to cosine when the query shares no term with the
corpus (BM25 scores everything zero then, and `rank` returns insertion order,
i.e. an arbitrary corpus prefix).

Set #4 (`cases_day3d.json`, 12 cases, 4 boundary-straddling, every file one no
earlier set touched) reverses it:

| holdout | `bm25` | `current_pack` (cosine) | best candidate |
|---|---|---|---|
| set #1 (9 cases) | **5/9** | 3/9 | hybrid 3/9 |
| set #2 (12 cases) | **4/12** | 3/12 | hybrid 3/12 |
| set #3 (12 cases) | **7/12** | 4/12 | expand 4/12 |
| set #4 (12 cases) | 3/12 | **5/12** | lexical 3/12 |

**Verdict: `KEEP_BASELINE`.** The lexical candidate ties its own baseline (3/12)
and *loses* to the shipped cosine arm (5/12). No promotion, default unchanged.

**The honest reading is not "BM25 was wrong".** It is that neither leg dominates
and the winner depends on what the query looks like:

- Set #4 is dominated by definition-seeking queries in large files
  (`CellClassifier`, `WorldState`, `CoherenceNode`, `compare_structures`). BM25
  ranks by term frequency, so it prefers windows that *mention* the identifier
  often — usage sites — over the window that *defines* it. Cosine prefers the
  definitional window. BM25 lost `sym-cell-classifier` and `sym-coherence-node`
  exactly this way.
- Sets #1–#3 had more "explain how this works" queries over smaller files, where
  a lexical anchor lands on the right window.

So the earlier claim in this note — "a plain BM25 baseline beats the shipped
cosine pack on every frozen holdout" — was true of three sets and false in
general. Four holdouts, 3–1 to BM25, with a mechanism for the disagreement, is
the actual result. It is also the reason the plan demanded a fresh holdout per
candidate: three sets agreeing is not four sets agreeing.

The packer's value is still not the ranking — it is the token guarantee and the
`path:start-end` provenance, and every leg here shares the same chunk grid and
budget.

`PackedChunk::score` now documents what it holds: cosine under `Cosine`, an RRF
score under `Hybrid`, an Okapi BM25 score under `Lexical` — comparable within a
pack, not across modes.

## 10. The corpus was not frozen — fixed

The first set #3 run tripped the citation gate: `current_pack` cited
`bin/doc_tui.rs:1881-1881`, a window past the end of the file. The packer was
not wrong. A **second session was editing this clone during the run** (the file
was untracked, and its mtime moved at 11:48 and again at 11:49 while the run was
scoring). The pack had read a longer revision than the one the check measured.

The check was right and the measurement was still void. The harness now copies
each corpus root into a temp dir before measuring (`freeze()`), hashes the copy,
and packs from the copy — so a concurrent writer cannot move the ground under a
run. The re-run reproduced every score and cleared the citation gate
(`unresolved citations 0` on all four arms), which also confirms the earlier
scores were unaffected by the concurrent edit.

## 10b. Fifth instrument: a generated holdout (and `Auto`, falsified)

Sets #1–#4 were hand-authored, and sets #2 and #4 returned opposite verdicts on
the same comparison. The reason was authoring, not retrieval. CausalARC's design
factor 1 (`research/RELATED_WORK_2026-09-16.md` §3) is the fix, so the fifth
instrument is generated, not written:

- `benchmarks/retrieval/gen_cases.py` samples cases from a corpus root. A case is
  a **definition site found by pattern** (`pub fn|struct|enum|trait`) and its
  query is built from that site — ground truth by construction, nothing asserted
  by judgement.
- Three query templates: `id` (the symbol alone), `sig` (the literal signature
  line), `body` (common words drawn from the span, deliberately the words a
  lexical leg cannot localise with).
- The difficulty axis is recorded per case: corpus-wide document frequency of the
  query's tokens (`df-low|mid|high`), span length, definitions per file.
- `equal_budget.py --generate N` samples from the **frozen copy** rather than a
  file, so the ground truth and the axis describe exactly what is scored. A
  required-span check runs before any arm and fails loudly on a stale set — it
  caught two spans one line past the end of their files on the first attempt.

**Result, 60 cases, seed 1, budget 3000, one frozen pass:**

| arm | case hits | span recall | line recall |
|---|---|---|---|
| `file_heads` | 0/60 | 0.000 | 0.000 |
| `bm25` (Python baseline) | **33/60** | **0.550** | **0.752** |
| `current_pack` (cosine) | 14/60 | 0.233 | 0.290 |
| `mode@lexical` (Rust BM25 + cosine fallback) | 32/60 | 0.533 | 0.746 |
| `mode@auto` (identifier-aware routing) | 21/60 | 0.350 | 0.457 |

Three findings:

1. **The hand-authored result reproduces at scale.** Lexical 32/60 against cosine
   14/60, on cases nobody chose. This is the strongest evidence the track has
   that the shipped cosine pack is the weaker leg on definition-finding queries.
2. **`PackMode::Auto` is falsified as a routing rule.** It scores 21/60 — worse
   than *either* leg it chooses between. The identifier gate sends `body` queries
   (no underscore, no CAPS) to cosine, where they score 2/20 while lexical scores
   14/20 on the same cases. "No identifier ⇒ semantics" is the wrong inference;
   common words are exactly what a lexical leg localises well.
3. **The `df` axis does not predict which leg wins.** Lexical beats cosine in
   *every* class on this set, including `df-high` (18/31 vs 4/31), where the
   hypothesis predicted the opposite. The axis that separates the legs on this
   instrument is the **template**, not the token frequency.

`PackMode::Auto` stays available and is **not promoted**; the default is
unchanged. The generated instrument is now the one to extend: it disagrees with
set #4 (cosine 5/12 vs bm25 3/12 there), and resolving that needs generated
cases in the set-#4 shape — large files with many mention sites — rather than
another hand-authored set.

## 11. Next candidates

1. **Resolve the instrument disagreement.** The generated set says lexical by
   2.3×; set #4 says cosine. Extend the generator with a set-#4-shaped
   population — large files, many mention sites per symbol, definition-seeking
   queries — and see whether the split is the case population or the corpus
   position. This is the one question standing between the evidence and a
   default change.
2. **Minimum chunk size** — the packer still spends budget on 1–7 token
   fragments (`web/shifts.rs:201-201`, `capability.rs:121-125`); a floor removes
   the cheapest way for a fragment to displace evidence.
3. **Per-file cap** — the pack takes 4+ windows from one file; a cap spreads the
   budget over more files, which is what a multi-file evidence span needs.

Both remaining candidates are testable on the **generated** instrument
(`--generate N`), so no hand-authoring is needed. Sets #1–#4 are spent — and the
reason four were spent is the fourth finding of the day: hand-authoring holdouts
makes difficulty an accident. Sets #2 and #4 gave **opposite verdicts on the same
comparison** (BM25 4/12 vs cosine 3/12, then BM25 3/12 vs cosine 5/12) because one
was 7/12 boundary-straddling and the other was definition-seeking. Set #5 should
be **sampled from a parameterized generator** — target file, span, query
template, lexical-overlap knob, span length — with performance reported over the
distribution, which is CausalARC's design factor 1
(`research/RELATED_WORK_2026-09-16.md` §3). That is a change to
`benchmarks/retrieval/equal_budget.py`, not to the packer.

Day 3's gate is met as written — all citations resolve, budgets hold, no arm
beats the strongest simple baseline on the sets where it was measured, and the
misses are recorded. Three candidates were implemented (`Hybrid`, `Lexical`,
`--expand`); all three are documented negatives and none is promoted, and the
default is unchanged.

What the four holdouts *did* establish, and what they did not:

- established: the chunk grid, the budget and the provenance are sound (0
  over-budget, 0 unresolved citations on every arm of every set); the ranking leg
  is not the whole story; 3000 tokens over ~2900 chunks is ~0.4% of the corpus
  and that is what limits evidence coverage.
- not established: that any single leg is better. The 3–1 split is
  query-dependent and the mechanism is understood but not yet exploited.

**Actor**: continuation session (OpenCode)
**Date**: 2026-09-16
**Revisions**: Pro `96a98c4` + working tree, Core `d65858b` + working tree
