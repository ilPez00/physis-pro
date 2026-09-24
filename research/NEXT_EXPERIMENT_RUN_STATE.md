# Next experiment run — state

Date: 2026-09-24
Mode: experiment-first (attack empirical weaknesses; manuscripts only where
experiments force a change)

---

## Current commit

| | |
|---|---|
| HEAD | `6c3ec59` — "paper4: measure the sequential permuted null; paper1: leak-free cold-start" |
| Branch | `agent/P-037-substrate` (not main) |
| Repository | `/home/gio/dev/physis-pro` → symlink to `/home/gio/physis_pro` |
| Remote | `git@github.com:ilPez00/physis-pro` (private) |
| Uncommitted | pre-existing only: `docs/wiki/MODULE_MAP.md`, `physis-core` submodule, `research/perspective-discovery/rerank_{gold,task}.json`, rebuilt PDFs, one new docs plan |
| Competing writer | none detected (`ps` shows no research script running at inspection time) |

## Current manuscripts

`research/papers/` — 7 live: paper1_agent_priors, paper2_trajectories,
paper3_ontology_instability, paper4_propose_certify, paper9_dataset,
paper10_empedocles, paperM_methodology. Plus `archive/superseded/` (papers
5/8/11, merged into M).

All 7 compile clean (undefined refs 0, missing figures 0).

## Current result sources

| artifact | feeds | status |
|---|---|---|
| `research/e74_replication_results.json` | P1, P9 | canonical |
| `research/bootstrap_unit_check.json` | P1, P9 | canonical |
| `research/e1_multiplicity.json` | P1 | v1 correction; superseded by v2 for arithmetic |
| `research/p1_multiplicity_v2.json` | P1 | **new this run** |
| `research/e1_coldstart_paired.json` | P1 | one seeded 30/12 split — 1B generalises |
| `research/p1_repeated_splits.json` | P1 | **new this run** |
| `research/e15_e18_rerun.json` | P3, P4 | canonical |
| `research/e21_{proposer,permutation_null_1000,sequential_split}.json` | P4 | canonical |
| `research/dataset_baselines.json` | P9 | point estimates only — needs session-level CIs |
| `research/empedocles_{multiseed,fits}.json` | P10 | no positive control |
| `benchmarks/behaviour/corpus-redacted.json.gz` | P1, P9 | scrubbed (see release audit) |

Registry: `research/results_registry/{claims.json,validate.py}` — 15 claims,
**37 checks, 0 failures** after this run's additions.

Gates wired into `research/papers/build_all.sh`: registry validator + release
audit. A paper build fails if either fails.

Corpus: `benchmarks/behaviour/corpus-redacted.json.gz` — 402 sessions,
75,933 actions, 871 kinds, 4 harnesses. 42 claude-code / 288 opencode /
69 cline / 3 codex sessions.

## Missing experiments

Ranked by the assigned order of importance:

1. **P1 1B** — repeated train/test partitions. *Running.*
2. **P1 1C** — cold-start across all target harnesses × all sources.
3. **P1 1D** — shrinkage/hierarchical prior (can cross-agent information be
   *used* even though the naive transfer fails?).
4. **P2 2A–2G** — external replication: ≥10 public repos, ≥500 files,
   multiple embedders, overlap null, intrinsic dimensionality, prediction test.
5. **P4 3A** — second rater; 3D external taxonomy replication.
6. **P10 4A–4C** — known-critical positive control, negative control,
   finite-size scaling.
7. **M 5** — executable ledger invariants.

## Stale manuscripts

- **P3** contains proposer figures superseded by P4. Policy per brief: mark
  SUPERSEDED / HISTORICAL, do not delete, do not maintain.
- **P2** claims outrun its evidence (6 files, one repo, one embedder).
- **P4** says "correct cell" in places where the target is a rater's decision;
  and "human decision leverage" although no human was tested.

## Blockers

| blocker | status |
|---|---|
| Second human rater for P4 | **needs a human.** Falls back to a distinct model rater, labelled secondary evidence only |
| P4 adjudication chronology | needs logs/metadata; if unrecoverable, keep sequential split with correct label |
| P2 external repos | needs network to clone; if unavailable, record blocker and continue |
| P1 repeated splits | running |
| Token rotation (GitHub PAT, NVIDIA key) | **operator action** — see `research/release_audit.md` |

## Exact commands

