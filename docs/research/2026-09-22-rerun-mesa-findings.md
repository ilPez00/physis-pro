# Rerun + Mesa integration findings (2026-09-22)

> Program: `docs/plans/2026-09-22-rerun-mesa-integration.md`.
> Governing principles: **NO existing Physis module deleted**
> (comment/disabled/feature-gate/deprecate/adapter/dormant/preserve/alternative);
> reuse/wrap/adapt over rebuild; build the most powerful human+agent HUD (the
> impossible machine), not the cleanest internal architecture; offline-first.
> Source-level inspection only — no Rerun/Mesa build was run. Where evidence is
> absent, the claim is marked NOT-MEASURED.

## Environment (measured)

| repo | state | HEAD |
|---|---|---|
| `/home/gio/dev/rerun` | clone | `35143bce37` (prerelease-1369), v0.39.0-alpha.1 |
| `/home/gio/dev/rerun/crates` | 10 sections | build, data_flow, panels, store, store_app, tests, top, utils, viewer_support, views |
| `/home/gio/dev/mesa-sdk-rust` | cloned | `mesa-dev` v1.24.0 (archived notice on README:1) |
| `/home/gio/dev/mesa-examples` | clone | `just-bash-shell` + 17 sandbox variants |
| `/home/gio/dev/mesa` | absent | core `mesafs` not public |
| `/home/gio/physis_pro` ≡ `/home/gio/dev/physis-pro` | branch `agent/H6-langpacks` HEAD `8fb8ddb` | one checkout |

## Rerun store/query — verdict YES (standalone temporal typed store)

`re_chunk_store::ChunkStore` is an in-memory time-series DB (all timelines,
all entities, out-of-order, `O(log N)` latest-at/range); `re_query::QueryCache::latest_at(
report_mode, &LatestAtQuery, &EntityPath, components)` is the deterministic
lookup. No viewer dependency. `re_sdk` is a write-only logging client; `re_log_encoding`
is the `.rrd` byte layout.

| primitive | rerun location | form |
|---|---|---|
| Entity/id | `store/re_log_types/path/entity_path.rs:93` `EntityPath { hash, parts: Arc<Vec<EntityPathPart>> }` | locator, not identity |
| Component | `store/re_types_core/component_descriptor.rs:21` | column descriptor |
| Archetype | `store/re_types_core/archetype.rs:16` trait | component bundle |
| Chunk | `store/re_chunk/chunk.rs:242` `Chunk { entity_path, timelines: IntMap<TimelineName,TimeColumn>, components }` | dense Arrow; empty `timelines` ⇒ static |
| Store | `store/re_chunk_store/store.rs:527` `ChunkStore` | single time-series DB |
| Timeline | `store/re_log_types/index/timeline.rs:19` | name + TimeType |
| static vs temporal | `store/re_log_types/index/time_point.rs:26`; `TimeInt::STATIC = Self(None)` `time_int.rs:44` | discriminator |
| latest-at query | `store_app/re_query/latest_at.rs:43` `QueryCache::latest_at` | cached, Clear-shadowing |
| range/SQL | `re_dataframe` / `re_datafusion` | deterministic |
| serialize | `store/re_log_encoding` (.rrd FileSink/GrpcSink/MemorySink `top/re_sdk/src/lib.rs:34,84`) | append stream |

What Rerun does NOT give: embeddings, semantic similarity, ontology, identity
semantics. `EntityPath` is a locator; Physis `WorldObjectId` is identity.

## Rerun visualization — verdict reuse seams, don't embed the shell

View crates (`crates/views/`): `re_view_bar_chart`, `re_view_dataframe`,
`re_view_graph`, `re_view_map`, `re_view_spatial`, `re_view_state_timeline`,
`re_view_tensor`, `re_view_text_document`, `re_view_text_log`,
`re_view_time_series`.

