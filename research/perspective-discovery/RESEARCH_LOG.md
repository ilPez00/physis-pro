# Research log — perspective-conditioned ontology discovery + JEPA

Mission: investigate whether recursively scanning a shared semantic space,
conditioned on a reference node, discovers richer perspectival ontologies
than ordinary nearest-neighbor discovery, and whether a JEPA-style predictive
latent substrate helps. Full mission brief preserved in this session's
transcript; not reproduced here.

**Status after Iteration 1: hypothesis not yet supported — real signal found,
but weaker and messier than the mission's illustrative example suggests, and
one candidate mechanism (Δ-magnitude) is already falsified.**

## Reconnaissance (before any code)

Read `physis-core/src/{embed,embed_onnx,discovery,classify,core,ontology,
quality}.rs`. Facts that shape every later decision:

1. **The dependency-light default embedder (`RandomProjectionEmbedder`) is
   character-n-gram hashing, not semantics.** "engine" and "car" share almost
   no character n-grams. Any experiment run under this embedder measures
   lexical-overlap artifacts, not perspectival semantic structure — it cannot
   be used as evidence for or against this hypothesis. The codebase's own
   `semantic_self_test` (embed.rs:123) is itself confounded by shared-prefix
   lexical overlap in its two example sentences, worth noting for whoever
   relies on it as a semantic quality gate.
2. **A real semantic embedder is available on this machine right now**:
   `models/model.onnx` (all-MiniLM-L6-v2, 90MB, 384-d, mean-pooled) plus
   `models/tokenizer.json`, loadable via `physis_core::embed_onnx::OnnxEmbedder`
   with zero additional downloads. This is the CPU-small model the mission
   asks for (Section 15) — used for every experiment below.
