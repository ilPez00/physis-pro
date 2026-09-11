---
id: PH-000
track: physis
phase: 0
status: done
rung: 1
depends: []
gate: "cargo clippy --all-targets -- -D warnings && git status --short | wc -l | awk '{exit ($1>2)?1:0}'"
commit: "branch agent/PH-000; conventional commits; per PROTOCOL §6"
claimed_by: claude-sonnet-5-session-01AX97EUNrXfUcXu6hN61anC
claimed_at: 2026-09-05T18:35:00Z
reviewed_by: cline-feather-2026-09-11
reviewed_at: 2026-09-11T12:10:00Z
---

# Baseline hygiene: clippy green, tree clean

## Why
CI runs clippy with `-D warnings`; main was RED with 9 warnings (measured
2026-09-01, 4 in src/tree.rs from the newest commit). Every later packet gates
on clippy, so this must be green first. Separately: uncommitted trees are how
work disappears on this fleet.

## Context
- clippy warnings (2026-09-01): 2x very-complex-type, 2x assert_eq!(bool),
  1x iterate map keys, 1x useless format!, 1x unnecessary clone-to-slice,
  1x io::Error::other, 1x redundant ref in format!, 1x len-compare-zero.
  Locations: src/routes.rs:123, src/tree.rs:{323,386,391},
  src/web/search.rs:{142,143}, src/web/mod.rs:3892, src/operational.rs:444,
  tests/http_surface.rs:{4189,5318}. RE-MEASURE — this list will be stale.
- This clone (pansa) had uncommitted edits in src/cli.rs, src/core.rs,
  src/embed/mod.rs, src/license.rs and docs/plans/2026-09-01-hardening-recheck.md
  when this packet was written. ANOTHER SESSION MAY HAVE BEEN MID-WORK.

## Steps
1. `git status --short`. For EVERY dirty file: decide (a) it is finished work →
   commit on a branch `agent/quarantine-<date>` with an honest message, or
   (b) it is broken/incomplete → `git stash push -m "quarantine <date>"`. Never
   discard. If unsure, stash and record in the report.
2. `cargo clippy --fix --lib -p physis_pro` and `--fix --test http_surface`;
   fix the remainder by hand.
3. Forbidden: `#[allow(...)]` to silence; changing any test's assertion
   semantics (`assert_eq!(x, true)` → `assert!(x)` is allowed, it is the fix).
4. Behaviour must not change. This is a cleanup, not a refactor.

## Tests you must add
- None new (this is cleanup), BUT: `cargo test` must pass unchanged, and you
  must NOT weaken any test (gate no-weakened-tests behaviour: diff the test
  files and assert only mechanical rewrites appear).

## Gate
```bash
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test            # Tier-1: once, at the end (~13 min)
git status --short | wc -l   # must be 0 (or only your new branch's committed state)
```

## Commit strategy
- Separate commits: `chore(quarantine): ...` for stashed/committed foreign
  work, then `style(clippy): fix N warnings (PH-000)`. Never mix them.

## Never
- `git add -A`. `#[allow]`. Editing test semantics. Deleting comments.
