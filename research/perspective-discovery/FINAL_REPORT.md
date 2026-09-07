# Final report — perspective-conditioned ontology discovery + JEPA

> ## ⚠️ READ STAGE 7 BEFORE ANY VERDICT BELOW
>
> **Everything in this document that was evaluated against the 5×14 grid's cell
> assignments is provisional.** Stage 7 (2026-09-07) adjudicated a deterministic
> sample of the shipped corpus and found **43.3% of entries clearly filed in the
> wrong cell**. Verdicts that treated those assignments as ground truth were
> measured against labels that are ~43% wrong.
>
> **The triage principle is that label noise attenuates effects; it does not
> fabricate them.** That makes the two directions of result fail differently:
>
> - **Negative verdicts below are UNSAFE.** "NOT SUPPORTED" against a 43%-wrong
>   ground truth may mean the mechanism failed, or may mean the experiment
>   could not see it. These need re-testing and are listed in Stage 8.
> - **Positive results that survived controls are CONSERVATIVE.** They held
>   *despite* the noise, which can only have weakened them. They stand, and
>   several are probably understated.
>
> Nothing here is retracted for being wrong. It is marked for being **untested**.


Eight iterations, three datasets, real production data, and one caught NaN
bug later. This is the mission's Section 31 report. All raw code, commits,
and the full iteration-by-iteration reasoning are in `RESEARCH_LOG.md` in
this directory and on branch `research/perspective-discovery-v2`
(physis-pro) / `master` (physis-core, commits `e91aaab`..the Dataset D
commit); this document is the synthesis, not a replacement for that trail.

## Executive conclusion

The hypothesis, as stated, is **FALSIFIED for the two concrete mechanisms
tested on exclusive-membership data** (geometric reference-conditioning; a
JEPA-style trained predictor), **PARTIALLY SUPPORTED for recursion**
(tested independently of the falsified mechanisms), **PARTIALLY SUPPORTED,
non-uniformly, for input-representation quality** (a finding the mission
did not ask about but that dominated the early iterations), **and
SUPPORTED for one specific, narrower claim tested last**: that
reference-conditioned (non-hard-clustering) representations can carry
real multi-field membership signal that a single-cluster assignment
structurally cannot express. Broken out by sub-claim, because a single
global label would hide the real structure of what was learned:

