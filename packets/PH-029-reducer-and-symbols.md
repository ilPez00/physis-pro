---
id: PH-029
track: physis-pro
phase: 0
status: review
claimed_by: kiro
claimed_at: "2026-09-21T07:36:26+02:00"
rung: 3
depends: [PH-027]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --lib --features tui hud:: && cargo test --features tui --bin physis-hud"
commit: "branch agent/PH-029; conventional commits; per PROTOCOL §6"
---

# The reducer owns navigation, and symbols project as objects

Two gaps PH-027 measured and deliberately did not close. Neither is speculative:
both have a caller waiting and a failing behaviour to point at.

## 1. One mutation path for switching, focusing and selecting

Today `FocusState` is updated by the bin in two ways: `open_view` through the
library switch, and everywhere else by direct field writes (`self.sel`,
`self.hits`, `self.selected`). PH-026 added `HudEvent`/`reduce` and PH-027 routed
nothing new through it. The criterion from the mission holds and is testable:

> switching, focusing and selecting should each have exactly one mutation path.

Steps:

1. Add the events navigation actually needs — `MoveCursor`, `Select`,
   `ClearSelection`, `FocusObject`, `Descend`, `Ascend` — and **no others**. Each
   gets a caller in the same commit; a variant without a producer is the defect
   PH-027 just deleted five of.
2. Move the key arms that mutate those fields behind `reduce`, then make a test
   that fails if a second path appears (e.g. a debug assertion or a
   `#[cfg(test)]` guard that counts mutations).
3. Measure the win: `grep -c 'self.sel =\|self.selected' src/bin/hud.rs` before
   and after, in the report. A criterion that cannot be counted will not be held.

## 2. Structural symbols that this repository actually declares

`src/scanner.rs::extract_structural_lines` matches `^fn`, `^struct`, `^enum`,
`^trait`, `^impl`, `^mod` — unqualified. Measured in PH-026: a file full of
`pub fn` projects **zero** symbols, which is what this repo is made of. Until
that is fixed, a file opens to no children and semantic zoom stops at the file.

Steps:

1. Extend the Rust arm to the qualifiers in use: `pub`, `pub(crate)`,
   `async`, `unsafe`, `extern "C"`, and `impl<T>` generics. Keep it a cheap
   regex pass — no parser, no model, no new dependency.
2. Fixtures: one file per qualifier shape plus a negative case (a `fn` inside a
   comment or a string must not become an object).
3. Gate the behaviour end to end: a file object must list its `pub fn` children
   in `ObjectLens`/`Child` rows, and `Enter` must descend into one.

## Deferred with reasons (not gaps to close here)

* `project`, `schema`, `content`, `dependency`, `temporal`, `activity`, `fleet`
  lenses: each needs its own data source measured first (directory rollup,
  ontology facets, `doctree`, `mvp::deps`, the observation log, `agents/tasks`,
  `machines/`). One packet each, in that order.
* A `Lens` trait and a `LensRegistry`: wait for a lens that is not expressible as
  `Projection` data. Two are not enough to design it honestly.
* `ExactFact`/sensors: needs the prototype `sensors.rs` + `fact.rs` ported with a
  caller, a test and a genuinely useful structured question (see PH-028).
