# Day 4 — distinguish when something happened from when we learned it (2026-09-16)

Day 4 of `docs/plans/2026-09-15-core-pro-development.md`. The problem statement
was already precise: "replay of recorded knowledge and a query about past world
state are different operations… `is_valid_at` also needs auditing before reuse
because one time argument cannot express both questions independently."

Both halves of that turned out to be true, and the fix was smaller than the
plan expected because half the machinery was already there.

## 1. What already existed

The repository had already met this problem once, at the *stream* level, and
solved it there:

| piece | state before this change |
|---|---|
| `EpistemicEvent::timestamp` | arrival / transaction time — when the engine got it |
| `EpistemicEvent::asserted_at` | G3: episode reference time — when the source said it |
| `HighWaterMark::last_arrival` / `last_episode_valid_at` | **two clocks**, per stream, with `IntakeReceipt::late` flagging a late episode |
| `reconstruct_status_at(subject, when)` | replays in **assertion** order, so arrival order cannot change the answer |
| `point_in_time_status_at(subject, when, validity)` | gates the audited status by a validity window |

So the two clocks existed for intakes and for replay ordering. What did **not**
exist:

1. **No world-time interval on an event.** `asserted_at` says when the source
   said it; nothing said *what period the claim is about*. `validity` lived only
   in a query argument.
2. **No query that can ask the two apart.** `point_in_time_status_at` takes one
   `when` and uses it for both axes — the validity gate *and* the knowledge cut.
   The plan's "one time argument cannot express both questions independently" is
   exactly this signature.

## 2. The change (Core only)

**`EpistemicEvent::validity: Option<TemporalValidity>`** — the world time this
revision is about, separate from `assertion_time()` and `timestamp`.
`#[serde(default)]`, so records written before today load as `None`, and `None`
means **unknown**, never "always true". `with_validity()` stamps it.

**`EpistemicAuditTrail::state(subject_id, valid_at, known_at) -> StateAt`** —
the plan's `state(valid_at=T, known_at=K)`. Three steps, in this order because
the order is the semantics:

1. **Knowledge cut** — keep events with `assertion_time() <= known_at`.
2. **Replay** — apply them in assertion order, so arrival order cannot matter.
3. **Validity partition** — keep the revisions whose interval covers `valid_at`;
   revisions with **no** world time go to `unknown_time` and contribute nothing.

```rust
pub struct StateAt {
    pub status: Option<HypothesisStatus>,        // from the applicable revisions alone
    pub unknown_time: Vec<String>,               // known, but unplaceable at valid_at
    pub supporting_event_ids: Vec<String>,       // what produced the status
    pub excluded_by_validity: Vec<String>,       // true at some other time, not this one
}
```

Every field is reported, including the ones that produced nothing, so "no
revision covers that instant" cannot be confused with "revisions cover it but
their date is unknown". Collapsing those two would make an unknown date look
like evidence of absence.

**`evaluate_status()`** was extracted so `reconstruct_status_at` and `state`
share one definition of what a sequence of revisions means. Two queries that
drift apart on the same data would be worse than one query.

**Nothing was removed or redefined.** `reconstruct_status_at` keeps its G3
meaning (replay in assertion order), `point_in_time_status_at` keeps its G6
meaning (audited status gated by one validity window), and a pre-Day-4 record
still answers both exactly as before — asserted in the tests.

## 3. The gate

`physis-core/src/epistemic.rs`, 6 tests. Every clause of the plan's gate is
covered:

| gate clause | test |
|---|---|
| "a January reading learned in March is absent from a February knowledge query but available to an April query about January" | `a_january_reading_learned_in_march_is_absent_in_february_and_present_in_april` |
| "arrival-order permutations with the same final evidence produce the same retrospective world-state answer; intermediate knowledge answers may legitimately differ" | `arrival_order_does_not_change_the_retrospective_answer` (both trails hold the *same* cloned revisions, so the comparison is replay, not uuids) |
| "test interval boundaries" | `validity_endpoints_are_start_inclusive_and_end_exclusive` — the existing `TemporalValidity` contract, pinned so it cannot drift |
| "later corrections" | `a_correction_only_applies_inside_its_own_interval` — and the excluded revision is *reported*, not silently ignored |
| "unknown dates" | `unknown_world_time_is_reported_and_contributes_nothing` |
| "restart" / "legacy records retain their documented meaning" | `world_time_survives_serialization_and_legacy_records_load_as_unknown` — a hand-written pre-Day-4 JSON record loads with `validity: None`, reports as unknown time, and still answers `reconstruct_status_at` |

Result: **7 passed, 0 failed** (`cargo test -p physis-core --lib epistemic`).

## 4. What was not done

- **Pro import normalization** (`extract_explicit_dates_and_source_spans`,
  `append_evidence_revision`). The gate is about the Core contract and that is
  what landed; the importers (`vault`, `history`, `mails`, `praxis`) still stamp
  `asserted_at` without a world-time interval, so their records answer as
  `unknown_time` — which is the honest reading of them today, and is why the
  field is optional rather than defaulted to a guess.
- **A `state` verb on the CLI / MCP / HTTP surfaces.** `src/core.rs` and
  `src/ported/studio.rs` still call `reconstruct_status_at`; wiring the two-clock
  query outward is a separate change with its own compatibility questions
  (what a surface returns when `unknown_time` is non-empty).
- **Timezone normalization.** `TemporalValidity` stores UTC instants and the
  comparison is instant-based, so nothing here depends on a local zone — but
  that is an observation, not a test. A date-only input (`2026-01-15`) has no
  defined zone conversion anywhere in this path yet.

## 5. Evidence

| check | command | result |
|---|---|---|
| Day 4 tests | `PHYSIS_EMBEDDER=random-projection cargo test -p physis-core --lib epistemic` | 7 passed, 0 failed |
| Pro lint | `cargo clippy --all-targets -- -D warnings` | exit 0 |
| Core compiles standalone | covered by the boundary gate | — |

**Actor**: continuation session (OpenCode)
**Date**: 2026-09-16
**Revisions**: Pro `96a98c4` + working tree, Core `d65858b` + working tree