3. **Physis-core's existing "ontology discovery" (`discovery.rs`) is not what
   the mission calls "ordinary discovery."** It does coverage-gap detection
   against a *fixed* 70-cell grid (5 domains × 14 modes): score each corpus
   text against the nearest grid cell, cluster what falls below threshold,
   propose new grid entries from the clusters. There is no free-form
   nearest-neighbor ontology tree anywhere in the codebase, no graph/DAG
   construction from raw corpus similarity, and no reference-conditioning of
   any kind. The mission's baseline diagram (`corpus → embedding → nearest
   neighbors → ontology`) does not describe any existing Physis mechanism —
   it would have to be built from scratch as the "ordinary discovery"
   control, using `discovery.rs`'s own single-link greedy clustering shape
   (threshold cosine, transitively merge) as the closest existing precedent,
   applied directly to raw embeddings with no ontology grid involved.
4. **`physis_core::models::cosine_sim`** is the existing normalized-vector
   cosine function; reused directly rather than reimplemented.
5. No existing graph/relation/provenance substrate is wired to embeddings in
   a way this experiment could reuse yet (`relation.rs`'s `TypedEdge` exists
   but is untouched here — Iteration 1 works entirely outside the graph,
   on raw vectors, by design: test the *representation*, not the storage,
   first).

## Experiment 1 — reference-conditioned discovery vs. naive baselines

File: `physis-core/examples/perspective_discovery_experiment.rs`.
Run: `cargo run -p physis-core --features embed-onnx --example perspective_discovery_experiment`
(from workspace root; picks up `models/model.onnx` automatically, falls back
to `RandomProjectionEmbedder` with a loud warning — and a note that the
fallback result isn't evidence — if the model isn't found).

**Dataset B** (multi-perspective CAR corpus, mission's own example): 21 terms,
7 each labeled mechanical / driver / economic ground truth.

**Methods compared, per reference r ∈ {engine, steering wheel, purchase price}:**

- **Naive NN**: rank all other 20 items by `cos(x, r)`, take top-6 (= field
  size), hit-rate = precision = recall by construction (fixed-size retrieval).
  This is exactly what mission Section 7 says does *not* count as reference
  conditioning — included as the control that must be beaten.
- **Δ-magnitude ranking** (`Δ(x,r) = embed(x) - embed(r)`, rank by `|Δ|`):
  the mission's own suggested starting formula (Section 7). Tested for
  whether it differs from naive NN at all.
- **Δ-direction clustering**: compute `Δ(x,r)` for all x, cluster those
  *delta vectors* by `cos(Δ(x,r), Δ(y,r))` (not by `cos(x,y)` and not by
  `cos(x,r)`), find the cluster containing r's own nearest raw neighbor,
  report precision/recall/F1 of that cluster against the true field.

### Result 1 — Δ-magnitude ranking is mathematically identical to naive cosine ranking

`delta_magnitude_vs_cosine_rank_agreement = 1.000` for every reference
tested. Confirmed algebraically before running: for L2-normalized vectors,
`|Δ(x,r)|² = 2 - 2·cos(x,r)`, so ranking by ascending `|Δ|` is a monotonic
transform of ranking by descending `cos(x,r)` — provably the same ordering,
not just empirically similar. **The literal formula the mission proposes in
Section 7 as a starting point contributes zero information beyond plain
cosine similarity.** This was the mission's own suspicion ("start with
Δ(x,r) = embed(x) - embed(r)... but test alternatives") and it is now
falsified as a distinct mechanism, cleanly, with a proof rather than a
sample-size caveat.

### Result 2 — a real bug in my own first metric produced a false positive

The first version of Δ-direction clustering picked "the largest cluster" as
r's field and scored precision-free recall against a fixed denominator.
Result: for `steering wheel`, F1-equivalent score = **1.000** — looked like
a clean win. Inspection of the actual retrieved set showed why: at
threshold=0.3, the "largest cluster" contained **20 of 20** other items —
i.e. everything. The true 6-member field was trivially a subset of "the
entire corpus," so any metric without a precision term scores this as
perfect regardless of whether the method discriminated anything. Same
degenerate collapse hit `engine`'s cluster (18/20 members). This is exactly
the "generic-node collapse" / "duplicate explosion" failure mode the mission
pre-warns about in Section 20, caught by inspecting retrieved content rather
than trusting the number (Section 26's iteration loop, working as intended).

**Fix**: select "r's cluster" as the one containing r's actual nearest
neighbor (not the largest one), and report precision separately from recall
instead of a single hit-rate. Re-running with the fix:

| reference       | true field  | naive NN | Δ-direction P | Δ-direction R | Δ-direction F1 |
|-----------------|-------------|---------:|---------------:|---------------:|----------------:|
| engine          | mechanical  |    0.833 |           0.278 |           0.833 |            0.417 |
| steering wheel  | driver      |    0.333 |           0.300 |           1.000 |            0.462 |
| purchase price  | economic    |    0.833 |           1.000 |           0.500 |            0.667 |

Naive NN beats Δ-direction on F1 in 2 of 3 references; only ties/edges ahead
on the weakest naive case (steering wheel, where naive NN itself is barely
above chance at 0.333). **This is not the clean, one-sided win a
demonstration would want to show, and I am reporting it as such rather than
re-running until a friendlier dataset appears.**

### Result 3 — the one genuinely interesting finding, and why the current ground truth can't credit it properly

For `purchase price`, Δ-direction's chosen cluster is exactly `["resale
value", "depreciation rate", "trade-in value"]` — 3 items, 100% precision,
0% contamination from mechanical or driver terms. But it *misses*
"insurance premium," "fuel cost," and "loan interest," which are also
labeled "economic" in my ground truth but apparently relate to "purchase
price" through a different axis than "value that changes over time." Scored
against my coarse 3-label ground truth this reads as merely "50% recall" —
a mediocre number. Read as "did the method find a coherent, non-contaminated
sub-structure," it may be the most on-hypothesis result in this run: a
reference-conditioned scan finding a *narrower, more specific* relational
field than the flat category label predicts, i.e. exactly the kind of
finer-than-nearest-neighbor structure the mission hypothesizes — but my
3-label ground truth is the wrong instrument to confirm or deny that, and I
am not going to claim it as support without a ground truth built to detect
it. **Flagged as the top candidate for Iteration 2, not scored as a win.**

### Result 4 — the "ordinary discovery" baseline is currently unmeasurable, not merely losing

`flat_cluster` (single-link greedy, fixed cosine threshold, direct port of
`discovery.rs`'s clustering shape applied to raw embeddings with no
ontology grid) produces **exactly 1 cluster containing all 21 items, at
every threshold from 0.30 to 0.80.** MiniLM's raw embedding space does not
separate this corpus into anything at any single global threshold — the
whole "CAR" domain is too internally homogeneous relative to its
inter-item variation for greedy single-link clustering to find a cut
point. This means **the comparison against "ordinary discovery" is not yet
valid** — I don't have a working baseline to lose or beat. A fixed-k
clustering method (k-means or agglomerative cut at k=3) is needed before
any baseline comparison means anything; single-link-at-a-global-threshold
is the wrong algorithm for this embedding geometry, independent of the
reference-conditioning question entirely.

## Interim classification (Iteration 1 only — NOT the final verdict)

**INCONCLUSIVE, leaning toward "the simple version is not obviously true."**
Δ-magnitude is falsified outright (proven identical to cosine, not just
weak). Δ-direction clustering shows a real but inconsistent effect — wins
narrowly once, loses meaningfully twice, and its most interesting behavior
(the purchase-price sub-cluster) can't be scored properly with the current
evaluation design. The "ordinary discovery" comparison point is currently
broken, not merely beaten, so no A-vs-B claim is safe yet either way.

## Next actions (Iteration 2 candidates, not yet done)

1. Fix the baseline: k-means or agglomerative-cut-at-k=3 instead of a global
   threshold, so "ordinary discovery" is an actual working comparison point.
2. Build a ground truth with sub-perspective labels (e.g. split "economic"
   into "value-over-time" vs "recurring-cost" vs "acquisition-cost") to
   properly score whether Δ-direction's narrower clusters are a feature
   (finding real structure nearest-neighbor blends away) or noise.
3. Repeat across more references per perspective (currently n=1 reference
   per perspective — far too small to trust the per-reference numbers
   individually; Section 21's reference-root experiment wants this anyway).
4. Only after 1-3: recursion (mission Section 9), then the JEPA ablation
   (Section 14) — not attempted yet, and not worth attempting while the
   non-recursive, non-predictive base case is still this unsettled.

## Files

- `physis-core/examples/perspective_discovery_experiment.rs` — Experiment 1,
  runnable, self-contained, writes `experiment1_results.json`.
- This log.

## Commit discipline note

Working on branch `research/perspective-discovery`, not `main` — nothing
here is production code, no existing behavior touched, purely additive
(`physis-core/examples/` + this log). Per PROTOCOL: negative results
preserved, not discarded; the buggy first metric's exact failure is
documented above rather than silently replaced.