| crate | purpose | egui/wgpu | lib? |
|---|---|---|---|
| `re_view` | ViewClass execution, `execute_systems_for_view` | no | yes (rlib) |
| `re_viewport` | viewport panel | egui+egui_tiles | yes (rlib) |
| `re_viewport_blueprint` | layout model `ViewportBlueprint` (`viewer_support/re_viewport_blueprint/src/viewport_blueprint.rs:37`); Arrow coupling `Cargo.toml:35`, `view_contents.rs:3,18` | egui+egui_tiles | yes (rlib) |
| `re_renderer` | wgpu renderer | wgpu, no egui | yes (rlib, publish) |
| `re_viewer` | viewer app/shell | eframe+egui+egui-wgpu+wgpu | yes rlib+cdylib, heavy |
| `rerun-cli` | `rerun` binary | none direct | no |

Registration seam: `re_view/src/system_execution.rs:134-139`
`ctx.view_class_registry().new_context_collection(...)/.new_visualizer_collection(...)`;
iteration `re_viewport_blueprint/src/lib.rs:72-81`. `ViewClass`/`ViewClassRegistry`
defined in `re_viewer_context` — NOT-MEASURED.

Integration ranking: `re_renderer` reuse; `re_view` reuse (ViewClass); `re_viewport_blueprint` reuse; views reproduce via ViewClass pattern; `re_viewer` embed only if full app wanted; `rerun-cli` reuse as binary.

## Mesa — verdict borrow contract, do not depend

`mesa-dev` v1.24.0 README:1: *"archived and is now kept closed source."* Deps:
`reqwest 0.12 rustls` + `tonic 0.14.4` to `https://vcs.depot.mesa.dev`
(`crates/mesa-dev/Cargo.toml`, `client/mod.rs:43`). No Rust local-FS primitive.
API surface: branches `client/branches.rs:31-104` (`list/create/delete`);
change ops `client/change.rs:64-364` (`create_from_ref`, `apply_ops`, `snapshot(msg)`,
`move_bookmark`, `expected_last_op_id`/`expected_update_seq` optimistic lock);
content `client/content.rs:12-37` (`get(ref, path, depth)`). Examples
`just-bash-shell/index.ts:23-39`: `mesa.fs({layout}).mount()` + `bash({cwd})`;
writes session-local until snapshot; auth `MESA_PRIVATE_KEY`/`MESA_REPO`.

Three borrowable shapes (re-backed locally, no gRPC):
1. Change journal: `apply_ops→snapshot→move_bookmark` + optimistic `expected_op_id` → Physis `edit_node`/ledger batch; branch/bookmark.
2. Layout-mount + session-write FS → `NavWorkspace` façade + `WorldObject` view; read-through, explicit commit.
3. Ref/path/depth content reader → `NavIndex`/object-lens projections; degrade to lexical when no embedder.

## Physis architecture map (source-backed)

Key modules (classification / flow):
- HUD contract: `src/hud/{mod,world,event,bridge,intent,lens_*,row,palette,config}.rs` — renderer-neutral, projection-switch keeps focus (`world.rs:220`), reducer-only authority (`event.rs`), 9 `ProjectionId` (`world.rs:133`).
- World model: `world_object.rs: WorldObject/WorldObjectId` (read-only view, identity ≠ path).
- Nav/storage: `nav/mod.rs` `ObjectId/NavIndex/RetrievalScore` hybrid query `:608`; `nav/workspace.rs` `NavWorkspace` façade; JSON index + atomic rename `:777-822`.
- Embed/classify: `embed/{mod,registry,onnx,jina}.rs`; `classify.rs` facade.
- Temporal: `ledger.rs` append-only per-tenant; `address.rs` FileAddress.
- Agents/sensors/guided: `agents/{mod,attention,similar}.rs`, `sensory.rs`, `guided/{mod,compile}.rs`.
- MCP: `mcp.rs` OPTIONAL (feature gated); `nav_old/` LEGACY (NO-DELETIONS risk).
Feature tiers `Cargo.toml:140-202`. Six flows: DATA(walk→hash→embed→classify→ledger), STATE(JSON+sled), QUERY(nav/mod.rs:608 hybrid), ACTION(reduce-only), AGENT(MCP/roster), UI(projector invariant), CONFIG(4-layer merge).

