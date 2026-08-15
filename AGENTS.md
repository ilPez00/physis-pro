# Physis Pro — Industrial Backoffice Intelligence

## Overview
Physis Pro extends the core engine with **industrial backoffice intelligence**:
- **Quality feedback loop** — embed failures against the semiotic grid, apply penalty, persist
- **Industrial monitoring** — machine adapter pattern (MQTT, Modbus, serial, REST)
- **Document ingest** — classify and process structured/unstructured documents
- **Inventory tracking** — SMA-30 forecast + human-in-loop ordering with cooldown
- **Ontology extensions** — 75 office domains + 23 machine process + 21 AI agent workflow

## Multi-Tenancy (Web API — `physis-pro-web`)

The web server (`src/bin/web_server.rs`) is **per-user**. Session state
(`goals`, `actor`/PDCA, `dreams`, coherence `core`) lives in a tenant map keyed
by the **`X-Physis-User`** request header:

- Header absent/blank → `"_default"` tenant (the original single global state;
  the holarchy background threads drive this one). Back-compatible.
- Any other value → an isolated `RuntimeState`, created on first use, so one
  user's PDCA/dream/coherence/goals never bleed into another's.

Shared, immutable infra stays global in `SharedCtx`: embedder, classifier,
ontology, provider cascade. **Stateless** endpoints (`/chat`, `/embed`,
`/classify`, `/compress/logs`, `/context/filter`, `/translate`, `/semiotic/*`,
`/reconstruct`) ignore the header. The **holarchy** (`/holarchy/*`) stays global
— it's the shared knowledge graph fed by a single ingest/sensory pipeline.

Tenant-scoped handlers: `goals`, `scan`, `query`, `stats`, `pdca/*`, `dream/*`,
`coherence/*`. Helper: `AppCtx::with_user(uid, |rt| …)`.

Praxis callers must send `X-Physis-User: <praxis_user_id>` on those endpoints
(see `physisProClient.ts` `tenant()` helper).

## Multimodal & Sensory

Physis is one multimodal engine — voice, images, and video all reduce to vectors
in the shared embedding space, then run through the same classify / cohere /
dream core. README → "Multimodal Sensory" has the full picture.

- **Voice** — `src/bin/voice.rs` (`physis-pro-voice`, features `voice,web`).
  arecord → whisper-large-v3 → llama3.2 extraction → `OntologicalMap` → live
  graph UI on `:3000`. **Folds in the standalone `physis_voice` repo** (now
  superseded; keep for reference only).
- **Images** — `embed_image()` on `embed/clip.rs` + `embed/jina.rs`; `/classify`
  and `/embed` accept base64 `image`, fall back to text on failure (never 500).
- **Video** — planned: frame-sample → `embed_image` → temporal pool. Not built.
- **Sensory bus** — `src/sensory.rs` `AuraFrame` over a Unix socket;
  `SignalType` = `AudioPCM | VisionFeature | Acceleration | Gyroscope |
  ThoughtCapture`. Single ingestion path into the holarchy.

## Wiki (in-process, `wiki-embedded` feature)

