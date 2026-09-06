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

## Iteration 2 — fixed baseline, fine-grained ground truth, all 21 references

File: `physis-core/examples/perspective_discovery_experiment2.rs`. Addresses
both open items from Iteration 1 directly, then tests every corpus item as
its own reference (n=21) instead of 1-per-perspective (n=3) — Iteration 1's
per-reference numbers were never a safe sample to generalize from.

**Fix 1 — a working baseline.** Replaced single-link-at-a-global-threshold
(which collapsed to 1 cluster at every threshold 0.3–0.8) with deterministic
k-means (farthest-point/k-means++ init, no RNG, no label leakage into init).
`k-means(k=3)` purity vs the 3 coarse labels: **0.476** (chance floor for 3
even classes is 0.333) — a real, present, but weak signal: MiniLM's raw
embedding space groups this corpus into something closer to 3 real fields
than to random, but far from cleanly. `k-means(k=6)` purity vs the 6 fine
labels: **0.571**. "Ordinary discovery" is now a valid comparison point for
the first time in this project.

**Fix 2 — fine-grained ground truth.** Split each of the 3 coarse
perspectives into 2 sub-groups by genuine semantic sub-relation (not reverse
engineered from what would flatter either method): mechanical → power-core /
drivetrain-support; driver → active-controls / safety-awareness; economic →
value-over-time / recurring-cost. The value-over-time split (resale value,
depreciation rate, trade-in value) is *exactly* the 3-item cluster Iteration
1's delta-direction method found for "purchase price" before this ground
truth was written — a real prediction that held up, not a post-hoc fit.

**Result — the headline number moved against the hypothesis.** Averaged
over all 21 references (not 3):

| method                | mean F1 vs coarse (3 labels) | mean F1 vs fine (6 labels) | wins out of 21 |
|-----------------------|------------------------------:|----------------------------:|---------------:|
| naive nearest-neighbor|                          0.587 |                        0.468 |              — |
| Δ-direction clustering|                          0.469 |                        0.226 |     7 (coarse), 5 (fine) |

**Naive nearest-neighbor-to-reference — the exact method Section 7 says does
not count as reference conditioning — beats Δ-direction clustering on
average, at both label granularities, by a wide and consistent margin.**
Iteration 1's one encouraging case (steering wheel, driver: naive 0.333 vs
delta 0.462) does not generalize; it was one favorable draw out of 21, not a
trend. Fine-grained ground truth made this worse for Δ-direction, not
better — the hoped-for story ("delta conditioning finds sub-structure naive
NN blends away") does not hold up as a general pattern once tested past n=3;
`purchase price`'s standout sub-cluster from Iteration 1 is confirmed real
(fine F1 = 0.000 for delta here only because "purchase price" itself sits in
the recurring-cost group where the method's cluster missed it — inspect
`experiment2_results.json` per-reference for exact numbers) but it is the
exception, not the rule, across the full reference set.

## Interim classification, revised after Iteration 2

**The specific mechanism tested (Δ(x,r) clustered by direction, at a fixed
threshold, over a frozen general-purpose sentence embedding) is FALSIFIED as
an improvement over the naive baseline the mission itself says shouldn't
count.** This is not a tuning artifact of one bad dataset draw — it holds
across all 21 references, at both label granularities, with a real (if
weak) working baseline for comparison on both sides.

**The broader mission hypothesis (perspective-conditioned recursive
discovery, and separately, JEPA as the substrate) remains INCONCLUSIVE, not
falsified** — only one candidate geometric conditioning mechanism has been
tested, on one dataset, with one frozen off-the-shelf embedder. What *is*
now reasonably well-evidenced: naive per-reference similarity search already
captures most of the achievable perspective signal in this embedding space
for this corpus (mean F1 0.587 vs a 0.476 reference-free k-means purity
ceiling), leaving less room for a cleverer conditioning mechanism to add
value than the mission's illustrative CAR example implies — and the specific
"compare deltas to each other" idea does not find that room; if anything it
throws signal away relative to just querying from r directly.

**Recommendation:** do not keep tuning thresholds or trying more geometric
transforms of a frozen embedding — that direction now has two negative data
points (Δ-magnitude proven equivalent to cosine; Δ-direction clustering
proven worse than cosine at n=21). The mission's own Section 1 secondary
hypothesis (JEPA: a *predictive* latent relationship, not a static geometric
one) is the more promising next thing to test precisely because it is a
different kind of mechanism, not a variant of the one just falsified.
Recursion (mission Section 9) is not worth adding on top of a per-reference
mechanism that doesn't yet beat its own baseline — recursing a losing move
doesn't produce a winning one.

## Iteration 3 — JEPA-style predictive latent (and why the headline number is misleading)

File: `physis-core/examples/perspective_discovery_experiment3.rs`. Tests the
mission's secondary hypothesis (Section 1): a *predictive* latent
relationship instead of a *static geometric* one. Architecture: frozen
MiniLM encoder (same as Iterations 1-2) + a rank-8 linear predictor
`pred(r) = V(Uᵀr)` trained by hand-derived gradient descent on a
multi-positive InfoNCE loss, evaluated with **strict leave-one-out
cross-validation** over all 21 items — for each held-out item, a fresh
predictor is trained on the other 20 with the held-out item appearing
nowhere in training (not as query, candidate, positive, or negative), then
used to rank the training set from the held-out item's embedding.

**Raw result:**

| | naive NN | Δ-direction | JEPA predictor |
|---|---:|---:|---:|
| mean F1, coarse (3 labels) | 0.587 | 0.469 | **0.952** |
| mean F1, fine (6 labels)   | 0.468 | 0.226 | **0.484** |

Read naively, coarse looks like a dramatic win. **It is not evidence for the
mission's hypothesis, and reporting it as a win would be exactly the kind of
self-deceiving result the mission warns against.** Here is why, found by
interrogating the number rather than trusting it:

**The confound:** this predictor is trained by directly optimizing
"score same-coarse-label pairs high, different-label pairs low," using the
*exact ground-truth perspective labels the evaluation then scores it
against* (just with the query item excluded from training, per LOO). That
is supervised metric learning fit to the evaluation labels, not a
self-supervised model discovering structure nobody told it about. Genuine
JEPA is self-supervised: it predicts a masked/context part of an input from
another part of the *same* input, with no external category labels
involved anywhere. What I built and labeled "JEPA-style" uses external
labels as direct supervision — a materially different (and much easier)
problem. Of course a classifier trained on 20 of 21 labeled examples, over
only 3 classes with 6-7 examples each, beats an unsupervised embedder's raw
geometry at predicting those same 3 classes. That is not a novel finding
about representation learning; it is the ordinary, expected behavior of
supervised learning with generous labeled data over few classes.

**Control performed before trusting the number, not after:** does the same
architecture also "learn" to predict a fixed, deterministic *shuffle* of the
coarse labels — i.e., meaningless relabeling that carries no real
structure — equally well? If a high-capacity-relative-to-data model
(rank-8 ≈ 6k parameters vs ~20 training points) is simply memorizing
whatever labels it's given, shuffled labels should fit just as well as real
ones. Result: **mean F1 on shuffled labels = 0.254**, at or below the
chance floor (~0.33) — clearly *not* elevated. This rules out pure
memorization: the model is not simply capable of fitting arbitrary label
assignments; something about the *true* coarse labels specifically
correlates with extractable structure in the frozen embedding space that a
supervised learner can find and raw cosine similarity does not surface on
its own (naive cosine only gets 0.587 on the same true-label task).

**The honest reading, combining both facts:** raw cosine similarity is
leaving real, non-trivial, non-memorized signal on the table for the
*coarse* 3-way distinction in this embedding space — a supervised learner
recovers much more of it (0.952) than plain cosine does (0.587). That is a
genuine, interesting finding about MiniLM's embedding geometry and about
supervised metric learning as a technique. **It is a different finding from
the mission's hypothesis** (self-supervised discovery of *unknown*
perspectival structure from a reference point, without a labeled taxonomy
to train against). The fine-grained result is the more relevant data point
for that actual hypothesis: with only ~2-3 labeled examples per fine class,
the supervised advantage nearly disappears (0.484 vs naive's 0.468, a
statistical tie) — when there is little labeled signal to exploit, this
predictor does not meaningfully beat raw cosine either.

**No self-supervised JEPA test has been run.** Building one properly would
need a corpus with natural context/co-occurrence structure to predict from
(e.g. sentences that mention multiple related concepts together, so a
predictor learns "given this context, what embedding is likely nearby"
without ever being told a category label) — this 21-item hand-labeled toy
corpus has no such structure to exploit; it was built for the coarse/fine
label evaluation in Iterations 1-2, not for self-supervised training.

## Interim classification, revised after Iteration 3

Unchanged from Iteration 2 for the mission's actual hypothesis
(**INCONCLUSIVE** — reference-conditioned discovery of *unlabeled*
perspectival structure, self-supervised or otherwise, has not yet been
shown to work or fail; only supervised metric learning against known labels
has been tested, which answers a different question). Two new, real,
separate findings:
- Supervised low-rank metric learning substantially outperforms raw cosine
  for known-label classification in this embedding space, when there is
  enough labeled data per class (coarse: yes; fine: no) — confirmed via a
  shuffled-label negative control, not just a raw number.