## Primitive correspondence map (§22)

Status: ACTIVE Physis-native; ALTERNATIVE_BACKEND behind trait; EXTENDED (Rerun column under Physis semantics); LEGACY/EXPERIMENTAL/DISABLED; SUPERSEDED_BY_PRESERVED.

| Physis primitive | current impl | Rerun equiv | Mesa equiv | Physis adds | status | integration |
|---|---|---|---|---|---|---|
| Cell/Object | `world_object.rs: WorldObject` | Chunk+EntityPath | content node | 7 attention terms, cell verdict, affordances | ALTERNATIVE_BACKEND | `WorldStore` trait |
| Object ID | `world_object.rs: WorldObjectId; nav/mod.rs: ObjectId` | `EntityPath` (locator) | ref | survives rename/move/startup | ACTIVE | `EntityPath` = locator behind adapter (H19) |
| semantic cell | `nav/mod.rs NavIndex` | none | none | 7×9 grid + verdict feedback | ACTIVE | keep |
| component/property | `WorldObject` fields | `ComponentDescriptor` | change ops | typed value + provenance | EXTENDED | Rerun column as alt under `WorldStore` |
| archetype/schema | `OntologyEntry axis_kind` | `Archetype` trait | change schema | ontology/cell verdict | ACTIVE | map archetype→cell |
| relation | `WorldObject.relations`; `RelationOrigin` User/Deterministic/Inferred | graph edges | move_path | provenance never collapsed; plural parents | ACTIVE | `RelationEngine`? trait if useful |
| graph edge | `nav/mod.rs` pins/relations; `lens_object.rs` | `re_view_graph` | content-tree edges | origin-tagged glyphs | ALTERNATIVE_BACKEND | `GraphRenderer` trait |
| dependency | `dependencies: Vec<WorldObjectId>` | `re_importer` | apply_ops | versioned, survives refactor | ACTIVE | keep; render via re_view_graph |
| embedding | `embed/` registry; `NavIndex semantic_position` | none | none | named embedder, stamp refusal, lexical→semantic degradation | ACTIVE | keep (Rerun has none) |
| invariant | `guided::Constraint`; ledger causality | none | `expected_update_seq` | semiotic grid verdict | ACTIVE | borrow Mesa contract shape |
| classifier | `classify.rs` | none | none | nearest-entry max-cosine + attention readout | ACTIVE | keep; ChunkStore as alt evidence log |
| affordance | `WorldObject.affordances` | none | none | CapabilityRegistry-derived | ACTIVE | keep |
| action/tool | `guided::Action`, `mcp.rs`, `HudEffect` (no exec) | none | change ops | verbs gated on state; reducer-only | ACTIVE | keep |
| provenance | `Provenance`; `ledger.rs`; `address.rs` | none | change+bookmark+`get(ref,…)` | cell verdict, replay-stable keys | ACTIVE | truth spine; Mesa shape informs envelope |
| confidence | `agents/attention.rs` weights | none | none | per-term raw×weight, absent-not-zero | ACTIVE | keep |
| observation | `sensory.rs` | log lines | change ops | structured signals→ontology | ACTIVE | optionally emit Rerun components |
| temporal state | `ledger.rs`; `address.rs TimeRef` | Chunk timelines; `TimeInt::STATIC` | snapshot/bookmark | causal lineage, purposes | ALTERNATIVE_BACKEND | `TemporalStore` trait |
| delta | `temporal_history: Vec<Provenance>` | row_id diffs | change ops | semantic diff | EXPERIMENTAL | `SemanticDelta` over history |
| event | ledger `CauseEvent`; `hud/event.rs HudEvent` | log msg / row | change op | intent/actor/model/confidence/cost | ACTIVE | keep both |
| version | `temporal_history`; commit/origin | chunk lineage | snapshot/bookmark | semantic identity | ACTIVE | keep |
| branch | (new, see plan) | none | bookmark/branch | agent branch = provenance fork | ACTIVE(new) | borrow Maya bookmark locally |
| snapshot | (new) | .rrd dump | `snapshot(change_id,message)` | intent-scoped | EXPERIMENTAL | `AgentWorkspace` trait |
| query | `nav/workspace.rs:248 query` | `QueryCache::latest_at` | `get(ref,path,depth)` | semantic+lexical+recency+context+pin | ALTERNATIVE_BACKEND | `QueryEngine`? trait if needed |
| semantic query | same | none | none | intent→entity+component+time | ACTIVE | keep; fall back to Rerun latest-at |
| intent | `hud/intent.rs` | none | none | ExactObject/Intent routing | ACTIVE | keep |
| projection | `hud/world.rs ProjectionId`; `row.rs` | `ViewportBlueprint` | none | one world, many projections | ALTERNATIVE_BACKEND | Rerun layout as alt backend behind ProjectionBackend |
| HUD view | 9 `ProjectionId`; lenses | 10 `re_view_*` | none | keyboard-first, semantic zoom, agent traces | ALTERNATIVE_BACKEND | register Physis ViewClass; render via re_renderer |
| notebook | `notebook.rs` (widget-id+params) | `re_view_text_document` | none | cited answers over graph | ACTIVE | keep; render cells as Rerun text docs |
| terminal | (in physis_wow, NOT measured) | none | bash() | command as intent | EXPERIMENTAL | bash() over FS seam |
| agent workspace | (new, see plan P5) | none | workspace=repo ref | branch+diff+validate | EXPERIMENTAL | `AgentWorkspace` trait |
| sensor reading | `sensory.rs` | time-series components | none | typed, not text | ACTIVE | emit re_view_time_series components |
| process | (not mapped) | none | change ops? | state+provenance | EXPERIMENTAL | add via sensors/ledger |
| file | `nav` hash index; `address.rs SpaceRef` | entity path | repo path | content-addressed | ACTIVE | keep; mirror to ChunkStore |
| source symbol | (not mapped) | none | none | survives refactor | EXPERIMENTAL | add to WorldObject identity |
| machine/device | fleet index PH-028 (NOT measured) | none | none | cross-device identity | EXPERIMENTAL | EntityPath hierarchy as locator scheme |
| memory | JEV/Hermes/OpenCode; `mesh mem` | none | agent memory (opaque) | structured, replay-stable | ACTIVE | keep native (Mesa memory cloud-only) |

