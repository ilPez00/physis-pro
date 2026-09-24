# Research changelog

Every claim whose wording, numerical value, or evidential status changed.
Nothing is silently replaced: superseded values remain listed with the reason
they were withdrawn.

---

## 2026-09-24 — portfolio audit pass

### Headline result: P1's only positive claim removed

`research/papers/e1_multiplicity.py` → `research/e1_multiplicity.json`.
Session-level bootstrap, 10,000 replicates, Holm and Benjamini–Hochberg over
the 12 directional tests.

| | |
|---|---|
| raw $p < 0.05$ | 6 of 12 |
| survive Holm (family-wise) | **3 — all significantly BELOW chance** |
| survive BH | 6 (one above chance, five below) |
| `cline→opencode` (the old headline, 0.534) | raw $p = 0.0094$; **fails Holm** (threshold 0.006 at its rank); survives BH |

**No pair shows positive transfer after family-wise correction.** The three
Holm survivors are `claude-code→codex` (0.394), `codex→cline` (0.401) and
`codex→opencode` (0.416) — the borrowed prior is *anti-predictive*, not
merely uninformative. All three involve Codex; recorded as an observation,
not a mechanism.

P1's abstract, results (new Table 1), discussion and conclusion rewritten
accordingly. Claim added as `XFER-005`, enforced by the validator.

### Second: the cold-start curve had train/test leakage

Inspecting `e74_replication.py` while checking P1's pairing revealed the
curve is worse than unpaired — it is **contaminated**:

- `own_only` (n=42) built its prior from all 42 claude-code sessions and was
  scored on those same 42.
- `mixed_nN` built from `tgt[:n]` and was scored on all of `tgt`, which
  includes `tgt[:n]`.
- `transferred_only` (n=0) is the only clean arm — its prior is from
  opencode, a different harness — and it sits **below chance**
  (0.469 [0.445, 0.490]).

So the apparent 0.469 → 0.720 improvement, and the crossover at n=2, cannot
be attributed to local history. The subsection is marked withdrawn in-paper
with the figure captioned WITHDRAWN; the numbers are retained as an
error record. Claim added as `XFER-006`, status `superseded`.

Consistency worth noting: the two arms that looked good were the leaked ones;
the one clean arm agrees with the anti-predictive finding above.

### Security

Credential-class material found in the artifact described as redacted — see
`research/release_audit.md`. An NVIDIA API key inside
`corpus-redacted.json.gz` (`kind` is derived from command text, so it
survives `target` hashing), and a GitHub PAT plus full command text in the
raw captures. 39 tokens scrubbed across 6 files; derived numbers unchanged.
**Tokens must still be rotated and history rewritten before any public
release.**

### New controls

- **`research/results_registry/{claims.json,validate.py}`** — canonical result
  registry. 14 claims, each with experiment, result file, resolvable JSON
  path, source script, metric definition, inference unit, seeds, n,
  uncertainty method, status and reproduction command. The validator
  recomputes derived claims from raw artifacts and **fails** if two papers
  assert incompatible values. Currently **22 checks, 0 failures**.
- **`research/PORTFOLIO_AUDIT.md`**, **`research/CROSS_PAPER_CONSISTENCY.md`**,
  **`research/EXPERIMENT_PLAN.md`** — this pass's deliverables.
- **`research/release_audit.py`** — fails if any secret-shaped token is in a
  release artifact; wired into the paper build alongside the registry.
- Eight commits: the manuscript portfolio, its artifacts and its verifiers
  were previously **untracked working-tree state** and are now in git.

### Claim status changes

