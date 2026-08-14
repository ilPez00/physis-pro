# Sample output — chronos (temporal machine)

Demonstration of `src/machines/chronos.rs`. Reproduce with:

```bash
cargo run --example chronos_demo
```

## Input

A month of life-log events (25 total): a steady `gym` (~every 2 days), a weekly
`standup` whose last occurrence was 11 days ago, a monthly `pay rent`, an
irregular `call mum`, and one-offs (`dentist`, `flight booked`). Anchored to a
fixed `now = 2026-06-06` for determinism.

## Output

```
chronos — temporal machine
perceived 25 events · now = 2026-06-06

RHYTHMS (recurrence primitives, by confidence):
  • gym            every  2.0d  ×14  conf 1.00  next 2026-06-06
  • standup        every  7.0d  ×4   conf 0.80  next 2026-06-02
  • call mum       every  9.5d  ×3   conf 0.44  next 2026-06-13

DUE in the next 3 days:
  → gym            next 2026-06-06

SLIPPING (overdue past a half-period — surface before it breaks):
  ⚠ standup        4d overdue (period 7.0d)
```

## What it demonstrates

- **Reduction to primitives** — 25 raw events collapse to 3 *rhythms* (the
  elaborable unit), ranked by confidence.
- **Confidence = regularity × evidence** — perfectly regular, frequent `gym` →
  `1.00`; clean weekly `standup` → `0.80`; irregular `call mum` → `0.44`.
- **Prediction (elaboration)** — each rhythm carries its predicted `next` date.
- **The "dream" beat** — `standup` is surfaced as **slipping** (4 days overdue
  past its 7-day period) *before* the user notices the streak broke.
- **Restraint** — `pay rent` (only 2 occurrences → 1 interval) and the one-offs
  (`dentist`, `flight booked`) are correctly **not** promoted to rhythms; a
  primitive must be earned (≥ 3 occurrences), echoing euclid's "circles banned".

## Next

When wired into the shared graph (`register_rhythms` → bridged nodes + edges),
these rhythms become content-recallable in `physis-pro-wiki` and assertable as
relations (e.g. `gym` co-occurs-with `protein log`). As more machines land, their
samples join this folder.