## Disable-not-remove table

| module | replacement candidate | feature flag | original accessible via | risk | tests |
|---|---|---|---|---|---|
| `src/nav_old/` | `src/nav/` | `legacy-nav` (new) | feature off=not compiled; on=fallback | drift | none (LEGACY) |
| `re_viewer` shell if embedded | Physis native HUD | `use-rerun-viewer` | feature off=native HUD | weight/cost | `cargo check --no-default-features --features rerun-viewer` |
| Pro MCP `src/mcp.rs` | physis_system MCP | `feature = "mcp"` (already off default) | feature gate | dual surfaces | `cargo check --features mcp` |
| repair pipeline `semantic_delta/dependency/staleness/repair_regions/orchestration/metrics` | core nav/ledger | `repair-pipeline` (already off) | feature gate | none | `cargo check --features full` |

## Backend-trait seam decision

Add now only where it materially improves experimentation — Physis-native
default so behavior is unchanged unless opted in:
1. `WorldStore` — default `PhysisNativeStore` (index JSON + sled); alternative `RerunChunkStore` (re_chunk_store). Feature `store-rerun`; env `PHYSIS_STORE=rr`.
2. `TemporalStore` — default `LedgerStore`; alternative `RerunTimeSeries` (QueryCache::latest_at). Feature `temporal-rerun`.
3. `GraphRenderer` — default `PhysisTui`; alternative `RerunGraphView` (registered `ViewClass`) / `RerunViewer`. Feature `rerun-graph`/`rerun-viewer`.