| sub-hypothesis | verdict | evidence |
|---|---|---|
| Δ(x,r) magnitude reveals reference-conditioned structure | **FALSIFIED** | proven mathematically identical to cosine ranking for unit vectors (Iteration 1) |
| Δ-direction clustering beats naive similarity search (exclusive membership) | **FALSIFIED** | loses on mean F1 at both granularities, n=21 (Iteration 2) |
| a trained predictive latent (JEPA-style) beats raw cosine | **FALSIFIED at scale** | wins on a 21-item toy corpus only via a supervision confound (ruled out as memorization via a shuffled-label control, Iteration 3); loses badly on 730 real ontology entries, converging to chance-level training loss (Iteration 4) |
| recursive exploration discovers nested structure | **PARTIALLY SUPPORTED** | real, above-chance signal (0.625-0.75 purity) when recursing from a correct top-level grouping; dominated by error propagation when the top level is wrong (Iteration 7) |
| language/context matters for discoverable structure | **PARTIALLY SUPPORTED, non-uniform** | dramatic effect on one corpus (+0.333 k-means purity), zero effect on a second (+0.000) — conditional on how well-represented the bare vocabulary already is (Iterations 5-6) |
| per-field similarity carries real multi-membership signal hard clustering cannot express | **SUPPORTED, with a measured cost** | 4/4 then 5/5 cross-cutting items recovered across two datasets (Iterations 8, 12-13); costs a real, non-trivial false-positive rate (6.7-53% depending on category granularity and calibration strategy) that must be weighed against the benefit, not assumed free |
| the full multi-membership + certification + certified-recursion system beats a properly calibrated flat baseline on final accuracy | **PARTIALLY SUPPORTED, leaning FALSIFIED as an accuracy claim** | formal ablation matrix (Iteration 16): certified recursion never beats the flat baseline on this dataset (ties in one representation, -0.100 in the other) — its real, demonstrated value is refusing bad recursions and gracefully degrading to the baseline, not improving on it; blind (uncertified) recursion is confirmed actively harmful (-0.30 to -0.35) |
| a cheap, unsupervised, embedding-similarity-derived certification signal can catch a well-balanced but label-wrong split | **FALSIFIED for every signal tried** | four structurally distinct signal families (mean-margin, kNN-consistency, margin-distribution-shape, sub-bisection contamination — Iterations 9, 11, 13, 14) all fail, each on a confound tracing back to cluster size or tightness; recommend a non-geometric approach (labeled calibration set, human/LLM review) instead of a fifth metric |

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
- `physis-core/examples/experiment8_dataset_d_crosscutting.rs` (Iteration 8)
- `physis-core/examples/experiment9_certification_gate.rs` (Iteration 9)
- `physis-core/examples/experiment10_integration.rs` (Iteration 10)
- `physis-core/examples/experiment11_generalized_certification.rs` (Iteration 11)
- `physis-core/examples/experiment12_integrated_synthesis.rs` (Iteration 12)
- `physis-core/examples/experiment13_local_calibration_and_ensemble_gate.rs` (Iteration 13)
- `physis-core/examples/experiment14_borderline_item_detector.rs` (Iteration 14)
- `physis-core/examples/experiment15_real_ontology_prevalence.rs` (Iteration 15)
- `physis-core/examples/experiment16_ablation_matrix.rs` (Iteration 16)
- `physis-core/examples/experiment17_jepa_ontology_binding.rs` (Iteration 17)
- `physis-core/examples/experiment18_multi_embedder_comparison.rs` (Iteration 18)
- `physis-core/examples/experiment19_cross_embedder_corroboration.rs` (Iteration 19)
- `physis-core/examples/experiment20_fixed_point_discovery.rs` (Iteration 20)
- `physis-core/examples/experiment21_fixed_cell_linkage.rs` (Iteration 21)
- `physis-core/examples/experiment22_wordnet_discovery.rs` (Iteration 22)
- `physis-core/examples/experiment23_wordnet_disambiguated.rs` (Iteration 23)
- `physis-core/examples/experiment24_cross_embedder_split_agreement.rs` (Iteration 24)
- `physis-core/examples/experiment25_capacity_constrained_loss_gate.rs` (Iteration 25)
- `physis-core/examples/experiment26_persistent_registry.rs` (Iteration 26)
- `physis-core/examples/probe_embedding_determinism.rs` (Iteration 26,
  the probe that disproved this track's ONNX-nondeterminism claim)
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

## Benchmarks (datasets actually built — 3 of the mission's proposed 6)

- **Dataset B** (mission's own example): CAR, 21 items, 3 coarse
  perspectives (mechanical/driver/economic) x 2 fine sub-groups each = 6
  fine labels. Used for Iterations 1, 2, 3, 5.
- **Dataset A** (mission's own example): 15 animals, 2 coarse classes
  (mammal/bird) x 2 fine sub-groups each = 4 fine labels, genuine
  taxonomic hierarchy. Used for Iterations 6, 7.
- **Dataset D** (mission's own example, "fuel belongs to CAR, ENGINE,
  ECONOMICS, ENVIRONMENT"): the 21-item CAR corpus plus 4 deliberately
  cross-cutting items spanning two fields each. Used in Iteration 8 — the
  one dataset in this track that produced a clean positive result, because
  it is the one dataset whose structure (multi-membership) hard clustering
  cannot represent by construction, unlike A and B's exclusive membership.
- **Real production data**: physis-core's actual 70-cell ontology, 730
  entries, used in Iteration 4 — the only benchmark in this track built
  from real product data rather than a hand-built toy corpus.
- **Not built**: Dataset C (distractors), E (noisy corpus), F (deep
  synthetic hierarchy). Genuine gaps, not oversights papered over — flagged
  in Next Experiments below.

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
6. (Not a failure, included for symmetry with the above): **Iteration 8's
   positive result was checked for the inverse problem** — a metric that
   looks good because it's trivially satisfiable. The "dual margin" metric
   requires beating the *unrelated* field, not just scoring positively in
   absolute terms, and was computed against centroids built only from the
   21 exclusive-membership items (the 4 cross-cutting items never
   contribute to the centroids they're scored against) — ruling out the
   analogous confound to Iteration 3's supervision leak before reporting
   4/4 as a real result.

## Computational cost

Everything here runs in seconds to low minutes on CPU with a 90MB
sentence-transformer (mission Section 15's "start small" followed
throughout — no GPU, no large model, ever needed or used). The real
ontology experiment (Iteration 4, 730 items, 5-fold CV, rank-16 predictor,
150 epochs/fold) is the heaviest run in the track and still completes in
well under a minute in release mode. Cost was never the bottleneck for any
result in this track; correctness of the evaluation was.

## Recommended architecture

**No falsified mechanism (Δ-direction, the trained predictor) should
become part of physis-core's production discovery path.** Two things ARE
worth carrying forward, both low-risk and both already close to existing
primitives rather than requiring new architecture:

1. ~~**Plain unsupervised clustering**~~ — **FALSIFIED (Iteration 28).**
   This was recommended as "the one mechanism this track never found a
   way to make lose on exclusive-membership data". It was also the only
   mechanism never prototyped, so that standing was never earned against
   real data. Iteration 28 tested it directly: on the real 730-entry
   corpus, oracle k=5 gives ARI 0.022 against a random-partition floor of
   0.003, and no k in 2..20 exceeds ARI 0.041. Even the 15-item control
   set the claim came from yields ARI 0.051 and a split that is not the
   taxonomy. **Do not prototype this in `discovery.rs`** on the strength
   of this report — see roadmap item 24.
2. **Surfacing `CellClassifier`'s existing per-cell similarity scores
   instead of collapsing to one winning `(domain, mode)` per entry** —
   directly motivated by Iteration 8. `OntologyEntry` (`src/models.rs`)
   stores exactly one domain and one mode per entry; Test 1 in Iteration 8
   demonstrates that this loses real information for any concept that
   genuinely spans two fields, and Test 2 shows the embedding space
   already carries that information — `classify.rs` computes per-cell
   scores internally before picking a winner, so this is closer to
   *exposing* an existing signal than building a new one. Not implemented
   here — a scoped follow-up, not a code change this track made.

**Iterations 9-10, prompted directly by a user proposal to combine
multi-membership + coherence gating + certified recursion, checked three
things before that combination could be recommended as a system, not just
as three separately-validated pieces:**

- A cheap **certification signal for recursion** exists and is validated:
  size-balance ratio (min/max cluster size) correctly ranks Iteration 7's
  known-good 8-vs-7 split above its known-bad 13-vs-2 split (0.875 vs
  0.154). The naive alternative — silhouette-style cohesion-minus-
  separation scoring — gets it **backwards** (0.048 vs 0.089) because a
  large wrong cluster's centroid drifts toward the corpus average and
  looks deceptively cohesive. Use balance ratio; do not use naive
  cohesion scoring as a gate.
- Multi-membership needs **zero schema changes** to represent today —
  `CoherenceNode.properties` (restored in PH-014 as an open extension
  point) can hold a full per-cell membership vector; a literal graph
  "member-of" edge would need one new `RelationType` variant if graph
  traversal over memberships is wanted later.
- Run together on one 19-item corpus with real hierarchy AND
  cross-cutting structure (Iteration 10): the balance-ratio gate
  correctly rejects two bad automatic splits on real data, multi-
  membership correctly produces cross-branch DAG links that are real, not
  hypothetical, and a visited-node cache is **confirmed required** —
  naive expansion of those cross-links does not terminate. But the gate
  also lets through one split that is well-balanced (0.667) and still
  **wrong** (mislabels two flightless birds as flying) — balance checks
  size, not correctness, and nothing tested in this track catches that
  failure mode yet. The threshold calibrated for multi-membership
  (delta=0.03: zero false positives on 15 pure items, full recall on 4
  cross-cutting items) is usable but not crisp — cross items average 2.76
  memberships, not 2.0.

**Net verdict on the proposed synthesis architecture: the pieces compose
without a fundamental incompatibility, and each validated component earns
its keep on real combined data. It is not yet ready to recommend as
production code** — the open problem is a certification signal that
catches "balanced but wrong," not just "imbalanced."

**Iteration 11, directly hunting for that signal**: built a proper 2x2
(correct x balanced) with real/established ground truth in every cell —
every prior comparison had varied only one axis at a time. Five candidate
signals tested (balance ratio, silhouette-like, raw kNN-consistency,
size-adjusted kNN-consistency at fixed k, the same adjusted for adaptive
per-item k), plus an epsilon sweep for confident-item fraction rather
than an assumed threshold. **None cleanly separate correct from wrong
independent of balance.** Adaptive+size-adjusted kNN-consistency is the
closest: it is the only signal that correctly demotes the classic
bad-imbalanced case (13-vs-2) to the lowest score of all four (fixing a
diagnosed base-rate bias in the raw version), but it still scores
Iteration 10's balanced-but-wrong bird split (0.200) above a
deliberately-imbalanced-but-correct vehicle split (0.164) — because that
"wrong" split is only 20% wrong (2 of 10 items mislabeled, the rest
locally correct), while the correct vehicle split's ground-truth axis
(wheel count) is not necessarily the dominant semantic gradient the
embedding space encodes. **No single embedding-similarity signal found in
this track resolves the open problem; recommend an ensemble (this signal
combined with balance ratio, or a dedicated few-borderline-items
detector) as the next thing to try, not a from-scratch sixth candidate.**

**Iteration 12, the full end-to-end synthesis mission**: the user provided
a comprehensive 24-section mission prompt treating Iterations 1-11 as
established history and asking for one genuinely integrated test — three
calibrated baselines (raw cosine, flat hard k-means with ARI/NMI, and
critically a calibrated multi-membership-cosine "fairness" baseline)
against the full pipeline, on a NEW 20-item Vehicle ontology (3 coarse x
2 fine fields, 5 new cross-cutting concepts covering all 3 coarse pairs)
built specifically so no prior dataset was reused. Two implementation
bugs were caught and fixed before trusting results (a coarse/fine
ground-truth mismatch that silently zeroed recall, and a calibration
fallback that picked the worst- instead of best-FPR delta among ties) —
consistent with this track's practice of not reporting a number before
verifying the code producing it.

Two results stand out. First, **multi-membership calibration degrades
sharply with category granularity**: Dataset D's 3 coarse fields
(Iteration 10) calibrated cleanly at FPR=0.0; this dataset's 6 finer
sibling categories (e.g. Propulsion vs. Energy, both under Mechanical)
sit close enough in embedding space that achieving full cross-item
recall costs FPR=0.267-0.533 — a quarter to over half of genuinely
single-membership items get spuriously flagged. This is a real, newly
identified boundary condition: recursion inherently produces
increasingly fine-grained sibling categories at each depth, so
calibration should be expected to get harder, not easier, deeper into a
recursion tree. Second, **a third independent "balanced but wrong"
split** surfaced unprompted (the bare economic branch: balance=0.500
passes the Iteration-9 gate threshold exactly, with ARI=-0.250, worse
than random) — the same failure mode Iterations 10-11 already
identified and tried to fix, now confirmed on a completely
independently-designed dataset, which is stronger evidence than either
prior instance alone that this is a structural property of
balance-based certification rather than a one-off. The DAG
visited-cache requirement (Iteration 10) was also independently
reconfirmed on this third dataset.

**Net assessment, unchanged in direction but now better evidenced**: the
synthesis architecture remains coherent and each piece independently
useful, but two concrete, now multiply-confirmed open problems block a
production recommendation — a certification signal robust to
balanced-but-wrong splits (Iteration 11, unresolved), and a calibration
strategy that doesn't degrade as recursion produces finer-grained
siblings (Iteration 12, newly identified, unaddressed). Per the mission's
own Section 19 permission to conclude the synthesis is unnecessary if
calibrated cosine already suffices: it does not straightforwardly
suffice either — Baseline C cannot even ask the question the full
pipeline answers (discovering fine cells that do not yet exist), so
there is no clean "baseline wins" verdict, only the two specific
technical gaps above.

**Iteration 13 directly attempted both gaps.** Result: one closed, one
confirmed still open, not just suspected.

*Calibration-degrades-with-granularity (closed)*: splitting Iteration
12's single flat delta into two separately-calibrated checks — same-
branch sibling confusion (needs a tight threshold, delta=0.00 achieves
FPR=0.0 here since this dataset has no genuine same-branch cross-cutting
pair to trade off) and cross-branch detection (can tolerate a looser
threshold since different-domain categories sit further apart) — cut
FPR from 0.267/0.533 (bare/contextual) to ≤0.067/≤0.133 while preserving
full 5/5 recall in both. **Recommend this split-calibration strategy for
any production implementation.**

*Balanced-but-wrong (still open, now more rigorously confirmed)*: an
ensemble gate (balance ratio AND Iteration 11's adaptive kNN-consistency)
was tested against an exhaustive threshold sweep over all 6 known real
cases spanning five iterations. No combination achieves perfect
separation; best possible is 5/6, and the failure is structural — the
knn threshold needed to reject both known balanced-but-wrong cases
(knn≈0.200) also rejects the deliberately-imbalanced-but-correct control
(knn=0.164, which is LOWER than the wrong cases it would need to sit
above). This is not an ensemble-tuning problem; it means kNN-consistency
and balance ratio jointly carry no signal that separates "mostly right, a
few items wrong" from "genuinely correct but on an axis the embedding
doesn't foreground" — a materially different, more specific signal is
needed, not a combination of what exists.

**Iteration 14 then targeted the wrong cases' specific character
directly** — three margin-distribution-shape signals (misfit fraction,
worst-item margin, margin std-dev) plus a structurally different
sub-bisection-contamination signal, all tested against the same 6 real
cases. All four fail. Misfit fraction is additionally shown to be
mechanistically degenerate for this use case (measuring exactly 0.000 on
every case): a margin computed against a split's OWN centroids can
barely ever go negative for a converged k-means assignment, since
non-negative margin IS what k-means's convergence criterion enforces.
Sub-bisection contamination fails via its own base-rate artifact
(smaller sub-clusters look artificially "tighter" almost by sampling
size alone). **Four structurally distinct signal families have now been
tried (Iterations 9, 11, 13, 14) and all fail on a confound tracing back
to cluster size or tightness — reasonably strong evidence this specific
problem is not solvable with a cheap unsupervised geometric statistic on
data of this kind.** Recommend a non-geometric path instead (a small
labeled calibration set, or human/LLM review gated on low-confidence
splits) rather than continuing to search for a fifth metric.

**Iteration 15 redirected to a different, higher-value open question**:
does multi-membership even matter on physis-core's real data, or is it a
toy-corpus artifact? Measuring (not calibrating — no ground truth exists)
on the real 730-entry ontology: 40.5% of entries show negative margin at
the 5-domain level, implying centroid-argmax accuracy of only ~59.5%;
independently, Iteration 4's own 1-NN baseline on this exact task
achieves only 40-46% (barely double the 20% chance rate for 5 classes).
Real cross-domain overlap is far more widespread than any hand-built
toy corpus in this track showed. Whether this reflects genuine conceptual
multi-membership or a representation-quality limit of a general-purpose
embedder on this specific taxonomy cannot be fully disambiguated here —
the qualitative spot-check (entries like "Self-Awareness" and
"Introspection" plausibly spanning HEAL/STUDY) leans toward at least
partly genuine, but this is judgment, not proof. The domain-vs-cell
granularity comparison from Iteration 12 partially, not cleanly,
replicates: cell-level shows more overlap at tight thresholds but
crosses over to less at looser ones — real data shows a more complex
relationship than the clean synthetic result. **Practical implication**:
if this level of overlap is real, multi-membership would need to be
treated as the common case in physis-core's actual ontology, not an edge
case — raising the stakes on Iteration 14's unresolved certification gap,
and surfacing a more basic, previously untested question this track does
not answer: whether the embedding model itself is adequate for
physis-core's actual domain taxonomy, independent of any discovery
mechanism.

**Iteration 16 ran the mission's own capstone experiment: the formal
ablation matrix (Section 18), finally answering Section 19's fairness
rule directly rather than by argument.** Using Iteration 13's fixed
local/split calibration, with coarse assignment held IDENTICAL across
all 6 configurations (real, non-oracle argmax cosine against known
coarse centroids — 100% accurate on this dataset, a different and easier
task than Iteration 12's unsupervised k-means coarse discovery, and the
more realistic analog of how `CellClassifier` actually works against
known cells):

- Blind (uncertified) recursion drops accuracy from 1.000 to 0.650-0.700
  — confirmed actively harmful, exactly the risk the mission warned
  about.
- Certified recursion recovers most or all of that loss: in the
  contextual run the gate rejects all three candidate branches, making
  the certified system IDENTICAL to the flat baseline (1.000=1.000)
  rather than worse; in the bare run one branch passes and costs a small
  amount of accuracy (0.900 vs 1.000).
- **The full system (multi-membership + certified recursion) does NOT
  beat the flat baseline on final accuracy in either representation** —
  it ties in one, and is slightly worse in the other. Certification's
  demonstrated value is preventing recursion from making things worse,
  not making them better.
- Multi-membership shows zero effect on this accuracy metric by
  construction (it adds a second label without changing the scored
  primary one); its real contribution — 5/5 cross-cutting recall no
  single-label baseline can express — comes with a newly-measured 53%
  FPR at the coarse level even for this dataset's three broad fields,
  confirming Iteration 12-13's granularity finding is not solely about
  fine-grained categories: broad categories can also calibrate poorly if
  they are not well-separated to begin with.

**This is the clearest answer this track has produced to its own
Section 23 success criteria, and it is not one label but two**:
multi-membership as a standalone capability is **SUPPORTED**, with a
real cost now attached rather than reported recall-only; the full
recursive-recursion-plus-certification system, evaluated on final
accuracy against a properly calibrated baseline exactly as Section 19
demands, is **PARTIALLY SUPPORTED, leaning FALSIFIED as an accuracy
improvement** — it is safe (never demonstrably worse than not recursing,
across two representations tested) but not yet shown to be better.

**Iteration 17, user-directed, asked whether JEPA could be reframed to
bind discovered ontology structure to the embedding space, fixing
Iteration 3's mislabeling** by targeting the discovery pipeline's own
train-only centroids (stop-gradient) instead of a human label. Two
results, kept carefully separate: the genuinely self-supervised version
(targets = discovered/oracle centroids) is **INCONCLUSIVE** on the 6
known toy cases — held-out sets of 2-5 items give far too little
resolution to detect a subtle generalization signal, a sample-size limit
rather than a finding. The real-scale extension is explicitly
**supervised** metric learning (true domain labels), not a JEPA claim,
and closes Next Experiment 4: a properly-converged version (loss
1.609→0.558) still only reaches 0.534 held-out accuracy versus ~0.50-0.56
raw cosine, with a 0.277 generalization gap — the original chance-level
result was not primarily undertraining; this architecture has a real
ceiling on real data. The self-supervised binding proposal itself remains
untested at adequate scale, not falsified — see Next Experiment 5.

## What should NOT be merged

- Δ-direction clustering or any geometric transform of a frozen embedding
  as a "reference-conditioned discovery" primitive — falsified twice.
- The JEPA-style trained predictor as a similarity-scoring alternative to
  `CellClassifier`'s cosine-based scoring — beaten badly on real ontology
  data, and its one apparent win elsewhere was a supervision artifact.
- Any "coherence visualization" combining these signals for the Studio
  (the idea raised mid-track) — would visualize a mechanism with no
  demonstrated value as though it were validated. Iteration 8's positive
  result changes this only for the specific multi-membership display idea
  above, not for JEPA/Δ-conditioning-based visualizations.

## Next experiments (only if justified — none started without a reason above)

1. Datasets C/E/F, never built.
2. **DONE (Iteration 15).** Measure how common cross-cutting concepts
   actually are in physis-core's real 730-entry ontology. Finding: far
   more widespread than any toy corpus in this track — 40.5% of entries
   show negative margin at the domain level, and 1-NN accuracy on the
   same task is only 40-46% (barely double chance). Genuinely ambiguous
   whether this is real conceptual overlap or an embedding-quality limit
   (see the new item below); the qualitative spot-check leans toward at
   least partly real.
3. Whether Iteration 5's context effect holds on physis-core's *real*
   ontology (Iteration 4's task uses `name + hints`, shorter than a full
   sentence) — untested combination.
4. **DONE (Iteration 17).** A properly hyperparameter-tuned rerun of
   Iteration 4's predictor. Result: after diagnosing and fixing an
   undertrained first attempt and a diverged (too-aggressive-lr) second
   attempt, a properly converged version (loss 1.609→0.558, train
   accuracy 0.811) still only reaches 0.534 held-out accuracy — barely
   above raw cosine (~0.50-0.56) — with a 0.277 generalization gap. The
   original chance-level result was NOT primarily an undertraining
   artifact; this predictor architecture has a real ceiling on
   physis-core's real ontology that more training doesn't overcome.
5. **PARTIALLY ATTEMPTED (Iteration 17), genuinely self-supervised this
   time** (targets = discovered/oracle cluster centroids, never a human
   label) — tested for held-out generalization across the 6 known toy
   cases from Iterations 13-14. Result: INCONCLUSIVE, not falsified —
   held-out sets are only 2-5 items per case, giving far too little
   resolution to detect a subtle signal. Needs a larger-scale test with
   matched good/bad splits, which does not currently exist at production
   scale (physis-core's real ontology has true domain labels but no
   ground-truth "known-bad split" to test generalization against). A
   corpus with real contextual co-occurrence structure (the original
   framing of this item) remains a separate, larger undertaking, not
   attempted.
6. **LIKELY NOT SOLVABLE WITH A CHEAP GEOMETRIC SIGNAL — four attempts,
   four structurally distinct failure mechanisms.** A certification
   signal that catches a well-balanced but label-wrong split. Iteration 9
   tried mean-margin (silhouette-like): backwards. Iteration 11 tried
   kNN-consistency (raw, size-adjusted, adaptive-k): closest but still
   fails. Iteration 13 tried an ensemble (balance AND adaptive
   kNN-consistency) via exhaustive threshold sweep over 6 known real
   cases: best possible is 5/6, structurally blocked (the threshold that
   catches the wrong cases also rejects a genuinely correct control).
   Iteration 14 tried signals targeting the wrong cases' specific
   character directly (misfit fraction, worst-item margin, margin
   std-dev, sub-bisection contamination): all fail, with misfit fraction
   shown to be mechanistically degenerate (can't go negative for a
   converged k-means split by construction) and sub-bisection showing its
   own base-rate artifact (smaller sub-clusters look artificially
   tighter). **Recommend stopping the search for a geometric signal and
   pursuing a non-geometric path instead**: a small labeled calibration
   set per domain, or gating low-confidence splits (by balance ratio
   alone, which DOES work for the extreme-imbalance failure mode) for
   human or LLM review rather than full automatic certification.
7. **CLOSED.** A calibration strategy for multi-membership that doesn't
   degrade as sibling-category granularity increases. Iteration 13
   confirmed the cause (a single flat threshold conflates same-branch
   sibling confusion, which needs a tight threshold, with cross-branch
   detection, which can tolerate a looser one) and fixed it by splitting
   into two separately-calibrated checks: FPR dropped from 0.267/0.533
   to ≤0.067/≤0.133 (bare/contextual) at full recall. Recommended for any
   production implementation. Residual gap: this dataset happened to
   have zero genuine same-branch cross-cutting items, so the sibling
   check's recall was never tested under a real trade-off — worth
   re-testing on a dataset with at least one genuine same-branch
   cross-cutting concept before fully trusting the sibling threshold.
8. **NEW (surfaced by Iteration 15).** Whether all-MiniLM-L6-v2 is an
   adequate embedding model for physis-core's actual 5-domain semiotic
   taxonomy at all — 1-NN accuracy of 40-46% on real production data
   (barely double the 20% chance rate) is a more basic and previously
   unmeasured question than anything else in this track: before
   investing further in multi-membership/certification/recursion
   mechanisms built on top of this embedding, it's worth checking whether
   a different embedding model (e.g. a larger general-purpose model, or
   one fine-tuned/prompted for this domain) achieves meaningfully higher
   raw separability on the same 5-domain task — if so, some of Iteration
   15's "widespread overlap" finding would be a representation artifact
   rather than genuine multi-membership, changing how urgent the
   certification problem (item 6) actually is in practice.
9. **DONE (Iteration 18).** Cross-embedder real-domain separability
   comparison. Pulled BGE-base-en-v1.5's ONNX weights (previously
   absent) and tested BGE (mean AND CLS pooling — checked empirically,
   not assumed: they perform almost identically, 0.488 vs 0.489 LOO-1NN)
   against MiniLM on the identical real 730-entry task. Result: BGE wins
   consistently but modestly (LOO-1NN +0.023, centroid-argmax +0.048) —
   both still land in the same rough range (46-49% / 60-64%). A stronger
   general-purpose embedder is not the fix; physis-core's 5-domain
   taxonomy has genuine, largely model-independent conceptual overlap.
   This tempers item 12's expected payoff (see below) without ruling it
   out.
10. **NEW — jina-clip-v2 as a second embedder, explicitly higher-risk.**
    Its ONNX weights (`model_fp16.onnx`) are already present locally, but
    it is a `JinaCLIPModel` (multimodal, matryoshka embeddings), not a
    plain sentence-transformer — the existing `OnnxEmbedder` looks for a
    file literally named `model.onnx` and assumes a standard
    input_ids/attention_mask → last_hidden_state pooling convention that
    may not match CLIP's text tower. Treat as a real integration task
    (verify output shape/pooling empirically before trusting any
    embedding it produces), not a drop-in swap — lower priority than
    item 9 given BGE is architecturally simpler and lower-risk.
11. **DONE (Iteration 24) — NOT SUPPORTED, fails backwards.** Cross-
    embedder agreement as a certification signal for clustering splits.
    Result: the worst known-bad case scored ARI=1.000 (perfect
    agreement) while the best known-good case scored -0.069 (random).
    Verified the mechanism directly: both MiniLM and BGE independently
    isolate the same genuine cross-cutting item as an outlier, which is
    real signal but not evidence the 2-way split itself is meaningful;
    meanwhile BGE's own unsupervised clustering doesn't rediscover the
    true good split either, so there's nothing for it to agree with.
    Sixth structurally distinct certification-signal family to fail.
12. **Expectations tempered by Iteration 18, not ruled out.**
    Domain-specific embedding fine-tune. A legitimate supervised
    contrastive fine-tune (or lightweight adapter/projection) on
    physis-core's own (domain,mode) labels — explicitly declared
    supervised, unlike Iteration 3's mislabeled predictor. Iteration 18
    showed a strong off-the-shelf embedder (BGE) only closes a modest
    fraction of the gap, suggesting the taxonomy's overlap is largely
    embedding-independent — a fine-tune might still help more than a
    model swap (it optimizes directly for this task), but should not be
    expected to produce near-perfect separability. Do item 11 first; it
    is much cheaper and tests a more specific, more useful question.
13. **DONE (Iteration 25) — NOT SUPPORTED, and the record is corrected.**
    Iteration 17's own code already tested training loss as a
    certification signal and it failed (near-zero loss, 0.001-0.003, on
    all 6 cases) — this item incorrectly claimed it was untested; it
    was, just never written up. Iteration 25 tested the natural
    follow-up: does deliberately under-parameterizing the predictor
    (rank swept 1-8) fix it by preventing memorization? No — losses stay
    near-zero at every rank tested, including rank=1. Sharper diagnosis:
    the ambient 384-dimensional input space, not the bottleneck rank, is
    what overwhelms 4-14-item training sets; any partition of that few
    points is trivially separable along almost any direction regardless
    of bottleneck capacity. Would need far more training data or input-
    space regularization to test properly — out of scope here.
14. **NEW — close the residual gap from Iteration 13.** The local/split
    calibration fix (sibling vs. cross-branch, validated, recommended for
    production) was never tested against a real same-branch cross-cutting
    item, because none existed in the Vehicle ontology dataset — add one
    (e.g. a concept spanning Propulsion and Energy within Mechanical) and
    confirm the sibling threshold's recall works under a genuine
    trade-off, not just its zero-cost case.

**Updated priority order after Iteration 18**: item 9 is done. Next:
11 → 13 → 14 → 10 → 12. Item 11 (cross-embedder agreement as a
certification signal) now has zero setup cost — BGE weights already
exist — and directly tests the certification thread, which item 9's
result did not resolve; 13 and 14 need no new infrastructure either. Item
12 (the fine-tune) has moved down in expected payoff, not out of the
plan — item 9 showed a strong off-the-shelf embedder only closes part of
the gap, so a fine-tune's ceiling is now a more tempered bet than it
looked before this evidence existed.

15. **DONE (Iteration 19) — NOT SUPPORTED.** Cross-embedder
    corroboration as a multi-membership filter (refined from
    "disagreement" to the sharper "does BGE independently corroborate
    the SPECIFIC category MiniLM flagged"). On the Vehicle ontology
    (ground truth known): zero aggregate FPR improvement over Iteration
    13's local/split calibration, AND recall dropped from 5/5 to 4/5 in
    the contextual representation — the filter discarded a true positive
    along with noise. On the real 730-entry ontology: 92.6% of
    MiniLM-flagged entries survive BGE corroboration at delta=0.03 —
    barely filters anything, because the ambiguity is NOT primarily
    model-specific (both architecturally different models perceive
    nearly the same overlap), which is itself evidence for the
    genuine-taxonomy-overlap interpretation (reinforcing item 9's
    finding) even though it doesn't solve the FPR problem. Do not carry
    this specific corroboration design into production calibration.
    Byproduct worth keeping: the qualitative spot-check found that broad
    lifestyle/activity entries (Gaming & Esports, Music & Performance,
    Wealth & Assets) show their OWN true domain as the LEAST corroborated
    label across both models — a specific, actionable lead for item 16
    below, not a uniform ontology-wide problem.
16. **NEW, user-proposed — data-driven taxonomy expansion.** If two
    architecturally different, generally strong embedders (Iteration 18:
    MiniLM and BGE) both find the same domain pairs hard to separate,
    that is evidence about the TAXONOMY, not just the embedding. Use the
    entries with persistently negative margins ACROSS BOTH embedders
    (robust to model choice, not a MiniLM artifact) as candidates for
    where the domain/mode grid may need a new cell, a split, or a merge.
    Explicit caution, from this track's own discipline (mission Section
    20, "do not confuse semantic similarity with ontological validity"):
    this identifies WHERE a human should look, it does not decide WHAT
    the right taxonomy is — physis-core's domain boundaries may
    deliberately separate concepts that are semantically close for
    reasons embeddings can't see (downstream business logic, not
    semantic salience). Treat this as a diagnostic report to a human
    curator, not an automated taxonomy-editing pipeline. **Sharpened by
    Iteration 19's qualitative finding**: the overlap is not uniform
    across the ontology — broad lifestyle/activity entries (gaming,
    music, investing, hobbies) show their own assigned domain as the
    LEAST corroborated label across two independent models, suggesting a
    specific, identifiable class of entries drives a meaningful share of
    the overlap. Starting point: pull every entry whose true domain is
    the least-corroborated of its flagged set (both embedders agree on
    this, per Iteration 19's method) and hand that specific, bounded
    list to a human curator, rather than treating item 16 as "review all
    666 flagged entries."
17. **NEW, user-proposed, longer-range — graph connectivity as a third,
    non-geometric signal.** physis-core already has a relation layer
    (`TypedEdge`/`RelationType`, `src/relation.rs`) — if a real entry has
    existing causal/epistemic edges to entries in a DIFFERENT domain,
    that graph connectivity is independent evidence for cross-domain
    relevance, orthogonal to embedding geometry entirely. Untested
    whether the real 730-entry ontology currently has enough such edges
    to be useful — check this before investing further; if edges are
    sparse or domain-homogeneous, this signal has nothing to contribute
    yet.

**Recommended next action, updated after Iteration 19**: item 15 is done
(not supported as a fix). Next: item 16, narrowed to the specific,
bounded list of least-corroborated-true-domain entries Iteration 19's
qualitative pass surfaced — a concrete, small, human-reviewable list
rather than an open-ended "review the taxonomy" task. Item 17 (graph
connectivity) remains a real but unstarted precondition-check. Item 12
(fine-tune) remains the most expensive option, now further tempered:
two independent negative-to-mixed calibration results (Iterations 13's
residual gap, 19's corroboration filter) plus item 9's modest
embedder-swap gain all point toward the taxonomy, not the embedding or
the calibration heuristic, being the dominant constraint.

18. **NEW, user-directed, DONE (Iteration 20) — NOT SUPPORTED, but
    resolves the architecture question.** Requested directly: stop
    certifying unstable clustering after the fact; find reference points
    stable BY CONSTRUCTION. Implemented density-peak discovery (a fixed
    point is a real data item whose density — mean similarity to its M
    nearest neighbors, physis-core's existing `coherence_score` concept —
    is a local maximum among its neighbors; no K, no randomness,
    deterministic given the embeddings). Result: decisively unstable
    across embedders (0% overlap on both toy datasets, 15.4% on real
    data) — and a follow-up diagnostic ruled out "the strict peak rule is
    just brittle": a SOFTER top-K-by-density criterion with no peak
    requirement showed 0% overlap, worse than the strict rule. The
    density RANKING itself, not a threshold on top of it, differs by
    embedding architecture. **This closes the "discover new stable
    points from raw embedding geometry" direction — it has now failed
    under four different mechanisms across this track (clustering,
    certification, cross-embedder corroboration, density-peak
    discovery).**

    **The resolution, not just the negative result**: the one genuinely
    stable structure identified anywhere in this whole track is
    physis-core's existing, hand-authored 70-cell semiotic grid — fixed
    by design, immune to every instability found above because it isn't
    discovered at all. **Recommended architecture going forward: stop
    trying to discover new fixed points; use the existing cells as the
    fixed points (already "always the same" by construction) and build
    the LINKAGE layer between them using the multi-membership machinery
    already validated across Iterations 8, 10, 12, and 13** (which
    items span two or more cells, and how strongly) — a well-posed,
    tractable problem, unlike discovering stable points from scratch.
    This is the concrete next build, not a further experiment.

19. **DONE (Iteration 21) — the linkage layer, built.** Uses the
    existing 31 well-populated cells as fixed points, links them by
    bridge count on real data. First version reused Iteration 13's
    calibration and degenerated (98% flag rate — no discriminative
    signal); fixed with a deterministic, threshold-free rule (own cell +
    single next-best cell, always). Result: sparse (215/465 cell pairs),
    semantically plausible. Stability tested directly (ran twice):
    aggregate structure matches exactly; small item-level flips traced
    to a specific, identified cause (physis-core's own `embed_onnx.rs`
    hardcodes ONNX intra-op threads to all cores, and multi-threaded
    inference isn't bit-exact) — not a flaw in the new logic, and much
    smaller than every prior instability in this track. A real fix
    exists (configurable thread count, default 1) but requires editing
    production code; flagged as recommended, not done without asking.

    **CORRECTED (Iteration 26)**: the ONNX-threading cause cited above
    is wrong — embeddings were measured bit-identical across runs and
    thread counts. The item-level flips came from `HashMap`-randomized
    entry order in `classification_domains()` plus an untied sort in the
    experiment itself. Both fixed (physis-core `041b401`); runs are now
    byte-identical. See Iteration 26 in RESEARCH_LOG.md.
20. **DONE (Iteration 22), promising but not yet trustworthy.** Word
    discovery vs. Princeton WordNet 3.1 (pulled from a GitHub mirror,
    new `wordnet-db`/`wordnet-types` dependencies), embedder used only
    afterward to bind unmatched jargon to its nearest lexically-anchored
    neighbor — inverting every prior iteration's division of labor.
    97.9% coverage. But naive first-sense lookup (no disambiguation)
    produces wrong anchors for ambiguous words, and several "jargon"
    entries are actually Italian text whose words accidentally collide
    with unrelated English WordNet entries ("con"->"argument",
    "piano"->"keyboard instrument"). Not fixed here. Next: language
    detection before lookup, and sense disambiguation via embedding
    similarity between the entry and each candidate synset's gloss (a
    second, different role for "binding to the embedder" — disambiguation,
    not coverage-extension).

    **DONE (Iteration 23) — half fixed.** Language detection: fixed
    cleanly with a WordNet-self-referential coverage-fraction heuristic
    (>=40% of tokens recognized) after `whatlang` massively over-rejected
    this dataset's telegraphic keyword-list text. 96.4% pass, and the
    specific Italian false positives are now correctly excluded — **ready
    for core consideration**. Disambiguation: genuinely harder than
    expected. Two designs (whole-entry embedding, then a local +/-2-word
    window) BOTH still pick "log"->"exponent" over the intended "written
    record" sense — diagnosed as a real property of how the embedding
    model associates quantitative-flavored surrounding vocabulary with
    logarithm, not fixable by window size. Separately, "give"/"serve"
    fail because WordNet's only NOUN sense for each isn't the intended
    meaning (a VERB sense) — a scope gap, not a disambiguation failure.
    **Not ready for core**; would need verb-sense coverage plus a
    non-embedding disambiguation signal (e.g. lexical overlap with the
    gloss) for the harder cases.

21. **DONE (Iteration 26) — the determinism bug, found and fixed in
    production code.** This track claimed twice (Iterations 17, 21) that
    run-to-run variation came from ONNX Runtime's multi-threaded float
    reductions. It did not. A direct probe shows embeddings are
    **bit-identical** across runs and thread counts. The real causes were
    both local: `OntologyLoader::classification_domains()` chained
    `HashMap::values()`, and Rust randomizes `HashMap` iteration order
    per process, so every run saw the 730 entries in a different order;
    and experiment 21's link sort was untied, so a stable sort preserved
    that random order among equal counts. Both fixed (physis-core
    `041b401`) — `classification_domains()` now sorts by
    `(name, domain, mode)`. Three consecutive runs are byte-identical;
    143 tests pass. `OnnxConfig::intra_threads` was also added as an
    opt-in knob (default unchanged) with a doc comment stating it is NOT
    the fix. **Process note worth keeping**: Iteration 21 observed that
    forcing one thread didn't help and kept the hypothesis anyway. The
    disconfirming test was already in hand.

22. **DONE (Iteration 26) — persistence-by-identity (PermaTrack),
    tested, and it answers two different questions differently.**
    Iteration 20 concluded "stop discovering fixed points from geometry"
    without testing the persistence alternative; this closes that gap.
    *Model swap (MiniLM->BGE)*: persistence raises structure agreement
    ARI 0.034 -> 0.102 — still near-random, with heavy per-slot churn
    (one anchor 124 -> 48 members, 19 retained). Diagnosed: PermaTrack
    assumes a stable sensor with intermittent observation; swapping
    embedders swaps the metric space, so its precondition is absent.
    Model-swap stability stays unsolved. *Corpus growth under a fixed
    embedder* (physis-core's real situation): RE-DERIVED gives 58%
    anchor overlap and ARI 0.198; PERSISTENT gives 100% and ARI 1.000 —
    **the 1.000 being true by construction, and labeled as such in the
    experiment's own output; the finding is the RE-DERIVED row**, i.e.
    +25% new data silently replaces half the anchors and destroys ~80%
    of the structure. That is the previously-unmeasured cost of the
    re-derive-every-time approach used in Iterations 1-20.

23. **DONE (Iteration 27) — the six negative results re-audited, and they
    hold.** Every failed certification family had been measured while the
    Iteration 26a ordering bug was live, so the entire "go non-geometric"
    recommendation rested on numbers from an unstable pipeline. All six
    were re-run against fixed code (physis-core `f4dd2a1`), twice each:
    **6/6 are now byte-reproducible, and 6/6 conclusions are unchanged**
    — silhouette still ranks BAD above GOOD (0.089 vs 0.048); all six
    signals in Iteration 11 still `false` at every epsilon; cross-embedder
    corroboration still leaves FPR at 0.133 while costing recall; density
    peaks still overlap 0.0% across embedders (0/13 under the softer
    criterion); split agreement still fails *backwards* (max-bad 1.000 vs
    min-good -0.069); capacity-constrained loss still separates at no
    rank. This closes the live risk that the bug manufactured the
    negatives. It does **not** re-validate the six designs — every
    limitation recorded in Iterations 9-25 stands. What changes is that
    the numbers are now fixed quantities and therefore citable, which
    they were not before.

24. **DONE (Iteration 28) — NOT SUPPORTED. The last surviving mechanism
    does not survive.** Carried from the "worth
    carrying forward" list at the top of this report, where it is
    described as "the one mechanism this track never found a way to make
    lose on exclusive-membership data." Twenty-seven iterations have
    tried to certify splits and failed six times; the one mechanism that
    never failed is also the one never actually prototyped. The suspicion
    to test head-on is that **it never lost because it was always handed
    the correct k.** Choosing k is the certification problem wearing a
    different hat, and Iterations 9 and 11 already showed silhouette
    cannot rank a good split above a bad one. So the experiment is not
    "does clustering work" but "does it still work when nobody tells it
    how many clusters there are" — with random-partition and
    single-cluster baselines, and a deliberate attempt to make it lose.


    **Result.** Deterministic spherical k-means (k-means++ seeded, fixed
    RNG, ties to the lower index), three runs byte-identical.
    *Control (Dataset A, 15 items — the kind of set the claim was
    established on)*: oracle k=2 gives ARI 0.051, purity 0.667, and a
    10/5 split that is not the taxonomy at all — {dog cat lion tiger
    eagle sparrow owl penguin ostrich kiwi} vs {horse sheep wolf bear
    swan}. *Real corpus, oracle k=5*: ARI 0.022 vs a 0.003 random floor;
    purity 0.292 vs 0.232 for labelling everything one class. "Beats
    random: true" — which is what every earlier comparison reported — is
    worth **+0.019 ARI and +6.0 purity points**. *Choosing k from the
    data*: silhouette picks k=2 (ARI 0.001, **below** the random floor),
    elbow picks k=6 (ARI 0.021, above it); neither recovers true k=5.
    Best ARI anywhere in k=2..20 is 0.041.

    The suspicion going in was that k-selection was the weak point. It
    is not: **the oracle result was already near-worthless, so k was
    never the binding constraint.** Recorded per-criterion rather than as
    one verdict, because silhouette and elbow disagreeing sharply is part
    of the finding. Caveat stated in the experiment's own output: this is
    a fresh implementation rather than a reproduction of Iteration 7 (the
    purity match, 0.667, comes with a different split shape — 10/5 here
    vs 13/2 there), it covers the 5 coarse domains on one corpus and one
    embedder, and per Iteration 26b none of it transfers across a model
    swap.
25. **DONE (Iteration 29) — NO CODE CHANGE NEEDED; this report described
    the code wrongly.** Surface `CellClassifier`'s per-cell similarity
    scores** instead of collapsing to one winning `(domain, mode)`.
    Motivated by Iteration 8: `OntologyEntry` (`src/models.rs`) stores
    exactly one domain and one mode, Test 1 showed that loses real
    information for concepts spanning two fields, and Test 2 showed the
    embedding space already carries it. `classify.rs` computes per-cell
    scores internally before picking a winner, so this exposes existing
    information rather than adding a mechanism — and unlike everything in
    the certification thread, it requires no threshold to be trustworthy.


    **Correction.** `classify()` does **not** collapse to a winner. It
    returns `Vec<CellScore>` with every populated cell scored and ranked
    (70.0 per query on the real ontology), and `studio.rs`,
    `studio_lab.rs` and `main.rs` all already receive the full vector.
    What is single-valued is *storage* — `OntologyEntry` holds one domain
    and one mode — which is an authoring limit, not a query limit.

    **So the measurable question became what the ranked list carries
    beyond its argmax**, over all 730 entries: top-1 0.874, top-2 0.886,
    top-3 0.897, top-5 0.923, top-10 0.958. Ranks 2-5 recover **+0.049**
    over the argmax (6% relative) — marginal. The 2nd-ranked cell is in a
    different domain for **169/730 (23.2%)**, mean rank-1-to-rank-2
    margin 0.1072; that figure bears on Iteration 8's cross-cutting
    claim, but Iterations 13/15/19 established three times that such
    flags cannot be validated without labels, so it is a measurement, not
    evidence of genuine multi-membership. Caveat stated in the output:
    entries are classified against an ontology built from them, so top-1
    is an upper bound, not held-out accuracy — the gains (differences)
    are trustworthy, the absolute levels are not.
26. **OPEN, awaiting owner decision — what gets promoted into
    physis-core.** Ready: language detection (Iteration 23, the WordNet
    coverage-fraction heuristic, 96.4% pass) and the fixed-cell linkage
    layer (Iteration 21, deterministic, no k and no threshold). NOT
    recommended: WordNet sense disambiguation (Iteration 23 — two designs
    both failed on the same cases for a diagnosed reason, plus a
    verb-sense scope gap).

27. **DONE (Iteration 30) — SUPPORTED. The first positive result in this
    track, and it reopens discovery.** User-proposed: treat an embedding's
    correlates as objects to go *search for*. Structurally unlike the seven
    failures — they asked a closed question with no referent outside the
    geometry; this asks an open one with an external referent, making the
    embedder a search heuristic rather than a truth criterion. Stratified
    10% hold-out, 73 of 730: near-neighbour midpoints point at a genuinely
    missing concept **13.2%** of the time vs **9.2%** for manifold-matched
    random pairs — **1.44x, z = +4.87, p < 1e-5**. Three runs byte-identical.
    Honest limits: ~87% of proposals point at nothing, so the external check
    is the system; and recall runs the other way (0.548 vs 0.699) because
    proximity-chosen proposals cluster. Two measurement bugs each produced a
    false negative first (a saturating fixed-tau metric; then a competition
    that included the proposal's own parents, adjacent by construction).
    **Next: build the outward loop — propose, check against a source
    causally independent of the embedder, commit confirmations as authored
    entries.**

28. **DONE (Iterations 31-32) — the outward loop built, then dismantled by its
    own control. NOT SUPPORTED.** The check design (a list of known
    ontologies; unmatched-but-coherent promoted as UNCANNY) was built against
    WordNet — the right lexicon, being hand-built rather than web-trained and
    so causally independent of MiniLM. Iteration 32 gave it 5-fold
    cross-validation, 1616 pooled regions.

    First pass: convergence **z = +5.05** with a monotone dose-response from
    0.256 to 1.000. **That was an artifact of my own metric.** The hit test
    excluded each region's parents, and parent count *is* convergence — so
    high-convergence regions had more near competitors removed and a weaker
    rival to beat. Under a FIXED-size exclusion swept over k (reading only the
    unsaturated range, overall rate 0.10-0.90), the sign **flips**: mean
    **z = -2.23**, with k=2,3,4 at -3.49, -3.80, -2.80. High-convergence
    regions contain a removed concept *less* often. Plausibly because many
    pairs converge where the ontology is already thick, and thick regions are
    where least is missing. WordNet check at 5x sample: z = +1.33, still dead.

    Also corrected here: the claim that "WordNet is nouns-only" was my loader,
    not the lexicon (13,789 verb synsets ship with it), and loading verbs
    barely moved the buckets — the UNCANNY head is Italian-language entries an
    English lexicon cannot match, a coverage artifact rather than novelty.

    **Net: Iteration 30's proposer stands; nothing built on top of it does.**
    The geometry can say where to look. Nothing tried so far can say whether
    what is found is worth having.

29. **DONE (Iteration 33) — the disposer can be a machine. SHIPPED as
    `physis_core::coverage` in 0.1.17.** physis-pro's `planner.rs` already
    implements the check (`ImpactAnalysis::newly_covered`); this validated it
    and put the narrow primitive in core, where `discovery.rs` proposes entries
    but could not say which were worth keeping. 5-fold CV, 691 records, 2000
    candidates/arm: re-adding an existing entry rescues **0/2000** (correctly),
    interpolations 0.073, **Welch t = +12.11** — 2000 candidates cut to 141.
    But it does **not** prefer Iteration 30's proposer: near-neighbour 0.073 vs
    medium-rank 0.070, t = +0.29. The first pass, using only Iteration 30's
    weak random-pair control, reported +3.34; construction-matched controls
    collapsed it. **Coverage is not correctness** — a candidate swallowing
    records into a wrong cell scores like one capturing a real gap. A filter,
    not an approver.

30. **DONE (Iterations 34-40) — the substructure/becoming line, and where it
    actually stops.** Seven iterations testing whether a term's *context*
    corroborates a link or reveals a meaning-change. One robust positive
    (Iteration 35: shared substructure from an INDEPENDENT source adds
    predictive power beyond cosine, z = +6.64 same-cell, versus +0.62 for
    hint-derived substructure the embedder had already read). Everything
    downstream of it failed, and always at the SAME step — producing a sense
    partition:

    | attempt | partition from | failure |
    |---|---|---|
    | 39a | whole event text | clusters document GENRE |
    | 39b | local window vectors | separation measures BREADTH of use |
    | 40 | ontology cell labels | labels SCATTER (20 distinct / 88 occurrences) |

    `physis_core::becoming` is shipped and sound — its runs test separates
    `AAAAAABBBBBB` from `ABABBABAABAB`, which no clusterer can — but it is
    **correct given a sense partition and cannot produce one**. Word sense
    induction is the unsolved input. This is SemEval-2020 Task 1 territory
    (arXiv 2007.11464); best published figures are 66.5% accuracy / 51.8
    Spearman, so this is an open problem, not a tuning failure.

31. **NEXT — n-gram signatures with BACKOFF, not fixed-n.** The user's
    proposal, and the one remaining option that is symbolic rather than
    geometric — which matters after nine geometric mechanisms failed while both
    survivors (`coverage`, independent substructure) were not similarity
    statistics. An n-gram is an *exact symbol*: broad use yields many
    non-recurring signatures, two senses yield two RECURRING families. That is
    precisely the distinction all three partition attempts above could not make.

    **Measured viability (this session, on the 6234-event corpus)**: exact
    3-gram recurrence ranges 11-83% by term (`disposer` 83%, `linkage` 61%,
    `core` 56%, `test` 35%, `coverage` 21%, `commit` 14%). Signatures are
    semantically coherent — `build_linkage_layer` x8, `iteration_disposer_
    machine` x6, `cargo_test_features` x18 — and critically, `coverage` shows
    BOTH families at signature level: `warranty_coverage_driver` (insurance)
    alongside the module sense. The distinction is visible symbolically where it
    was invisible geometrically.

    **The load-bearing dependency, measured rather than assumed**: a naive
    variation allowance makes it WORSE. Skip-grams over nearest content words
    scored *below* exact adjacency on every term tested (coverage 18% vs 21%,
    key 25% vs 27%). Loosening the match cost more precision than it bought.

    So the variation layer must be principled, and infini-gram supplies the
    mechanism: **suffix-array backoff over unbounded n** (arXiv 2401.17377;
    FM-index variant 2506.12229 for scale). Rather than fixing n = 3 and
    hoping, take the longest n with sufficient support and back off when it
    runs out — which is exactly the sparsity problem measured above, solved
    properly instead of by skipping words.

32. **NEXT — an n-gram table as first-class addressable entries (Qwen-style).**
    Qwen3.8-Flash-Next carries a 51B-parameter N-gram Embedding layer: a
    20,000,000-entry bigram/trigram table at layer 2, where a lookup is a memory
    read rather than a matrix multiply. The transferable idea is not the scale
    but the *addressability* — n-grams as entries you can point at, annotate and
    relate, instead of a similarity recomputed per query.

    That is what the semiotic relations need in order to be stored rather than
    inferred: two signatures co-varying in the same slot are SYNONYMS, occupying
    the same slot but never together are CONTRARIES, partially overlapping are
    SIMILES. Substitutability is a table lookup, not a cosine. It also gives the
    Greimas relations a home — physis-pro's `dual()` currently implements only
    contrariety, with contradiction and implication missing.

33. **NEXT — rhythms: extend `chronos` from labels to signatures.**
    `src/machines/chronos.rs` already reduces timestamped events to recurrence
    primitives — `period_days` (median inter-arrival), `confidence` = regularity
    (1 - CV of intervals) x evidence, and `slipping()` as the broken-beat flag.
    Iteration 40 confirmed it works on real data where `becoming` could not:
    plausible periods (5.1d `key`, 6.7d `coverage`, 6.9d `core`) and confidence
    0.35-0.61 for regular terms against 0.12 for `test`, which genuinely is
    irregular. Tempo detection over labels needs no sense induction — which is
    exactly why it survived.

    Three concrete extensions, in order of cheapness:

    a. **Rhythm over SIGNATURES rather than cell labels.** Item 31 supplies a
       label vocabulary that is exact instead of scattered; chronos then reads
       each signature family's own beat. This is the natural join of items 31
       and 33 and needs no new statistic.
    b. **Mistake vs variation as a PDCA cycle.** `becoming::deviations` already
       decides by uptake — a deviation that never recurs is noise, one that
       recurs was taken up. `physis-core/src/process.rs` already models
       PLAN -> ACTION -> OBSERVATION -> DEVIATION -> INTERVENTION -> OUTCOME.
       Wiring `slipping()` into `ProcessDeviation` closes that loop with the
       machinery already present; `process_cycles` is currently 0.
    c. **Global vs local tempo.** chronos estimates one period per label. A
       document has nested periods (a section rhythm inside a chapter rhythm),
       which is the user's "for each note there is a period, and then there are
       global ones". Multi-scale periodicity is untried and is the only part of
       this item needing new statistics.

34. **DONE (Iteration 38b) — NOT SUPPORTED. Pydantic-style executable
    constraints fail, and the reason is a DATA-MODEL gap, not a method gap.**

    Tested at the gate this item specified, on the identical injected-misfiling
    harness and against the identical baseline:

      CONSTRAINT (axis_name/category unattested in domain)   AUC = 0.433
      COSINE     (geometric baseline)                        AUC = 0.741

    It does not beat chance, let alone 0.741.

    **Diagnosed, not merely reported.** A constraint is a detector only if it
    stays QUIET on correct data. Under the TRUE assignment this one fires on
    **548/957 correctly-filed entries (57%)** for `axis_name` and 207/957 (22%)
    for `category`. A rule that flags most correct cases cannot flag incorrect
    ones. Root cause: `axis_name` has 553 distinct values over 957 entries,
    `category` 155 — they are free text, not controlled vocabularies, and
    Cheung's mechanism needs enums on BOTH sides of the dependency.

    The one genuinely enum-like field fails the opposite way: `axis_kind` has
    41 values (23.3 entries each) and fires on only **4%** of correct entries —
    a well-formed constraint — but its purity toward domain is 0.331 against
    0.304 for a cardinality-matched shuffle. **Controlled but uninformative.**

    **So physis has no field that is both a controlled vocabulary AND
    informative about the grid**, which is exactly what a dependency constraint
    requires. The prerequisite is therefore authoring work, not an algorithm: an
    `axis_kind`-style enum that actually correlates with domain or mode. Until
    that exists, this route is closed — and it should be re-opened by writing
    that vocabulary, not by trying another checker.

    (Cardinality-matched shuffles were load-bearing throughout: `unit` shows
    0.761 raw purity toward mode purely because 572 distinct values over 957
    entries is a near-unique key. Also: dspy is NOT installed — what is on disk
    is a Hermes skill with reference docs, no dist-info, and the local gateway
    reports an empty model list. This gate never needed it.)

    *Original proposal retained below for the reasoning that motivated it.*

34b. **The original argument, kept because the reasoning was sound even though
    the result was negative.**

    Iteration 33 shipped `coverage` with a stated hole: *coverage is not
    correctness*. A candidate that swallows records into the WRONG cell scores
    exactly like one capturing a real gap. Cheung's second paper names the
    vocabulary — `coverage` is a **completeness** check (incomplete facets
    reduce recall); the missing half is **soundness** (unsound facets reduce
    precision). Iteration 38 then tried the mechanism that paper recommends,
    non-lattice auditing, and it failed for a reason that is now measured
    rather than guessed:

    > physis's 5x14 grid is **not a taxonomy**. Real cells are indistinguishable
    > from a size-matched shuffle (|MCA|=1 rate 0.653 vs 0.668; ancestor Jaccard
    > 0.133 vs 0.117). Non-lattice auditing detects missing or redundant IS-A
    > links in a subsumption hierarchy; a misfiled entry in a *functional* grid
    > is simply not that kind of defect.

    **Why the Pydantic route is the right next attempt, and not just the next
    thing in the paper.** Cheung's §7.2 mechanism is a *dependency map* —
    mechanism → required component, "deck building requires cards" — and it
    flags an output that names a mechanism while lacking its component. That
    constraint is **functional, not taxonomic**: it says "if X is present then Y
    must be", which is exactly the shape of a grid that groups by function. The
    thing that made non-lattice auditing inapplicable is the thing that makes
    this applicable.

    **The real design question is not which library — it is whether constraints
    are CODE or DATA.** physis is Rust; typed validation functions need no
    Python at all. Pydantic's actual contribution is that schemas are
    *declarative and round-trippable through JSON Schema*, which would let
    constraints be authored in `config/` and versioned alongside the ontology
    they constrain, rather than compiled in. Given that hand-authored cells are
    the one thing this entire track measured to be stable, constraints as
    authored data is the option consistent with the evidence.

    **Prior art already in-tree, and its limit.** physis-pro's `DreamContract`
    (a0d28a4) implements the validation-contract idea on generated dreams — "an
    output the grid cannot name is structure-shaped noise" — with rejection
    accounting and a raw pool that keeps what the contract refuses. But it gates
    on a **cosine threshold**, which Iteration 38 independently vindicated as
    the right choice for this grid (cosine AUC 0.741 beat every structural
    scorer tried). It is a *nameability* gate, not a *dependency* gate. The
    constraint layer proposed here is the part that is missing.

    **Staged, same discipline as items 31-33:**

    a. **Author a dependency map.** Someone writes "mode WORK requires an
       action-shaped facet", "domain HEAL requires a subject". This is
       hand-authoring, and given the track record that is a feature rather than
       a shortcut.
       *Gate*: ≥20 constraints covering ≥50% of entries.
       *Kill*: if no constraint can be stated that a competent author would
       agree with, the grid has no functional semantics to check and this ends.
    b. **Run it against the injected-misfiling harness from Iteration 38**,
       which already exists: 91 entries deterministically moved to a wrong cell,
       exact ground truth, and the geometric baseline (cosine AUC **0.741**)
       already measured on the identical split.
       *Gate*: constraint violations must beat **AUC 0.741**. Not "find
       something" — beat the detector already shipped.
       *Kill*: below 0.741 it adds nothing over `coverage` + cosine and should
       not ship.
    c. **Only if (b) passes: decide code vs data**, and only then whether
       Python enters the build at all. pydantic 2.13.5 / pydantic-core 2.46.5
       are installed; `dspy` is not, so the paper's generation pipeline is not
       runnable as written and is out of scope here regardless.

    **What this cannot do, stated up front.** A dependency constraint catches
    *structural* incoherence — a mechanism without its component. It does not
    catch a semantically wrong but structurally complete entry, which is the
    harder half of soundness and remains open after nine geometric mechanisms,
    non-lattice auditing, and this.

## What actually worked

Everything in this section survived a control designed to kill it. Results that
merely produced a pleasing number are in the failure list below, because five
positives died to stronger controls in this track and the distinction is the
only thing that makes the rest trustworthy.

### Shipped, and load-bearing

| what | evidence | where |
|---|---|---|
| **Deterministic ontology iteration** | `classification_domains()` iterated `HashMap`s — order randomized per process, so every run saw the corpus differently. Fixed; then found the fix INCOMPLETE (the key was not a total order: 6 built-ins collide on `(name, domain, mode)`). Regression test verified by sabotage. | `ontology.rs`, 0.1.15 + `b08aa7a` |
| **`linkage`** | Which cells real data bridges, via which items. No threshold, no k, no seed; ties break on cell key. Byte-identical across runs; reproduces Iteration 21's qualitative result. | `linkage.rs`, 0.1.16 |
| **`coverage` — the machine disposer** | Re-adding an entry the ontology already has rescues **exactly 0 of 2000** trials (correctly — those records were already covered); interpolations 0.073, **Welch t = +12.11**. Cuts 2000 candidates to 141 with no human and no lexicon. | `coverage.rs`, 0.1.17 |
| **`becoming` — the runs test** | Separates `AAAAAABBBBBB` from `ABABBABAABAB`: identical partitions, identical counts, distinguishable only by order. No clusterer can do this. Unit-proven; no RNG. | `becoming.rs` |
| **`chronos`** (pre-existing, validated here) | Plausible periods on real data — 5.1d `key`, 6.7d `coverage`, 6.9d `core` — with confidence 0.35-0.61 for regular terms against **0.12 for `test`**, which genuinely is irregular. | `machines/chronos.rs` |

### Measured findings that hold

1. **Re-derivation destroys structure; persistence preserves it.** +25% corpus
   data replaced half the anchors (58% identity overlap) and destroyed ~80% of
   the structure (ARI 0.198). *Stated honestly*: the persistent arm's ARI 1.000
   is true BY CONSTRUCTION and is not the finding — the re-derived row is.
2. **Nothing survives an embedder swap.** MiniLM vs BGE agreement on the same
   corpus is near-random (ARI ~0.10) even holding cluster identity fixed. The
   embedder is part of the ontology's identity, not a swappable backend.
3. **Independent-source substructure corroborates links** (Iteration 35).
   z = **+6.64** same-cell from WordNet glosses versus **+0.62** from
   hint-derived substructure the embedder had already read. It *strengthened*
   under tighter control (+6.31 → +6.64) and survived strict pairwise
   independence — the opposite of how the five artifacts behaved.
4. **Interpolation beats random pairs; the specific pair choice does not
   matter.** Iteration 30's proposer beat a manifold-matched control 1.44x
   (z = +4.87), but Iteration 33 showed near-neighbour ≈ medium-rank
   (t = +0.29). What carries value is interpolating between *related* content
   at all.
5. **The six negative results are reproducible.** All re-run under fixed
   ordering: 6/6 byte-identical, 6/6 conclusions unchanged. The ordering bug
   did not manufacture them.
6. **n-gram discretization separates what geometry could not** *(promising,
   not yet validated)*. Exact 3-gram recurrence 11-83% by term; signatures
   semantically coherent; and `coverage` shows BOTH families symbolically —
   `warranty_coverage_driver` beside the module sense. Also measured: a naive
   variation allowance makes it **worse** (skip-grams below exact adjacency on
   every term).

### What did not work, in one line each

Nine geometric certification mechanisms (margin/silhouette, three
kNN-consistency variants, cross-embedder corroboration, density peaks,
cross-embedder split agreement, capacity-constrained loss, plain k-means);
non-lattice auditing (physis's grid is not a taxonomy — real cells are
indistinguishable from a size-matched shuffle); and sense induction three ways
(document genre / breadth of use / label scatter).

---

## The refined plan: n-grams + becoming + chronos

### Why the combination is coherent rather than hopeful

Each component's known failure mode is covered by another's known strength:

```
  becoming   runs test WORKS, needs a partition it cannot produce
  n-grams    produce an EXACT partition, no induction, no embedder
  chronos    works on LABELS, needs no induction — signatures ARE labels
```

n-grams supply the label vocabulary that both of the others need. That is the
whole join, and it is why this is worth building rather than a fourth variation
on a statistic that has failed three times.

### Staged, with a kill criterion at every gate

Each stage is cheap relative to the one after it, and each has a number that
stops the work rather than an impression that continues it.

**Stage 0 — fix the evaluation before building anything.**
SemEval-2020 Task 1's organisers name evaluation as the field's most pressing
problem, and this track currently has eight terms chosen by me on a corpus of
one project's own documentation with no real polysemy. Use the public
SemEval-2020 English data (annotated, four languages, gold standard) or the
DIACR-Ita set.
*Gate*: ≥30 terms with ground truth nobody in this project authored.
*Kill*: if no external gold standard can be obtained, stop — every result after
this would be unfalsifiable, which is the failure mode of Iterations 39-40.

**Stage 1 — signature extraction with backoff.**
Suffix array over the corpus (infini-gram, arXiv 2401.17377); per occurrence
take the **longest** n-gram with support ≥ k rather than fixing n = 3.
*Gate*: median recurrence ≥ 50% across evaluation terms (fixed n = 3 gives 21%
on `coverage`, 14% on `commit`).
*Kill*: if backoff cannot lift recurrence above fixed-n, the sparsity is
structural and the whole line ends here.

**Stage 2 — signature families by SUBSTITUTABILITY, not similarity.**
Two signatures belong to one family when they occur in the same slots. This is
the synonym layer, and it is the step measured to be load-bearing: without it
exact n-grams are too sparse, and loosening the match naively made things worse.
*Gate*: `coverage` separates into ≥2 families recovering the warranty/module
split that is already visible in the raw signatures.
*Kill*: if families do not form, n-grams are a vocabulary and not a sense
inventory; report and stop.

**Stage 3 — `becoming` over family labels.**
`classify_labeled` already takes discrete labels; Stage 2 supplies them.
*Gate*: ≥5/8 on the known terms (current: 1/8 with induced clusters, 1/8 with
cell labels).
*Kill*: below 3/8, the partition is still not the problem and the trajectory
question should be abandoned rather than tuned.

**Stage 4 — `chronos` over family labels.**
Per-family rhythm rather than per-cell. No new statistic; this is the natural
join of Stages 1 and 3.
*Gate*: distinct periods per family where the senses are known to differ in
cadence.

**Stage 5 — relation typing, and the Greimas gap.**
Co-varying in the same slot = **synonym**; same slot but never together =
**contrary**; partial overlap = **simile**. Substitutability becomes a table
lookup rather than a cosine, and the relations are *stored* rather than
recomputed — the transferable idea from Qwen3.8-Flash-Next's addressable n-gram
table, not its scale.
*Gate*: recovers `dual()`'s existing oppositions (CREATE↔DESTROY,
LEARN↔GUIDE, SENSE↔BRAINSTORM) **without being given them**. That is a real
held-out test, since those pairs are hand-authored and independent of any
corpus statistic.
*Kill*: if it cannot recover known oppositions, it will not find unknown ones.

**Stage 6 — close the PDCA loop.**
`becoming::deviations` already decides mistake-vs-variation by uptake.
`process.rs` already models PLAN → ACTION → OBSERVATION → DEVIATION →
INTERVENTION → OUTCOME. Wiring `chronos::slipping()` into `ProcessDeviation`
closes it with machinery that exists; `process_cycles` is currently 0.

### The three standing constraints any of this must respect

1. **Pin the embedder** — nothing transfers across a model swap (ARI ~0.10).
   Where a stage uses one at all, it is part of the artifact's identity.
2. **Persist, never re-derive** — re-derivation costs ~80% of the structure on
   +25% new data.
3. **Construction-matched controls, not manifold-matched.** Three positives
   died to this distinction. A control must differ from the treatment in
   exactly one way; matching only the manifold is not enough.

## Architectural conclusion (updated after Iteration 40)

The user's ask — "some fixed points discovered in data, always the same,
see how they link to each other" — is **achievable, and now demonstrated
end-to-end**. It requires three things, none of which substitutes for
another:

1. **Deterministic ordering** in the pipeline. Was broken; fixed in
   shipped code (Iteration 26a). This alone accounted for instability
   this track had been attributing to embedding geometry.
2. **A pinned embedder.** No persistence scheme tested recovers
   cross-model stability (0.102 ARI). Treat the embedder as part of the
   ontology's identity, not an interchangeable backend.
3. **Persisted anchor identity**, not re-derivation (Iteration 26b).

And separately, unchanged and now reinforced from an independent
direction (ai-knowledge-graph's LLM-judgment approach): **certification
of which splits are real needs a non-geometric signal.** Six
structurally distinct geometric mechanisms have failed — Iterations 9,
11, 19, 20, 24, 25. Adding LLM triple extraction was considered here and
deliberately declined for now: it would reintroduce sampling
nondeterminism into the exact property Iteration 26a just fixed.

---

## The refined plan, executed (Stages 0-6)

Run 2026-09-07. Three gates passed, two killed, one untestable, one moot. The
plan's own kill criteria were applied as written rather than renegotiated after
seeing the numbers.

| stage | what it asked | result |
|---|---|---|
| 0 | >= 30 terms, ground truth nobody here authored | **PASSED** — 37 |
| 1 | median recurrence >= 50% under backoff | **PASSED** — 92.0% vs 34.6% |
| 2 | families form, no giant component | **PASSED** — largest 21.1%, 35/37 |
| 3 | beat the majority baseline on gold | **KILLED** — 51.4% vs 56.8% |
| 4 | distinct periods per family | **UNTESTABLE** — no timestamps |
| 5 | recover `dual()`'s oppositions unaided | **KILLED** — 0/12, chance 1.0 |
| 6 | close the PDCA loop | downstream of 3; not attempted |

### Stage 0 — the evaluation, fixed first

The previous corpus was this project's own documentation, eight terms I chose,
no real polysemy, ground truth I wrote. Every result on it was unfalsifiable,
which is how Iterations 39-40 went wrong. Replaced with **SemEval-2020 Task 1
English**: 37 lemmas, 16 changed / 21 stable, annotated by the task organisers,
over CCOHA1 (1810-1860) and CCOHA2 (1960-2010). Not vendored — ~61MB, fetched
to `~/datasets`, located by `SEMEVAL_DIR`.

Published state of the art on this set is 66.5% accuracy / 0.518 Spearman. Every
number below is against that, not against an impression.

### Stage 1 — backoff works, and its control takes back the headline

Longest-window-with-support, per infini-gram, support counted within a term's
own occurrences:

    median recurrence, fixed n = 3 : 34.6%
    median recurrence, backoff     : 92.0%     gate wanted >= 50%

Then the standing rule — treat a positive as artifact until a
**construction-matched** control says otherwise, after three positives in this
track died exactly there. The control resamples each context token independently
from the term's own pooled context distribution: same occurrences, same window
geometry, same token frequencies, collocation destroyed, differing in one way.

It reaches **78.7%**. So 85% of the headline is explained without any
collocation structure at all, and recurrence *rate* is largely a frequency
artifact. What survives is signature **length**:

    median length         3.0 real vs 2.0 shuffled
    share of length >= 4  41.4% real vs 2.6% shuffled

That 16x separation is the real Stage 1 result, and it set the constraint the
next stage had to respect.

### Stage 2 — families form; the predicted collapse does not happen

Two signatures are one family when they share length and target offset and
differ in exactly one context position; families are the connected components.
A set operation — no cosine, no embedder, no threshold, nothing a model swap
can move.

The expected failure was transitive collapse through a promiscuous frame
("the _ of" chaining everything into one component). It did not occur: median
largest-family share **21.1%**, median 3 families above 10%, 35/37 terms forming
at least two. The synonym layer is real.

### Stage 3 — killed, and the diagnosis is the useful part

| arm | accuracy | baseline | Spearman(JSD, gold graded) |
|---|---|---|---|
| signatures len >= 2 | 48.6% | 56.8% | 0.140 |
| signatures len >= 4 | 51.4% | 56.8% | 0.183 |
| SemEval published | 66.5% | | 0.518 |

The len>=4 arm was not a tuning pass: Stage 1's control had already concluded
"build families from the long signatures", and the first cut ignored it.
Applying it moved both numbers in the predicted direction — corroborating
Stage 1 — and still did not reach a majority-class baseline.

**Why**, which matters more than the score. The verdict is degenerate:

    trajectory distribution   {Split: 31, Becoming: 6}   len >= 4
    median families per term  57
    share seen by the top 2   28.7%

`Stable` never fires. `classify_labeled` reads only the two leading labels, so
with 57 families it judges a term on under a third of the evidence, on two
labels arbitrarily drawn from a long tail. Every term looks interleaved.

So neither half failed. The runs test is sound and unit-tested; the families
survive their own gate. What fails is the **interface**: `classify_labeled`
wants labels on the order of the *sense* count, substitutability delivers labels
on the order of the *collocation* count, two orders of magnitude apart. The
missing piece is an aggregation step between them — and inventing one is the
sense-induction problem that has now failed in ten distinct forms.

### Stage 4 — untestable on this data, stated rather than skipped

CCOHA sentences are **randomly shuffled within each period**. The only real
order is the C1 -> C2 block, which is enough for a runs test and useless for
`chronos`, whose primitives are inter-arrival times. No timestamps exist to
read. A dataset with finer time bins (RuShiftEval, or COHA by decade) would be
needed; nothing about Stage 4 was disproved here.

### Stage 5 — killed at chance, and the wall is not geometric

Tested against `SemioticGrid::dual()`'s seven hand-authored mode oppositions —
written by a person into a match arm, independent of every corpus statistic, on
a corpus (CCOHA) that has nothing to do with this project.

    dual ranked 1st : 0/12 = 0.0%    (chance 8.3%, expected 1.0)
    dual in top 3   : 4/12 = 33.3%   (chance 25.0%)

Zero, on the primary measure. `move`, `play` and `work` head nearly every
ranking regardless of the query: Jaccard normalises set size but not the fact
that frequent verbs fill more distinct frames, so the ranking tracks frequency
and genre. Only create<->destroy (rank 2 both ways) and walk<->work (3 and 2)
show bidirectional signal, and even for `create`, `maintain` outranks `destroy`.

This was the **second symbolic** mechanism to fail, after nine geometric ones,
so the wall is not a property of embeddings. Substitutability finds words that
fill the same slot; near-synonyms, frequency peers and antonyms all fill the
same slot, and nothing in the distribution says which is which.

### What this changes

The plan was built on the claim that each component's failure mode is covered by
another's strength — n-grams supply the exact partition `becoming` cannot
induce. That claim was **half right**. n-grams do supply an exact partition
(Stage 2), and it is not the partition `becoming` needs (Stage 3): exactness was
never the missing property, *granularity* was.

The standing conclusion is unchanged and now reached from a third direction:
**certification of which distinctions are semantic needs a non-geometric,
non-distributional signal.** Ten mechanisms across two families have failed at
it. The two things in this track that survived their controls — `coverage`
(operational: does a candidate make records classifiable that were not) and
independent-source substructure (corroboration from a source the embedder never
read) — are both *external* checks. That is the pattern, and it is the only one
with a positive result behind it.

### Stage 6 — the cell vocabulary, and a published verdict withdrawn

Iteration 38c left one prerequisite: the CONSTRAINT scorer needs a controlled
vocabulary that is *informative about the grid*. The hand-authored anchors
(`config/mode_anchors_ontology.json`, 70 cells) scored AUC 0.532 against a 0.650
cosine baseline, diagnosed as an idiom mismatch — the anchors say "deadlift" and
"squat heavy" where corpus entries say something else, and lexical overlap
between two independently-authored vocabularies is thin.

So: build the vocabulary in the *entries'* idiom, and make the LLM falsifiable
by running a no-LLM arm on identical inputs.

  A  `vocab_frequency`  top-25 IDF-weighted terms from the cell's TRAIN entries
  B  `vocab_generated`  qwen3:4b, same anchor + same TRAIN examples, asked for
                        ADDITIONAL terms in the same register, shape-constrained
                        by a Pydantic `CellVocab` contract, temperature 0

`scripts/gen_cell_vocab.py` splits every cell deterministically (even/odd by
sorted name), builds vocabulary from TRAIN only, and writes a committed JSON
artifact the Rust scorer reads. The model is an authoring assistant whose output
is reviewable, never a runtime dependency.

**Two controls were added to the harness, and both mattered.**

First, the scored population. `experiment38_soundness_audit` now honours
`PHYSIS_AUDIT_HALF` (`test` / `train` / `all`), so a cell's vocabulary is never
tested against the entries it was derived from.

Second, and this is the one that changes a published claim: the injected
misfilings were a *single* deterministic perturbation — every 7th anchored
entry, target cell `f(i) = cell_names[(i*13+5) % N]`. That is one arbitrary
sample from a large space of equally valid perturbations. `PHYSIS_AUDIT_STRIDE`
and `PHYSIS_AUDIT_MULT` now resample it, holding the corpus fixed. Eleven
perturbations, each scorer measured against cosine on the same rows:

| population | scorer | mean Δ vs cosine | wins | paired t |
|---|---|---|---|---|
| held-out half | CONSTRAINT | −0.026 | 2/11 | −1.77 |
| held-out half | VOCAB-FREQ (A) | −0.007 | 4/11 | −0.39 |
| all entries | CONSTRAINT | −0.059 | 1/11 | −6.06 |
| all entries | VOCAB-FREQ (A) | **+0.110** | **11/11** | **+10.46** |

Three things follow, in ascending order of how much they cost.

**1. Iteration 38's headline is withdrawn.** "The structural audit BEATS the
geometric baseline (0.654 vs 0.619)" was the `stride=7 / half=test` cell — one
of only two positives in eleven. `stride=13` yields +0.088 the same way and
`stride=5` yields −0.072. Across the sweep CONSTRAINT loses to cosine, and on
the full population it loses *significantly* (t = −6.06). There is no label-free
structural soundness check here. The scorer is worse than distance.

**2. Arm A is a textbook contamination artifact.** On `half=all` it wins 11/11
perturbations at t = +10.46 — a result that, written up on its own, would read
as a landmark. On the held-out half the same vocabulary, the same scorer and the
same perturbations give −0.007 at t = −0.39: chance. The entire margin was the
vocabulary recognising the entries it was built from. The train/test split is
the only thing standing between that number and the report.

**3. The noise floor is now measured, and it is large.** Changing nothing but
the injection stride moves the *cosine baseline itself* from 0.565 to 0.686 —
a spread of 0.121. Every margin this harness has ever reported (0.599 vs 0.650,
0.654 vs 0.619, 0.532, 0.623) sits inside that band. Single-run AUC differences
below roughly 0.06 in this harness carry no information, and earlier iterations
read them as if they did.

The general lesson is the same one that killed Iterations 32, 33, 34 and 38b,
arriving from a new direction: **a positive result must survive resampling the
thing the experimenter chose arbitrarily.** Previously that was the control
construction. Here it was the ground truth itself. The injected perturbation felt
objective — "there is no argument over what the right answer is", as this report
put it — and that was true and irrelevant. Objective ground truth sampled once
is still a sample of one.


**Provenance of these numbers.** The sweep binary was built at 16:42:59 against
`physis-core` at `cb4e89a`; `config/{machine,health_fitness,software_dev}_ontology.json`
were then modified at 16:45:28 by concurrent work folding pro's duplicate config
copies into core. Those configs are `include_str!`'d, so the corpus could have
moved under the sweep. It did not: rebuilding after the change still loads 659
entries, 572 anchored, and reproduces stride=7/half=test at 0.654 / 0.619 / 0.574
exactly. `ONTOLOGY_SOURCES` is a file registry, not a load list — `load_all` still
resolves through `BUILTIN_ONTOLOGIES` and `EXTRA_ONTOLOGIES`, and the newly added
`space_`/`time_ontology.json` are in neither. The 995 domain+mode entries now
present across `config/*.json` are a count of files on disk, not of what Core
classifies with. These figures survive the refactor.

### Stage 6b — why arm B is not a dspy pipeline, measured

Arm B was specified as dspy + a Pydantic `CellVocab` contract against local
qwen3:4b. It produced nothing in two runs of 20+ minutes with no error, which is
the worst possible failure mode: a silent stall. Probing ollama directly
separates the causes.

| configuration | result |
|---|---|
| `think` default (on), `format`=schema, num_predict 600 | **TimeoutError at 600s** — never terminates |
| `think=False`, `format`=schema, num_predict 600 | 74.5s, valid parse, eval=56 tokens |

qwen3:4b is a reasoning model. Under an imposed JSON schema with thinking
enabled it does not converge, so dspy's adapter never receives parseable output
and retries — at ~145s per attempt, indistinguishable from slow progress. The
earlier latency probe pointed the same way: the `/no_think` *prompt tag* is
ignored (144.7s, 417 reasoning chars emitted anyway); only the API-level
`think=False` field suppresses it. Setting `think=False` at the ollama layer is
what fixes it, and dspy 3.3.1 does not expose that field through `dspy.LM`.

So arm B calls ollama's native structured output directly. **Pydantic is
retained and is doing the real work** — it defines the schema constraining
generation and validates every response; that was the part of the plan worth
investigating, and it holds up. Only dspy's adapter negotiation is dropped.

**The result that makes arm B doubtful before it runs.** With thinking
disabled, the probe returned for HEAL/LIFT:

    ['deadlift','squat','heavy','morning','barbell','session','press','overhead','5x5']

The anchor hints were "deadlift, squat heavy" and the examples "morning barbell
session; press overhead 5x5". All nine terms are copied verbatim from the
prompt, against an explicit instruction to produce words for OTHER entries and
not to restate the examples. Suppressing reasoning appears to cost exactly the
capability this arm depends on.

That is one cell, so the generator now records, per cell, what fraction of its
terms appeared in its own prompt (`generated_echo`). If the echo rate is near
1.0 across the grid, arm B is not a weaker result than arm A — it is a strictly
worse arm A, since arm A at least ranks IDF over the whole train half rather
than repeating eight examples. Measured, not asserted.


### Stage 6c — the two corpora were not the same corpus

Arm A's first artifact was built by globbing `physis-core/config/*.json`: 885
entries over 38 cells. The Rust scorer loads 659 entries. The generator and the
scorer were reading different corpora, and nothing in either would have said so.

`load_all` resolves through `BUILTIN_ONTOLOGIES` and `EXTRA_ONTOLOGIES` — 29
files. The glob picked up all of `config/`, including `space_ontology.json` and
`time_ontology.json`, which appear in neither static and which the concurrent
config refactor had just added. Those two files carry entries in cells that ARE
loaded, so they did not merely add unscored rows: they changed the parity of the
even/odd split on **12 of 33 shared cells**, and added a `STUDY/PLAN` cell that
does not exist downstream.

Two consequences, one procedural and one substantive.

Procedurally, this would have silently invalidated the A-vs-B comparison. Arm A
was generated before the refactor and arm B after, so the two arms would have
been built from different train halves and scored against different held-out
sets, while every label in the artifact claimed they matched. The comparison
would have produced a number, and the number would have meant nothing. It was
caught only by diffing the regenerated split against the committed one before
trusting it.

Substantively, arm A's reported −0.007 was measured with a vocabulary derived
partly from entries the scorer never sees, holding out names absent from the
scored corpus. It has been re-measured on the corrected corpus.

`load_list()` now parses the two statics out of `src/ontology.rs`, so the
generator's corpus is the loader's corpus by construction and stays correct if
those lists change. Corrected figures: 659 entries, 35 cells, train 338 /
test 321.

The general form of this is worth stating, because it is the third distinct way
the same experiment nearly produced a false positive today: **an evaluation is
only as trustworthy as the agreement between the thing that generates and the
thing that scores.** The split was principled, the statistic was sound, the
control was real — and a glob pattern quietly made them describe different data.


### Stage 6d — the sweep re-run on the corrected corpus

Everything in Stage 6 was re-measured after the corpus fix, on 659 entries with
a 321-name held-out set. The conclusions do not change; two of the numbers do,
and one of them is a correction to this report rather than a refinement.

| population | scorer | mean Δ vs cosine | wins | paired t |
|---|---|---|---|---|
| held-out | CONSTRAINT | −0.056 | 1/11 | −3.08 |
| held-out | VOCAB-FREQ (A) | −0.009 (sd 0.017) | 4/11 | −1.81 |
| all entries | CONSTRAINT | −0.059 | 1/11 | −6.06 |
| all entries | VOCAB-FREQ (A) | **+0.121** | **11/11** | **+12.97** |

CONSTRAINT is now 1/11 on *both* populations and significantly worse than plain
cosine on both (t = −3.08 held-out, t = −6.06 overall). On the mismatched corpus
it had been −1.77 on the held-out half, i.e. not significant. **Fixing the
corpus strengthened the refutation.** Iteration 38's structural soundness check
is not merely unsupported; it is measurably worse than distance.

Arm A's held-out null is unusually clean: sd 0.017 across eleven perturbations,
clustered on zero. The same vocabulary scored against the entries it was built
from reads +0.121 at t = +12.97. **A 0.130 swing produced by nothing but which
rows you score.**

**Correction to Stage 6.** That section reported the harness noise floor as
0.121, from the cosine baseline ranging 0.565–0.686. That range was measured on
the mismatched corpus and is wrong. On the corrected corpus the baseline spans
0.621–0.695 held-out and 0.607–0.682 overall: **a noise floor of 0.074**. The
qualitative claim is unchanged and still governs — Iteration 38's 0.599-vs-0.650
and 0.654-vs-0.619 both sit inside 0.074 — but the figure to carry forward is
0.074, and single-run margins below roughly 0.05 in this harness remain
uninterpretable.


### Stage 6e — arm B completed: the LLM's contribution is zero, and structurally so

Arm B ran to completion over all 35 cells: 450 terms, temperature 0,
`think=False`, ollama native structured output under the Pydantic `CellVocab`
contract. Arms A and B share a byte-identical split and an identical
`frequency` vocabulary, so the comparison isolates the model.

**The generator's own measurement, before any scoring.** Echo rate — the
fraction of proposed terms that appeared verbatim in the prompt that requested
them — is **0.951 overall (428/450), median 1.00, with 19 of 34 cells at
exactly 1.00**. Each cell's prompt supplied its anchor and five real entries and
asked for words appearing in OTHER entries, explicitly forbidding repetition.
In the majority of cells the model returned its input unchanged.

**Scored, 11 perturbations, identical rows:**

| comparison | population | mean Δ | wins | paired t |
|---|---|---|---|---|
| B vs A | held-out | −0.062 | 0/11 | −6.50 |
| B vs A | all | −0.185 | 0/11 | −39.18 |
| B vs cosine | held-out | −0.071 | 0/11 | −9.00 |
| B vs cosine | all | −0.064 | 0/11 | −6.89 |

Arm B loses every trial to arm A on both populations, and loses to plain cosine
as well: it is worse than not scoring at all. **The question the design was
built to answer — "does B beat A" — has the cleanest possible answer, 0/22.**

**A second, independent confirmation of the mechanism.** On the contaminated
`half=all` population arm A gains +0.121 while arm B gains −0.064. A vocabulary
genuinely derived from the training half would inherit A's contamination
advantage. B does not, because B only ever saw *five* examples per cell and
echoed them: it recognises five training entries where A's IDF ranks 338. The
absence of the contamination signature measures how little the model added.

**What this closes.** The Iteration 38c prerequisite was a controlled vocabulary
in the entries' own idiom. Both ways of building one have now been tested. Arm A
(no LLM) is at chance on held-out data. Arm B (LLM) is significantly worse than
both A and cosine. The prerequisite is not merely unmet — the two available
routes to meeting it are measured and closed. CONSTRAINT does not become viable
by being given a better vocabulary.

**The honest caveat.** This is one 4B model on local hardware, and its failure
mode is entangled with its deployment: qwen3:4b does not terminate under a JSON
schema with reasoning enabled, and disabling reasoning is what produces the
echo. On this hardware those are not separable, so the finding is "the locally
available model contributes zero", not "no LLM could". A larger model that can
hold a schema while reasoning has not been tested and is the only untried
variant of this arm. Given eleven prior mechanism failures and a 0.074 noise
floor, it is not obviously worth the attempt.


### Stage 7 — the corpus is the problem, and it was visible all along

Arm C was going to be "a larger model plays the generator". Dumping the exact
prompts arm B received made that unnecessary, because the prompts show the
answer:

    CONSTRUCT/WORK  = "pour concrete, frame the wall, lay bricks, do the build"
      contains: Access Control, Booking & Logistics, Code (semiotics),
                Database Engineering, Dicisign (Peirce)

    BOND/WORK       = "resolve conflict, mediate, repair the bond"
      contains: Account Management, Agent Versioning, Bus & Backplane

    HEAL/REST       = "rest day, recover, sleep deeply, calm down"
      contains: Firstness (Peirce), Power Regulation,
                Semiconductor Cleanroom Environment

Small cells are fine — CONSTRUCT/REST holds Build Platform, Fermo Impianto,
Infrastructure Standby, Workshop Wind-Down, all genuinely about being idle. The
large cells have become junk drawers.

**Measured, not asserted.** Every 11th entry by (cell, name) — 60 of 659,
deterministic, no cherry-picking — adjudicated against its own cell's anchor
definition. Full verdicts in `cell_adjudication.tsv`.

| verdict | n | share |
|---|---|---|
| YES — clearly belongs | 19 | 31.7% |
| MARGINAL — defensible loosely | 15 | 25.0% |
| **NO — clearly does not belong** | **26** | **43.3%** |

**Why this ends the investigation.** The audit injects misfilings and asks each
scorer to find them. At a 43.3% ambient rate, 572 scored entries contain ~248
real misfilings that the ground truth labels CLEAN:

| injected | as % of corpus | signal : ambient noise |
|---|---|---|
| 35 | 6.1% | 1 : 7.1 |
| 81 | 14.2% | 1 : 3.1 |
| 114 | 19.9% | 1 : 2.2 |

**Every scorer was asked to find 35 planted misfilings while 248 real ones sat
in the same cells, labelled correct.** A scorer that works — that genuinely
detects a bad filing — is *penalised* for every ambient misfiling it flags,
because the ground truth scores those as false positives. This is sufficient on
its own to explain every null in this report, and it explains them better than
any property of the mechanisms:

- ANCHOR-VOCAB at 0.505: the anchors describe the cell; 43% of the entries are
  about something else entirely.
- Arm A at chance: IDF over a cell's entries cannot find shared vocabulary in a
  cell whose members are Access Control, Booking & Logistics and Peirce.
- CONSTRAINT below cosine: it reasons from cell co-membership, which is largely
  arbitrary here.
- The grid beating a size-matched shuffle by only 0.657 vs 0.642: exactly what a
  mostly-arbitrary assignment with a coherent minority looks like.
- The 0.074 noise floor: ambient misfilings move with the perturbation.

**What this retracts and what it does not.** It does not rescue any mechanism —
none has been shown to work. It does retract the *diagnosis*. Twelve mechanisms
were rejected for failing to detect misfilings against a ground truth that was
itself ~43% wrong. Those rejections are not safe: the experiment lacked the
power to distinguish a working scorer from a broken one. **The honest status of
Iterations 28–40 is UNTESTED, not REFUTED.** The 0.1.18 status notice stands —
the crate does not do what it claims, and that is now better supported than
before, since the labels it classifies against are themselves unreliable.

**The methodological point, which is the day's real lesson.** Every control
added today — the train/test split, the resampled injection, the corpus-derived
load list — tightened the *measurement*. None of them questioned the *labels*,
and the labels were the broken part. Three sophisticated controls on top of a
43%-wrong ground truth produce precise, reproducible, well-controlled noise. The
cheapest possible check — reading sixty rows of the input — outperformed all of
them, and was not run until the very end of a multi-day investigation.

**What to do next, if anything.** Not a thirteenth mechanism. Either fix the
filing (hand-adjudicate the corpus, or regenerate it with the disposer in the
loop) or stop. No scorer can be evaluated on this corpus as it stands.


## Stage 8 — the re-test plan

Stage 7 invalidated the *evaluation*, not the *code*. Nothing below needs
rebuilding; it needs re-running against labels that mean something. This section
says which results are affected, in what order to redo them, and what must
happen first.

### Gate 0 — fix the filing, or stop. Nothing else runs before this.

No scorer can be evaluated on a corpus that is 43.3% misfiled. Re-running any
experiment before this gate reproduces the same underpowered result with more
decimal places.

Three options, in descending order of how much they'd be trusted:

1. **Hand-adjudicate the 659 loaded entries.** At the Stage 7 rate (~60 entries
   per pass) this is ~11 passes. The disposer stays human, per the standing
   decision; `cell_adjudication.tsv` is the format and the first 60 rows are
   already done. **Cost: hours. Value: an actual gold standard**, which this
   project has never had for its own grid.
2. **Restrict to the coherent subset.** Small cells adjudicate cleanly
   (CONSTRUCT/REST, CONSTRUCT/LEARN, FABRICATE/REST, FABRICATE/LEARN). Evaluate
   only on cells whose sampled entries are ≥80% YES. Cheap, but shrinks the
   corpus and biases toward easy cells — a lower bound, and must be labelled as
   one.
3. **Regenerate the filing with the disposer in the loop.** Most work, and the
   only option that also fixes the shipped product rather than just the
   experiment.

**Kill criterion for the whole track:** if hand-adjudication of a further 120
entries puts the clear-misfiling rate above ~25%, the corpus is not repairable
by editing and option 3 or abandonment are the only honest choices.

### What must be re-tested (ground truth was the cell assignments)

| # | result | current verdict | why it is unsafe |
|---|---|---|---|
| 1 | Nine geometric certification mechanisms | NOT SUPPORTED | all scored against cell labels |
| 2 | Non-lattice auditing ("the grid is not a taxonomy") | NOT SUPPORTED | the shuffle control compares one noisy labelling to another; 0.657 vs 0.642 is what ~43% noise predicts |
| 3 | Iteration 38 family: CONSTRAINT, ANCHOR-VOCAB, arms A and B | REFUTED / at chance | the injected-misfiling harness, signal:noise 1:7 |
| 4 | Sense induction, three ways (Iterations 39–40) | killed | used cell labels as the sense inventory |
| 5 | Re-derivation vs persistence (ARI 0.198) | holds | ARI is computed over the same suspect labels |
| 6 | Iterations 32, 33, 34 convergence/interpolation controls | killed | evaluation population defined by cells |

**Re-test order.** #3 first — it is fully automated, the harness already
supports resampling and held-out scoring, and it is the only one with a
measured noise floor (0.074) to beat. If CONSTRAINT still loses to cosine on a
clean corpus, it is genuinely refuted and #1, #2 and #6 can be closed without
re-running, since they are weaker versions of the same claim. If it wins,
everything here reopens.

### What stands, and is probably understated

Label noise cannot manufacture a positive that survived a construction-matched
control, so these need no re-test — but their effect sizes are lower bounds:

- **The determinism fix.** A code-level fact, verified by sabotage. Unaffected.
- **`becoming`'s runs test.** Unit-proven on synthetic sequences. Unaffected.
- **`chronos` periods.** Measured from timestamps, not labels. Unaffected.
- **Embedder swap destroys agreement (ARI ~0.10).** Label-free; it compares two
  clusterings to each other.
- **WordNet gloss corroboration, z = +6.64.** This one is *strengthened*: it
  found same-cell corroboration from an independent source **through** 43% label
  noise. On a clean corpus it should be larger.
- **n-gram discretization** and the exact-vs-skip-gram result. Operates on the
  operational corpus text, not the grid.
- **`coverage`.** Evaluated by construction (re-adding an existing entry rescues
  0 of 2000), not by cell correctness.

### What the day cost, stated plainly

Twelve mechanisms were built and rejected against a ground truth nobody checked.
The check took one pass over sixty rows and could have run on day one. Every
control added along the way — train/test split, resampled injection,
corpus-derived load list — improved the precision of a measurement whose labels
were wrong, which is the most expensive way to be careful.


## Stage 9 — could physis have predicted this? Measured: no.

The obvious hope after Stage 7 is that physis triages its own corpus: rank all
659 entries by how badly each fits its assigned cell, and hand the disposer the
worst 150 instead of all 659. The machinery exists — `coverage::uncovered()` is
built for exactly this, and the audit's cosine scorer reaches AUC ~0.650 on
injected misfilings.

The 60 Stage 7 adjudications are a labelled validation set, so this is testable
rather than arguable. Injection disabled (`PHYSIS_AUDIT_STRIDE=0`), every entry
scored against its assigned cell, joined to the verdicts (50 of 60 are in the
scored population; the rest lack WordNet anchoring). Per-entry scores in
`corpus_fit.tsv`.

| scorer | AUC detecting REAL misfilings | 95% CI |
|---|---|---|
| **COSINE fit to assigned cell** | **0.410** | [0.251, 0.569] |
| CONSTRAINT | 0.525 | — |
| ANCESTRY | 0.493 | — |

**Chance sits inside the interval. Physis cannot find its own misfilings.** The
point estimate is below chance and the direction is backwards — mean fit is
**0.196 for correctly-filed entries and 0.213 for misfiled ones** — and the
single worst-fitting entry in the entire corpus is `Companionship` in BOND/REST
("keep company, sit together, shared silence"), which is filed exactly right.

**The most useful number here is the gap: 0.650 on injected misfilings, 0.410 on
real ones.** The same scorer, the same corpus, the same cells. Synthetic
misfilings are detectable; real ones are not.

This report predicted that and then ignored it. From the Iteration 38 footnote,
written weeks before Stage 7:

> *"A synthetic perturbation is a lower bound on difficulty — real misfilings are
> the ones a careful author already thought were right."*

That is now measured, and the effect is large enough to invert a scorer. An
injected misfiling moves an entry to a random cell, producing an obvious
mismatch. A real misfiling is a plausible-looking judgement someone made on
purpose — "Collaboration" in BOND/WORK, "DevOps" in CONSTRUCT/WORK — and
plausible is precisely what embeddings score highly.

**Consequences for Stage 8.** Gate 0 cannot be automated. Options 1 and 3
(hand-adjudicate, or regenerate with the disposer in the loop) are the only
routes; there is no cheap machine triage to shrink the queue, and the ranking
must not be used to prioritise review because it is anti-correlated with being
wrong. Option 2 (restrict to coherent cells) survives, since it is judged by
cell rather than by entry.

**And it answers the standing question about the disposer directly.** The
disposer was kept human on principle. It turns out to be required in fact: on
this data the machine's judgement of "does this belong here" is at chance, while
a human pass over sixty rows found the defect that twelve mechanisms and four
days of controlled experiments had missed.

