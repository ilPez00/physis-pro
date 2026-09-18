# Quantitative event streams: research specification

Date: 2026-09-17. Status: PROPOSED / NOT MEASURED.
This document specifies experiments, not implemented capabilities or results.
It extends [Phase 27](../PLAN.md#phase-27--measured-structure-of-change-2026-09-17)
and the existing MS packets. It authorizes neither agent dispatch nor production
changes. Q1–Q8 are local experiment labels, not replacements for existing E IDs.

## 1. Question and priority

Can inexpensive statistical methods identify stable relative relationships,
changing operating conditions, predictive event influence and better context
allocation beyond Physis's existing signals?

Financial analogies are candidate methods, not explanations of language.
Useful forecasting is not causal identification. A stable statistical residual
is not a transformation invariant. Neither proves that recomputation can stop.

Execution preference after the measurement contract:
1. Q6 context allocation: direct operational objective and strong MMR baseline.
2. Q2 change-points plus Q1 EWMA: small state, interpretable diagnostics.
3. Q5 Hawkes: conditional on reliable timestamps and sufficient events.
4. Q4 cointegration: priority invariant investigation, conditional on admissible
   trajectories rather than applied automatically to all semantic features.
5. Q3 factors and Q7 tail impact only with suitable sample size and outcomes.
6. Q8 routing after individually validated signals and real worker outcomes.

Scientific importance and readiness differ: cointegration remains a primary
invariant hypothesis even when data prerequisites delay execution.

## 2. Reuse and packet ownership

| Existing seam | Research use | Limitation to verify |
|---|---|---|
| physis-core/src/observe.rs; src/ledger.rs | Events, timestamps, ancestry, provenance | Recorded ancestry is not complete computational dependency |
| physis-core/src/hypothesis.rs; epistemic.rs | Evidence survival, revisions, outcomes | Historical records may lack required features or pre-fix transitions |
| physis-core/src/transform.rs; relation.rs | Symbolic constraints, typed relation candidates | Substitution preservation is partly true by construction |
| physis-core/src/delta_engine.rs | Replay, mutation reports, declared dependencies | Separate inspected from changed state; include global coherence work |
| src/dependency/; semantic_delta/ | Existing affectedness and delta representations | Heuristics must not become ground truth |
| physis-core/src/becoming.rs; ngram_table.rs | Drift and transition baselines | Verify live consumers and data joins |
| src/rag.rs; pack.rs; nav/ | Candidate retrieval and context budgets | Keep candidate pools and tokenizer consistent |
| src/quality.rs; existing quality engine | Feedback and standing | Observational feedback is selected, not randomized |
| benchmarks/behaviour/ | Session records and established evaluation helpers | File-touch order alone cannot supply elapsed time |
| src/orchestration/; existing agent integrations | Potential routing seam | Verify actual workers; placeholders cannot measure task success |

Recheck paths, implementations, callers and tests at the pinned revision.
Do not treat this table as a wiring certificate or create parallel engines.

| Packet | Additional research responsibility |
|---|---|
| MS-001 | Event/state schema, admissibility, missingness, data manifest |
| MS-002 | Independent mutation/output oracle; labels for Q1/Q7 |
| MS-003 | Q2/Q3/Q4; representation mappings and cross-scale survival |
| MS-004 | Tail diagnostics, dependence tests, multiple-testing protocol |
| MS-005 | Q6 and Q5 prioritization ablations; preserve correctness fallback |
| MS-006 | Interpret excitation/cascades without assuming criticality |
| MS-007 | Q8, only after usable preceding evidence |
| MS-008 | Independent promotion review; no automatic production extension |

These are research-design additions. Existing packet ownership and release
gates still apply. A worker must obtain a reviewed scope amendment before
adding an experiment not covered by its current contract. Do not silently widen
a claimed packet or restart completed work. No new packet fleet is necessary.

## 3. Measurement contract before fitting

Use existing event IDs and state representations where available. Research
exports should retain:
- event identity, source, actor, project/session/task, event type and object;
- event time, ingestion time, clock resolution, timezone and ordering ambiguity;
- declared parent IDs separately from inferred predictive influences;
- before/after values and units, relation type, representation/model version;
- observable outcome, availability time, token/CPU/latency accounting;
- missingness, censoring, duplicate/retry markers and observation coverage.

Distinguish event time, ingestion time and event index. Never invent timestamps
from sequence numbers. Missing observation is not zero activity. Handle retries
and duplicate emissions before estimating excitation. Do not expose private
trace content merely to label an experiment.

Define each x_t(c) explicitly: windowed activation count, exposure-normalized
rate, retrieval membership, relevance score, hypothesis fitness, relation
presence or dependency impact. Keep unlike units separate. Use ordinary
differences by default. Log ratios require nonnegative quantities and a
declared zero/pseudocount policy; they do not apply blindly to signed cosine.

Freeze resampling/bin widths on training data. Record changing denominators:
corpus growth, active-agent count, observation uptime and candidate-pool size.
Retain raw measurements alongside training-fitted normalization. Embedding
changes, ontology remapping and instrumentation changes are explicit breaks,
not unexplained semantic events.

Before each model, examine mean/variance drift, missingness, autocorrelation,
cross-correlation, tails and sensitivity to time binning. Record denominators
per project/window. Finite-window stationarity tests do not certify permanent
stationarity. Insufficient history is a valid blocker.

## 4. Common validation protocol

Preregister hypothesis, outcome horizon, availability time, baseline, model
family, search budget, tolerances and stopping rule before viewing holdout.
Use train-past/test-future walk-forward folds, with project/session holdouts
for transfer. Fit vocabulary, scaling, factors, covariance and thresholds on
training data only. Purge overlapping labels/windows across fold boundaries;
embargo where the outcome horizon would leak future information.

Report all attempted configurations and all eligible projects, including
failures. Use nested training/validation for tuning. Define multiple-testing
families in advance; apply a justified FDR or family-wise correction. Use
paired project/session or block uncertainty estimates appropriate to dependence,
not independent-event bootstrap by default.

Nulls must preserve nuisance structure and destroy the claimed mechanism:
within-source order permutation, frequency-preserving resampling, time
reversal, graph rewiring or phase randomization as appropriate. Bootstrap
estimates uncertainty; it is not automatically a mechanism-destroying null.
Time reversal alone is not causal identification.

Primary baselines: frequency, recency, BM25, cosine, RRF, MMR, transition
counts, rolling averages and existing structure/order signals where applicable.
Give each arm equivalent inputs, budgets and tuning opportunities. Report
incremental value after frequency, length, source, format, recency and project
size confounders. Fit residualization only on past data.

Cost includes extraction/embedding, fitting, updates, covariance construction,
candidate retrieval, selection, maintenance and downstream calls. Report cold
start separately from amortized steady state; no free full-history scan.

## 5. Q1 — Volatility as early warning

Hypothesis: recent magnitude of change predicts future material changes or
failures beyond recency, activity volume and recent failures.
Start with existing deltas and EWMA:
v_t = lambda v_(t-1) + (1-lambda) delta_t^2.
Compare rolling absolute delta, rolling variance, MAD and a fixed threshold.
Tune lambda/window length on training periods only.

Labels: independently measured changed-output events, dependency expansion,
contradictions or failed task tests in a frozen future horizon. A model's own
threshold crossing is not an independent success label.
Metrics: precision-recall, calibration, warning lead time, false alerts per
session, and recall at fixed review budget. Report alert availability time.

Test raw delta autocorrelation against magnitude/squared-delta dependence,
controlling for activity-rate changes and representation breaks. Only then
consider GARCH against EWMA. Reject added complexity without incremental
held-out prediction and operational savings.
Check: a known injected disruption can be detected without using its future
label; quiet controls do not automatically trigger alerts.

## 6. Q2 — Change-points and working regimes

Start with one cheap online CUSUM or Page-Hinkley detector. Compare fixed
windows, no-change, simple recent-activity thresholds and shuffled-order
controls. PELT is an offline segmentation comparator, not an online arm that
may inspect the future. BOCPD/kernel methods require an explicit complexity
comparison before expansion.

Features may include tool/action mix, retrieval distribution, edit/test rates,
delta magnitude and source/task boundaries. Predefine whether a task boundary
is an input or a target; never use it as both.
Ground truth: independently annotated boundaries or recorded task transitions,
with an allowed timing tolerance and adjudication of ambiguous cases.
Metrics: boundary precision/recall, delay, false alarms and downstream retrieval
or next-action value.

A change detector locates statistical shifts. It cannot name them research,
planning or debugging without a separately evaluated label mapping. LLM phase
labels, if already available, are comparators rather than truth.
HMM/switching models are conditional extensions only if transition tables plus
change-points lose on held-out operational outcomes.
Check: step change versus smooth drift, missing intervals and source switches.

## 7. Q3 — Factors and exposure

Start with PCA; use NMF only for appropriate nonnegative measurements.
Incremental PCA follows only if batch analysis earns its maintenance cost.
Robust PCA or ICA needs a stated noise/source assumption and a matched control.

Fit factor count on training/validation data. Compare with frequency-selected
features, existing ontology grouping and a matched-dimensional projection.
Measure reconstruction AND task utility, compression, update cost and
cross-project/window stability. Explained variance alone cannot pass.
Account for sign/permutation/rotation ambiguity when comparing factor spaces.
Label factors by top-loading source objects; labels do not establish semantics.

For exposure r_i,t = alpha_i + beta_i f_t + epsilon_i,t, estimate beta using
past data and distinguish common movement from residual novelty. Do not
equate high loading with causal influence or low loading with useful novelty.
Check whether exposures add to degree/frequency on future impact prediction.

## 8. Q4 — Cointegration and relational residuals

Hypothesis: some drifting state variables retain a stable relative combination
that forecasts relationship breakdown or useful future structure.

Admissibility comes first. Standard Engle–Granger assumes individually I(1)
series and tests whether a fitted combination is stationary. Bounded activation
shares/cosines, deterministic trends and cumulative activity totals need
separate treatment; common corpus growth is an obvious confounder. Do not
accumulate counts simply to manufacture integrated series.
Use complementary ADF/KPSS-style diagnostics with declared intercept/trend,
lag selection, sampling and structural-break handling. Reject unsuitable
candidates or use a simpler stationary relationship model instead.

Begin with a small preregistered pair set grounded in existing typed relations.
Estimate intercept/beta on training data; apply cointegration-appropriate
residual critical values, not an ordinary ADF interpretation after arbitrary
regression. Correct the pair-search family for multiple testing. Johansen is
conditional on enough observations for the multivariate dimension and lag order.

Compare with correlation, shared-factor residuals, frequency-matched pairs,
existing transform relations and independent random walks. Include a known
cointegrated synthetic positive and common deterministic-growth negative.
Hold out future periods, projects and meaningful transformations separately.

Track spread s_t = x_t - a - beta*y_t using past fitted parameters. Standardize
with training/past moments and guard zero variance. Excursions are alerts,
not contradictions by definition. Test against independent relation-break
labels, predictive utility and false alerts; report coefficient stability.

Paraphrase, scale, model and ontology changes require explicit identity
correspondence and measurement mappings. Distribution stationarity does not
prove relational preservation across those mappings. No spread can authorize
a propagation barrier without a downstream dependency contract.

## 9. Q5 — Event excitation and propagation

Use actual timestamped events, beginning with few event types and an
exponential-kernel Hawkes model:
lambda_j(t) = mu_j(t) + sum_i sum_(t_k^i<t) alpha_ij*g(t-t_k^i).

Declare kernel normalization and units. Integrated kernel mass, not an
arbitrary alpha coefficient alone, determines the branching interpretation.
For multivariate stationary linear models examine the integrated-kernel
matrix's spectral radius under the model assumptions; do not substitute graph
degree. Self-excitation cannot model every inhibitory relationship.

Compare homogeneous and time-varying-background Poisson models, transition
counts, recency, co-occurrence and source-conditioned rates. Session starts,
scheduled work and concurrent agents may create common-rate bursts.
Retain censoring and event-time ties; do not assign arbitrary ordering to
simultaneous events without sensitivity analysis.

Metrics: future-event log score, calibration/time-rescaling diagnostics,
next-event prediction and held-out actual-impact prioritization. Compare
shuffled-order and time-reversed controls without claiming either proves
causality. Restrict dimensionality/regularization using training data.

Inferred parent probabilities produce inferred cascades with uncertainty.
Keep those separate from recorded causal/dependency chains and from measured
state changes in MS-002. Report sizes/depth/durations with boundary effects.
A fitted branching statistic near one does not demonstrate engine criticality.
Check: independent rate bursts should not automatically become mutual
excitation; a seeded excitation process provides a positive control.

## 10. Q6 — Context allocation under a token budget

Hypothesis: redundancy-aware allocation improves answer-bearing coverage and
task utility at equal total token budget beyond MMR and utility/token greed.

Reuse the existing candidate generator, token counter and packing output.
Use binary chunk selection w_i in {0,1}, with
maximize mu^T*w - lambda*w^T*Sigma*w, subject to sum C_i*w_i <= B.
Define expected utility using past labels or an explicitly uncalibrated score;
never read test answers to derive it. Define whether Sigma is actual covariance
of utility, pairwise overlap, or a similarity penalty. These are different
objects; a generic similarity matrix is not automatically a PSD covariance.

Freeze diagonal treatment, relevance scaling and lambda on training data.
Consider supporting/complementary chunks, source provenance and mandatory
dependencies: duplicate wording may still supply independent corroboration.
Require source independence evidence before calling diversity support.

Compare BM25, cosine, RRF, existing TokenFixedRetriever, greedy utility/token
and MMR under the same candidate pool and token budget. A second end-to-end
comparison may vary the pool but must report candidate recall separately.
Start with greedy marginal utility/token selection; use heavier optimization
only if its quality gain exceeds solver and matrix costs.

Metrics: answer-in-context, recall, coverage, redundancy, source concentration,
tokens, latency, memory and independently tested downstream outcomes.
Report quality-cost frontiers, not quality/token alone: tiny low-quality contexts
can flatter ratios. Include empty candidates, over-budget chunks, near
duplicates, rare evidence and two chunks jointly required for an answer.
Any source cap or HHI penalty is an ablation, not an automatic warning rule.

## 11. Q7 — Tail impact and fragile support

Use independently observed files/tasks/outputs affected, failures or actual
compute, preserving the distinction between consequence and occurrence rate.
Start with empirical quantiles and held-out coverage, not EVT.
Report sample counts, censoring and uncertainty at each tail probability.

Compare power law, truncated power law, lognormal, exponential and appropriate
stretched exponential using common support and likelihood measure, following
MS-004. POT/GPD is conditional on adequate exceedances, threshold stability
and dependence/declustering diagnostics. Extreme-value modeling is not
restricted to heavy tails; test whether its assumptions fit this population.

For discrete losses use a quantile-integral definition of expected shortfall
or fractional weighting at the boundary; E[L | L > VaR] can misstate the tail
when many observations equal the quantile. Validate exceedance frequency and
tail-loss forecasts against simple percentile heuristics.

Support diversity/liquidity and HHI are candidate features. Trace shared
upstream sources so copied documents do not masquerade as independent support.
High concentration may be justified by one authoritative source; test outcome
value before imposing diversification or alerts.

## 12. Q8 — Walk-forward compute allocation

Only consume individually validated signals. Begin with one calibrated
threshold or simple normalized risk score; compare with static roles and
historical failure priors before complex optimization.
Use real workers with independent task tests. Compare strong everywhere,
cheap everywhere, static routing, impact-only and incremental signal ablations.
Include exploration to measure regions historically skipped by the router.

Report tokens-to-passing-task alongside success rate, failed/unresolved tasks,
rework, test failures, corrections, escalation, cost and wall time. Do not
report costs only among successful runs. Control task difficulty, tools,
context, model version and random variation. Observational logs alone do not
supply outcomes for workers that were never chosen; require paired runs or a
properly designed randomized/logged-policy evaluation.

Extra compute should follow estimated incremental outcome benefit versus cost.
Expected impact alone does not tell whether a stronger model helps.
Risk parity and Kelly analogies remain optional descriptions, not production
formulas. No unconstrained fractions, paid calls or automatic model escalation
are authorized by this research document.

## 13. Multiscale, online and interface extensions

Repeat only admissible experiments across explicitly mapped token/phrase/
sentence/paragraph/document/concept/session/project representations. Maintain
physical time when comparing event rates; event-index coarse-graining changes
the clock. Compare cross-scale factors as aligned subspaces and relations as
mapped predicates, with mapping-destroyed controls and held-out scales.

Long-memory analysis (Hurst, DFA, autocorrelation or information decay) must
separate trends, breaks, finite samples and missing intervals from persistence.
Use known short/long-memory controls. Persistent descriptive dependence does
not by itself prove that a longer context budget improves tasks.
Lead-lag and Granger-style regressions require past-only baselines, shared-driver
controls, correction for lag/pair search and out-of-window predictive gains.
Call their output predictive influence, never true causality.

Microstructure-like arrival rates, task pressure, contention and support-minus-
contradiction imbalance are optional observable features. Define polarity and
exposure explicitly; include zero-denominator handling and source confounders.

Prefer streaming forms only after benefit is established. Measure cost per
event and scaling with features, history and active edges; do not assume
O(delta) for covariance or Hawkes updates. Bounded histories, sparse candidate
edges and periodic recalibration must appear in cost and correctness reports.

Notebook overlays are deferred until useful signals pass independent review.
Show a compact status with timestamp, uncertainty and provenance hyperlinks;
keep statistical detail behind drill-down. No raw beta/volatility values
without units, window, baseline and limitations.

## 14. Decisions, artifacts and stop conditions

For each Q experiment, record:
1. Frozen hypothesis, applicability checks, outcome, baseline and decision margin.
2. Manifest/revisions, missingness, folds, tuning budget, seeds and versions.
3. One reproducible command and a check that rejects a deliberately wrong arm.
4. Per-fold/project measurements, denominators, uncertainty and all negatives.
5. Total cost, failure cases, transfer limits and independent reviewer decision.

Outcome states: positive, negative, inconclusive, blocked. Research completion
does not imply promotion. A positive needs a preregistered meaningful held-out
gain over the strongest relevant cheap baseline, acceptable failure rates and
net value after maintenance costs. Do not invent universal effect thresholds;
freeze task-specific margins before evaluation.

Demote when the signal disappears under valid controls, merely duplicates
frequency/cosine, fails transfer, is tuning-sensitive, lacks operational gain
or costs more than it saves. Lack of sample power is inconclusive, not proof
of absence. Never repair a failed result by changing its metric after holdout.

The earlier research/criticality/run.py and results.json remain invalid draft
evidence; they are neither baseline nor reusable statistical authority.
This specification creates no results. Promotion stays in MS-008 and a
separately authorized minimal implementation packet.

## 15. Method references

These describe methods, not evidence that they work in Physis:
- [Engle–Granger API assumptions, statsmodels](https://www.statsmodels.org/v0.11.1/generated/statsmodels.tsa.stattools.coint.html).
- [Hawkes processes with a time-dependent background rate](https://arxiv.org/abs/1702.04443).
- [Sparse and low-rank multivariate Hawkes processes](https://www.jmlr.org/beta/papers/v21/15-114.html).
- [Goldstein and Carbonell: MMR and diversity-based reranking](https://aclanthology.org/X98-1025/).

Before implementation, verify numerical-library documentation and primary
method assumptions for the exact pinned version. Prefer existing numerical
tools in research code; do not hand-roll hypothesis-test distributions or
promote numerical dependencies into production without evidence.