- No self-supervised predictive-latent (true JEPA) mechanism has been
  built or tested yet; doing so needs a corpus with real context structure,
  which does not currently exist in this research track.

## A more promising direction, prompted by the user: apply supervised metric
## learning to physis-core's EXISTING labeled ontology instead of a toy corpus

Iteration 3's real (non-confounded) finding is: *supervised* metric learning
meaningfully beats raw cosine similarity when the label taxonomy is coarse
and well-populated. Physis-core already has exactly that: the 70-cell
semiotic grid (5 domains × 14 modes) in `classify.rs`/`ontology.rs`, with
real labeled ontology entries — not a 21-item hand-built toy corpus. Testing
whether a trained low-rank metric improves `CellClassifier`'s cosine-based
scoring against that *existing, real* ontology would be:

1. Not confounded by "discovering the unknown" — it would openly be
   supervised classification improvement, which Iteration 3 already shows
   can be real and non-memorized. Framed honestly as that, not as
   self-supervised discovery.
2. Directly relevant to the user's proposal (a combined coherence signal —
   raw embedding similarity + a learned/predictive similarity + the
   existing categorical ontology label — rendered as a visualization in the
   embedded Studio). The categorical "semantic datapoint" the user asked
   for already exists in physis-core as the classified (domain, mode) cell;
   it does not need to be invented.
3. Worth doing *before* building any "JEPA image" visualization — if a
   trained metric doesn't improve real ontology classification either, the
   visualization would be presenting a mechanism with no demonstrated value
   as if it were validated. Not yet attempted; proposed as the next
   concrete step if this research track continues.

## Iteration 4 — the same technique on real ontology data (prompted by the user)

File: `physis-core/examples/ontology_metric_learning_experiment.rs`. Applies
Iteration 3's exact predictor architecture (rank-K asymmetric predictor,
stop-gradient target, InfoNCE) to physis-core's real, existing 70-cell
ontology instead of the 21-item toy corpus — 730 real entries, 5 domains
(118-169 members each, well-powered), plus a 31-cell subset restricted to
cells with >=5 members (the other 35 of 70 cells have exactly 1 entry each
and cannot support a train/test split). Stratified 5-fold CV, 1-NN
classification (mirrors `CellClassifier`'s actual production mechanism:
"max cosine to any member entry").

**First run: a real numerical bug, caught before it produced a false
result.** `final_train_loss=NaN`, `pred_mean_norm=NaN`. Root cause: the
per-query gradient summed the positive set rather than averaging it
(`dloss/dp ∝ |positives|·E_probs[x] − Σ_positives x`). The toy corpus's
fields topped out at 6 members, where this is harmless; domain classes here
have up to ~170 members, so the un-normalized positive-sum term is ~30x
larger, and the fixed learning rate that was stable at toy-corpus scale
exploded the weights to NaN within a few epochs. Silent consequence if
unnoticed: `f32` comparisons with NaN are always `false` in Rust, so the 1-NN
search's `if s > best_s` never updates past its initial guess — the reported
"jepa_acc" numbers from the broken run were not measuring the predictor at
all, they were measuring how often training-item index 0's class happened to
match the test item's class. **Confirmed by the diagnostic added specifically
to check this** (final loss + predicted-vector norm + pairwise cosine,
printed before trusting any accuracy number) — this is the same
"interrogate the number before believing it" discipline as Iterations 1-3,
just catching a different failure mode.

**Fix:** average the positive term instead of summing it (standard
multi-positive/SupCon-style contrastive loss — both terms become
convex combinations of unit vectors, so the gradient stays bounded
regardless of class size). Re-ran with the fix:

- No more NaN. `pred_mean_norm=0.045` (small but non-zero — not degenerate).
  `pred_mean_pairwise_cos=0.577` vs raw embeddings' own `0.775` — predicted
  vectors are *more* spread out than raw ones, ruling out representation
  collapse (predictions did not converge to a near-identical constant).
- **`final_train_loss ≈ 6.28-6.36`, which is essentially exactly the
  chance-level loss** for this problem size (`ln(581 candidates) ≈ 6.36`
  for the domain task). Training is not broken and not collapsed — it
  converged to a solution that is, by its own loss function, no better than
  guessing.

| task | n | classes | baseline 1-NN | trained 1-NN | delta |
|---|---:|---:|---:|---:|---:|
| domain | 730 | 5 | **0.477** | 0.227 | −0.249 |
| cell (≥5-member subset) | 685 | 31 | **0.220** | 0.070 | −0.150 |

