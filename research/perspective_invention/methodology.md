# Methodology — perspective invention experiment

Everything here was frozen before the run (see `hypothesis.md`). Code:
`physis-core/examples/perspective_invention_experiment.rs` on branch
`research/perspective-invention` (physis-core + physis-pro).

## Reproduce

```bash
cd /home/gio/dev/physis-pro
 cargo run -p physis-core --features embed-onnx --release --example perspective_invention_experiment -- --seed 20260911
# optional: --skip-bge (MiniLM route only), --out <dir>, --probe (calibration only)
```

Deterministic: fixed seed for dataset generation, k-means++, bootstrap,
random projections. Embeddings are ONNX MiniLM (384-d, mean pooling) and
BGE-base (768-d, mean pooling) — the same pinned models prior work used
(embedder is part of the artifact's identity; nothing transfers across a
model swap, per FINAL_REPORT constraint 1). Embeddings cached to
`datasets/embeddings_cache.json` so ablation re-runs are byte-identical.

## Pipeline (one stage per mission section)

### S0 — dataset generation (seeded, in-code)

- Surface: 4 domains × (8 actors, 12 objects, 6 settings) of real English
  nouns — the dominant token mass of every sentence.
- Latent: 3 flows. Flow cue = one pseudo-word per flow
  (`keltrin` / `vozane` / `halmex`) inside a short suffix clause, included
  with probability `p_cue`; plus one shared phase-verb template per flow
  included with probability `q_cue`. Pseudo-words have no pretrained
  semantics — the only signal they carry is sub-lexical overlap, which is
  exactly the "difficult but not impossible" register the mission requires.
- Bridges: per domain, 3 of 60 train items (5%) carry TWO flow clauses
  (multi-membership ground truth). Test: 3 of 30 per domain.
- Split: train 240, test 120 (same grammar, fresh draws), transfer 120
  (domains: clinic, studio, barnyard, radio-room — same flows, new nouns).
- **Difficulty calibration** (train-labels only, declared): grid over
  `p_cue ∈ {0.15, 0.3, 0.5, 0.7, 0.9}` × `q_cue ∈ {0.0, 0.3, 0.6}` on a 60-item
  probe; select the point minimizing |latent-ARI − 0.2| subject to surface
  purity ≥ 0.7 and latent ARI ≥ 0.05 (recoverable). The chosen point and the
  full grid are recorded in `datasets/calibration.json`.

### S1 — baseline (the strongest available organization)

k-means on raw embeddings, k swept 2..=6, k selected by mean silhouette
(cosine distance) — internal criterion, no labels. Report surface purity and
latent ARI (evaluation-only numbers, printed but never used upstream).
This is "ordinary classification" in the mission's diagram.

### S2 — residual extraction (what the first classification discarded)

For each cross-baseline-cluster pair (i, j):

```text
w(i,j) = cos(x_i, x_j) − mean-cos(cluster(i), cluster(j))
```

(the amount by which the pair is *more related than its two clusters
normally are*). Threshold: keep w > 0 AND above the 95th percentile of a
degree/size-matched null (baseline labels shuffled 20×, seeded). The result
is the residual graph: cross-links a hard partition treats as noise. Items
are also scored by residual degree; multi-modal soft memberships
(softmax over cos-to-centroids, τ = 0.1) are kept for `P_soft`.

### S3 — candidate generation (mission Section 5; no labels anywhere)

- `P_edge`: greedy agglomeration over the residual graph (average residual
  weight, ties by index) down to the silhouette-chosen k.
- `P_soft`: k-means over n × k soft-membership profiles.
- `P_ref`: references = top-12 items by residual degree; x ↦
  [cos(x, r)] r∈R; k-means in that space.
- Controls: `P_rand` (Gaussian projection to 64-d, k-means), `P_perm`
  (P_edge labels shuffled), ablations `P_edge-nores` (same agglomeration on
  the FULL similarity graph — residual removed, everything else identical)
  and `P_ref-randref` (12 random items as references), plus evaluation-only
  `P_oracle` (latent labels).

### S4 — re-representation + re-discovery

For each candidate P: re-represent, re-run the SAME discovery machinery
(k-means, k swept 2..=6 by silhouette in P-space), then score the discovered
partition's **silhouette in RAW embedding space** — primary internal score,
chosen so no perspective can win by grading its own space. Internal gain =
raw-silhouette(discovered partition) − raw-silhouette(baseline).

### S5 — internal selection, recursion, stopping (mission Section 7)

Winner = max raw-space silhouette over candidates (never labels). Recursion:
re-extract residual under the winner's partition, regenerate `P_edge` and
`P_ref` children, accept a child only if raw-silhouette improves; stop at the
first non-improvement or depth 3. Every node records the Section-7 fields:
`perspective_id, parent, generator/transformation, evidence (residual edge
count, degree top-k), coherence/silhouette before→after, internal_gain`.

### S6 — cross-perspective convergence (mission Section 8)

- Partition agreement: ARI + NMI between D(P_edge), D(P_soft), D(P_ref)
  (same embedder), and between the SAME generator across MiniLM/BGE.
  Nulls: (a) permuted-partition ARI (20 size-matched shuffles), (b) the
  published cross-embedder partition null ≈ 0.10 (FINAL_REPORT).
- Relational agreement: same-partition indicator matrices correlated
  (Pearson) across routes; consensus pairs (co-grouped by all three)
  evaluated against latent co-membership — precision/recall reported.

### S7 — held-out prediction (mission Section 10)

Split fixed at generation; perspectives discovered on TRAIN ONLY, frozen,
then applied to TEST embeddings:

- latent-role 1-NN accuracy in P-space vs raw cosine 1-NN,
- same-flow link AUC over cross-domain test pairs, P-space vs raw,
- `P_rand` / `P_perm` / ablation variants through the identical protocol,
- compression: 1-NN accuracy using only the top-k references
  (k ∈ 3, 6, 12, 24) — winner vs raw-centroid curve,
- stability: 5×80% bootstrap re-discovery, mean ARI to full-run partition.

### S8 — transfer (mission Tier 4)

Dataset 2 (new surface domains, same latent grammar) re-represented by the
frozen winner (reference texts / centroids embedded under the same model);
latent 1-NN vs raw-cosine 1-NN on identical items.

### S9 — verdicts

Tiers resolved exactly as pre-registered in `hypothesis.md`; every claim
labeled OBSERVED / SUPPORTED / PARTIALLY SUPPORTED / FALSIFIED / UNRESOLVED;
all failures preserved in `failures.md`.

## Ablations (mission Section 12 → which component earns the improvement)

| ablation | removes | tests |
|---|---|---|
| `P_edge-nores` | residual weighting | is residual extraction necessary, or does any agglomeration do? |
| `P_ref-randref` | residual-chosen references | is reference choice by residual load-bearing (vs prior track's arbitrary references)? |
| selection = max P-space coherence | raw-space silhouette | does the internal selection signal matter? |
| selection = random | any signal | lower control |
| no-recursion | level ≥ 2 | does recursion add or destroy? |

## Anti-leakage contract (mission Section 21)

- Latent labels live ONLY in the dataset sidecar and in evaluation structs.
- Selection, recursion, stopping, k-choice: internal measures only.
- Names of discovered flows are generated post hoc as `flow-<argmax-token>`
  style opaque ids; never human semantics.
- Baselines, controls, and nulls are construction-matched (the standing rule
  this track earned seven times).
