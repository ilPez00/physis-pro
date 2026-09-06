# Final report — perspective-conditioned ontology discovery + JEPA

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

1. **Plain unsupervised clustering** — the one mechanism this track never
   found a way to make lose on exclusive-membership data. If anything is
   worth prototyping in `discovery.rs`, it's a from-scratch
   (non-grid-anchored) clustering mode.
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
4. A properly hyperparameter-tuned (validation-split-gated, not
   toy-corpus-carried-over) rerun of Iteration 4's predictor, since its
   chance-level loss is consistent with undertraining and not fully
   excluded as the explanation for its loss.
5. A genuinely self-supervised JEPA test (predict context from natural
   co-occurrence, no category labels anywhere) — needs a corpus with real
   contextual structure, which none built in this track provide; not
   attempted because building one properly is a materially larger
   undertaking than any single iteration here, not because it was judged
   unpromising.
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
