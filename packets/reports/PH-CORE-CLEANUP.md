# PH-CORE-CLEANUP — delete dead artifacts named by PLANNING §2c

**Status:** implementation done, awaiting independent review.
**Branch/commit:** `physis-core` `agent/PH-CORE-CLEANUP` @ `f3a0e11`.

## Task

PLANNING.md §2c named four dead artifacts. Each was verified dead before
deletion.

## What changed (4 deletions, 0 additions)

| file | evidence it was dead |
|---|---|
| `src/experiments.rs` | not declared in `lib.rs`; zero callers (`grep -rn 'experiments::\|mod experiments'` = none); contents are comments only |
| `Cargo.lock.bak` | stray backup; nothing reads it |
| `config/engineering_ontology.json.bak` | stray backup; the live file is `engineering_ontology.json` |
| `sw_add_domains.py` | hardcoded absolute-path script; already in `Cargo.toml` `exclude` |

## Verification (native)

```
test ! -f src/experiments.rs                        -> absent
test ! -f Cargo.lock.bak                            -> absent
test ! -f config/engineering_ontology.json.bak      -> absent
test ! -f sw_add_domains.py                         -> absent
grep -c 'experiments::' ../tests/snapshots/physis_core_api.txt -> 0
```

The parent API snapshot has no `experiments::` entries, so deleting the file
does not drift it; no bless was needed.

## What I could not verify

- **No independent review.** Single agent; `status: review`, not `done`.
- **`Cargo.toml` `exclude` still lists `sw_add_domains.py`.** Harmless once the
  file is gone, but stale. Removing it was outside this node's declared writes;
  left as a follow-up.
- **Tracked references remain in historical documents** (`docs/MODULE_MAP.md`,
  `docs/CORE_AUSTERITY_AUDIT.md`, `docs/CORE_PRO_SPLIT_AUDIT.md`, `PLANNING.md`,
  `CHANGELOG.md`, `benchmarks/behaviour/actions-feather.json`). These are the
  record of the audit that ordered the deletion and were intentionally left;
  `docs/MODULE_MAP.md:37` is the only one that reads like a current index and
  should be updated in a docs packet.

---

## Verification note (2026-09-20, independent reviewer) — ACCEPTED

Reviewer: opencode. Verdict: **accept** (`status: done`). The gate passes in
full, and the deletions are exactly what the report claims.

### Revisions

| | report time | review time |
|---|---|---|
| `physis-core` | `f3a0e11` | `5728a32` (descends from `f3a0e11`) |

### Reproduction (all rerun, gate in full)

```
test ! -f src/experiments.rs                     -> absent
test ! -f Cargo.lock.bak                         -> absent
test ! -f config/engineering_ontology.json.bak   -> absent
test ! -f sw_add_domains.py                      -> absent
grep -c 'experiments::' ../tests/snapshots/physis_core_api.txt -> 0
git show --stat f3a0e11
  4 files changed, 2863 deletions(-)            -> exactly the four named files
```

### Claims confirmed

- **`src/experiments.rs` was comments only.** The deleted file has **0**
  non-comment lines; it is 63 lines of `//!` doc comments describing five
  never-implemented measurements. Nothing referenced it: `mod experiments` is
  absent from `lib.rs`, and no `experiments::` path survives anywhere in `src/`.
- **The other three are what they say.** `Cargo.lock.bak` (2095 lines),
  `config/engineering_ontology.json.bak` (655) and `sw_add_domains.py` (50) are
  gone, and the deletion commit adds nothing.
- **No snapshot bless was needed.** The parent snapshot's `experiments::` count
  is 0, matching the report.

### One follow-up citation corrected

The report's follow-up says `docs/MODULE_MAP.md:37`. The file is
`docs/wiki/MODULE_MAP.md`, and line 37 is indeed the stale
``| `experiments.rs` | 62 | … |`` row. But that file is **generated** by
`scripts/gen-wiki.sh` ("Regenerate after any module add/remove/rename"), so
hand-editing it would be reverted by the next run. It was regenerated as part
of this pass instead: the row is gone, and the regeneration also cleared drift
left by the Core/Pro split — the `physis-core` section went from 65 modules /
37739 lines to 44 / 24542. The stale `Cargo.toml:14` `exclude` entry for
`sw_add_domains.py` is confirmed present and left alone, as the report says.
