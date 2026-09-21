---
id: PH-026
track: physis-pro
phase: 0
status: review
rung: 3
depends: [PH-024, PH-025]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --lib --features tui hud:: && cargo test --lib world_object:: && cargo test --features tui --bin physis-hud"
commit: "branch agent/PH-026; conventional commits; per PROTOCOL §6"
claimed_by: cline-pansa-2026-09-21
claimed_at: 2026-09-21
---

# The world surface — object reconciliation, HUD library extraction, projection seam

## Why (the failure this prevents)

The HUD has three overlapping notions of "an object" and no place to put a
fourth kind of surface. `src/bin/hud.rs` is 6.1k lines: every new view is
another `match` arm in the bin, and every view switch is a fresh lookup, so
nothing guarantees a projection switch keeps you on the same object. The
mission ("one world, many projections") cannot be built on that shape without
manufacturing a fourth object model — which is exactly what it forbids.

## Reconciliation (measured on `agent/PH-025` @ `64c5f60`)

The prompt's premise needed correcting first: `git rev-list --count HEAD..main`
is **0**, and `src/world_object.rs` does **not** exist on `main`. The
WorldObject work lives on `local-main-import` (`468eba6`, `e3de500`,
`304300e`, `f777492`, `9e01f94`, `ee7bae7`, `4c138c5`, plus `sensors.rs` and
`fact.rs`), unmerged. So this is a port decision, not a merge decision.

| question | answer |
|---|---|
| Does `nav::PhysisObject` remain the canonical navigable identity? | **Yes.** `ObjectId{origin,key}` stays the identity: content-derived, rename-surviving. `WorldObject.identity` is its `origin:key` **string spelling**, and `world_object::parse_object_id` parses it back. No second id type, no migration. |
| Is `WorldObject` a richer read-only projection over nav/registry/task/agent objects? | **Yes** — it is a view type. It reads; it never stores, and nothing writes it to disk. |
| Can the prototype contract be ported while reusing `nav::ObjectId`? | **Yes**, and it was: the port renames the alias to `WorldObjectId`, makes the identity spelling explicit, and adds `parse_object_id`. |
| What fields should be derived rather than persisted? | All of them. Embeddings come from nav objects, state from `CoherenceNode`, affordances from the registry, relations from sled. Nothing in `WorldObject` is persisted. |
| Can Guided `ObjectHandle` wrap/reference that same identity? | **Yes**, and that is how the palette is built: `hud::palette::palette_for` derives rows from `guided::WorldView` (which derives from `Registry`), and `ObjectHandle` still cannot fabricate a target. |
| How do we avoid three `ObjectKind` enums drifting? | By **scope, not by merge**: `nav::ObjectKind` (what the index stores), `world_object::ObjectKind` (view vocabulary incl. agent/machine/service, `Other(String)` carrying unknown writers verbatim), and the registry's string kinds in `capability.rs`. One explicit mapping per crossing: `hud::lens_object::kind_of`. |

## Steps (done)

1. Port `src/world_object.rs` (12 tests) onto the PH-025 tree with nav
   identity. Added axis: `Relation.origin: nav::RelationOrigin`, so a view edge
   states *how it is known*. Scanned and symbol edges are `Deterministic`; a
   bare `relate()` is `Inferred` (`scanned_edges_are_deterministic_not_inferred`).
2. `src/hud/` (tui-gated library surface) — `world.rs` (`Cursor`, `Selection`,
   `Focus`, `Trail`, `ProjectionId`, `FocusState`), `config.rs` (four-layer
   merge with per-key layer provenance), `intent.rs` (the deterministic
   bare-text chain + `IntentIR` compile), `lens_object.rs` (object projection
   over recorded relations + retrieval neighbours), `palette.rs` (capability-
   derived rows), `bridge.rs` (bin/library adapters).
3. **The invariant**: `FocusState::switch_projection` changes the projection
   and resets only the cursor. Focus, explicit selection and trail survive.
   `App::open_view` routes every view change through `bridge::switch_to`, so
   the shipped keys (`g N T A S W ?`, `Tab`) inherit it.
4. Headless slice: `--query <text>` gives the frame a focus, and
   `--view graph|tasks|agents|help` opens through the command path so the frame
   is the real body.

## Tests added

- `src/world_object.rs` (12): identity round-trip to `nav::ObjectId`;
  scanned/symbol edges `Deterministic` vs `relate()` `Inferred`; provenance
  chain; unknown kind round-trip; node projection; agent run is `Model`.
- `src/hud/world.rs` (4): projection switch keeps focus/selection/trail;
  descend/ascend; selection toggle; projection names equal `--view` names.
- `src/hud/config.rs` (4): layer precedence; effective merge; per-key save
  layer; malformed layer named and ignored.
- `src/hud/intent.rs` (3): bare text compiles to IR with NIL bind tasks; one
  object = exact-object route; every route has a label.
- `src/hud/palette.rs` (3): rows trace to real affordances; unknown target =
  no rows; high risk asks first.
- `src/hud/bridge.rs` (3): selection names the hit id; view switch through the
  bridge keeps focus; cursor clamps.
- `src/bin/hud.rs` (2): `opening_a_view_keeps_the_focused_object_and_selection`
  (through `App::open_view`, non-vacuous precondition asserted);
  `object_lens_renders_recorded_edges_in_graph_grammar`.

## No symbol is renamed or removed

Nothing on this branch deletes an existing API: the port is additive, and
`Relation.origin` is serde-defaulted so prototype-era JSON still loads.

## Never

- No second object model, no world database, no persisted view type.
- No model, filesystem walk or subprocess inside a projection.
- No projection that resets focus or selection.

## Follow-ups (packets to write, not built here)

- Temporal/activity lenses over the observation log; rescan to delta badge.
- Editor and notebook as projections; observed shell execution.
- Crystallization preview to human accept to deterministic structure.
- Fleet/hardware objects (PH-101) as ordinary world objects.
- `sensors.rs` + `fact.rs` from `local-main-import`: deterministic structured
  facts (cpu temperature) before any LLM path — port with a caller and a test.

