# Hostile review pass

Date: 2026-09-24
Scope: the seven manuscripts in `research/papers/`.
Reviewer: the author (see "Reviewer independence" at the end — this is a
weakness of this pass, not a detail).

## Method

Each claim was recomputed from the stored result file rather than read from
the manuscript. Checks run: leakage/circularity in centroid and prior
construction; the bootstrap unit in the code that produces the cited CIs;
every 3-decimal number in every paper traced to a result file; independent
recomputation of headline counts and ratios; citation metadata verified
against Crossref or arXiv; internal contradictions searched for by
comparing sentences within a paper.

Automated trace checks are in `verify_paper1.py`, `verify_papers234.py`,
`verify_paper4.py`, `verify_papers910.py`, and `check_bootstrap_unit.py`.

## Findings that required a fix

### 0. paper4 — the sequential split destroys the headline gap [BLOCKER, fixed]

The paper named a temporal split as an unrun control and disclosed that its
published figures came from a *random* split. That control has now been run
(`e21_sequential_split.py`, results in `e21_sequential_split.json`), and it
was the single most damaging finding of this pass.

| frac | train | test | geometry top-1 | top-3 | top-5 | prior top-3 | majority top-3 |
|---|---|---|---|---|---|---|---|
| 0.10 | 58  | 524 | 0.195 | 0.345 | 0.422 | 0.250 | 0.218 |
| 0.25 | 145 | 437 | 0.195 | 0.348 | 0.430 | 0.272 | 0.224 |
| 0.50 | 291 | 291 | 0.323 | 0.577 | 0.677 | 0.309 | 0.230 |
| 0.75 | 436 | 146 | 0.308 | 0.541 | 0.644 | 0.267 | 0.123 |

Three consequences, all weakening the paper:

1. **The random split inflates top-3 by about +0.13** (0.668 random vs 0.541
   sequential). A test entry sitting *between* training entries, rather than
   after them, is worth about thirteen points.
2. **The learning curve is not monotone in sequence.** It rises 0.345 ->
   0.577 then *falls* to 0.541. The paper's claim that "the learning curve is
   still climbing at 436" is false under this split and has been withdrawn.
3. **The headline gap nearly vanishes.** Proposal-minus-certification was
   +0.149 under the random split (0.668 vs 0.519) and is **+0.022** under the
   sequential split (0.541 vs 0.519). The abstract previously called this
   "the measurable finding". It is now reported as suggestive, with the
   sequential figure as the conservative estimate and the random figure
   explicitly labelled an upper bound.

What survives: the *ordering*. Geometry beats the lookup prior, majority and
length-only baselines at every fraction under both protocols, and
certification stays at chance. So the claim is directional --- a better
proposer than judge --- and its magnitude was inflated by the split.

Honest caveat recorded in the paper: `refiling_gate0.tsv` has a sequence
column but no timestamps, so we cannot confirm the file order is the rater's
adjudication order. It is a *sequential* split, not a demonstrated
chronological one.

### 1b. paper1 — the coverage section claimed sufficiency from a correlation [MAJOR, fixed]

The coverage section read: "So low coverage appears sufficient to predict
failure, while high coverage does not guarantee success." That is a
mechanistic claim drawn from a correlation whose own section reports
$p > 0.05$ and whose sample has eleven of twelve pairs indistinguishable
from chance.

Worse, the matrix refutes the ordering twice, and both counterexamples were
already printed two paragraphs earlier in the same section:

* **Monotonicity fails within one source.** codex$\to$claude-code has
  *lower* coverage than codex$\to$cline (0.241 vs 0.353) and a *higher* AUC
  (0.489 vs 0.399). Coverage down, performance up.
* **The best pair contradicts it.** cline$\to$opencode has the highest AUC
  in the matrix (0.534) at coverage 0.375, only 0.022 above the pair cited
  as failing for low coverage --- a 0.135 AUC difference across a 0.022
  coverage difference.

The section now states that coverage does not order the pairs, reports it as
a diagnostic to inspect rather than a gate or predictor, and notes that the
apparent pattern is what one expects by construction when eleven of twelve
pairs are at chance.

