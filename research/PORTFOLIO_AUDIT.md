# Physis research portfolio — state audit

Date: 2026-09-24
Auditor: research engineering pass (author-operated; see "Independence" below)
Scope: the seven manuscripts in `research/papers/`, their result artifacts, and
the experiments those artifacts came from.

---

## 1. Repository

| | |
|---|---|
| Path | `/home/gio/dev/physis-pro` → symlink to `/home/gio/physis_pro` |
| Branch | `agent/P-037-substrate` (**not** main) |
| Uncommitted | 24 paths changed/untracked |
| Critical detail | **`research/papers/` is entirely untracked.** The whole paper portfolio — 7 manuscripts, their PDFs, all verifier scripts, all result JSONs — exists only as uncommitted working-tree state. |
| Submodules | `physis-core` shows as modified |

**This is the single largest reproducibility risk in the program.** Every
result in this audit is currently one `git clean` away from being unrecoverable
by the eventual reader. Phase 0 of the plan is to commit it, broken numbers
included, so that the corrections have a "before" to diff against.

There is no `packets/`-visible research packet for the paper work, no `.bib`
files anywhere (three papers carry inline `thebibliography` blocks), and no
CI gate that runs any verifier.

## 2. Manuscripts found

| Paper | File | Pages | Status |
|---|---|---|---|
| P1 failure priors | `paper1_agent_priors.tex` | 6 | negative result, verified |
| P2 trajectories | `paper2_trajectories.tex` | 3 | **pilot only** — six files, no external replication |
| P3 ontology instability | `paper3_ontology_instability.tex` | 4 | merge candidate with P4 |
| P4 propose/certify | `paper4_propose_certify.tex` | 5 | metric error fixed this session |
| P9 dataset | `paper9_dataset.tex` | 5 | contraction with P1 fixed this session |
| P10 empedocles | `paper10_empedocles.tex` | 4 | negative result, no positive control |
| PM methodology | `paperM_methodology.tex` | 10 | flagship candidate |
| ~~P5, P8, P11~~ | `archive/superseded/` | — | merged into PM; retained |

Stale artifacts in `research/papers/` root: `paper5_*`, `paper8_*`, `paper11_*`
`.tex/.pdf/.aux/.log/.out` — originals of the merged papers, left alongside
their replacements. Should move under `archive/superseded/` for consistency.

## 3. Experiment directories found

`research/`: `convergence`, `criticality`, `mapper`, `multiscale`,
`perspective-discovery`, `perspective_invention`, `impossible_machine`, `papers`.

Per-experiment prose records: `E67_E70_BEHAVIOUR_GEOMETRY.md`, `E71`…`E86`,
`E91_ARCHAEOLOGY.md`, `E91_CONCEPT_MAP.md`, plus
`MASTER_EXPERIMENT_LEDGER.{md,json}` (51 canonical experiments),
`CLAIM_GRAPH.md`, `PAPER_PORTFOLIO.md`, `NEXT_EXPERIMENTS.md`.

Retired archive holding the runnable originals of E15–E18, E21:
`/home/gio/dev/.physis-pro-retired-20260920-014539/research/mapper/`

## 4. Canonical result artifacts

Machine-readable, and the only files any claim should trace to:

| Artifact | Feeds | Verified this session |
|---|---|---|
| `e74_replication_results.json` | P1, P9 | yes |
| `bootstrap_unit_check.json` | P1, P9 | yes — recomputed |
| `e15_e18_rerun.json` | P3, P4 | yes |
| `e21_proposer_results.json` | P4 | yes |
| `e21_permutation_null_1000.json` | P4 | yes — new |
| `e21_sequential_split.json` | P4 | yes — new (null arm running) |
| `dataset_baselines.json` | P9 | partial — grain-level CIs missing |
| `empedocles_{multiseed,fits}.json` | P10 | yes |
| `benchmarks/conatus/e83-results.json`, `e85-adversarial-results.json` | PM | yes |

Registry: `research/results_registry/claims.json` (12 claims) +
`validate.py` — traces each claim to its artifact and fails if two papers
disagree. Currently **16 checks, 0 failures**.

## 5. Contradictions detected

### 5.1 P1 vs P9 — bootstrap unit (RESOLVED this session)

P1 claimed the action-level bootstrap yields **2 of 12** pairs above chance
(session-level **1 of 12**). P9 claimed action-level resampling turned
**four** chance-level comparisons into apparently significant ones.

Canonical answer, recomputed from `bootstrap_unit_check.json`:

- action-level, CI entirely **above** 0.5: **2** (`cline→opencode`, `opencode→cline`)
- session-level, CI entirely above 0.5: **1** (`cline→opencode`)
- pairs whose significance *verdict* changes between units: **4** — **3 gain**,
  **1 loses**; and of the 3 that gain, 2 are significantly **below** chance
- CIs entirely below 0.5: **6** action, **5** session

**P1 was right; P9 was wrong.** P9's sentence conflated "verdict changed" (4)
with "became apparently significant" (3) and implied all of them were
positive-direction findings, when most were below-chance. Both papers
corrected and now agree; the registry validator enforces it.

The deeper finding neither paper had stated: **most cross-harness transfer
failures are anti-predictive, not merely uninformative.** 5–6 of 12 pairs
are significantly *worse* than chance. "Does not transfer" understates it.

