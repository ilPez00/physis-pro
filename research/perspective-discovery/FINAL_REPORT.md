# Final report — perspective-conditioned ontology discovery + JEPA

Seven iterations, two datasets, real production data, and one caught NaN
bug later. This is the mission's Section 31 report. All raw code, commits,
and the full iteration-by-iteration reasoning are in `RESEARCH_LOG.md` in
this directory and on branch `research/perspective-discovery-v2`
(physis-pro) / `master` (physis-core, commits `e91aaab`..`17369ef`); this
document is the synthesis, not a replacement for that trail.

## Executive conclusion

The hypothesis, as stated, is **FALSIFIED for the two concrete mechanisms
tested** (geometric reference-conditioning; a JEPA-style trained
predictor), **with one PARTIALLY SUPPORTED exception** (recursion, tested
independently of the falsified mechanisms) **and one well-evidenced
finding the mission did not ask about but that turned out to dominate
every other result** (input-representation quality). Broken out by
sub-claim, because a single global label would hide the real structure of
what was learned:

| sub-hypothesis | verdict | evidence |
|---|---|---|
| Δ(x,r) magnitude reveals reference-conditioned structure | **FALSIFIED** | proven mathematically identical to cosine ranking for unit vectors (Iteration 1) |
| Δ-direction clustering beats naive similarity search | **FALSIFIED** | loses on mean F1 at both granularities, n=21, both label granularities (Iteration 2) |
| a trained predictive latent (JEPA-style) beats raw cosine | **FALSIFIED at scale** | wins on a 21-item toy corpus only via a supervision confound (ruled out as memorization via a shuffled-label control, Iteration 3); loses badly on 730 real ontology entries, converging to chance-level training loss (Iteration 4) |
| recursive exploration discovers nested structure | **PARTIALLY SUPPORTED** | real, above-chance signal (0.625-0.75 purity) when recursing from a correct top-level grouping; dominated by error propagation when the top level is wrong (Iteration 7) |
| language/context matters for discoverable structure | **PARTIALLY SUPPORTED, non-uniform** | dramatic effect on one corpus (+0.333 k-means purity), zero effect on a second (+0.000) — conditional on how well-represented the bare vocabulary already is (Iterations 5-6) |

**The single most useful thing this research track produced is not a
mechanism — it's a warning against curve-fitting to one dataset that the
mission itself explicitly asks for (Section 27) and that this track's own
first four iterations came close to violating** before Iterations 5-6
caught it: every mechanism that looked good on the CAR toy corpus either
failed to replicate at scale (JEPA) or turned out to depend more on how the
input was represented than on the mechanism itself (reference
conditioning, recursion). No mechanism tested here should be merged into
production discovery as-is.

## What changed (code)

Nothing in production code. Everything is additive, in `examples/` on a
dedicated research branch, exactly per mission Section 2's instruction to
keep production changes minimal until a hypothesis survives controlled
experimentation — none did, unconditionally.

- `physis-core/examples/perspective_discovery_experiment.rs` (Iteration 1)
- `physis-core/examples/perspective_discovery_experiment2.rs` (Iteration 2)
- `physis-core/examples/perspective_discovery_experiment3.rs` (Iteration 3)
- `physis-core/examples/ontology_metric_learning_experiment.rs` +
  `ontology_recon.rs` (Iteration 4)
- `physis-core/examples/perspective_discovery_experiment5_context.rs`
  (Iteration 5)
- `physis-core/examples/experiment6_dataset_a_hierarchy.rs` (Iteration 6)
- `physis-core/examples/experiment7_recursion.rs` (Iteration 7)
- `research/perspective-discovery/RESEARCH_LOG.md` (this track's full
  reasoning trail) + this file

## Existing architecture (what discovery in physis-core actually is today)

Traced before writing any code (Iteration 1 reconnaissance). Two facts
shape everything above:

1. **The dependency-light default embedder is character n-gram hashing,
   not semantics.** Every experiment used the real, already-present
   `models/model.onnx` (all-MiniLM-L6-v2) instead — a decision made once,
   at the start, and never revisited, since re-litigating it per
   experiment would have been exactly the kind of moving-the-goalposts the
   mission warns against.