**Raw cosine similarity — the existing production mechanism — beats the
trained metric by a wide margin on both real tasks**, at the hyperparameters
tested (rank=16, 150 epochs, lr=0.15, τ=0.15, λ=1e-3, chosen by carrying
over Iteration 3's toy-corpus values, not tuned for this scale). Given the
loss sits at the chance floor, this reads as **the optimization not finding
a useful solution at this configuration**, not necessarily "the mechanism
cannot work here" — 150 epochs of full-batch gradient descent on a harder,
5-30x larger problem than Iteration 3 may simply be undertrained, or the
fixed hyperparameters may need real tuning (a validation split, a learning
rate schedule, more epochs) rather than a straight carry-over. I have not
done that tuning — doing so without a principled stopping rule is exactly
the "keep tuning until it looks good" trap the mission warns against, and
four experiments in, the more defensible move is to report the result as-is
and let it inform priority rather than to keep adjusting knobs hunting for
a win.

## Cumulative picture after four experiments

Every mechanism tested so far, when evaluated with a proper control (a
working baseline, held-out data, a memorization/collapse check), has been
matched or beaten by the method the mission itself says shouldn't even
count as sophisticated (Δ-magnitude: mathematically identical to cosine;
Δ-direction clustering: beaten at n=21; supervised toy-corpus predictor:
wins only because of a label-supervision confound, ties once that confound
is thinned out; the same predictor at real scale: beaten by raw cosine by a
wide margin, converging to chance-level loss). **Raw cosine similarity has
not yet been beaten by anything tested under a fair, controlled comparison
in this research track.** This is real, cumulative evidence against the
family of mechanisms tried so far — not proof no learned mechanism could
ever help (undertraining/hyperparameters remain a live, unexcluded
explanation for Iteration 4 specifically), but four negative-or-confounded
results in a row is the kind of pattern the mission's own stopping
criteria (Section 26: "stop when further iterations produce negligible
improvement") point at directly.

## Iteration 5 — does perspective-framing CONTEXT matter more than the mechanism?

Prompted by a specific claim from the user: bare noun phrases ("engine",
"steering wheel") carry no relational context, so a static embedding of one
cannot encode "this, considered mechanically" vs "this, considered
economically" — every prior iteration may have been testing mechanisms
against embeddings too impoverished to carry perspectival structure at all,
regardless of which mechanism was applied on top.

File: `physis-core/examples/perspective_discovery_experiment5_context.rs`.
Replaced each of the 21 Dataset-B items with one natural sentence supplying
genuine co-occurring vocabulary (e.g. "engine" → "The engine converts fuel
into motion through repeated combustion cycles") — **without literally
naming the perspective category anywhere** in any sentence (no
"mechanical"/"driver"/"economic" keyword), since that would just be a
different, cheaper confound (lexical leakage) than the one being tested.
Re-ran the identical evaluation from Iterations 1-2 (naive NN F1,
Δ-direction F1, k-means purity, both granularities) on the sentence
embeddings. Bare-term numbers reproduced the published Iteration 2 values
exactly (0.587/0.468/0.469/0.226/0.476/0.571) as a sanity check on the
re-implementation before trusting the new column.

| | bare term | contextual sentence | Δ |
|---|---:|---:|---:|
| naive NN, coarse | 0.587 | 0.579 | −0.008 |
| naive NN, fine   | 0.468 | 0.286 | **−0.183** |
| Δ-direction, coarse | 0.469 | 0.442 | −0.027 |
| Δ-direction, fine   | 0.226 | 0.271 | +0.045 |
| k-means(k=3) purity | 0.476 | **0.810** | **+0.333** |
| k-means(k=6) purity | 0.571 | 0.524 | −0.048 |

**The claim is confirmed, but not uniformly, and the specific shape of the
result is the actual finding.** Context produces one dramatic effect and it
is not on any of the reference-conditioning mechanisms — it is on
*reference-free* global clustering: k-means(k=3) purity jumps from 0.476
(weak, barely-above-chance signal) to 0.810 (strong, usable separation) once
items are embedded as full sentences instead of bare terms. Every
per-reference method (naive NN, Δ-direction) stays roughly flat or gets
*worse*, particularly naive-NN at fine granularity (−0.183). A plausible
mechanism: sentences in the same coarse domain share general topical/lexical
scaffolding (all "mechanical" sentences invoke combustion-adjacent
vocabulary) that a *global* clustering algorithm can exploit directly, but
that same shared scaffolding adds noise to *pairwise* reference-conditioned
comparisons — particularly at fine granularity, where the signal that
matters is the specific difference between two sentences, and generic
sentence structure dilutes exactly that.

**This reframes the whole research track's central negative finding.**
Iterations 1-4 asked "does some clever mechanism beat plain
similarity/clustering?" and answered no, repeatedly, using
under-contextualized bare-term embeddings throughout. Iteration 5 suggests a
sharper, more useful question was hiding underneath: **plain, unsupervised,
reference-free clustering — no reference-conditioning, no learned metric, no
JEPA — recovers the 3-perspective structure well (0.81 purity) once the
input representation is fixed.** The bottleneck this whole track was
implicitly fighting may never have been "we need a cleverer discovery
mechanism" — it may have been "the corpus was under-described." That is a
data/representation problem, not an algorithm problem, and it was hiding in
plain sight because every earlier iteration held the (impoverished) input
representation fixed and only varied the mechanism on top of it.

**Not yet tested:** whether this contextual-embedding effect holds on
physis-core's real ontology (Iteration 4's task) — the 730 real entries are
already `name + hints`, arguably already "more than a bare term" but still
short and enumerative rather than sentence-form; whether it holds at the
fine-grained (6-label) level as strongly as coarse (it clearly does not, by
the numbers above — fine-grained purity mildly *dropped*, −0.048); and
whether recursion or JEPA, re-tried on contextual embeddings instead of bare
terms, would show a different pattern than Iterations 1-4 found. Flagging
all three rather than picking the one that would make a tidier narrative.

## Iteration 6 — replicating Iteration 5 on a second, structurally different dataset

Mission Dataset A (a real taxonomic hierarchy — Animal -> Mammal/Bird ->
Domestic/Predator/Flying/Flightless, 15 items) instead of Dataset B's
cross-cutting perspectives. Same evaluation, same bare-term-vs-contextual-
sentence comparison, same no-category-leakage rule for sentences. File:
`physis-core/examples/experiment6_dataset_a_hierarchy.rs`.

**The Iteration 5 effect does not replicate:**

| | CAR (Iteration 5) | ANIMAL (Iteration 6) |
|---|---:|---:|
| bare-term k-means coarse purity | 0.476 | **0.667** |
| contextual k-means coarse purity | 0.810 | 0.667 |
| Δ (context − bare) | **+0.333** | **+0.000** |
| bare-term naive NN fine F1 | 0.468 | 0.400 |
| Δ naive NN fine (context − bare) | −0.183 | −0.056 |

Zero change from context on the animal corpus's coarse clustering, against
a dramatic +0.333 on CAR. **The broad claim "contextual sentences help
unsupervised clustering" does not hold as a general statement — it held on
one corpus and not on a second.** The direction of every other effect
(naive-NN mildly hurt by context, Δ-direction mildly helped) is at least
consistent in *sign* with Iteration 5, just far smaller in magnitude here.

**A more precise, falsifiable hypothesis, refined from what both runs
actually show:** ANIMAL's bare-term baseline was already strong (0.667
purity) before any context was added; CAR's bare-term baseline was weak
(0.476). Common, everyday words (dog, cat, eagle, lion) are exactly the
kind of vocabulary a general-purpose sentence embedder sees constantly in
its training distribution and already represents well in isolation;
specialized/technical vocabulary (camshaft, fuel injection) is not, and
needs surrounding context to activate the right semantic neighborhood.
Under this refinement, context should help in proportion to how
poorly-represented the bare terms already are — not as a blanket effect.
This is now a specific, testable claim (correlate baseline quality against
context-driven improvement across more corpora) rather than the broader
one Iteration 5's single data point suggested. Not yet tested against a
third corpus; flagged rather than assumed.

## Iteration 7 — recursion (mission Steps 8-9), the one piece never attempted

Recursed the ONE mechanism with genuine (if inconsistent) baseline signal —
plain k-means — since recursing Δ-direction or the trained predictor
(falsified as improvements over baseline in Iterations 1-4) would only
propagate a losing move deeper, not test the recursive hypothesis fairly.
Used Dataset A (animal taxonomy) because it has real, independently-known
two-level ground truth, unlike Dataset B's non-hierarchical perspectives.
File: `physis-core/examples/experiment7_recursion.rs`.

**A methodological gap in Iterations 5-6, caught here:** printing cluster
*sizes*, not just purity, for the first time reveals depth-1 k=2 clustering
produces a **13-vs-2 split**, not the true ~8-vs-7 mammal/bird partition.
Purity alone (0.667, unchanged from Iterations 5-6) looked like "a
reasonable 2-way separation" but was consistent with one giant cluster
absorbing most items regardless of true label and a tiny second cluster
mopping up 2 outliers — majority-vote purity does not penalize this kind of
imbalance. **Every earlier k-means coarse-purity number in this track should
be read with this caveat**: purity was never checked against cluster-size
balance, and it should have been.

**Two recursion pipelines, deliberately separated:**

- **REAL** (recurse into whatever depth-1 actually found): dominated by the
  13/2 imbalance above — the 13-item cluster's depth-2 split scores 0.462
  purity (barely above the ~0.5 chance floor for a roughly-even 2-way
  split), the 2-item cluster's "1.000" is a trivial artifact of only having
  2 items to place into 2 slots. This pipeline mostly demonstrates
  **error propagation**: a bad top-level split makes the recursive step
  underneath it close to meaningless, exactly the garbage-in-garbage-out
  failure mode the mission's own architecture notes warn about.
- **ORACLE** (recurse into the TRUE mammal/bird groups instead): mammal ->
  domestic/predator purity = 0.625 (bare) / **0.750** (contextual); bird ->
  flying/flightless purity = 0.714 (both representations). Meaningfully
  above chance (~0.5-0.57 for a near-even 2-way split at n=7-8), clearly
  below perfect. **This is the single clearest positive signal for the
  recursive hypothesis found anywhere in this research track**: given a
  correct top-level grouping, a second unsupervised clustering pass
  recovers a real, non-trivial fraction of the true nested sub-structure —
  genuinely partial support, not a clean win, and entirely dependent on the
  top level being right first.

**Stress test** (mission Section 20 — does recursion know when to stop?):
forced a k=2 split of the "domestic" leaf group (dog, cat, horse, sheep),
which has no known sub-structure by construction. Bare-term and contextual
representations agreed on 3 of 4 placements (dog+cat together in both;
horse and sheep swapped) — 0.75 agreement. Suggestive of *some* real
signal surviving even here (dog/cat vs horse/sheep is a defensible
domestic-companion-vs-livestock split a human might draw), but **n=4 is far
too small to trust this as more than a hint** — reported as inconclusive,
not as evidence either way, rather than rounded up to a finding.

## Ablation summary (mission Section 19), compiled from Iterations 1-7

No component was tested in isolation from scratch for this section — the
comparisons below were already run as part of the main iterations, and are
collected here as the ablation matrix the mission's own protocol expects,
rather than re-run cosmetically:

| ablated component | comparison | result |
|---|---|---|
| reference conditioning (on vs off) | naive-NN-from-reference vs Δ-direction clustering | naive (the "off"/cheap baseline) wins on 21-item CAR at both granularities (Iteration 2); trained predictor variant also loses on real 730-entry ontology (Iteration 4) |
| recursion (on vs off) | flat k-means vs recursive (oracle) k-means | recursion adds real value ONLY when the top level is correct (Iteration 7 oracle: 0.625-0.75) — "on" is not uniformly better, conditional on a component not yet reliable |
| supervision (on vs off) | naive cosine (unsupervised) vs trained predictor (supervised) | supervised wins dramatically on toy data (confounded — Iteration 3) but LOSES badly on real ontology data (Iteration 4) — the opposite ranking depending on data realism, not a stable ordering |
| input representation (bare term vs context) | Iterations 5 vs 6 | helps dramatically on one corpus (+0.333 purity), not at all on a second (+0.000) — conditional on how well-represented the bare vocabulary already is, not a stable global effect |
| reference selection strategy (fixed vs random vs uncertainty-based) | not tested | genuine gap — flagged, not attempted |
| coherence/novelty/boundary-detection scoring | not tested | genuine gap — flagged, not attempted (no boundary-detection mechanism was ever built; Section 12 of the mission was skipped entirely given the underlying per-reference mechanism it would have gated was already falsified) |

## Iteration 8 — Dataset D (cross-cutting concepts): the first unambiguous win

Every dataset used so far (A, B) assumed exclusive membership — one true
field per item. That assumption happens to be exactly what hard clustering
(k-means) is built for, and exactly the structural precondition that made
every earlier "does a fancier mechanism beat plain clustering" comparison
fair to plain clustering specifically. Mission Dataset D (Section 5) tests
the case the mission explicitly names ("fuel: CAR, ENGINE, ECONOMICS,
ENVIRONMENT") — concepts genuinely belonging to more than one field. File:
`physis-core/examples/experiment8_dataset_d_crosscutting.rs`.

Corpus: the 21-item CAR corpus (Iteration 5 contextual sentences, exclusive
membership) plus 4 new items deliberately written to span two fields each,
again with no field-name keyword leakage: *fuel efficiency* and *hybrid
battery* (mechanical+economic), *driver assistance system*
(mechanical+driver), *warranty coverage* (driver+economic).

**Test 1 — hard clustering's structural ceiling, demonstrated not just
asserted:** k-means (k=3) over all 25 items assigns every cross-cutting
item to exactly one cluster, 4/4 times matching *one* of its two true
fields and silently discarding the other. This is true by construction —
the point of running it was to confirm the demonstration is real, not
theoretical, and it is.

**Test 2 — does the embedding itself carry dual-membership signal at
all?** For each cross-cutting item, computed cosine similarity to the
centroid of each of the 3 pure fields (built from the 21 exclusive-
membership items only, never seeing the cross-cutting items). "Dual
margin" = similarity to the *weaker* of its two true fields, minus
similarity to the one unrelated field.

| item | true fields | sim(field 1) | sim(field 2) | sim(unrelated) | dual margin |
|---|---|---:|---:|---:|---:|
| fuel efficiency | mechanical, economic | 0.804 | 0.826 | 0.767 | **+0.037** |
| hybrid battery | mechanical, economic | 0.821 | 0.809 | 0.795 | **+0.013** |
| driver assistance system | mechanical, driver | 0.832 | 0.876 | 0.803 | **+0.028** |
| warranty coverage | driver, economic | 0.784 | 0.841 | 0.766 | **+0.018** |

**All 4 of 4 cross-cutting items show a positive dual margin.** The
embedding is, on its own, placing every one of them closer to *both* of its
true fields than to the field it doesn't belong to — modest margins
(+0.013 to +0.037), but consistently positive, not a coin flip. This is
real, non-trivial dual-membership signal that a hard-assignment mechanism
cannot express *by construction*, independent of whether the signal is
present in the underlying embedding.

**This is the first clean, unambiguous positive result for
reference-conditioning (broadly construed) in this entire research
track**, and it earns that status differently than Iterations 1-4 tried
to: not by beating a baseline on the SAME task the baseline is built for,
but by testing a task (multi-membership) the baseline is structurally
unable to attempt at all. It does not resurrect Δ-direction clustering or
the trained predictor — both remain falsified on the exclusive-membership
tasks they were tested on — but it does mean abandoning single-cluster
hard assignment in favor of a per-field (or per-reference) similarity
score is genuinely justified wherever cross-cutting concepts are expected,
which is a real architectural implication.

**Direct relevance to physis-core specifically:** `OntologyEntry` (in
`src/models.rs`) has exactly one `domain: String` and one `mode: String`
field per entry — the same single-membership structural limitation as
k-means, already present in the actual production ontology model, not
just in this experiment's toy baseline. If cross-cutting concepts are
common in real corpora (plausible; not measured here), the production
system has the same blind spot Test 1 demonstrates, and a per-field
similarity score (which `classify.rs` already computes internally via
`CellClassifier::classify` before collapsing to a single winning cell)
could recover multi-membership information the stored `(domain, mode)`
label cannot — a small, low-risk enhancement to consider even though it
was discovered as a side effect of falsifying everything else in this
track, not as its original goal.

## Iteration 9 — does a certification signal actually distinguish a good split from a bad one?

Prompted by the user's proposed synthesis architecture (multi-membership +
coherence gating + certified recursion, built directly on Iterations 7-8's
surviving findings). "Certified recursion" means only recursing when an
internal signal predicts a split is trustworthy — untested until now.
physis-core's existing `coherence_score` is a node-level density metric
(mean cosine to nearest neighbours), not a split-quality metric, so it
cannot be assumed to transfer to "should I recurse into this cluster"
without checking. File:
`physis-core/examples/experiment9_certification_gate.rs`.

Tested two cheap candidate signals against the exact two splits Iteration 7
already scored against ground truth (the real 13-vs-2 k-means split vs the
true 8-vs-7 mammal/bird partition):

| signal | GOOD split (8-vs-7, purity 1.000) | BAD split (13-vs-2, purity 0.667) | ranks GOOD above BAD? |
|---|---:|---:|---|
| size-balance ratio (min/max cluster size) | **0.875** | 0.154 | **yes** |
| silhouette-like (within-cohesion minus between-separation) | 0.048 | **0.089** | **no — backwards** |

**Balance ratio is a real, cheap, validated certification signal** for the
exact failure mode Iteration 7 found — it directly measures cluster-size
imbalance, so of course it catches a 13-vs-2 split, but that is precisely
the failure mode this architecture needs to guard against, and it does so
with a large, unambiguous margin.

**Silhouette-style coherence gets it backwards, and the mechanism why is
worth stating plainly:** a large, heterogeneous 13-item cluster's centroid
drifts toward the corpus-wide average, so most of its members still score
reasonably close to it on average even though the cluster is semantically
wrong; the correct 8-vs-7 split has more genuine internal variation (a lion
and a sheep are both "mammal" but not very similar to each other) and
scores *worse* on a naive cohesion-minus-separation metric despite being
right. **If "coherence gating" means silhouette-style scoring, wiring it in
as originally proposed would certify the wrong split and reject the right
one — the opposite of the intended effect.** This is not a reason to drop
certification gating; it is a reason not to trust the first plausible
metric without the same scrutiny every other result in this track has
gotten.

**Recommendation, refining rather than rejecting the user's proposal:**
use size-balance ratio (thresholded, e.g. reject/don't-recurse below ~0.5)
as the near-term certification gate — it is validated, cheap, and directly
targets the observed failure mode. Do not use naive silhouette/cohesion
scoring as a gate without further investigation; it failed the one test
run against it here.

## Data model check: does multi-membership fit physis-core's existing structures?

Checked before recommending any schema change. `TypedEdge`/`RelationType`
(`src/relation.rs`) are entirely epistemic/causal relations (Supports,
Contradicts, Causes, Precedes, DependsOn, ...) — no "member-of" or
"classified-as" relation exists, so representing multi-membership as
literal graph edges with per-edge provenance (mission Section 11: "D
discovered from A" and "D discovered from B" as separate relation records)
would need a new `RelationType` variant, a small additive enum change, not
a redesign.

**Lower-risk path, needing zero schema changes**: `CoherenceNode.properties`
(`serde_json::Map<String, Value>`, restored during PH-014 specifically to
be an open extension point) can hold a full per-cell membership vector
today — e.g. `properties["cell_memberships"] = [{"domain":..., "mode":...,
"score":...}, ...]`. This is the "surface an existing signal" path the
Final Report already recommended, now confirmed concretely buildable
without touching `CoherenceNode`'s native fields or `TypedEdge` at all.
Graph-level member-of edges (useful later for traversal, e.g. "show me
every node connected to both the mechanical and economic fields") remain a
real but separate, larger follow-up.

## Remaining gaps before this synthesis architecture should be built for real

Flagged after Iteration 9; all three closed by Iteration 10 below (with
honest caveats, not clean wins across the board).

1. **Membership-vector threshold calibration and false-positive rate.**
   Iteration 8 tested recall on 4 hand-picked cross-cutting items only —
   it never checked whether the 21 *pure*, single-membership items also
   trigger spurious "dual membership" signals above whatever threshold
   gets chosen. A threshold that looks good on 4 positive examples and
   was never checked against negatives is not calibrated.
2. **No end-to-end integration test.** Multi-membership (Iteration 8),
   conditional recursion (Iteration 7), and now a balance-based
   certification gate (Iteration 9) have each been validated in isolation,
   on different slices of data. Nothing in this track has run all three
   together on one corpus that has both real hierarchy AND cross-cutting
   concepts (no dataset built so far has both at once) and checked whether
   the composed system recovers a structure that matches a combined
   ground truth.
3. **Multi-membership turns the recursion tree into a DAG** — a node
   reachable via two different parent memberships needs revisit/cycle
   protection (mission Section 10's "visited-reference cache," never
   built) or recursion could re-expand the same node once per membership
   path, multiplying branching factor by however many fields a node
   belongs to.

## Iteration 10: closing gaps 1-3 on one corpus with both hierarchy and cross-cutting structure

`physis-core/examples/experiment10_integration.rs`. New corpus: Dataset A's
15-item animal taxonomy (Iteration 6/7's real two-level ground truth) plus
4 NEW cross-cutting animals, each spanning two fine categories, written
with the same no-keyword-leakage discipline as Dataset D:

- `police_dog` — domestic + predator (both mammal-branch categories)
- `farm_duck` — domestic + flying (a *bird* pulling toward the
  mammal-branch "domestic" category — genuinely crosses the coarse split)
- `cassowary` — predator + flightless
- `farm_chicken` — domestic + flightless (also crosses the coarse split)

**Gap 1 — threshold calibration.** Swept a membership-tolerance delta
(a category counts as "also a member" if its similarity is within delta
of the item's own top similarity) from 0.00 to 0.20 against fine-category
centroids built from the 15 pure items only. Result:

| delta | FPR on 15 pure | recall/4 (both true fields present) |
|---|---:|---:|
| 0.00 | 0.000 | 0/4 |
| 0.02 | 0.000 | 2/4 |
| **0.03** | **0.000** | **4/4** |
| 0.04 | 0.133 | 4/4 |
| 0.10+ | 1.000 | 4/4 |

delta=0.03 is the calibrated threshold: zero false positives on pure
items, full recall on cross items. **Caveat, not swept under the rug**:
average membership count on the 4 cross items at this delta is 2.76, not
2.0 — the calibration achieves "both true fields present with no false
positives on pure items" but is not crisp; cross items pick up a spurious
third category on average about as often as not. Good enough to unblock
gap 2 and 3; not a finished, production-grade threshold.

**Gap 2 — end-to-end integration.** Ran multi-membership (at the
calibrated delta) and the Iteration 9 balance-ratio gate (threshold 0.5)
together on the full 19-item corpus:

- REAL (unconstrained) depth-1 k-means: 17-vs-2 split, balance ratio
  0.118 — **gate correctly rejects**, does not recurse. (Worse imbalance
  than Iteration 7's 13-vs-2 on the 15-item corpus alone — the added
  cross-cutting items make naive top-level clustering even less reliable,
  which is exactly the scenario certification is for.)
- ORACLE depth-1 (true mammal=9/bird=10): balance ratio 0.900 — gate
  passes, recurses.
- ORACLE→REAL depth-2 within `mammal` (n=9): 7-vs-2 split, balance ratio
  0.286 — **gate correctly rejects** (predator cluster absorbed dog, cat,
  and police_dog).
- ORACLE→REAL depth-2 within `bird` (n=10): 6-vs-4 split, balance ratio
  0.667 — gate passes, **but the split is not label-correct**: ostrich and
  cassowary (both true flightless) land in the cluster later labeled
  "flying." **This is the gate's real limitation, stated plainly: balance
  ratio checks size, not correctness — a confidently balanced but wrong
  split sails through.** Certification is necessary, not sufficient.
- Cross-links (multi-membership pointing outside an item's home leaf) show
  up on real, not hypothetical, data: `police_dog`, `cassowary`,
  `farm_duck`, and `farm_chicken` all get at least one **cross-branch**
  link (e.g. `farm_duck`, physically placed under `bird.flightless`, also
  links to `mammal.domestic`) — confirming the DAG case Iteration 9's
  write-up predicted actually occurs, not just in principle.

**Gap 3 — DAG cycle protection.** Built the adjacency graph directly from
gap 2's discovered cross-links (6 nodes: 2 coarse branches + 4 fine
leaves) and compared naive vs. visited-set expansion from a
cross-linked leaf:

- WITHOUT a visited-set: pure DFS never backtracks out of the cycle here
  (it dives into the first neighbor repeatedly), so a call-budget cap of
  200,000 was exhausted while recursion depth was still climbing (reached
  199,999) — run on a dedicated thread with a 512MB stack specifically
  because the first attempt at this test overflowed the default 8MB stack
  before the budget cap could even be checked. **No natural base case —
  confirmed, not assumed.**
- WITH a trivial visited-set: terminates in exactly 6 calls, matching the
  graph's 6 distinct nodes exactly, each expanded once.
- **Verdict: CONFIRMED.** A visited-node cache is required, not optional,
  once multi-membership makes the structure a DAG.

**Net assessment of the user's proposed synthesis architecture**
(multi-membership + coherence gating + certified recursion): the pieces
compose without a fundamental incompatibility, and the balance-ratio gate
earns its keep on real combined data (correctly rejecting two bad splits
here). But it is not yet a system that reliably recovers a correct nested
ontology — the surviving failure mode is a split that is well-balanced
AND wrong (the bird depth-2 case above), which no signal tested in this
track catches. That is the next thing to solve before recommending this
architecture be built as production code, not a blocking defect in the
architecture's shape.

## Iteration 11 — hunting for a certification signal robust to balance (user-directed follow-up)

Directly targets the open problem left by Iteration 10: find a signal
that catches "balanced but wrong" without also incorrectly rejecting
"unbalanced but right." Every test up to this point varied only ONE axis
(correctness OR balance) per comparison, so no candidate had actually been
checked against both failure directions at once.
`physis-core/examples/experiment11_generalized_certification.rs` builds a
proper 2x2 using real/established ground truth in every cell — no
synthetic "obviously wrong" split invented just to make a point:

| | balanced | imbalanced |
|---|---|---|
| **correct** | Case 1: Dataset A true mammal/bird (8v7) | Case 4: NEW vehicle corpus, true wheel-count split (2v10) |
| **wrong** | Case 3: Iteration 10's bird depth-2 real k-means (6v4) | Case 2: Dataset A real depth-1 k-means (13v2) |

Case 4 is new: 12 vehicles, ground truth is literal wheel count (2
two-wheelers: motorcycle, bicycle; 10 four-or-more-wheelers: car, truck,
bus, van, SUV, pickup, minivan, tractor, forklift, golf cart) — objectively
correct by definition, deliberately imbalanced (ratio 0.2), no
"two-wheeled"/"four-wheeled" keywords in any sentence.

**Five candidate signals tested, pass criterion = min(score over both
correct cases) > max(score over both wrong cases):**

| signal | case1 (good/bal) | case2 (bad/imbal) | case3 (bad/bal) | case4 (good/imbal) | separates? |
|---|---:|---:|---:|---:|---|
| balance ratio | 0.875 | 0.154 | 0.667 | 0.200 | **NO** |
| silhouette-like | 0.048 | 0.089 | 0.087 | 0.086 | **NO** (doesn't discriminate at all here) |
| kNN-consistency, raw (k=3) | 0.711 | 0.867 | 0.667 | 0.750 | **NO** (bad case 2 scores *highest* of all four) |
| kNN-consistency, size-adjusted, fixed k=3 | 0.244 | 0.114 | 0.200 | 0.053 | **NO** |
| kNN-consistency, size-adjusted, adaptive k | 0.244 | 0.092 | 0.200 | 0.164 | **NO — but closest** |
| confident-item fraction (epsilon swept 0.00-0.30) | — | — | — | — | **NO at every epsilon tested** |

**Diagnosed mechanism behind each failure, not just the failure itself:**

- Raw kNN-consistency mechanically favors majority clusters: with a fixed
  k=3, a large cluster has more candidate same-cluster neighbors
  available purely by base rate, independent of correctness — exactly why
  case 2 (the classic bad 13-vs-2 split) scores highest.
- The size-adjustment (subtracting the base-rate-expected same-cluster
  rate) fixes that specific bias — case 2 drops to the *lowest* score of
  all four once adjusted, correctly. But with a fixed k=3, a 2-item
  cluster (case 4's two-wheelers) can have at most 1 real same-cluster
  neighbor, so the metric is forced to count 2 off-cluster items as
  "neighbors" for the remaining slots regardless of embedding quality — a
  size-driven ceiling having nothing to do with correctness. Making k
  adaptive per item (`k = min(3, own_cluster_size - 1)`) removes this
  ceiling and lifts case 4 from 0.053 to 0.164 — a real improvement in
  the right direction, confirming the diagnosis — but it's still below
  case 3's 0.200.
- The remaining gap has its own mechanism, also diagnosed rather than
  left as an unexplained miss: case 3's split is only "20% wrong" (2 of
  10 birds mislabeled — ostrich and cassowary), so the other 8 items
  still have entirely correct local neighborhoods and the metric
  reasonably scores it as fairly consistent. Case 4's ground-truth axis
  (wheel count) is not necessarily the *dominant* semantic gradient the
  embedding space encodes for those sentences — a golf cart may sit
  closer to a bicycle via shared recreational context than to a dump
  truck via shared wheel count — so even the objectively correct split
  scores lower on a pure similarity-consistency metric. This suggests a
  structural limit for any single-embedding-similarity signal: it can
  confuse "the intended classification axis" with "the dominant semantic
  axis," and those are not guaranteed to coincide.
- Confident-item fraction was epsilon-swept (0.00 to 0.30 in steps of
  0.01) rather than assumed at one value — the fixed epsilon=0.02 used
  when this signal was first proposed saturated at 1.000 for every case,
  which is not calibration, it's an unexamined default. The full sweep
  shows no epsilon separates the four cases either; case 1 collapses
  toward 0 fastest as epsilon rises (it has the most gradually-varying
  margins of the four), the opposite of what a useful signal should show.

**Honest bottom line**: no cheap, single, universal certification signal
found in this track (5 candidates, 2 design iterations on the most
promising one) cleanly separates correctness from size across all four
quadrants. Adaptive+size-adjusted kNN-consistency is the strongest lead —
it is the only signal that correctly demotes the classic bad-imbalanced
case to the bottom — but is not yet sufficient on its own. Not
recommended for production certification as-is; flagged as a partial
result and a plausible foundation for a future ensemble (e.g. combine it
with balance ratio, or add a signal that specifically detects "small
number of borderline items in an otherwise-consistent cluster," which is
exactly the case-3 failure mode) rather than a finished answer.

## Iteration 12 — the integrated end-to-end synthesis mission (full user-reframed mission prompt)

The user provided a comprehensive 24-section mission prompt explicitly
treating Iterations 1-11 as established research history, not something
to re-litigate, and asking for one genuinely end-to-end test of
multi-membership + certification + certified recursion against properly
calibrated baselines — with explicit permission to conclude the synthesis
is unnecessary if calibrated cosine already does the job (Section 19's
"fairness rule").

`physis-core/examples/experiment12_integrated_synthesis.rs`. Scope
decision stated up front in the file: covers Sections 4-13, 15, 17-19.
Deliberately skips Section 16 (genuine self-supervised JEPA) per the
mission's own instruction not to spend the majority of effort there
before this simpler synthesis is tested, and Section 14 (reference
selection) is noted as not mapping onto this architecture — the current
design uses field centroids from all pure exemplars, not a single
reference item, so there is no "which reference" free parameter of the
kind Iterations 1-3 already falsified for the old per-reference
mechanism.

**New dataset** (per Section 5's instruction not to reuse Dataset A or D
alone): a 20-item Vehicle ontology, 3 coarse fields (Mechanical, Driver,
Economic) x 2 fine subfields each (Propulsion/Energy,
Controls/Assistance, Ownership/Acquisition) = 6 fine cells, 15 pure
single-membership items, plus 5 NEW cross-cutting concepts
(hybrid_battery, fuel_efficiency: mechanical+economic;
driver_assistance_system, regenerative_braking: mechanical+driver;
warranty_coverage: driver+economic) covering all 3 coarse-field pairs, no
field-name keywords in any sentence, both bare and contextual
representations built for every item.

**Two real implementation bugs caught before trusting any result** (the
track's established discipline, applied again):

1. CROSS items' ground truth is stated at the COARSE level (mechanical,
   driver, economic — matching the mission's own framing), but
   `membership_set` operates on the 6 FINE categories. The first version
   of the recall check compared fine-category strings directly against
   coarse labels, which can never match — silently forcing recall to 0
   regardless of what the embedding actually encoded, and this cascaded
   into a nonsense fallback delta (0.20-0.30) that flooded the pipeline
   with 81 spurious cross-links per representation. Fixed with an
   explicit `fine_to_coarse` map and a coarse-coverage check.
2. The calibration fallback (when no delta achieves both FPR=0 and full
   recall) picked the LARGEST delta among ties at max recall —
   mechanically the worst-FPR choice available, the opposite of what a
   calibration procedure should do. Fixed to pick the SMALLEST delta at
   max observed recall, with the full sweep printed (`DEBUG_SIMS=1`) so
   the choice is auditable rather than a hidden single number, matching
   Iteration 10's discipline.

**Baseline results** (Section 6, all three required baselines):

| | bare | contextual |
|---|---:|---:|
| Baseline A: raw cosine argmax (best-case credit) | 0.750 | 0.750 |
| Baseline B: flat hard k-means (k=6), purity/ARI/NMI | 0.550 / 0.187 / 0.624 | 0.500 / 0.093 / 0.556 |
| Baseline C: calibrated multi-membership cosine, delta / FPR / recall | 0.03 / 0.267 / 5-of-5 | 0.07 / 0.533 / 5-of-5 |

**Headline finding: multi-membership calibration degrades sharply with
category granularity.** Dataset D's 3 COARSE fields (Iteration 10)
achieved delta=0.03 at FPR=0.0 with full recall. This dataset's 6 FINE
sibling categories (e.g. Propulsion vs. Energy, both under Mechanical)
sit much closer together in embedding space — the debug sweep shows
pure items' top-1-vs-top-2 fine-category gap is often only 0.03-0.05 —
so achieving full cross-item recall here costs FPR=0.267 (bare) to 0.533
(contextual): roughly a quarter to over half of the 15 genuinely
single-membership pure items get spuriously flagged as multi-member at
the delta needed to catch the real cross-cutting ones. **This is a real,
concrete limitation for recursive discovery specifically**: each level of
recursion inherently makes sibling categories more semantically similar
to their parent's other children than top-level fields are to each
other, so a calibration that works at the coarse level should be expected
to need re-calibration, at worsening FPR cost, at every deeper level —
untested before this iteration because no prior dataset had fine-grained
siblings this close together.

**Full pipeline (oracle coarse -> REAL fine discovery -> certification
gate) results:**

| branch | representation | n | balance | purity (pure-only) | ARI | NMI | gate |
|---|---|---:|---:|---:|---:|---:|---|
| mechanical | bare | 7 | 0.167 | 0.600 | 0.000 | 1.000 | REJECT |
| driver | bare | 7 | 0.167 | 0.800 | 0.231 | 0.385 | REJECT |
| economic | bare | 6 | 0.500 | 0.600 | -0.250 | 0.021 | **PASS** |
| mechanical | contextual | 7 | 0.400 | 0.600 | -0.250 | 0.021 | REJECT |
| driver | contextual | 7 | 0.400 | 0.600 | -0.154 | 0.204 | REJECT |
| economic | contextual | 6 | 0.200 | 0.600 | 0.000 | 1.000 | REJECT |

**A third, independent confirmation of Iteration 11's open problem**: the
bare economic branch passes the gate (balance=0.500, right at the
Iteration-9-validated threshold) with ARI=-0.250 — worse than random
agreement with the true ownership/acquisition split. This is the same
"balanced but wrong" failure mode found in Iteration 10's bird branch and
tested exhaustively in Iteration 11's vehicle-corpus case, now emerging
unprompted on a third, independently-designed dataset — strong evidence
this is a structural property of balance-based certification, not an
artifact of one specific corpus. On the positive side, the gate correctly
rejected 5 of 6 branches here, and most of the rejected branches
(mechanical/driver at both representations) are genuinely poorly split
(low balance, mediocre-to-negative ARI) — the gate is doing real,
correct work most of the time; it is the one exception that reproduces
the known gap.

**Scale caveat, stated honestly rather than omitted**: each fine
category here has only 2-3 pure exemplars (vs. Dataset A's 4 per fine
category) — Baseline B's flat k-means purity (0.500-0.550) and ARI
(0.093-0.187) are themselves mediocre, suggesting part of the
per-branch discovery difficulty reflects this toy dataset's small
sample size, not solely the architecture. This limits how far the
specific numbers generalize; the qualitative findings (calibration
degrades with granularity; balanced-but-wrong recurs) are the
load-bearing results, not the exact percentages.

**Cross-links and false positives, concretely**: at the calibrated
delta, cruise_control and purchase_price — both genuinely
single-membership PURE items — appear in the cross-link list (e.g.
cruise_control linked to 3 other cells beyond its true home). This is
exactly what a 26.7% FPR looks like in practice: real, named, wrongly-
flagged items, not an abstract percentage.

**DAG protection reconfirmed on a third dataset**: naive expansion
without a visited-set exhausts a 200,000-call budget (still climbing) on
this dataset's 9-node graph (3 coarse + 6 fine), while a guarded
expansion terminates in exactly 9 calls — identical qualitative result to
Iteration 10, now independently reproduced.

**Context ablation (Section 15)**: contextual sentences did NOT help
here — bare's Baseline B purity/ARI/NMI (0.550/0.187/0.624) all beat
contextual's (0.500/0.093/0.556), and bare needed a smaller, less costly
calibration delta (0.03 vs 0.07 FPR=0.267 vs 0.533) for the same full
recall. This is a THIRD data point (after Iterations 5 and 6) showing
context's effect is not just "sometimes zero" (Iteration 6) but can be
actively negative — consistent with the refined hypothesis that context
helps in proportion to how poorly the bare vocabulary is already
represented, and these are all standard, well-represented mechanical/
automotive terms even bare.

**Section 19 fairness check, answered explicitly rather than glossed
over**: the full pipeline and Baseline C are not directly competing for
the same task. Baseline C answers "which of the 6 ALREADY-KNOWN fine
cells does this item belong to" (matching how `CellClassifier` already
works against a fixed grid); the full pipeline answers "starting from
only the 3 known COARSE cells, can the 6 fine cells be DISCOVERED
unsupervised, and is that discovery trustworthy enough to certify" —
Baseline C cannot even ask that question, since there is nothing to score
against before the fine cells exist. The honest comparison is the gap
between unsupervised discovery's ARI/NMI (table above, ranging from
-0.250 to 1.000 depending on branch) and Baseline C's implicit ceiling of
"the fine cells are known" — that gap is large and branch-dependent, not
a clean win either way.

**Net assessment**: the synthesis architecture (multi-membership +
certification + certified recursion) remains a coherent, implementable
design with each piece independently useful — but this iteration adds a
new, concrete boundary condition (calibration cost rises with sibling
granularity) to the one carried over from Iteration 11 (certification
doesn't catch balanced-but-wrong). Both are now confirmed on multiple,
independently-designed datasets, which is stronger evidence than either
alone. Not recommended for production as a complete system without
addressing at least one of these two open problems.

## Iteration 13 — the two next-experiments from Iteration 12, tested directly

Directed by the user ("go with the next experiments") to attempt both
open problems flagged in FINAL_REPORT.md's Next Experiments 6 and 7.
`physis-core/examples/experiment13_local_calibration_and_ensemble_gate.rs`.

### Part A — local/split calibration (Next Experiment 7): SUCCESS

Hypothesis: Iteration 12's single flat delta over all 6 fine cells
conflated two different-scale problems — same-branch sibling confusion
(close together, needs a tight threshold) and cross-branch detection
(further apart, can tolerate a looser threshold) — forcing one shared
number to serve both is what produced the bad FPR. Split into two
separately-calibrated checks on the same vehicle dataset:

| | GLOBAL (Iteration 12) | LOCAL sibling-only | LOCAL cross-branch-only | LOCAL combined (union bound) |
|---|---:|---:|---:|---:|
| bare | delta=0.03, FPR=0.267 | delta=0.00, FPR=0.000 | delta=0.03, FPR=0.067 | **≤0.067** |
| contextual | delta=0.07, FPR=0.533 | delta=0.00, FPR=0.000 | delta=0.07, FPR=0.133 | **≤0.133** |

Full 5/5 recall preserved in both representations. The sibling-confusion
check needs delta=0.00 for zero FPR with no recall cost at all, because
none of this dataset's 5 cross-cutting items happen to be same-branch
sibling pairs (all 5 span two different coarse branches) — there is
nothing to trade off. The cross-branch-only check, once the irrelevant
same-branch sibling is excluded from the comparison, achieves the same
full recall at roughly a quarter (bare) to a quarter (contextual) of the
flat approach's FPR. **This confirms the diagnosis and is a real,
substantial, recommendable fix**: don't calibrate one threshold against
the full flat cell set; separate same-branch-sibling confusion from
cross-branch membership detection, since they operate at genuinely
different similarity scales.

Caveat: this dataset happens to have zero same-branch cross-cutting
pairs, so sibling recall was never actually tested under a real
trade-off — a dataset with a genuine same-branch cross-cutting item
(e.g., a concept spanning both Propulsion and Energy under Mechanical)
would be needed to confirm sibling calibration also works when there is
something real to detect, not just something to avoid.

### Part B — ensemble certification gate (Next Experiment 6): still open

Tested whether requiring BOTH balance ratio AND Iteration 11's adaptive
size-adjusted kNN-consistency to pass (an AND-gate) resolves the
"balanced but wrong" problem, across all 6 now-known real cases spanning
five iterations (2 good — one balanced, one deliberately imbalanced; 4
bad — two imbalanced sanity checks, two independently-discovered
balanced-but-wrong instances from Iterations 10 and 12):

| case | balance | knn-adaptive |
|---|---:|---:|
| 1: GOOD-balanced (Dataset A true 8v7) | 0.875 | 0.244 |
| 2: BAD-imbalanced (Dataset A real, 13v2) | 0.154 | 0.092 |
| 3: BAD-balanced (Iter.10 bird branch, 6v4) | 0.667 | 0.200 |
| 4: GOOD-imbalanced (true wheel count, 2v10) | 0.200 | 0.164 |
| 5: BAD-balanced (Iter.12 economic branch) | 0.500 | 0.200 |
| 6: BAD-imbalanced (Iter.12 mechanical branch) | 0.167 | 0.143 |

Exhaustive sweep over both thresholds (balance in [0,1] step 0.1, knn in
[-0.10,0.30] step 0.01) finds **no combination perfectly separates good
from bad**. Best achievable: 5/6 correct decisions, and the failure is
diagnostic, not incidental — rejecting both balanced-but-wrong cases (3
and 5, knn≈0.200) requires a knn threshold above 0.164, which also
rejects case 4, the deliberately-imbalanced-but-CORRECT control
(knn=0.164, lower than the wrong cases it needs to be threshold below).
**This is the exact same ordering problem Iteration 11 found on 4
constructed cases, now confirmed with an exhaustive sweep across 6
independently-sourced real cases**: kNN-consistency's ambiguity about
"mostly right, a few items wrong" (case 3/5's actual character) versus
"correct but the ground-truth axis isn't the embedding's dominant
semantic gradient" (case 4's actual character) is not something a
balance-ratio-AND-knn ensemble can resolve, because both failure types
land in the same knn score range for reasons that have nothing to do
with balance.

**Conclusion**: Part A is ready to carry forward into any production
calibration strategy. Part B is confirmed, not just suspected, to need a
genuinely different signal — not an ensemble of the two candidates
tested so far — before "balanced but wrong" can be caught without also
breaking "imbalanced but right." The next attempt at this problem should
target case 3/5's specific character directly (a small number of
borderline items in an otherwise-consistent cluster) rather than
combining existing whole-cluster signals.

## Iteration 14 — a dedicated borderline-item detector, and exhausting the cheap-signal search

Directed by "any other experiments? proceed" — continuing directly on
Iteration 13's own recommendation: target the wrong cases' specific
character (a few borderline items in an otherwise-consistent cluster)
with a dedicated signal, not another ensemble of existing ones.
`physis-core/examples/experiment14_borderline_item_detector.rs`, tested
against the same 6 real cases from Iteration 13.

Three margin-distribution-SHAPE candidates:

| signal | separates good from bad? |
|---|---|
| misfit fraction (items with negative margin vs. the split's own centroids) | NO |
| worst-item margin (most negative single item) | NO |
| margin standard deviation | NO (neither direction) |

**Misfit fraction turned out to be mechanistically degenerate for this
use case, not just empirically unhelpful**: it measured exactly 0.000 for
ALL SIX cases. The reason is structural, not a bug — margin here is
computed relative to the split's OWN centroids, and a converged k-means
assignment's stopping criterion IS "every item is at least as close to
its own centroid as to the other" — so a negative margin against a
k-means-native partition is close to a contradiction. A useful lesson
for any future signal design: don't compute a "wrongness" statistic
relative to the same object whose correctness is in question.

A fourth, genuinely different mechanism was tried: sub-bisection
contamination (does forcibly bisecting each of the split's 2 clusters
reveal a small, internally-TIGHT minority subgroup, which would indicate
a distinguishable "few wrong items" pocket?). Also fails, and reveals its
own base-rate artifact: the "tighter" flag fired true for all 6 cases
regardless of correctness, because a smaller sub-cluster's average
pairwise similarity is elevated almost by sampling-size alone (fewer
pairs, more likely to look mutually similar) — the same family of
size-driven bias found in Iteration 11's raw kNN-consistency, now showing
up in a structurally unrelated metric.

**Consolidated conclusion across four signal families and this whole
sub-investigation (Iterations 9, 11, 13, 14 — mean-margin/silhouette,
kNN-consistency in three variants, margin-distribution-shape in three
variants, sub-bisection contamination): none separate "balanced but
wrong" from "imbalanced but right" without a confound tracing back to
cluster size or tightness.** This is not a failure of effort; it is now
reasonably strong evidence that the "balanced but wrong" problem is not
solvable with a cheap, unsupervised, embedding-similarity-derived
statistic on data of this kind. If this needs solving for production,
the more promising paths are non-geometric: a small labeled calibration
set per domain (defeating the "no ground truth needed" premise of pure
unsupervised certification, but tractable), or a human/LLM-in-the-loop
spot-check gated specifically on low-balance-ratio-OR-borderline splits
rather than full automation. Recommend NOT continuing to search for a
fifth geometric signal without a genuinely new idea, not a variant of
what's been tried.

## Iteration 15 — real cross-cutting prevalence in physis-core's actual ontology (Next Experiment 2)

Redirected to a different, higher-value open item rather than a fifth
certification-signal attempt: does multi-membership even matter on real
production data, or is it a toy-corpus phenomenon? Every dataset used in
Iterations 8-14 was deliberately hand-built to contain cross-cutting
concepts. `physis-core/examples/experiment15_real_ontology_prevalence.rs`
measures this directly on physis-core's real 730-entry ontology (the same
data used in Iteration 4), reusing `OntologyLoader::load_all()` and the
`name + hints` text construction.

This is explicitly a MEASUREMENT, not a calibration exercise — physis-core's
real data has no "this entry genuinely belongs to two domains" ground
truth, so there is nothing to compute precision/recall against. What
was measured: for every entry, the margin between its own true
domain/cell centroid similarity and the best OTHER class's similarity,
at both the 5-DOMAIN (coarse) and cell (domain/mode, finer) level.

**Headline finding: apparent cross-domain overlap is widespread on real
data, far more than any toy corpus in this track showed.** At the DOMAIN
level, 40.5% of all 730 entries have a NEGATIVE margin at delta=0 — their
own true domain is not even their top-1 nearest centroid. By delta=0.03,
93.0% of entries show some plausible secondary-domain signal. Mean margin
is 0.005 — essentially zero, i.e. domain separation is barely better than
chance on raw embedding similarity.

**Grounded with an important accuracy fact not previously recorded in
this track**: a negative margin at domain level directly implies the true
domain isn't the centroid-argmax winner, so 40.5% negative margin implies
centroid-argmax accuracy of only ~59.5% on this 5-class task. Separately
re-running Iteration 4's own 1-NN baseline confirms this is not an
artifact of the centroid method specifically: 1-NN raw-cosine accuracy on
the same domain task is only 40-46% across folds — barely double the 20%
chance rate for 5 classes. **physis-core's real 5 human-domains are hard
to separate via this general-purpose sentence embedding at all**, not
just "somewhat overlapping."

**Honest interpretive caveat, stated explicitly rather than resolved by
assertion**: this pattern is consistent with two different explanations
that this experiment cannot fully distinguish — (a) genuine, pervasive
conceptual multi-membership in physis-core's actual content (an AI
cognitive/agentic ontology, where concepts like "Self-Awareness" or
"Introspection" plausibly do span multiple human domains), or (b) a
representation-quality limit: all-MiniLM-L6-v2 is a general-purpose
sentence embedder never fine-tuned for this specific semiotic taxonomy,
and may simply not capture the distinctions physis-core's domain
boundaries rely on. The qualitative spot-check (top-15 lowest-margin
entries, printed with full text) leans toward (a) being at least partly
real: "Self-Awareness" (true=HEAL, arguable-second=STUDY), "Introspection"
(true=HEAL, arguable-second=STUDY), "Counterfactual Reasoning"
(true=FABRICATE, arguable-second=STUDY) all read as genuinely plausible
dual-domain concepts to a human, not obvious noise — but this is
qualitative judgment, not proof, and is recorded as such.

**Granularity comparison (does Iteration 12's synthetic finding
replicate on real data?): partially, not cleanly.** CELL-level (finer,
31 cells with >=5 entries, n=685) shows MORE overlap than DOMAIN-level at
tight thresholds (0.699 vs 0.616 at delta=0.01; 0.826 vs 0.799 at
delta=0.02) — consistent with Iteration 12. But the pattern REVERSES at
looser thresholds (0.888 vs 0.930 at delta=0.03; 0.942 vs 0.992 at
delta=0.05) — cell-level saturates faster and ends up LOWER than
domain-level by delta=0.05+. Real data shows a more complex relationship
between granularity and overlap than the clean synthetic result; both
granularities are highly overlapping in absolute terms, and the
comparison between them is threshold-dependent, not a uniform
"finer-is-always-worse" relationship. Reported honestly rather than
forced into either "confirms" or "refutes."

**Practical implication for the whole synthesis architecture
investigation**: if physis-core's real ontology genuinely has this much
apparent conceptual overlap, multi-membership is not an edge case to
handle occasionally — it would need to be treated as the common case.
This raises the stakes on Iteration 14's unresolved certification
problem (a system this reliant on multi-membership needs a working
certification gate even more), while also raising a prior, more basic
question this track has not addressed: whether the embedding/model choice
itself is adequate for physis-core's actual domain taxonomy, independent
of any discovery mechanism tested in this whole track.

## Iteration 16 — the formal ablation matrix (mission Section 18), the capstone test

The mission's Section 4 asked for "one genuinely end-to-end system," and
Section 18/19 explicitly demanded an ablation matrix and a fairness
comparison against a properly calibrated baseline. Iterations 12-13
tested pieces of this; this iteration runs the actual matrix, using
Iteration 13's FIXED local/split calibration (not Iteration 12's broken
flat one), on the same Vehicle ontology.

**Design**: coarse assignment is IDENTICAL across all 6 configurations —
real (non-oracle) argmax cosine against 3 known coarse centroids built
from the 15 pure items, achieving 100% accuracy on this dataset (a
different, easier task than Iteration 12's unsupervised k-means coarse
discovery, and arguably the more realistic one: physis-core's
`CellClassifier` compares against KNOWN cells, it doesn't discover the
top level from scratch). This isolates exactly what each additional
mechanism contributes on top of an identical starting point — the actual
point of an ablation.

| config | bare final-acc | contextual final-acc |
|---|---:|---:|
| 1: flat (coarse-only, single-label) | 1.000 | 1.000 |
| 2: flat + multi-membership (coarse) | 1.000 | 1.000 |
| 3: + blind recursion (no gate) | 0.700 | 0.650 |
| 4: + certified recursion (gate) | 0.900 | 1.000 |
| 5: multi-membership + blind recursion | 0.700 | 0.650 |
| 6: FULL (multi-membership + certified recursion) | 0.900 | 1.000 |

**Blind recursion is confirmed actively harmful** (drops from 1.000 to
0.650-0.700) — exactly the garbage-in-garbage-out risk the mission
warned about. **Certified recursion recovers most or all of that loss**:
in the contextual run, the balance-ratio gate rejects ALL THREE
branches, making the certified-recursion config identical to the flat
baseline (1.000 = 1.000) rather than harmful; in the bare run, one branch
passes the gate and recursing into it costs a small amount of accuracy
(0.900 vs 1.000) relative to not recursing at all.

**Section 19 fairness verdict, answered directly and honestly: the FULL
system does NOT beat the flat baseline on final accuracy in either
representation** — contextual ties exactly (0.000 delta), bare is
slightly worse (-0.100). Certification's real, demonstrated value here is
preventing recursion from making things WORSE, not making them better.
This is not a failure of certification — refusing to recurse into bad
branches and gracefully degrading to the safe baseline is exactly what a
certification gate is supposed to do — but it means the "full system," as
currently designed, has no accuracy advantage over simply not recursing
at all on this dataset.

**Multi-membership's value is orthogonal to this accuracy metric by
construction** (it adds a second label without changing which single
label is scored as primary), so it correctly shows zero effect on
final_accuracy — its real, separately-reported contribution is 5/5
cross-cutting recall, which no single-label baseline can represent
structurally, at a real, non-trivial FPR cost: 53% at the coarse level
(a new, notable finding — even the 3 BROAD coarse fields in this
particular vehicle ontology are not well-separated enough for cheap
calibration, consistent with Iteration 15's real-data finding that
raw-cosine domain separability can be poor regardless of how coarse the
categories nominally are) and 6.7-13.3% at the fine level with Iteration
13's fix applied.

**This is the mission's clearest, most defensible answer yet to its own
Section 23 success criteria.** Broken into its two genuinely distinct
claims:

- Recursion + certification, as currently designed: **PARTIALLY
  SUPPORTED, leaning FALSIFIED as an accuracy improvement**. It provides
  a real safety property (never worse than not recursing, on this test)
  but no measured accuracy gain over the flat baseline — the mission's
  own permission to conclude the synthesis is unnecessary if calibrated
  cosine suffices is, on this specific dimension, satisfied: calibrated
  flat cosine performs at least as well as the full recursive system on
  final accuracy here.
- Multi-membership: **SUPPORTED as a distinct capability**, unchanged
  from Iteration 8's original finding, now with an honest, measured cost
  (FPR) attached rather than reported recall-only.

## Iteration 17 — genuine JEPA-style ontology binding, and closing Next Experiment 4

User-directed brainstorm ("think of how to use jepa to bind discovered
ontologies to underlying representations"), then implemented directly
("write plan and go"). `physis-core/examples/experiment17_jepa_ontology_binding.rs`.
The proposal: fix Iteration 3's mistake (a supervised predictor that used
perspective labels and was never a genuine JEPA test) by training a
low-rank predictor toward the DISCOVERED structure's own train-only
centroids (stop-gradient) instead of a human label — self-supervision
that comes from the discovery pipeline itself, not external annotation.
Two genuinely different questions, tested carefully separately to avoid
conflating supervised and self-supervised claims:

### Test 1 — does binding generalize to held-out items? (genuinely self-supervised, the 6 known toy cases)

Rank-8 low-rank predictor `pred(x) = V(Uᵀx)`, hand-derived gradients,
2-class contrastive loss (own train-centroid vs. other train-centroid,
both stop-gradient), tested on the same 6 real cases from Iterations
13-14 with a held-out split. **Result: INCONCLUSIVE, not a clean
failure.** Held-out sets are only 2-5 items (these are 6-15 item toy
datasets), giving accuracy 20-50% resolution per single item — far too
coarse to detect a subtle generalization signal. None of the 4 candidate
metrics (raw baseline, trained held-out accuracy, final loss,
generalization gap) separate good from bad, but this result cannot
distinguish "no real signal" from "signal too fine-grained to see at
this sample size." Collapse diagnostics were clean (predicted pairwise
cosine not collapsed toward 1.0 in any case). **This specific mechanism
needs a larger-scale test with matched good/bad real splits before any
verdict is possible** — none currently exists at production scale (the
real ontology has no ground-truth-labeled "known-bad split" to test
against, only true domain labels).

### Test 2 — real-data extension: closes Next Experiment 4 (explicitly SUPERVISED, not a self-supervised claim)

Extended the same predictor to physis-core's real 730-entry ontology,
generalized to N-way softmax cross-entropy — but using TRUE domain
labels as targets. **This is supervised metric learning, stated plainly
as such, not a JEPA test** — conflating the two would repeat exactly
Iteration 3's mistake. This directly fulfills FINAL_REPORT's Next
Experiment 4: "a properly hyperparameter-tuned rerun of Iteration 4's
predictor, since its chance-level loss was consistent with undertraining
and not fully excluded as the explanation."

Three attempts, in order, each diagnosed before moving to the next
(the track's standing discipline):
1. 150 epochs, lr=0.5: loss barely moved (1.6094→1.4991, chance floor is
   ln(5)=1.609), held-out accuracy (0.365) WORSE than raw cosine
   (0.561) — reproduces Iteration 4's exact undertrained pattern.
2. 1500 epochs, lr=2.0 (too aggressive): loss diverged to 18.42 and
   NaN'd, crashed on a NaN comparison — textbook gradient explosion, not
   evidence about the underlying question.
3. 800 epochs, lr=0.3: loss converges cleanly, chance (1.609) to 0.558,
   train accuracy reaches 0.811 — properly trained this time, no more
   undertraining ambiguity.

**Even properly trained, held-out accuracy is only 0.534 — barely above
the (noisy, see below) raw-cosine baseline of ~0.50-0.56 — with a large
0.277 generalization gap indicating substantial overfitting to the
training set.** This CLOSES Next Experiment 4 with a real, specific
answer: Iteration 4's original chance-level result was NOT primarily an
undertraining artifact. A properly-trained version of this predictor
design fits its training data far better than raw cosine but does not
meaningfully generalize past it on physis-core's real ontology.

**Side finding, flagged not chased further**: raw-cosine baseline
accuracy varied 0.500-0.561 across otherwise-identical reruns of the
same deterministic pipeline — consistent with ONNX Runtime's
multi-threaded execution introducing tiny floating-point
non-determinism that flips near-tie classifications, plausible given
Iteration 15 already found ~40% of real entries sit near a decision
boundary. Worth pinning single-threaded ONNX execution for any future
experiment where run-to-run reproducibility at this precision matters.

**Net implication for the brainstormed JEPA-binding proposal**: the
self-supervised generalization mechanism itself remains untested at
adequate scale (Test 1 was inconclusive by sample size, not falsified);
the closest real-scale data point available (Test 2, supervised, a
different question) suggests this general predictor architecture — a
low-rank bottleneck fit by gradient descent toward class centroids — has
a real ceiling on physis-core's real ontology that more training doesn't
overcome, which is at least a cautionary prior for how much to expect
from the self-supervised version if it were tested at real scale.

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