An error was introduced and caught inside this same fix: the first rewrite
claimed 0.375 lies "between" 0.353 and 0.172 (it is above both), then the
second claimed 0.022 was the smallest coverage gap in the matrix. It is not
--- the smallest is 0.004. Both were caught by recomputing from the JSON
rather than reading the prose, which is the only reason they did not ship.

### 1. paper1 — action-level bootstrap count was wrong [BLOCKER, fixed]

The abstract and the inferential-unit section both claimed that an
action-level bootstrap yields **four of twelve** established transfer pairs.
Recomputing both bootstraps (`check_bootstrap_unit.py`, output in
`bootstrap_unit_check.json`) gives **two of twelve** under action-level and
**one of twelve** under session-level.

The claim was load-bearing: it was the paper's methodological selling point.
Corrected in the abstract, the inferential-unit paragraph and the
conclusion, with the recomputation stored.

### 2. paper1 and paper9 — session-size median was wrong [MAJOR, fixed]

Both papers stated the median session carries **137** actions. The corpus
median is **101** (max 2,014 is correct). Recomputed directly from the
corpus and added to `dataset_baselines.json` as `corpus.session_sizes` so it
can no longer drift.

This mattered because it appears inside the clustering argument that
justifies session-level bootstrapping — a wrong number in the one paragraph
whose job is to establish that clustering is severe.

### 3. paperM — E83 recall misreported, and the flattering number chosen [MAJOR, fixed]

Case 2 stated recall **0.998** on the generated fixture. The artifact
(`e83-results.json`) gives the combined antonym+negation rules recall
**0.996**, F1 **0.998**. The paper had quoted the F1 and labelled it recall.
Corrected to 0.996 in both the table and the prose, with the reason stated:
recall is the quantity that collapses, so quoting F1 would flatter the rule.

### 4. paperM — E85 firing count contradicted its own artifact [MAJOR, fixed]

Case 3 stated "four of fifteen fired". The stored artifact
(`e85-adversarial-results.json`) records **five** rule firings
(quantifier 2, comparative 1, antonym 1, domain 1) and contains no pair
count. The prose report had said four false positives and attributed one to
`modal`, while the artifact shows `modal: 0` and `domain: 1`.

Resolved by citing the artifact and recording the discrepancy in the text
rather than choosing the tidier figure. Unresolved: whether four or five
*pairs* fired is not answerable from the artifact.

### 5. paperM — internal contradiction in the definition of the null [BLOCKER, fixed]

Section 2 opened by asserting that a construction-matched null is one that
"places errors where the system cannot detect them", and the Principles
section stated that such a null "is not a control; it is a tautology". Both
cannot hold.

The second is correct. Section 2 was rewritten around the defensible
definition — *preserves irrelevant aspects of the construction while
selectively perturbing the claimed mechanism* — and the error is now
reported in the paper as one of the findings, since a wrong definition of
the central instrument is itself a case study.

### 6. paperM — AGM and TMS overclaim [MAJOR, fixed]

The earlier draft said the workflow "is an implementation of [AGM]
principles". We have not demonstrated the AGM postulates hold for our
revision edges, and our objects are empirical claims with effect sizes, not
logical propositions. Rewritten to "AGM-inspired" and "related to", with an
explicit statement of what would be required to claim more, and the same
treatment for Doyle's TMS.

### 7. paperM — "the workflow caught E10" overstated [MAJOR, fixed]

E10's power gate returned STOP and the run proceeded. An earlier draft said
the workflow "caught" this. It did not: it recorded a protocol violation
after the fact. The invariant is now stated with its true status —
**records violations, cannot prevent them** — because a system described as
preventing will be trusted more than one described as logging, and that gap
is exactly what an agent under pressure exploits.

### 8. paper3 and paper4 — cited numbers resolved to nothing [BLOCKER, fixed]

Both papers cited figures from E15–E18, which had shipped as printing
scripts with no saved output anywhere on disk. They were re-run from the
surviving corpus and ground truth and reproduced their reported values
(COSINE 0.882 random-injection and 0.519 real-misfiling; CONSTRAINT 0.612
and 0.536; ANCHORVOC 0.664 with entry-length control 0.619; E17 tests
PASS/PASS/FAIL). Stored in `e15_e18_rerun.json` with the raw output in
`e15_e18_rerun_raw.txt`.

