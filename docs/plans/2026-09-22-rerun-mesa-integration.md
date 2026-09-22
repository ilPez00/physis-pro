# Rerun + Mesa integration — the impossible machine (added 2026-09-22)

> **Governing principles, in force for every packet under this programme:**
> 1. **NO DELETIONS.** An existing Physis module that a better architecture
>    supersedes is commented out where safe, feature-gated, deprecated, moved
>    behind an adapter, made dormant, preserved in place, or left as an
>    alternative backend — never deleted, purged, or stripped of history.
>    Architectural decisions must remain reversible.
> 2. **Do not build from scratch** what can be wrapped, composed, adapted, or
>    reused from a stable existing primitive (Rerun crates, Mesa SDK, git,
>    Arrow, sled, existing Physis seams).
> 3. **Build the most powerful human + agent interface**, not the cleanest
>    internal architecture. Target: the impossible machine — semantic
>    computer, JARVIS/HUD, living knowledge graph, NotebookLM, IDE, terminal,
>    system monitor, agent cockpit, temporal debugger fused into one
>    object-space. Views are projections over one world, not applications.

## Source brief

User directive 2026-09-22 (full text in session log): determine at source-code
level how Physis integrates/wraps/adapts/borrows from **Rerun** and **Mesa**;
map every Physis primitive (cell/object, id, relation, embedding, provenance,
temporal state, branch, query, projection, HUD view, …) against Rerun and Mesa
equivalents with recommended status (ACTIVE / WRAPPED / EXTENDED /
ALTERNATIVE_BACKEND / LEGACY / EXPERIMENTAL / DISABLED_BY_DEFAULT /
SUPERSEDED_BUT_PRESERVED); identify modules that can be disabled-not-removed
with feature-flag migration paths; prototype agent branching semantics
(WorldState → fork → AgentWorkspace → CandidateState → SemanticDiff →
Validation → Merge/Reject) with actor/agent/model/timestamp/parent/intent/
tool/affected-objects/confidence/cost on every mutation.

## Environment, measured 2026-09-22

| repo | state |
|---|---|
| `/home/gio/dev/rerun` | clone present, HEAD `35143bce37` (prerelease-1369-g35143bce37), 9812 commits, crates: build, data_flow, panels, store, store_app, tests, top, utils, viewer_support, views |
| `/home/gio/dev/mesa` | **absent**. The core versioned filesystem ("mesafs", per `symlink-test-fixtures` E2E description) is **not public**. Inspectable surface: `mesa-dot-dev/sdk-rust` (Rust SDK), `sdks` (TS/Python mirrors), `examples` (CLI agent on Mesa cloud filesystem), `agent-gateway`, `mesa-sandbox`, forks `fuser` (FUSE) / `jj` (VCS) / `bashkit` |
| `/home/gio/dev/physis-pro` | same checkout as `/home/gio/physis_pro` (same HEAD `8fb8ddb`, branch `agent/H6-langpacks`) — one clone, not two |
| `/home/gio/dev/physis-global` | `d32afe7` main — classify/storage/sensory/video/mcp coverage tests |
| `/home/gio/dev/physis_wow` | `15bbe32` main — dashboards, structured-output workflow, plan experiments (UI experiments live here) |

## Phases (one packet each, per `packets/PROTOCOL.md`)

| # | phase | packet | scope | status |
|---|---|---|---|---|
| P0 | 0 | `PH-107-physis-architecture-map` | DATA/STATE/QUERY/ACTION/AGENT/UI/CONFIGURATION flows from code, not docs; module classification (ACTIVE…REFERENCE_IMPLEMENTATION); stale docs ignored | todo |
| P1 | 0 | `PH-108-rerun-data-model` | Entity/EntityPath/Component/Archetype/Chunk/Recording/Store, static vs temporal, Arrow, storage, indexing → can Rerun be the typed temporal substrate under Physis as the semantic layer? latest-at/range/dataframe queries; deterministic-retrieval test ("CPU temperature" → entity+component+latest → 67.2 °C, zero model calls) | todo |
| P2 | 1 | `PH-109-rerun-visualization` | GraphView, dataframe view, time-series, tensor, spatial, logs, timelines, selection model, blueprint system, egui integration, embeddability; evaluate A–H (embed viewer / fork / reuse crates / renderer-only / temporal backend / debug backend / Physis-native reproduction / hybrid) against visual power, latency, huge datasets, multimodal, offline, keyboard-first, AR future | todo |
| P3 | 1 | `PH-110-mesa-filesystem` | sdk-rust + examples surface: versioned filesystem semantics, branches, checkpoints, concurrent agent edits, merge/rollback, provenance, human approvals, POSIX/FUSE, copy-on-write cost, conflicts; architectural borrowing preferred over dependency; offline-first | todo |
| P4 | 2 | `PH-111-correspondence-map` | the full primitive correspondence table (§22 of the brief) + disable-not-remove list with feature flags + backend-trait seam decision (WorldStore/TemporalStore/GraphRenderer/… only where they materially improve experimentation) | todo |
| P5 | 2 | `PH-112-agent-branching` | every agent run as a branch of reality: fork/diff/validate/merge semantics, mutation provenance fields, HUD visibility of branches, pending merges, conflicts, confidence, cost; Hermes/OpenCode/JEV as native actors with structured context packages (context compression: intent + relevant objects + relations + dependencies + recent changes + affordances + tools) | todo |
| P6 | 3 | HUD presets + magic textbox | DEVELOPER/RESEARCHER/SYSTEM/NOTEBOOK/AGENT COMMAND/DEBUG/ROBOTICS/KNOWLEDGE/MINIMAL/VISOR presets over one object-space; semantic zoom (MY COMPUTER → pansa → project → crate → module → function → value); declarative `hud.toml` views/layouts/hotkeys/themes/graph/agents config; magic textbox escalation ladder (deterministic parser → structured resolver → small model → large agent) | todo |

## Standing constraints

- H11 (scope discipline) applies to *product surface* claims; this programme is
  explicit about which new seams are interfaces vs features.
- Every integration keeps Physis offline-capable: Rerun/Mesa paths are
  alternative backends, never the only path.
- Semantic identity ≠ storage location (§19): EntityPath and file paths are
  locators; identity survives renames, moves, restarts, refactors, devices.
- Cross-device world model (§20) builds on PH-028's fleet index (pansa/omo/
  feather/air objects already reachable from one HUD session).
- Carry-forward from the 2026-09-22 session: H6 accuracy floor re-baseline
  (CORE measured 0.836 < 0.85 floor on `agent/H6-langpacks`; baseline run on
  `0fc6a94` pending — H6 itself is pushed and reviewed separately; the floor
  investigation does not block this programme).
