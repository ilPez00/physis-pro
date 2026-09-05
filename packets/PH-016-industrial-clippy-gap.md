---
id: PH-016
track: physis-pro
phase: 0
status: todo
rung: 1
depends: [PH-000]
gate: "cd ~/dev/physis-pro && cargo clippy --all-targets --features industrial,cli -- -D warnings"
commit: "branch agent/PH-016; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
---

# Clippy is red under `--features industrial` and CI never checks it

## Why (the failure this prevents)

`.github/workflows/ci.yml`'s `check` job runs `cargo clippy --all-targets --
-D warnings` exactly once, on default features. It separately runs
`cargo check --features industrial,cli` (a plain check, no clippy). So
`src/industrial.rs` has been able to accumulate clippy warnings invisibly —
confirmed 2 there right now — because nothing in CI ever runs clippy against
that feature combination. This is the same class of gap PH-000 exists to
close for default features, just one feature flag over.

## Context (measured 2026-09-05 during PH-000)

```
$ cargo clippy --all-targets --features "web industrial" -- -D warnings
error: wildcard pattern covers any other pattern as it will match anyway
   --> src/industrial.rs:470:9
error: unnecessary use of `clone` to create a slice from a reference
   --> src/industrial.rs:497:21
```
(Plus the 8 default-feature warnings PH-000 already fixes — re-measure
after PH-000 lands; this packet's own gate only needs the two above to be
gone.)

## Steps

1. Fix `src/industrial.rs:470` — read what the wildcard arm actually does;
   the preceding arms already cover every real case, so the wildcard is
   dead code once removed, or (if there's a case it alone catches) merge it
   into whichever arm it duplicates.
2. Fix `src/industrial.rs:497` — same `&[x.clone()]` -> `std::slice::from_ref(&x)`
   pattern PH-000 fixed in `src/operational.rs:448`.
3. Add a clippy step for this feature combination to
   `.github/workflows/ci.yml`'s `check` job (next to the existing
   `cargo check --features industrial,cli` step) so this can't silently
   regress again.

## Tests you must add

- None new — mechanical clippy fixes, same rule as PH-000: `cargo test
  --features industrial` must pass unchanged before and after.

## Gate

```bash
cd ~/dev/physis-pro
cargo clippy --all-targets --features industrial,cli -- -D warnings
cargo test --features industrial,cli
```

## Report

Append `packets/reports/PH-016.md` per PROTOCOL §5.

## Never
- `#[allow(...)]` to silence instead of fixing.