```bash
# state / gates
cd /home/gio/dev/physis-pro
git log --oneline -5
python3 research/results_registry/validate.py
python3 research/release_audit.py
bash research/papers/build_all.sh

# 1A  Holm correctness + tests
python3 research/papers/p1_multiplicity_v2.py
python3 research/papers/p1_multiplicity_v2.py --selftest

# 1B  repeated partitions
python3 research/papers/p1_repeated_splits.py --partitions 500

# 1C / 1D  (this run)
python3 research/papers/p1_all_targets_coldstart.py
python3 research/papers/p1_shrinkage_prior.py

# trace checks
python3 research/papers/verify_paper1.py
python3 research/papers/verify_paper4.py
python3 research/papers/verify_papers910.py
```

## Run order (from the brief)

1. state ✔
2. Holm fix + tests ✔
3. P1 repeated splits ▶
4. P1 all-target cold-start
5. P1 shrinkage prior
6. commit
7–10. P2 external corpus, overlap nulls, dimensionality, prediction
11. commit
12–13. P4 rater / chronology / external taxonomy
14. commit
15–17. P10 controls + finite-size
18. commit
19–20. M ledger invariants, commit
21. registry update
22. adversarial consistency audit
23–24. manuscripts + PDFs, final commit

---

## Update: 2026-09-24 (experiment-first run)

### P1 — complete
1A Holm fixed + tested (`p1_multiplicity_v2.py`, 5 tests incl. FWER 0.0493)
1B 500 partitions; 1C 200 partitions x 9 pairs; 1D 150 partitions + control
Report: `research/P1_ROBUSTNESS_REPORT.md`. Every quoted number
machine-verified against its artifact.

### M — complete
9 invariants incl. new I9 (artifact run-parameter provenance), adversarially
tested. `research/methodology_validation.json`.

### P2 — IN PROGRESS, two blockers found
1. **BLOCKER (environment): the pilot's embedder cannot be reproduced here.**
   No torch, no sentence-transformers, no BGE weight cache. Installing torch
   (2 GB+) onto a box with exhausted swap was judged a worse trade than using
   the embedder the fleet already serves. **Deviation from the brief's
   representation list**: `nomic-embed-text` via the local ollama HTTP API
   replaces BGE. BGE comparison is therefore *not available* and any claim
   resting on "consistent across embedders" is unsupported until it is.
2. **BLOCKER (resource): root partition is 100% full (~3.1 GB free).** Not
   caused by this work (P2 footprint was 30 MB when discovered). Corpus caps
   reduced to fit: MAX_TEXT 8 KB, gzipped shards (6x reduction), float32
   embedding cache. A corpus-size experiment on this box is operating inside a
   small budget, and disk exhaustion risks the whole machine, so no P2 run may
   grow without check.

Fixed en route: a global `url.git@github.com:.insteadOf=https://github.com/`
rewrites github to SSH, which is denied for public repos and also breaks a
partial clone's lazy blob fetch. Neutralised via `GIT_CONFIG_GLOBAL=/dev/null`
in the corpus builder (root-cause fix, applies to every git call it makes).

Scripts: `research/papers/p2_build_corpus.py`, `research/papers/p2_analysis.py`
Tasks 2C identity / 2E overlap-killer / 2F dimensionality are implemented;
2D direction and 2G prediction are not yet.

### Verification-code failures (a pattern, recorded)

Three times in this run a *verification* script of mine reported a problem that
did not exist, while the artifact under test was correct:

1. a regex that could not read U+2212 MINUS SIGN, so every negative number in a
   report table read as "row not found";
2. a tolerance of 5e-4 applied to report values deliberately written at 2 dp;
3. a corpus check that summed rows per path instead of only paths with >=8
   revisions, producing a phantom 65-revision mismatch against a correct
   manifest (real figures: 727 files / 17974 revisions / 10 repos, verified
   per-repo, delta 0 on every repo).

Each was caught only by drilling into per-item detail -- per-repo, per-row --
rather than trusting a single aggregate number. The lesson is not "be careful";
it is that a checker is itself an artifact and needs the same scepticism as the
thing it checks. Every verification script written in this run should be
treated as unverified until it has been shown to fail on a deliberate
violation.

Cross-reference: M invariant I9 exists because a hand-checked value drifted
from its artifact; these three exist because the hand-check was wrong in the
other direction.
