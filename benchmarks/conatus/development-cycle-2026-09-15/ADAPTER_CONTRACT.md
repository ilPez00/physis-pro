# Adapter contract: `state(valid_at, known_at)` — a spec, not an implementation

Status: **PROPOSED. NOT IMPLEMENTED. NOT MEASURED.** Nothing in this file has
been built or tested against physis-core; it exists so Core/Pro can implement
the query the Day-4 plan describes against a frozen target, and so the fixture
suite (`fixtures/temporal_fixtures.json`, validated by `fixture_validator.py`)
has a concrete interface to be pointed at once it exists. Runnable schema/
fixture validation (this directory) is deliberately kept separate from
runtime integration against the real CLI/engine, which remains unbuilt.

## What exists today (verified by reading source, 2026-09-15)

- `physis-core/src/temporal.rs::TemporalValidity` already carries
  `valid_from` / `valid_until` (world-time interval) and `expired_at` (system
  supersession leg), with `is_valid_at(when)` implementing the half-open
  `from <= when < until` test this fixture suite's oracle reuses verbatim.
- `physis-core/src/hypothesis.rs::Evidence` has a single `observed_at: Option<DateTime<Utc>>`
  field — "when this evidence was observed **or** recorded" — which conflates
  the two questions this directive asks to separate. There is no `known_at`
  field anywhere in `hypothesis.rs`.
- `physis-core system replay --subject <id> --at <T>` (`main.rs::run_replay`)
  calls `PhysisCore::reconstruct_belief_at`, which delegates to
  `epistemic_audit.reconstruct_status_at(subject_id, when)` — an audit trail
  keyed by **when events were recorded into the trail**, i.e. write/ingestion
  time. `<T>` is compared against recorded-transition time, not against any
  evidence's world-time interval. This is exactly what
  `benchmarks/conatus/e77_temporal.py` measured: replaying at three different
  timestamps after all evidence is ingested returns the write-time answer,
  not an event-time answer. **`replay --at` is a `known_at`-only query today.**
  It should keep this meaning under its existing flag name for backward
  compatibility (Day-4 plan: "retain the current meaning of `replay --at K`").
- `hypothesis evidence <id> <claim> --polarity P` has no flag for the claim's
  own world-time interval at all — E77's dates live only in the free-text
  claim string. This is the "dates in prose" gap the plan calls out; it is
  not the same defect as the replay-indexing defect above, and both must be
  fixed for `state(valid_at, known_at)` to exist.

## Field naming — corrected 2026-09-15 against the coordinator's contract

`docs/semantic-computer/CONTRACT.md` (coordinator skeleton, adopted after
this section was first drafted) already fixes the vocabulary this proposal
needs: **`recorded_at` is the mandatory ingestion-time field name**,
**`occurred_at` is the optional world-time field name**, and query-time
**views expose both `valid_at` and `known_at`**. This document's CLI/JSON
surface below uses `recorded_at` on the stored `Evidence` struct and
`known_at`/`valid_at` only as query parameters and response fields, matching
that split. An earlier draft of this file used `known_at` as the stored
field name directly; that was wrong and is corrected here rather than left
for a reader to notice. `valid_from`/`valid_until` (an interval, from the
already-existing `TemporalValidity`) are kept as the stored world-time
representation for `Evidence` rather than collapsing to a single
`occurred_at` instant, since hypothesis/evidence claims are routinely
interval claims; this is a deliberate divergence from the event-envelope
contract's simpler `occurred_at`, not an oversight, and is called out
explicitly in `REQUESTS.md` R1 for whoever implements this to confirm or
override.

## Proposed CLI surface (additive, does not change existing flags)

```
physis-core hypothesis evidence <id> <claim> \
    --polarity supporting|contradicting \
    [--valid-from <RFC3339>] [--valid-until <RFC3339>] \
    [--valid-unknown]                      # mutually exclusive with the two above
    [--recorded-at <RFC3339>]              # default: now (ingestion instant); stored field
    [--supersedes <evidence-id>]           # marks this as a correction

physis-core replay --subject <id> \
    [--at <RFC3339>]                       # EXISTING flag: known_at-only, unchanged meaning
    [--valid-at <RFC3339>] [--known-at <RFC3339>]   # NEW query params: both required together;
                                                     # known_at reads the stored recorded_at field
```