`src/wiki.rs` historically shelled out to the `llmwiki`/`wiki` binary. With the
`wiki-embedded` feature it instead links the **`llmwiki` crate**
(`../llmwiki_physis`, package `llmwiki`) in-process — no subprocess. Phase 1 is
**Tantivy BM25 only** (`default-features = false` → no `fastembed`/`usearch`, so
no `ort` clash with physis's embedder). `search_wiki()` tries in-process first,
falls back to binary/grep. HTTP: `POST /api/v1/wiki/search` + `/api/v1/wiki/index`
(gated). Phase 2 = swap llmwiki's fastembed for physis's `VectorEmbed` (one model).

## Key Modules

| Module | File | Description |
|--------|------|-------------|
| Quality | `quality.rs` | `QualityTracker` — `report_failure()`, `report_success()`, `save()`, `load()`. Persists 384-dim embeddings to `~/.physis/quality_failures.json`. 6 tests |
| Wiki | `wiki.rs` | `llmwiki` CLI fallback with recursive grep fallback |
| Industrial | `industrial.rs` | `MachineAdapter` trait, `MachineReading`, `ProcessAnomaly`, `detect_anomalies`, stub `MqttAdapter`. 4 tests |
| Ingest | `ingest.rs` | `DocKind`, `IngestedDoc`, `classify_file()`, `ingest_all()`, `sort_by_kind()`. 4 tests |
| Inventory | `inventory.rs` | `InventoryTracker` — SMA-30 forecast, sled persistence, order cooldown. 8 tests |
| Semiotic editing | `core.rs` + `web.rs` | **Exchange-parts edit**: `PhysisCore::edit_node()`/`delete_node()` swap node content while preserving structure (id/verdict/edges/provenance); `CoherenceNode.cell_pin` hybrid-pin semantics. API: `POST /api/v1/coherence/edit` + `/delete`. CLI: `node-edit`/`node-delete`. See `docs/SEMIOTIC_EDITING.md` |
| Fixed-token search | `web.rs` + `rag.rs` | `POST /api/v1/search/nodes` + CLI `node-search` — budget-bounded similarity search over coherence + goal nodes (`total_tokens ≤ token_budget`), reusing `rag::TokenFixedRetriever` + `count_tokens` |

## Ontology Extensions

Three new ontology domains added on top of the 9 built-in ontologies (511 entries):

| Ontology | File | Type | New Domains |
|----------|------|------|-------------|
| Machine Process | `config/machine_ontology.json` | machine (kind) | 23 — Thermal Zone, Pressure Reg, Torque & Force, Speed, Flow, Level, Material Feed, Power Draw, Machine Mode, Startup Sequence, Changeover, Batch Tracking, Machine Health State, RUL, Safety Interlock, Environmental Condition, Alarm Categorization, Anomaly Detection, Production Throughput, OEE, Cycle Time, Yield & Scrap, SPC Quality |
| AI Agent Workflow | `config/agent_ontology.json` | agent (kind) | 21 — System Prompt, User Input, Assistant Response, Context Window, Prompt Template, Working Memory, Episodic Memory, Semantic Memory, Tool Invocation, Tool Result, Output Publishing, Error Recovery, Quality Check, Confidence Threshold, Human Escalation, PDCA Cycle, Workflow Chain, Agent Loop, Agent State, Agent Versioning, Telemetry & Log |
| Office | `config/office_ontology.json` | office (kind, fresh axis_kind) | 75 across 12 categories: Communication, Finance, Procurement, People, Projects, Customer, Reporting, IT, Facilities, Legal, Marketing, Travel, R&D |

## Config (`PhysisProConfig`)
- `PhysisProConfig` extends the base config with ontology loader fields
- Office ontology loaded as a new file with `kind: "office"` and `axis_kind: "office"`
- `wenyan_enabled=false`, `sanskrit_enabled=false` by default (business mode)
- Token trie built by `scan` is per-invocation only (not persisted)
- Quality feedback persists to `~/.physis/quality_failures.json`

## File Theme Graph

An ontology-driven **file browsing graph** that organizes files by thematic similarity.

### Architecture

```
Ingested files ──→ embedding ──→ cosine similarity ──→ ontology domains
                                                              │
                    ┌─────────────────────────────────────────┘
                    ▼
              FileThemeGraph (JSON persisted)
                    │
            ┌───────┴───────┐
            ▼               ▼
      browse-graph      wiki-export
      (interactive)     (uberwiki pages)
```

### Key Types (`graph.rs`)

| Type | Description |
|------|-------------|
| `ThemeNode` | Ontology domain (name, category, domain×mode, embedding) |
| `FileNode` | Ingested file (path, title, kind, embedding) |
| `ThemeFileEdge` | File-to-theme connection with relevance score |
| `FileThemeGraph` | Full graph — themes, files, edges + query methods |

### CLI Commands

| Command | Description |
|---------|-------------|
| `physis-pro ingest` | Scan ingest dirs, classify files against 370+ ontology domains, build graph |
| `physis-pro browse-graph` | Interactive browser — categories → themes → files → related themes |
| `physis-pro wiki <query>` | Search llmwiki + file-theme graph in one query |
| `physis-pro wiki-export` | Export graph as uberwiki markdown pages (per-category + per-file) |
| `physis-pro rebuild-graph` | Rebuild graph from scratch |

### Interactive Browsing Flow

```
Categories (Finance, Communication, ...)
  └── Themes (Budget Planning, Invoice, ...)
        ├── Linked Files (budget_report.txt, ...)
        └── Related Themes (85% similar → Quarterly Planning)
```

### Wiki Integration

- `physis-pro wiki "budget"` searches both llmwiki text AND the file-theme graph
- `physis-pro wiki-export` generates:
  - `~/.physis/wiki/index.md` — master index
  - `~/.physis/wiki/{category}.md` — per-category pages with themes + linked files
  - `~/.physis/wiki/files/{file}.md` — per-file detail pages
  - `~/.physis/uberwiki_index.md` — uberwiki-compatible master index
- Wiki pages use `[[wikilink]]` syntax for cross-references

### Graph Module (`graph.rs`)

| Method | Description |
|--------|-------------|
| `add_theme(name, category, domain, mode, axis_kind, embedding)` | Add ontology theme node |
| `add_file(path, title, kind, embedding)` | Add file node |
| `add_edge(theme_id, file_id, score)` | Connect file to theme |
| `list_categories()` | Get all category names |
| `themes_by_category(cat)` | Get themes in a category |
| `files_for_theme(id)` | Get files linked to a theme |
| `find_related_themes(id, threshold)` | Find similar themes by embedding similarity |
| `search_files(query)` | Find files by name/path |
| `search_themes_by_text(query)` | Find themes by name/category |
| `export_to_wiki(dir)` | Export full graph as uberwiki markdown pages |
| `save(path)` / `load(path)` | JSON persistence |

### Wiki Module (`wiki.rs`)

| Function | Description |
|----------|-------------|
| `search_all(query, wiki_path, graph_path)` | Combined llmwiki + graph search |
| `search_wiki(query, wiki_path)` | Search llmwiki index |
| `search_graph(query, graph)` | Search FileThemeGraph for matching themes/files |

## Study Log (`study.rs`)

| Type / fn | Description |
|-----------|-------------|
| `StudyEvent` | One study action: `observed_at` (date/time), `action` (generated/edited/opened/deleted), `path`, `place`, `file_type` |
| `diff_snapshots` / `snapshot_dir` | Pure recorder logic: mtime+size diff → actions (new→generated, changed→edited, gone→deleted) |
| `session(events)` | Aggregate into `StudySession`: `OperationalTimeline` (order + incident), tallies by type/dir/action/place, chronos rhythms |
| `to_op_event` | Projects onto `OperationalEvent` (provenance connector `study`) so the timeline pipeline is reused; never mixes with shop-floor |
| CLI | `physis-pro study <dir> --place <where>` (poll recorder, `--once` = opened) |
| Web | `POST /api/v1/study/log` · `GET /api/v1/study/session` · `POST /api/v1/study/clear` · page at `/study` (Study Log dashboard + Elenchus argument trainer), page `src/bin/ui/study.html` |
