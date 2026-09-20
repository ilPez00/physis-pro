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