The re-run also *upgraded* one claim from inference to measurement: E18's
port check shows the Python scorer reproduces the original Rust values for
CONSTRAINT (r = 0.943) and ANCESTRY (r = 0.976) but not COSINE
(r = 0.197), which is direct evidence that the structural scorers are
embedder-independent while geometry is not.

### 9. paper4 — split protocol was undisclosed [MAJOR, disclosed]

Reading `run_e21.py`: the train/test split is a **random shuffle**
(`rng.shuffle(idx)`), not chronological, and each reported point is a mean
over **five** splits. The permutation null likewise uses five shuffles —
which this project's own notes elsewhere call too weak.

Centroid construction is clean: `centroids(tr)` uses the training rows only,
so test entries never contribute to a centroid. No leakage.

Both weaknesses are now stated in the paper as an explicit split-protocol
subsection, since a learning-curve claim is only as good as its split. A
temporal protocol and a 1,000-permutation null are named as the outstanding
checks and have not been run.

### 10. paper4 — the permutation null was too weak, and understated the noise [MAJOR, fixed]

The paper disclosed that its permuted-label null averaged only five
shuffles, which the project's own notes call too weak for a permutation
baseline. That gap has now been closed: the null was re-run with 1{,}000
shuffles via `e21_perm_null_1000.py`, which imports the original
`run_e21.py` so both arms share a code path.

The better-estimated null is **higher** than the published one:

| | published (5 shuffles) | re-run (1,000 shuffles) |
|---|---|---|
| null top-1 / top-3 / top-5 | 0.036 / **0.136** / 0.233 | 0.048 / **0.159** / 0.265 |
| geometry top-1 / top-3 / top-5 | 0.438 / 0.667 / 0.755 | 0.419 / 0.668 / 0.769 |

So the geometry-minus-null gap narrows from +0.531 to **+0.509** top-3, not
the +0.576 the abstract had implied by pairing a geometry figure from one
configuration with a null from another. The abstract, the results table's
caption, the gap arithmetic and the discussion ratio (4.9x -> 4.2x) were all
corrected. The table itself is left at five seeds so its rows stay mutually
consistent, with the re-estimate stated in the text.

One overclaim was caught in my own correction before it shipped: I first
wrote that the two null estimates' 95% intervals do not overlap. There is no
interval for the five-shuffle figure --- the original script reported only a
mean --- so that claim was unsupported and was removed.

## Checks that found nothing

- **paper2** — 5 of 6 files beat the shuffled-x null independently
  recomputed (ρ 0.976/0.930/0.626/0.929/0.939; `quality.rs` fails at 0.250
  against a null band upper bound of 0.700). The honest counterexample is
  correctly reported as a failure.
- **paper10** — 0 untraceable numbers; every fit value, Vuong R and p, and
  alpha in the table matches `empedocles_fits.json`. The negative result is
  stated as a negative result.
- **paper9** — the two remaining untraceable figures (0.005, 0.070) are
  documentary Cline extraction values and are now sourced in the text to the
  preserved E74 report.
- **Citation metadata** — all seven references in paperM verified against
  Crossref or arXiv. Two earlier candidates were dropped rather than cited
  unverified: the Szegedy adversarial-examples paper (arXiv returned 429/503,
  absent from Crossref) and a Ford et al. citation that was supporting the
  wrong claim.
- **Leakage in paper1** — the leave-one-session-out prior rebuilds per-kind
  statistics with the held-out session excluded, and `predict` tags session
  ids so the bootstrap can resample them. No leakage found.

## Findings not fixed, and why

| Finding | Status |
|---|---|
| paper4 temporal (non-random) split not run | disclosed, unrun |
| paper4 permutation null uses 5 shuffles, not 1000 | disclosed, unrun |
| E5 (0.447) and E24 (0.712) still have no stored result file | disclosed in paper4; not load-bearing |
| paperM has no control group comparing workflows | disclosed in Limitations |
| paper10's fit pipeline has no positive control | disclosed in paper10 |
| E85 pair-vs-firing count unresolved from the artifact | discrepancy recorded in text |

## Reviewer independence

