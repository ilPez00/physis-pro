# Engine Pipeline Diagrams

How data flows through the physis engine. Two views:

1. **The ingestion → recall pipeline** — where every labeled node comes from and
   how it is retrieved.
2. **The live engine phases (1–7)** — the process modules and their wiring.

Keep this file in sync with `src/` when a module is added or rewired (check the
`AGENTS.md` module table for authoritative file mapping).

## 1. Ingestion → recall pipeline

```
                        ┌────────────── ingestion connectors (Phase 7B) ──────────────┐
                        │                                                             │
   file affinity  scan  │  watch-mode (auto)   vault (notes+git)   history   praxis   │
   ─────────────►/scan  │  /watch              /vault/import       /history/import    │
                        │  physis-pro watch    physis-pro vault    ...       /praxis/backfill
                        │                                                             │
                        ▼                                                             │
              ┌─────────────────────┐   label = title/path/headings/commit/message     │
              │  shared embedder     │◄─ embed every label ONCE                        │
              │  (bge-base / CLIP /  │   (vectors carry embedder provenance stamp)    │
              │   MiniLM / RP)       │                                                │
              └──────────┬──────────┘                                                │
                         ▼                                                             │
              ┌─────────────────────┐    label_index (label→id)  ⇒ re-register of a    │
              │  CoherenceNode        │◄── seen label returns the SAME id (dedupe)     │
              │  (id, label, assert) │                                                │
              └──────────┬──────────┘                                                │
                         │   sled persist (per tenant)                                 │
                         ▼                                                             │
      ┌───────────────────────────────────────────────────────────┐                    │
      │  recall  ·  search/nodes (token-fixed) · cohere · dream   │                    │
      │            · PDCA · holarchy edges (IdBridge)             │                    │
      └───────────────────────────────────────────────────────────┘                    │
                         │                                                             │
                         ▼                                                             │
      ┌──────────────┬──────────────────────┐   ┌─────────────────┐                    │
      │  wiki-export │  spanner sync (7C)   │   │ Umbrellapedia   │◄── wiki pages +    │
      │  markdown    │  durable memory      │   │ (external)      │    graph exports   │
      └──────────────┴──────────────────────┘   └─────────────────┘  ──────────────►   │
                                                                                      │
              every edge into Umbrellapedia carries a LABEL, never a bare vector      │
              └───────────────────────────────────────────────────────────────────────┘
```

### Key invariants

- **One embedder for everything.** The shared `SharedCtx.embedder` embeds both
  registered labels and query vectors. A provenance stamp on each node lets a
  loader detect model swaps (`stale_nodes` → re-embed) instead of comparing
  across spaces.
- **Labels are the dedupe key.** Re-ingesting the same label returns the existing
  node id; idempotent scans are the default, not a feature.
- **Verdicts are explicit.** Asserted verdicts (Praxis backfill, `assert_coherence`)
  are the only thing that moves the coherence score; a reported observation that
  was never judged stays at derived-only.

## 2. Live engine phases

```
 data ─► 1 holarchy + rachmaninov  ─► 2 embed + classify (grid)  ─► 3 gantt schedule
                              │                                    │
                              └────────────► 4 distributed (MCP mesh)
                              │
                              ▼
                 5 dreams ─► 6 skip/multimodal ─► 7 content recall (wiki + spanner)
```

### Phase 1 — Holarchy & real-time graph (`graph.rs`, `rachmaninov.rs`)

Live operational graph. Nodes are joined to coherence ids via `IdBridge` so edges
survive label edits. `rachmaninov` ticks on a context vector to drive the dream
loop.

| Piece | File | Note |
|-------|------|------|
| `HolonicGraph` slotmap nodes/edges | `graph.rs` | dense keys, `resolved_edges()` bridges to coherence |
| `IdBridge` coh↔holon | `graph.rs` | unlink on delete |
| `Rachmaninov` | `rachmaninov.rs` | context tick |
| Sensory ingest | `sensory.rs` | AuraFrame → holarchy |

### Phase 2 — Embedding & classification (`embed/`, `classify.rs`)

Text/image → vector → nearest `DOMAIN×MODE` grid cell. Cells are centroids built
from ontology entries; **facets** (`Facets`) carry dimensions the 2-D grid can't
hold (lifecycle, agency, scale, abstraction, sub-domain/mode) — query them with
`physis-pro facet`.

| Piece | File | Note |
|-------|------|------|
| Embedder cascade + self-test gate | `embed/mod.rs` | picks the first *working* ONNX model |
| `CellClassifier` | `classify.rs` | grid placement, `best_entry_sim` |
| `DynamicVectorTrie` | `trie.rs` | constant-cost keyword trie |

### Phase 3 — Gantt & temporal scheduling (`gantt.rs`)

Turns goal vectors into time-blocked schedules (PDCA interplay).

### Phase 4 — Distributed / MCP mesh (`mcp.rs`, `ai/`)

MCP server (TCP or `--stdio`) exposing engine tools + a provider cascade for
LLM-backed agents.

### Phase 5 — Dreams (`dream.rs`, `rachmaninov.rs`)

Coherence-graph proposals: pick a pair of nodes, interpolate, score the mutation
against asserted verdicts. Dreams are a rhythm, not a request.

### Phase 6 — Multimodal skip

Voice/images/video all reduce to vectors (`embed_image`, future frame-sampling),
run through the same classify/cohere core. Primarily builds on Phase 2.

### Phase 7 — Content recall (A: browser, B: feeders, C: durability)

The wiki is fed by labelled connectors (Section 1), browsed by the in-process
TUI (`wiki`, Tantivy BM25) or exported markdown, and mirrored durably to Spanner.

## Where a new endpoint lands

1. **Pure parser/reducer** → new module (e.g. `vault.rs`, `history.rs`, `praxis.rs`)
   with pure unit tests.
2. **Registration** → shared-embedder → `register_node_vec_labeled` (dedupe).
3. **Surface** → web handler + `web.rs` route (protective middleware) and a CLI
   command in `main.rs`/`cli.rs`; both tenant-aware (web) / `_default` (CLI).
4. **Docs** → `AGENTS.md` module row + this file if it changes the pipeline.