2. **`discovery.rs`'s existing "ontology discovery" is coverage-gap
   detection against a fixed 70-cell grid** (score against the nearest of
   70 known cells; cluster what falls below threshold; propose new grid
   entries), not the open-ended nearest-neighbor-to-DAG system the
   mission's baseline diagram assumes. There was no reference-conditioned
   or recursive discovery mechanism anywhere in the codebase to build on;
   everything in this track is new, evaluated against a from-scratch
   "ordinary discovery" baseline (plain k-means / 1-NN on raw embeddings),
   not against `discovery.rs` itself.

## Algorithm (reference-conditioned discovery, as actually tested)

Two distinct mechanisms, both eventually falsified as improvements:

- **Δ-direction clustering**: compute `Δ(x,r) = embed(x) - embed(r)` for
  every candidate x against reference r, cluster the delta vectors by
  `cos(Δ(x,r), Δ(y,r))` (not by `cos(x,y)` and not by `cos(x,r)` — the one
  formulation not reducible to a single-point similarity search), assign
  "r's field" as the cluster containing r's own nearest raw neighbor
  (fixing a real bug found in Iteration 1: picking the *largest* cluster
  instead let a near-total-corpus blob "win" with a false 1.000 score).
- **JEPA-style predictor**: `pred(r) = V(Uᵀr)`, a rank-K asymmetric
  low-rank map, trained by hand-derived gradient descent on a
  multi-positive InfoNCE loss (stop-gradient on the target side — the
  actual JEPA/BYOL-style design choice that motivated the name). Evaluated
  with strict leave-one-out (toy data) or stratified k-fold (real data)
  cross-validation throughout — never trained and evaluated on the same
  items.

## Recursive mechanism (mission Steps 8-9)

Built on plain k-means, deliberately not on either mechanism above:
recursing something already shown not to beat its own baseline would only
propagate a losing move one level deeper, not test the recursive
hypothesis. Two-level recursion (coarse split, then split within each
coarse cluster) on Dataset A's real animal taxonomy, which has genuine,
independently-verifiable two-level ground truth (unlike Dataset B's
perspectives, which are not hierarchical). Separated REAL (recurse into
whatever the top level actually found — dominated by top-level error, a
13-vs-2 split instead of the true ~8-vs-7) from ORACLE (recurse into the
*true* top-level groups — 0.625-0.75 purity, clearly above chance). No
exploration-policy, beam-width, or novelty-threshold controls (mission
Section 9-10) were built — recursion here is a fixed two-level pass, not
an open-ended explorer, since the underlying per-step mechanism's
reliability had to be established first and only turned out to be
conditional.

## JEPA experiment

See Algorithm above and Iterations 3-4 in the log. Two negative controls
were run specifically to avoid reporting a false positive: a shuffled-label
control (Iteration 3, rules out pure memorization: 0.254 F1, at/below
chance) and a training-loss/collapse diagnostic (Iteration 4, catches a
real NaN divergence before it silently produced meaningless numbers, then
confirms the fixed version isn't collapsing — pairwise cosine 0.577 vs raw
embeddings' 0.775, i.e. more differentiated, not degenerate — while still
converging to essentially the chance-level loss floor). The mechanism is
not proven impossible; it is proven, twice, not to work at the
hyperparameters and scale tested, with the training dynamics ruling out
the two most likely "it's secretly broken" explanations (memorization,
collapse) rather than leaving them open.

## Benchmarks (datasets actually built — 2 of the mission's proposed 6)

