---
id: PH-030
track: physis-pro
phase: 0
status: review
rung: 2
depends: [PH-029]
claimed_by: kiro
claimed_at: 2026-09-21
gate: "cargo clippy --all-targets -- -D warnings && cargo test --features tui --bin physis-hud"
---

# PH-030 — ResultState: collapse hits/rows/sel into one coherent struct

**Source**: PH-029 report "next measured bottleneck"; PH-027 report stale-hits hazard.

## Why

`App` still has three independently-mutable fields that encode one concept:

```
hits: Vec<NavHit>      — the ranked list
rows: Vec<Row>         — the Ratatui projection of hits
sel:  usize            — the Ratatui cursor into rows
```

`set_results()` is the single write path today, but it is not enforced —
callers can still assign `hits` before calling it (e.g. `back()` does exactly
this). PH-029 documented this as the "next measured bottleneck".

The `set_sel()` helper added in the pre-work commit enforces `sel ↔
focus_state.cursor` sync, but the three-field grouping itself is still loose.

## Steps

1. Define `ResultState { hits, rows, sel }` in `src/bin/hud.rs` with a
   constructor that builds all three atomically from `(hits, zoomed_out,
   intent_bands)`.
2. Replace the three fields in `App` with one `ResultState rs`.
3. `set_results` becomes `self.rs = ResultState::new(hits, ...)` — one
   assignment, no independent writes possible.
4. Add a `#[cfg(debug_assertions)]` invariant check in `ResultState::new`:
   `sel` points to a `Row::Hit` or the list is empty.
5. Thread `self.rs.sel`, `self.rs.hits`, `self.rs.rows` through all read sites.

## Tests (mandatory)

- `result_state_sel_invariant`: constructing `ResultState` with hits that
  produce rows where the first hit is a header panics in debug.
- `back_round_trip_restores_focus`: the existing PH-029 regression test shape —
  push state, pop via `back()`, assert focused object is restored.

## Never

- Do not restructure the entire App to get this. One struct swap.
- Do not rename the fields publicly — `rs.hits`, `rs.rows`, `rs.sel` are fine.
