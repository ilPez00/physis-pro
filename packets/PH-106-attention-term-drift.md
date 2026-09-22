---
id: PH-106
track: physis
phase: 0
status: claimed
claimed_by: opencode
claimed_at: 2026-09-22
rung: 2
depends: [PH-027]
gate: "cargo test --lib agents::attention && cargo clippy --lib --tests -- -D warnings"
commit: "branch agent/PH-106; one-line fix + report commit"
---

# `ATTENTION_TERMS` still names the retired `semantic` leg

## Why

PH-027 renamed the nav score leg `semantic` → `vector` (honest names:
`src/nav/mod.rs:491,527,649`, `weight_of` maps `"vector"` at
`src/agents/attention.rs:58`), but `ATTENTION_TERMS`
(`src/agents/attention.rs:20`) still lists `"semantic"`. The readout never
yields `"vector"`, so both `agents::attention` tests fail on
`find("vector").unwrap()` — `cargo test --lib` red tree-wide since PH-027
(1217 passed, 2 failed, 2026-09-22). PH-027's gate never ran `agents::`,
which is how it slipped through.

## Steps

1. Confirm direction: nav writes `"vector"` (`src/nav/mod.rs:649`), tests
   and `weight_of` agree — the terms table is the stale one (done 2026-09-22).
2. One line: `"semantic"` → `"vector"` in `ATTENTION_TERMS`.
3. Run the packet gate, then the full `--lib` suite (must be 1220/1220).

## Tests you must add

- None — the two failing tests already pin the fixed behaviour (fail
  before, pass after, demonstrated on 2026-09-22 before the edit).

## Gate

```bash
cargo test --lib agents::attention
cargo clippy --lib --tests -- -D warnings
```

Plus full `cargo test --lib` green (the red it clears).

## Never

- Rename anything else. The doc comment above the table (`:13-18`) still
  says "semantic" — flagged for the next sweep, deliberately out of this
  one line.