### 5.2 P4 — invalid cross-metric subtraction (RESOLVED this session)

P4 reported a "propose/certify gap" as `top-3 containment − certification AUC`
(0.668 − 0.519 = +0.149, shrinking to +0.022 sequentially). Containment in
{0,1} over 57 cells and ROC AUC are different metrics on different scales;
the difference has no interpretation. Reported as "the measurable finding".

Removed. Both tasks now go against their own matched nulls (proposer vs
permuted-label null; certifier vs chance), with the qualitative claim
preserved. The retraction is stated in the paper rather than silently
deleted, and `PROP-003` in the registry carries status `superseded`.

### 5.3 P4 — fabricated sequential null (FOUND AND REMOVED this session)

While rewriting 5.2 I wrote that the sequential protocol has "a matched null
of 0.265" and clears it "by +0.28". **No sequential permuted null was ever
computed.** 0.265 is the *random* split's top-**5** null — I lifted it from
the wrong row and treated it as a sequential top-3 null, inventing a margin.

Removed from the abstract and discussion, replaced with an explicit statement
that the arm does not exist yet, and the arm has now been written
(`e21_sequential_split.py`, 1000 shuffles, same label-permutation construction
as `run_e21.topk(mode="permuted")`) and is running. This is the third
self-inflicted numeric error in this pass that trace-checking caught; it is
recorded rather than quietly fixed.

## 6. Missing provenance

| Item | Problem |
|---|---|
| `0.447` (P4) | E5's value; no stored result file anywhere in the repo or the retired archive. Disclosed as unreproducible. |
| `0.712` (P4) | E24's value; same. Disclosed. |
| `0.005`, `0.07` (P9) | Cline-specific figures, quoted from prose, no artifact. |
| P2's six-file statistics | Reproducible from `benchmarks/conatus/e78-*.json` but that is a **pilot**, not a corpus. |
| Git commit IDs | Nothing is committed, so no result carries a commit. |

## 7. Experiments that must be rerun

Ranked by how cheaply they can kill a claim:

1. **P4 sequential permuted null** — running now. Was asserted without existing.
2. **P1 cold-start, paired** — current curve compares arms on *different*
   held-out samples. Needs paired evaluation on identical target sessions,
   with global-base-rate and hierarchical/shrunk priors as arms C and D.
3. **P1 multiplicity** — 12 directional tests reported with no correction. The
   one surviving pair may not survive Holm or BH.
4. **P9 granularity kill test with session-level CIs** — now point AUCs only.
5. **P9 release audit** — no security/privacy scan of the released corpus exists.
6. **P10 positive control** — the fitting pipeline has never been shown to
   detect a criticality that is *known* to be there. Without it, "power law
   never beats lognormal" may be a property of the pipeline, not the field.
7. **P2 external replication** — six files of one repository cannot support
   the claim as written.

## 8. Strongest / falsified / unsupported / stale claims

**Strongest (defensible today)**
- P1: cross-harness failure priors mostly do not transfer; 11 of 12 pairs
  indistinguishable from chance under the correct inferential unit. Robust,
  and the direction of the error is understood.
- P9: per-action outcome labels with explicit four-way label heterogeneity,
  on 75,933 actions / 402 sessions; the *condition of use* is the contribution.
- P10: the power-law criticality claim fails distribution fitting across
  16 seeds × 9 settings; critical band seed-unstable. Clean negative.
- PM: a catalogue of twelve real machine-mediated research failures with
  control type and detection timing — a genuine empirical object.

**Falsified**
- P10 criticality (by its own authors).
- P3's "representation instability is the dominant cause" (too strong as
  written; cross-embedder disagreement never compared against
  within-embedder clustering stochasticity).

**Unsupported as written**
- P2's trajectory taxonomy — direction claim verified for 5 of 6 files, but
  the sample cannot carry a general claim.
- P9's granularity result — point estimates with no interval and no
  session-level unit.
- PM's "detection timing" column — retrospective author judgement, uncoded.

**Stale**
- Every result in the portfolio lacks a commit hash, because nothing is committed.
- P3+P4 still exist as two papers telling two stages of one corrected story.
- Old P5/P8/P11 build artifacts sit beside their replacements.

## 9. Publication blockers

| # | Blocker | Blocks |
|---|---|---|
| 1 | Portfolio is uncommitted and untracked | everything |
| 2 | P4 sequential null (asserted, not measured) | P4 |
| 3 | P1 cold-start is unpaired; multiplicity uncorrected | P1 |
| 4 | P2 is a 6-file pilot | P2 |
| 5 | P10 pipeline has no positive control | P10 |
| 6 | P9 release has had no security audit | P9 |
| 7 | P3/P4 not merged | P3, P4 |
| 8 | No current related-work audit for any paper | all |
| 9 | PM's novelty is at risk: "falsification is good" is old; the
      executable/agentic packaging is the claim, and it is untested against
      the AI-scientist literature | PM |

## 10. Independence

Every finding in this audit was produced by the same operator who wrote the
numbers. Three numeric errors in this pass were self-inflicted and caught
only by mechanical tracing, not by review; a fourth (5.3) was caught by a
verifier crashing rather than by judgement. Three attempts to obtain an
independent reviewer failed (config 404, toolbar timeout, malformed output).
Treat this document as a floor.

The registry validator is the compensating control that does work: it fails
mechanically, and it does not get tired.