`--at` alone must keep answering exactly as it does today (a pure
`known_at`-only reconstruction) so that anything depending on current
`replay --at` behaviour does not silently change meaning. `--valid-at`/
`--known-at` is new, additive surface, not a replacement.

## Proposed JSON response contract

```json
{
  "subject": "<id>",
  "valid_at": "<RFC3339 or null if a known_at-only query>",
  "known_at": "<RFC3339>",
  "status": "SUPPORTED | CONTRADICTED | UNKNOWN_TIME | NO_BELIEF",
  "supporting_evidence_ids": ["..."],
  "contradicting_evidence_ids": ["..."],
  "unknown_time_evidence_ids": ["..."],
  "excluded_by_known_at_count": 0,
  "excluded_by_valid_at_count": 0,
  "engine_note": "free text, e.g. disagreement between log and derived view (PH-017 behaviour, unchanged)"
}
```

`UNKNOWN_TIME` must be a distinct status from `NO_BELIEF` in both the CLI text
and this JSON field — collapsing them silently turns "we have relevant
evidence but cannot place it in time" into "nothing was ever reported," which
is the exact failure mode `T06`/`T07` in the fixture suite are built to catch.

## Semantics Core should implement (matches the fixture oracle exactly)

See `fixture_validator.py::oracle_state` for the executable version of this.
In prose:

1. **Visibility**: an evidence item is visible at query `known_at=K` iff
   `item.recorded_at <= K`. This is a new gate; today nothing in
   `hypothesis.rs` or `epistemic.rs` filters evidence by an ingestion
   timestamp distinct from its world-time fields — everything currently
   ingested is always visible.
2. **Corrections**: if item `B.supersedes == A.id` and `B` is visible, `A` is
   excluded from visibility (not deleted — it must remain in the raw log/
   `history` for audit, per the "invalidate, don't delete" rule already
   documented on `TemporalValidity::expired_at`). This models Day-4's
   `supersedes=explicit_previous_evidence_id_or_none` field on
   `ingest_evidence`, which does not exist in the Rust struct yet.
3. **Applicability**: among visible, dated items, one is applicable at
   `valid_at=T` iff `TemporalValidity::is_valid_at(T)` on its own interval
   returns true — reuse the existing method rather than reimplementing the
   half-open test.
4. **Status**: contradicting-applicable beats supporting-applicable beats
   unknown-time-visible beats nothing, in that order. This precedence
   (contradiction wins) matches `HypothesisStatus::Contradicted` already
   outranking `Supported` in the existing status machine's spirit, but is an
   explicit design choice for this query and should be named as such in the
   implementation, not left implicit.
5. **`UNKNOWN_TIME`**: an item with no extractable `valid_from`/`valid_until`
   (E77's "dates in prose" case, or a genuinely undated report) must not be
   silently dropped from a `valid_at` query, nor must it silently satisfy
   every `valid_at` query. It surfaces as its own status only when no dated,
   applicable item overrides it.

## Note on the fixture file's field name

`fixtures/temporal_fixtures.json` stores each evidence item's ingestion
timestamp under the JSON key `known_at`, not `recorded_at`, because that
fixture was authored and validated (`fixture_validator.py`) before this
contract's naming was corrected against `docs/semantic-computer/CONTRACT.md`.
This is harmless: the fixture is test data for an oracle, not the production
`Evidence` struct, and its own `visibility_rule` field documents its meaning
precisely (`item.known_at <= K`) regardless of what a future implementation
calls the equivalent stored field. Do not infer from the fixture's JSON key
that this contract's renaming to `recorded_at` (above) was reverted — it was
not; only the pre-existing, already-validated fixture file's internal key
name was left as originally frozen rather than mutated after the fact.

## What this contract explicitly does NOT claim

- It does not claim `physis-core` implements any of this today — it does not.
- It does not claim the fixture suite has been run against a real CLI/binary
  — it has only been run against the Python oracle in this directory.
- It does not prescribe how Pro's ingestion layer extracts `valid_from`/
  `valid_until`/`valid_unknown` from raw text (that is Pro's date-extraction
  responsibility per the Core/Pro ownership split in
  `docs/plans/2026-09-15-core-pro-development.md` §1); it only fixes the
  typed interface Core should expose once that extraction hands it structured
  values.
- Concrete API/test requests for Core and Pro derived from this contract are
  in `REQUESTS.md` in this worker's report directory (outside this repo's
  ownership boundary for this task), not in this file.
