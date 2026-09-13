---
id: PH-018
track: physis-pro
phase: 0
status: done
rung: 2
depends: []
gate: "cd ~/dev/physis-pro && cargo test --test http_surface && cargo clippy --all-targets -- -D warnings"
commit: "branch agent/PH-018; conventional commits; per PROTOCOL §6"
claimed_by: claude-pansa-2026-09-13
claimed_at: 2026-09-13
---

# One machine reading raises two identical anomalies

## Why (the failure this prevents)

The industrial surface is the product. A single over-temperature reading that
reports itself twice is a wrong count on the one number a plant manager acts
on: an OEE report and a shift report built on doubled anomalies overstate how
often a machine is out of limits, and the duplicate carries the same
`machine_id`, `reason` and `severity`, so nothing downstream can tell the two
apart or de-duplicate them.

It is also the Tier-1 gate: `cargo test` is **red on `main`** because of this,
which is the condition PH-015 exists to prevent — a red gate that reviewers
learn to step over stops being a gate.

## Context (measured 2026-09-13 on `main` a62b85a, verify before relying on it)

Two tests in `tests/http_surface.rs` fail, and they fail **in isolation**, so
they are not the PH-015 concurrency flake:

```
test result: FAILED. 89 passed; 2 failed  (full cargo test, 277 s)
  machine_event_inherits_anomaly_detection
  funnel_scan_report_quality_dream_where_lineage_is_tenant_isolated
```

`tests/http_surface.rs:5721` — one reading in, two anomalies out:

```
assertion `left == right` failed: hot reading flagged:
{"accepted":1,"total":1,"skipped":0,
 "anomalies":[{"machine_id":"cnc-01","reason":"temperature 95 exceeds limit 90",
               "severity":0.0555555559694767},
              {"machine_id":"cnc-01","reason":"temperature 95 exceeds limit 90",
               "severity":0.0555555559694767}]}
  left: 2
 right: 1
```

Note `"accepted": 1, "total": 1` beside two anomalies: ingestion counted one
reading. The duplication is downstream of accounting, in whatever walks the
reading against `thresholds`.

`tests/http_surface.rs:5847` asserts the same invariant
(`body["anomalies"].len() == Some(1)`) inside the longer funnel test, which is
why the same defect fails two tests.

**Verified pre-existing**, not caused by the retriever fix in `3db5afa` /
physis-core `a8da284`: both tests fail identically with `src/rag.rs` and
`physis-core/src/rag.rs` checked back out at `ded5819` / `3a7321a`.

Leading suspects, cheapest first:
- the threshold check runs once per threshold *and* once per field, and
  `temp_max` matches both passes;
- the reading is evaluated once on ingest and again on the inherit/typed-node
  path that `machine_event_inherits_anomaly_detection` exercises, with both
  results appended to the same vector;
- a `Vec` that is extended rather than assigned across a retry or a
  tenant-scoped re-entry.

## Steps

1. Reproduce alone — `cargo test --test http_surface machine_event_inherits_anomaly_detection`
   fails in 2.3 s, so the loop is cheap. Do this before reading any code.
2. Find every place an anomaly is pushed for one reading. Grep the `reason`
   string format (`"temperature {v} exceeds limit {limit}"`) — the two entries
   are byte-identical, including `severity`, so both came from the same
   construction site or from two calls to it.
3. Decide which of the two is the intended one, and say so in the comment: per
   the repo rule, a comment that describes the removed path must be updated to
   describe the new reason, not deleted.
4. Fix at the source of the second push. Do NOT de-duplicate the output vector:
   that would hide a double evaluation that may also be doubling work
   elsewhere, and a later genuine duplicate (two readings, same values, same
   second) would then be silently swallowed.
5. Re-run both named tests, then the whole `http_surface` suite.

## Tests you must add

- **Unit:** on whatever function evaluates a reading against thresholds — one
  reading that breaches one threshold yields exactly one anomaly; one reading
  breaching two thresholds yields two, so the fix is not "cap it at one".
- **Integration:** the existing assertions at `http_surface.rs:5721` and `:5847`
  already fail before the fix and must pass after; no new integration test is
  needed if the unit test above distinguishes "one breach" from "two breaches".

## Gate

```bash
cd ~/dev/physis-pro
cargo test --test http_surface
cargo clippy --all-targets -- -D warnings
```

## Commit strategy

Branch `agent/PH-018` from `main`. One commit for the fix plus its unit test.

## Report

Append `packets/reports/PH-018.md` per PROTOCOL §5.

## Never

- Never make this green by relaxing the assertion to `>= 1` or by de-duplicating
  the anomaly list. The test is asserting the right thing; the count is wrong.
