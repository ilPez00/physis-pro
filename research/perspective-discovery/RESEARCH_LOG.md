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

Flagged, not attempted, in the interest of not overclaiming readiness:

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