- **Dataset B** (mission's own example): CAR, 21 items, 3 coarse
  perspectives (mechanical/driver/economic) x 2 fine sub-groups each = 6
  fine labels. Used for Iterations 1, 2, 3, 5.
- **Dataset A** (mission's own example): 15 animals, 2 coarse classes
  (mammal/bird) x 2 fine sub-groups each = 4 fine labels, genuine
  taxonomic hierarchy. Used for Iterations 6, 7.
- **Real production data**: physis-core's actual 70-cell ontology, 730
  entries, used in Iteration 4 — the only benchmark in this track that is
  not a hand-built toy corpus.
- **Not built**: Dataset C (distractors), D (cross-cutting concepts — the
  mission's own "fuel belongs to CAR, ENGINE, ECONOMICS, ENVIRONMENT"
  example was never tested), E (noisy corpus), F (deep synthetic
  hierarchy). Genuine gaps, not oversights papered over — flagged in Next
  Experiments below.

## Results

Full tables are in `RESEARCH_LOG.md` per iteration; the load-bearing
numbers are in the Executive Conclusion table above and are not repeated
here to avoid two documents drifting out of sync.

## Ablations (mission Section 19)

Compiled from comparisons already run (see `RESEARCH_LOG.md`'s "Ablation
summary" section, added in Iteration 7) rather than re-run in isolation:
reference-conditioning on/off, recursion on/off, supervision on/off,
input-representation bare/contextual. Two genuine gaps not attempted:
reference-selection strategy (random vs. uncertainty-based vs. fixed) and
coherence/novelty/boundary-detection scoring — both presuppose a
per-reference mechanism worth gating, and the ones tested here didn't earn
that investment.

## Failure modes found (and how each was caught, not just reported)

1. **A metric that silently rewards a mechanism for doing nothing**:
   Iteration 1's first Δ-direction result scored a false 1.000 by picking
   the largest (near-total-corpus) cluster and measuring recall with no
   precision term. Caught by inspecting the actual retrieved item names,
   not by trusting the number.
2. **A confound that makes a mechanism look like it discovered something
   it was actually just told**: Iteration 3's JEPA predictor "won" by
   0.952 vs 0.587 using direct supervision on the evaluation labels.
   Caught by asking what question the number actually answers, then
   running a shuffled-label control to confirm it wasn't pure memorization
   either.
3. **A numerical divergence that produces plausible-looking garbage**:
   Iteration 4's NaN weights made every 1-NN comparison silently fall back
   to index 0 (`NaN > x` is always `false` in IEEE754/Rust), producing
   accuracy numbers that reflected class-frequency priors, not the model.
   Caught by a diagnostic (loss, output norm, pairwise cosine) added
   *before* trusting the accuracy number, not after something looked odd.
4. **A single-corpus finding generalized too early**: Iteration 5's
   dramatic context effect (+0.333 purity) did not replicate on a second,
   structurally different corpus (+0.000, Iteration 6). Caught by
   deliberately re-running on different samples rather than writing up the
   first result as a general claim.
5. **A summary statistic hiding a degenerate split**: Iteration 7's
   depth-1 k-means purity (0.667) looked like "a reasonable 2-way split"
   but was a 13-vs-2 imbalance, not the true ~8-vs-7. Caught by printing
   cluster sizes, which nothing in Iterations 5-6 had done.

## Computational cost

Everything here runs in seconds to low minutes on CPU with a 90MB
sentence-transformer (mission Section 15's "start small" followed
throughout — no GPU, no large model, ever needed or used). The real
ontology experiment (Iteration 4, 730 items, 5-fold CV, rank-16 predictor,
150 epochs/fold) is the heaviest run in the track and still completes in
well under a minute in release mode. Cost was never the bottleneck for any
result in this track; correctness of the evaluation was.

## Recommended architecture

**None of the tested mechanisms should become part of physis-core's
production discovery path.** The one component with genuine, real,
non-artifactual signal — plain unsupervised clustering (k-means) — is
already representable with existing primitives and does not need new
architecture; if anything is worth prototyping next it is `discovery.rs`
itself gaining a from-scratch (non-grid-anchored) clustering mode, since
that is the one mechanism this track never found a way to make lose.

## What should NOT be merged

- Δ-direction clustering or any geometric transform of a frozen embedding
  as a "reference-conditioned discovery" primitive — falsified twice.
- The JEPA-style trained predictor as a similarity-scoring alternative to
  `CellClassifier`'s cosine-based scoring — beaten badly on real ontology
  data, and its one apparent win elsewhere was a supervision artifact.
- Any "coherence visualization" combining these signals for the Studio
  (the idea raised mid-track) — would visualize a mechanism with no
  demonstrated value as though it were validated.

## Next experiments (only if justified — none started without a reason above)

1. Datasets C/D/E/F, never built — Dataset D (cross-cutting concepts) in
   particular tests something no experiment here touched: shared/multi-
   field membership, which none of Datasets A or B's exclusive-membership
   structure could reveal either way.
2. Whether Iteration 5's context effect holds on physis-core's *real*
   ontology (Iteration 4's task uses `name + hints`, shorter than a full
   sentence) — untested combination.
3. A properly hyperparameter-tuned (validation-split-gated, not
   toy-corpus-carried-over) rerun of Iteration 4's predictor, since its
   chance-level loss is consistent with undertraining and not fully
   excluded as the explanation for its loss.
4. A genuinely self-supervised JEPA test (predict context from natural
   co-occurrence, no category labels anywhere) — needs a corpus with real
   contextual structure, which none built in this track provide; not
   attempted because building one properly is a materially larger
   undertaking than any single iteration here, not because it was judged
   unpromising.
