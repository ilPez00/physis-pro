# Datawork Plan — Organizing Files in Space, Time, Cause (+ their Ontologies)

> Created 2026-08-26. Extends Phase 6 (administration/datawork) and the file
> theme graph (PLAN 3.1) with the three filing axes meaning alone cannot carry.
> Framing follows the repo's semiotic lineage: Aristotle's four causes, read
> as *filing dimensions* for documents.

## Why

The theme graph answers **"what is this about?"** (formal cause: the DOMAIN×MODE
cell; material cause: DocKind/format). It cannot answer:

| Question | Axis | Today |
|----------|------|-------|
| Where does this live / belong? | **Space** | raw path strings only; study log has free-text `place` |
| When was it made, when does it matter? | **Time** | mtime scattered in scan/indexer, never indexed for recall |
| What produced it? | **Efficient cause** | watch/vault/history importers know their source, record nothing durable |
| What is it for? | **Final cause** | goal nodes exist but files are not attached to them |

## Design: one address, four coordinates

Every ingested file gets a durable **`FileAddress`** persisted next to its
theme-graph node (extend `graph.rs::FileNode` — `NodeView`'s exhaustive
destructure forces every consumer to handle the new fields):

```rust
pub struct FileAddress {
    pub space: SpaceRef,   // place id from the space ontology + canonical root
    pub time: TimeRef,     // created/modified/ingested + period membership
    pub cause: CauseRef,   // efficient: producing event; final: served purpose
}
```

- `SpaceRef { place: String, root: PathBuf }` — `place` resolves through the
  space ontology (below); unknown places auto-register as leaves.
- `TimeRef { created: DateTime<Utc>, modified: DateTime<Utc>, ingested: DateTime<Utc>, period: String }`
  — `period` names the rhythm bucket the file lands in (shift/day/sprint/…).
- `CauseRef { efficient: CauseEventId, final: Option<GoalId> }` — one producing
  event (append-only ledger), optionally one purpose link.

### Causal ledger (efficient + final cause)

Append-only `CauseEvent` stream per tenant (sled), projected onto the existing
`OperationalEvent` pipeline (study.rs `to_op_event` precedent — provenance
connector `"cause"`):

```rust
pub struct CauseEvent {
    pub id: CauseEventId,
    pub at: DateTime<Utc>,
    pub actor: Actor,        // person | tool(scan/watch/git/import) | machine
    pub mechanism: Mechanism,// generated | received | derived | corrected | exported | cited
    pub object: FileId,
    pub input: Option<FileId | NodeId>, // parent artifact, if any
}
```

Lineage queries walk this chain: "why does this file exist" → first event;
"what did this report feed" → downstream events. Final cause is a pointer:
attaching a file to a goal/workflow node writes a `serves` event, so purpose
is *also* ledger history — one store, no parallel truth.

## The two new ontologies

Both follow the office-ontology pattern (`kind` + fresh `axis_kind`, loaded by
`PhysisProConfig`; facet CLI picks them up automatically).

**Space** — `config/space_ontology.json`, `kind: "place"`, `axis_kind: "space"`.
Hierarchy via `sub-domain`. Seed entries (~30): Plant > Line > Station >
Machine; Building > Room > Desk; Repository > Service; Mailbox > Folder;
Cloud > Drive > Shared folder. Each entry carries optional geo hints
(lat/lng/radius — topos waypoints already model this in life-os).

**Time** — `config/time_ontology.json`, `kind: "period"`, `axis_kind: "time"`.
Seed entries (~20): Shift (A/B/C), Day, Week, Sprint, Month, Quarter, Fiscal
year, Maintenance window, Campaign, Season. Chronos rhythm detection from
study.rs classifies observed cadences into these kinds rather than inventing
new vocabulary.

## Capture points (no new pipelines)

Registration sites already enumerate every way a file enters; each gains an
address write:

| Entry point | Space source | Time source | Efficient cause |
|---|---|---|---|
| `scan` / watch mode | dir root → nearest place | fs birth/mtime | `generated(tool=watch)` |
| vault import (Obsidian/git) | repo/dir root | commit or note date | `received(source=obsidian)` / `derived(parent=commit)` |
| history importers | browser/mailbox kind | bookmark/feed timestamp | `received(source=chrome)` |
| praxis backfill | tracker name | entry date | `derived(parent=entry)` |
| shift evidence export | machine id → station | shift window | `exported(report=…)` |
| agent runs | workspace root | run time | `generated(actor=agent)` |

## Query surface

- **CLI:** `physis-pro where <query> [--place plant] [--within 14d] [--during sprint-34] [--cause generated] [--cell OPERATE/MONITOR]`
  — composes the four facets; output groups by place then period (browse-graph conventions).
- **Web:** `POST /api/v1/files/query` (tenant-scoped, `AppCtx::with_user`) returning
  `{ matches, by_place, by_period, by_mechanism }`; powers:
  - `/ui` panels: place filter + timeline strip alongside the force graph;
  - console Timeline tab: cause events join the existing operational timeline;
  - lineage view: ledger chains rendered as edges (graph FM page reuses cose layout).
- **Views derive from state by construction** (architecture constraint): the
  ledger *is* the timeline/map/lineage — no separate reporting assembly.

## Phases

| # | Deliverable | Acceptance |
|---|-------------|-----------|
| D1 | `FileAddress` on `FileNode` + capture at scan/watch | exhaustive destructure compiles; re-ingest preserves addresses |
| D2 | Space + time ontologies loaded, facet CLI lists them | `facet --kind place` returns entries |
| D3 | Causal ledger (sled, per-tenant) + projections | replay = same timeline; tenant A never sees B's events |
| D4 | Remaining importers write addresses | vault/history/praxis round-trip tests |
| D5 | `where` CLI + `POST /api/v1/files/query` + UI panels | facet composition e2e test; /ui renders place+time filters |
| D6 | Purpose links (`serves` events) from goal nodes | attaching file↔goal visible in lineage view |

## Constraints honored

- Embedder consistency untouched (addresses ride existing embeddings).
- Tenant isolation: ledger and addresses live in per-tenant sled scope.
- No adapter guessing: space ontology ships with generic seeds; plants rename
  during onboarding (setup wizard step).