NOT introducing `ProjectionBackend`/`AgentWorkspace`/`SemanticResolver`/`ViewerBackend`/`HistoryBackend` yet — no measured need; speculative abstraction.

## H19: identity ≠ locator
`EntityPath` (`entity_path.rs:93`) is a locator. `WorldObjectId(origin:key)`
(`world_object.rs`) is identity. Rerun address is a derived projection behind
`WorldStore::locate(identity) -> EntityPath`; renames/moves update only the
locator mapping.

## HUD architecture (one line)
Keep `src/hud/` renderer-neutral contract; add `feature = "rerun"` that
registers Physis objects as a `ViewClass` against `ViewClassRegistry`
(`re_view/system_execution.rs:134-139`), lays out via
`re_viewport_blueprint::ViewportBlueprint`, and renders through `re_renderer`
(wgpu, no egui). Same object-space, multiple projections; no Physis module
deleted — H6 floor result below.

## H6 floor baseline (measured 2026-09-22, detached 0fc6a94, bge-base-en-v1.5)
```
using embedder: bge-base-en-v1.5 (768-d)
CORE top-1 domain: 0.836 (46/55); cell: 0.473 (26/55)
HARD top-1 domain: 1.000 (9/9)
ITALIAN top-1 domain: 0.667 (10/15); cell: 0.467 (7/15) [model=bge-base]
test result: FAILED. CORE 0.836 below floor 0.85
```
Identical 0.836 vs H6 branch HEAD — H6 language-pack config did **not** regress classification; the floor's comment number (claimed 0.891, 2026-08-24) is stale. Floor is fail-hard (by design) and `#[ignore]`d; predeploy guards with `SKIP_ACCURACY=1` / model-presence check. No `tests/classification.rs` floor change made here — left for a dedicated calibration packet.

## H6 floor decision
Baseline on parent `0fc6a94` scored 0.836, identical to `agent/H6-langpacks`;
H6 is not the cause. Decision: **no code change to the test or to H6** — the
floor is deliberately fail-hard and `#[ignore]`d; the stale 0.891 comment
predates this session and 0.836 is a calibration issue, not an H6 regression.
A dedicated packet (`PH-113-floor-calibration`) owns the 0.836→target decision;
this programme does not touch it.

## Caveats / NOT-MEASURED (updated 2026-09-22 gap pass)
- Physis: `src/bin/hud.rs` now measured — 9-view enum (`World/Graph/Semantic/Notebook/Tasks/Agents/Settings/Setup/Help`, `bin/hud.rs:448-471`), single ratatui `draw(f,&App)` + `View::from_name` CI snapshots.
- Physis: `physis-core` measured — 53 files (`lib.rs:184-222` modules); seams `ontology::OntologyLoader:327`, `classify::Cell/CellClassifier:91`, `embed::VectorEmbed` trait + `RandomProjectionEmbedder:69` + `select:228`.
- MCP tools **verified 30** (`grep -oE 'tool("' src/mcp.rs` = 30); `physis_system` is the separate system MCP server (`.opencpn/opencode.json:10`).
- Rerun: `ViewClass` trait `viewer_support/re_viewer_context/src/view/view_class.rs:156`; `ViewClassRegistry` `view_class_registry.rs:199`/`new_context_collection:471` — now backed.
- Rerun blueprint Arrow coupling is `FromArrow` (`re_viewport_blueprint/src/view_contents.rs:18`), not `ArrowWriteable` — corrected.
- Remaining NOT-MEASURED: cross-repo dependency weights (no rerun build); exact `WorldObject`→`WorldObjectId` identity-mapping lines beyond `world_object.rs` (confirmed present, used by HUD lenses); `nav_old/` still-referenced check.