| claim | was | now | reason |
|---|---|---|---|
| P1 positive transfer | 1 of 12 pairs established (cline→opencode 0.534) | **0 of 12 after Holm**; cline→opencode is BH-dependent only | family-wise correction over 12 tests: threshold 0.006 at its rank, p=0.0094 |
| P1 cross-harness effect | "priors do not transfer" | **3 pairs significantly anti-predictive** (0.394/0.401/0.416) | the only effect surviving Holm runs the other way |
| P1 cold-start | borrowed prior beats thin local history (0.469→0.720, crossover n=2) | **withdrawn — train/test leakage** | local priors built from the sessions they were scored on |
| P9 bootstrap-unit effect | "four chance-level comparisons turned apparently significant" | 4 verdict changes = **3 gained, 1 lost**; most below chance | artifact records `{action: 2, session: 1}`; P1 was right, P9 wrong |
| P1 coverage sufficiency | "low coverage appears sufficient to predict failure" | diagnostic only; no sufficiency claim in either direction | refuted twice by its own matrix |
| P4 propose/certify gap | `+0.149` / `+0.022`, "the measurable finding" | **retracted**; tasks reported against their own nulls | containment in {0,1} minus ROC AUC is undefined |
| P4 sequential null | "matched null of 0.265, margin +0.28" | **fabricated; removed**; arm implemented and running | no sequential permuted null had ever been computed |
| P4 permutation null | 0.136 (5 shuffles) | **0.159** (1,000 shuffles) | 5 shuffles too weak; published null understated noise |
| P4 geometry−null gap | +0.531 | **+0.509** | follows from the corrected null |
| P4 learning curve | "still climbing at 436" | **withdrawn**; not monotone in file order | sequential split peaks at 0.577 then falls to 0.541 |
| P4 primary protocol | random split | **sequential split**; random retained as labelled upper bound | deployment is sequential |
| P4 top-3 headline | 0.667 / 0.668 | 0.668 random **and 0.541 sequential** | both now reported |
| P1/P9 median session length | 137 actions | **101 actions** | recomputed from corpus (402 sessions, max 2014) |
| P1 action-level pair count | 4 established | **2** | wrong-bootstrap count; session-level is 1 |
| PM E83 recall | 0.998 | **0.996** (0.998 is the F1) | mislabelled metric |
| PM E85 firings | "four of fifteen fired" | **five** recorded in the artifact | prose disagreed with artifact; pair count unrecoverable |

### Corrections caught inside this pass (self-inflicted)

Recorded because the pass is about provenance, and an auditor that hides its
own errors is not one:

1. Claimed 0.375 lies "between" 0.353 and 0.172 — it is **above** both.
2. Claimed 0.022 was the smallest coverage gap in the matrix — it is **0.004**
   (`claude-code→opencode` vs `opencode→claude-code`).
3. Wrote "two nulls' 95% intervals do not overlap" — no interval existed for
   the five-shuffle figure, which was a mean only.
4. **Invented a sequential permuted null of 0.265** by reading the random
   split's top-*5* row (see the table above). Found when the verifier crashed
   looking for a key that did not exist.
5. Asserted a statement about the largest baseline margin before computing it;
   replaced with measured values (prior +0.209, length-only −0.123).

### Numbers added

- Bootstrap in artifact form: 12 pairs × 2 units, with counts `{action: 2, session: 1}`.
- `e21_permutation_null_1000.json` — 1,000-shuffle null, both arms.
- `e21_sequential_split.json` — sequential geometry arm + in-order baselines
  at fracs 0.10/0.25/0.50/0.75; permuted arm running.
- P4 baseline margins over the permuted null: prior +0.209, majority +0.096,
  length-only −0.123, geometry +0.509.
- P4 certifier margin over chance: +0.019 (n = 574).

### Infrastructure (outside the science)

- Hermes `delegation.model` was pinned to groq's `llama-3.3-70b-versatile`,
  which groq no longer serves (404) with a 403-blocked key, so every
  `delegate_task` died in ~0.5 s. Repinned to
  `nvidia/nemotron-3.5-lightning-30b-a3b`. Backup:
  `~/.hermes/config.yaml.bak-before-delegation-repin`.

### Unresolved at end of pass

- P4 sequential permuted null: running.
- P1 paired cold-start (arms C and D missing), multiplicity correction.
- P9 granularity CIs and release security audit.
- P10 positive control.
- P3/P4 merge; P2 external replication; PM ledger validation.
- No result carries a commit hash (`provenance.json` incomplete).
- Independent review: three attempts failed (config 404, timeout with a broken
  checker, malformed output). Every finding in this pass is author-produced.

---

## Earlier (pre-audit, retained for provenance)

These were made in the session preceding the audit and are listed here so the
"before" state is not lost:

- P1 inferential unit corrected: session-level bootstrap adopted; established
  pairs 4 → 1.
- Papers 5, 8, 11 merged into `paperM_methodology`; originals retained in
  `research/papers/archive/superseded/`.
- E15–E18 re-run and persisted (`e15_e18_rerun.json`), removing an
  "unreproducible" caveat from P3 and P4.
- P10 reframed as a negative result after 16 seeds × 9 settings showed the
  power law never beats lognormal (Vuong).
- P9 fine-grained taxonomy kill test run (871 kinds, AUC 0.627–0.751 vs
  0.446–0.525 coarse) — still point estimates only.
- Citations verified against Crossref/arXiv; two unverifiable references
  dropped.
- **Retained as unreproducible:** `0.447` (E5) and `0.712` (E24) have no stored
  result file anywhere in the repo or the retired archive. Disclosed in P4's
  provenance section rather than deleted.
