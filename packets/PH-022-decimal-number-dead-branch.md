---
id: PH-022
track: physis-core
phase: 1
status: review
rung: 2
depends: []
gate: "cd ~/dev/physis-pro/physis-core && cargo test --lib map:: && cargo test --lib && cargo clippy --lib --tests -- -D warnings"
commit: "branch agent/PH-022; conventional commits; per PROTOCOL §6"
claimed_by: claude-pansa-2026-09-13
claimed_at: 2026-09-13
---

# `number_clash` was a dead branch — a decimal never survived parsing

## Why (the failure this prevents)

`build_map` decides contradiction with
`cos < 0.55 || na != nb || number_clash`. The third leg has **never once been
true**.

`leading_number` ended with `.take_while(|c| c.is_alphanumeric())`, which stops
at the `.`. So "2.6" came back as "2", and `decimal_number`'s
`contains('.')` filter could never match — it always returned `None`.

The doc comment states the intent exactly:

> A number with a decimal point — i.e. a *measurement*, not an instance id.

That distinction is a good one and the ground-truth corpus is built to exercise
it: a measurement clash (2.6 bar vs 4.2 bar) is a contradiction, an instance-id
difference (Release 17 vs Release 18, Invoice 9 vs Invoice 10) is not. The
parsing defeated the intent, so the corpus tested a rule that could not fire.

Consequence: `extra-device-spec-A/B`, the one pair the rule catches, was caught
solely by negation asymmetry ("must stop" vs "must not stop"), never by the
measurement clash the comment credits.

## Context (measured 2026-09-13)

- `src/map.rs` — `leading_number`, `decimal_number`, and the `number_clash`
  term in `build_map`.
- Found by replicating the parser in Python to score physis's own rule as a
  baseline arm in the NLI experiment. The incumbent has to be measured before
  anything can claim to beat it, and it would not reproduce.

## Steps (done)

1. Pin the intent with a test: `decimal_number("valve shall open at 2.6 bar")`
   must be `Some("2.6")`, and `decimal_number("Release 17 deployed")` must stay
   `None`.
2. Take a digit-or-dot run instead of an alphanumeric run, then trim a trailing
   `.` so "ends at 4." does not become a malformed number.
3. Update the comment to record the failure rather than deleting it.

## Tests added

- `map::tests::decimal_number_actually_finds_a_decimal` — fails before with
  `left: None, right: Some("2.6")`, and pins **both** halves: a measurement is
  recognised, an instance id is not. The second assertion is the one that fails
  if someone "fixes" this by accepting any number.

## Gate

```bash
cd ~/dev/physis-pro/physis-core
cargo test --lib map::
cargo test --lib
cargo clippy --lib --tests -- -D warnings
```

## Result: a real defect with no measured benefit yet

Recorded plainly because the honest outcome is the useful one. Re-running the
contradiction arms over the 86-pair candidate set
(`docs/plans/2026-09-13-nli-vs-cosine-contradiction.md`):

| arm | precision | recall |
|---|---|---|
| incumbent, **pre**-fix | 1.000 | 0.50 |
| incumbent, **post**-fix | 1.000 | 0.50 |

Identical. `num_fixed=True` now fires correctly on `spec-A/B` where
`num_old=False`, so the branch works — but that pair was already caught by
negation asymmetry, and the corpus contains no pair that *only* a measurement
clash would catch. The fix is correct; the corpus cannot show it paying.

## Never

- Never "fix" this by accepting any leading number. `Release 17` vs
  `Release 18` is not a contradiction, and the repeat families in the
  ground-truth corpus exist to catch exactly that mistake.