The hostile review was to run in a separate reviewer context. **Two attempts
both failed, and the independent pass did not happen.**

**Attempt 1** (11:23) died in 0.47 s: `delegation.model` was pinned to
groq's `llama-3.3-70b-versatile`, which groq no longer serves (HTTP 404),
and the groq key is additionally 403-blocked. This affected every
`delegate_task` call, not just this one. Repinned to
`nvidia/nemotron-3.5-lightning-30b-a3b` (script:
`repin_delegation.py`; backup: `config.yaml.bak-before-delegation-repin`).

**Attempt 2** (11:33) ran, but timed out at 900 s having spent its entire
budget on the number-extraction check, and **its own checker was broken**.
Its reported findings are false positives: for paper 2 it printed
`cited JSONs loaded: 6, json_numbers count: 0, absent: 26` --- it extracted
zero numbers from the cited JSON files and therefore flagged all 26 numbers
in the paper as untraceable. For paper 1 it flagged 20 numbers that are
plainly present in the cited file (the per-harness label rates 0.038/0.048/
0.055/0.061/0.074, and the in-corpus CI bounds 0.341/0.606/0.656/0.69/
0.718/0.806). It never reached the leakage, contradiction or overclaim
checks.

**Attempt 3** (11:49) was rescoped to a single paper and prose-only checks,
so it would fit the budget. It reported `status=completed` after 599 s and
five API calls --- and its final answer was the literal string
`</parameter>`. A malformed tool-call fragment, not a review. It read paper1
and ran two greps before producing nothing usable.

Three attempts, three failures, all on the same model. The model is the
bottleneck: 600--900 s for five calls, and unreliable at emitting a final
structured answer. No further retry is planned; the other configured
providers are dead (groq 403, deepseek 401, nous 401, omniroute pool
exhausted).

One thing did come out of attempt 3. Its strategic grep for causal vocabulary
surfaced the coverage-sufficiency sentence in paper1 (finding 1b above),
which two earlier passes had missed. So the attempt was not worthless --- but
the finding was made by reading the code and data afterwards, not by the
reviewer's own report.

The correct trace results come from the repo's own verifiers, which are
mechanical and do recurse into nested JSON:
`verify_paper1.py` (0 untraceable), `verify_papers234.py`,
`verify_paper4.py` (2, both disclosed), `verify_papers910.py` (paper 10: 0).

**What this means for this document.** Every finding below was produced by
the author, not an independent reviewer. The two BLOCKER-level numeric
errors found (the bootstrap count and the session-size median) were
self-inflicted in the same session that wrote the numbers, which is evidence
that self-review caught some of what an independent reviewer would catch,
and no evidence that it caught all. Treat the findings as a floor.

There is a small irony worth recording, given this paper's thesis: the
attempted independent control failed twice, and the second failure was a
measuring instrument that silently returned zero and therefore reported
everything as broken. That is Case 5 in the catalog (a parser that reads the
wrong field) happening to the reviewer rather than the author.

Recommended before external submission: run this pass in a genuinely
independent context with a working model, on one paper at a time.

## Per-paper verdicts

| Paper | Verdict | Strongest reason |
|---|---|---|
| paper1_agent_priors | ACCEPT-WITH-FIXES | Numeric errors fixed and traced; the result is a well-supported negative with one surviving pair |
| paper2_trajectories | ACCEPT-WITH-FIXES | Numbers verified; scope is one repository and needs external replication |
| paper3_ontology_instability | ACCEPT-WITH-FIXES | Headline traces to stored files; anisotropy slope rests on three points and is flagged exploratory |
| paper4_propose_certify | ACCEPT-WITH-FIXES | Sequential split now run and disclosed: it cuts top-3 from 0.668 to 0.541 and the headline gap from +0.149 to +0.022, so the claim is directional only; one-model rater still bounds it |
| paper9_dataset | ACCEPT-WITH-FIXES | Kill test passes decisively; one user, one fortnight, four label definitions must be front-page |
| paper10_empedocles | ACCEPT-WITH-FIXES | Negative result properly reported; no positive control for the fit pipeline |
| paperM_methodology | ACCEPT-WITH-FIXES | Contradiction and overclaims fixed; novelty is explicitly disclaimed; no control group |
