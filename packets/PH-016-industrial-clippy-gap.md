---
id: PH-016
track: physis-pro
phase: 0
status: done
rung: 1
depends: [PH-000]
gate: "cd ~/dev/physis-pro && cargo clippy --all-targets -- -D warnings"
commit: "branch agent/PH-016; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
reviewed_by: cline-feather-2026-09-11
reviewed_at: 2026-09-11T12:10:00Z
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

---

## Resolution (2026-09-07)

Closed by removing the gap's cause rather than adding a step around it.

`industrial` is no longer a feature: `rumqttc` and `tokio-modbus` are
unconditional dependencies and all twelve `#[cfg(feature = "industrial")]`
gates are gone, so `src/industrial.rs` is part of the default build. The
existing default-feature clippy step in CI therefore covers it, which is why
this packet's gate could be simplified to the plain
`cargo clippy --all-targets -- -D warnings` above — that command now compiles
the file this packet was written about.

Both warnings were fixed as specified:

- `src/industrial.rs` `"mqtt" | _` -> `_`, behaviour unchanged (the CLI already
  defaults `--protocol` to mqtt, so the arm is reached only for an
  unrecognised value; the comment records that a typo is treated as MQTT
  rather than rejected, which this packet did not license changing).
- `&[reading.clone()]` -> `std::slice::from_ref(&reading)`, the same fix
  PH-000 applied in `src/operational.rs`.

CI keeps a `cargo check --features industrial,cli` step so the no-op alias
keeps resolving for `scripts/predeploy.sh` and CONTRIBUTING.md, with a comment
saying not to re-gate the module without adding a matching clippy step.

Not done, and deliberately: a `clippy --features full` step. `full` pulls
tesseract/leptonica, which are not installed on this workstation, so it could
not be verified locally and an unverified red CI step is worse than the gap
it closes. Worth adding from a machine that can run it.

Tests unchanged and passing: 15 lib `industrial`/`license` tests,
`tests/industrial_demo.rs`, `tests/industrial_benchmark.rs`.
