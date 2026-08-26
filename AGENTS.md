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
| Watch mode | `network.rs` + `web.rs` + `cli.rs` | **Auto-ingest**: `PHYSIS_WATCH_DIR` background thread, `POST /api/v1/watch` on-demand sweep, CLI `physis-pro watch [--once]` — register new/changed files as labeled core nodes (shared embedder), drop nodes for removed files. Shared `FileInfo::labels()` + `PhysisCore::delete_nodes_by_label_prefix` |
| Vault import | `vault.rs` + `web.rs` + `cli.rs` | **Vault importers (Phase 7B)**: Obsidian/markdown, plain notes, `git log` → `VaultDoc` (title from H1/frontmatter, `h1:`…`h3:` headings) → labeled core nodes. CLI `physis-pro vault <dir> [--git N]`, web `POST /api/v1/vault/import` (scan-root confined, dedupe by label); `parse_git_log` tested without a git binary |
| Personal history | `history.rs` + `web.rs` + `cli.rs` | **Personal-history importers (Phase 7B)**: browser bookmarks (Netscape HTML), browser history (JSON), OPML feeds, chat JSONL → labeled core nodes. CLI `physis-pro history <file>`, web `POST /api/v1/history/import` (scan-root confined, dedupe by label) |
| Praxis backfill | `praxis.rs` + `web.rs` + `cli.rs` | **Praxis backfill (Phase 7B)**: notebook entries + trackers → `BehaviourRecord`s with Success/Inert/Failure verdicts (mood / value-target thresholds) → labeled behavioural nodes with **asserted** verdicts. CLI `physis-pro praxis <file>`, web `POST /api/v1/praxis/backfill` |
| Spanner sink | `spanner.rs` + `web.rs` + `cli.rs` | **Spanner Graph sink (Phase 7C)**: durable-memory mirror (`--features spanner`, off the hot path). Web `POST /api/v1/spanner/sync\|hydrate`, CLI `physis-pro spanner sync\|hydrate`; JWT service-account auth + emulator round-trip test |
| Facet query | `physis-core/models.rs` + `cli.rs` + `main.rs` | **Ontology facet CLI**: `physis-pro facet [--lifecycle OPERATE] [--agency SELF] [--scale …] [--abstraction …] [--sub-domain X] [--sub-mode X] [--kind X] [--domain X] [--mode X]` — filter ontology entries by orthogonal facets; `Facets::simple()` display |
| Docs | `docs/` | `SEMIOTIC_EDITING.md`, `UMBRELLAPEDIA_GUIDE.md` (wiki surface wiring), `PIPELINE_DIAGRAMS.md` (ingestion→recall pipeline), `PUBLIC_ARCHITECTURE.md` (cross-repo deploy), `AGENT_DEMO.md` (agent loop traces: SSE inside harness + direct library call) |

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

## Space·Time·Cause Datawork (Phase 10, D1–D3)

| Piece | Description |
|-------|-------------|
| Address | `src/address.rs` — `FileAddress { space: SpaceRef, time: TimeRef, cause: CauseRef }` derived from fs metadata at ingestion (`Mechanism`: generated/received/derived/corrected/cited/serves, serde snake_case); period = `YYYY-MM` month bucket. `FileNode.address` is `Option`, serde-defaulted → old graph JSON loads |
| Ontologies | `config/space_ontology.json` (kind `place`, 22 entries) + `config/time_ontology.json` (kind `period`, 16) → dedicated `OntologyLoader::space_domains`/`time_domains`; NOT classification themes; facet CLI: `physis-pro facet --kind place\|period` |
| Ledger | `src/ledger.rs` — `CausalLedger` per-tenant on `RuntimeState`; append-only `CauseEvent`s; `lineage()`/`descendants()` walk input chains (Serves links excluded); `purposes()` = final cause; `to_op_events()` projects onto OperationalEvent (connector `cause`, replay-stable keys); sled rows `ledger:{uid}:{seq}`, idempotent save; watch thread records Generated/Corrected per file |
| Capture points | /scan (`generated(scan)`) + watch (`generated`/`corrected` per diff, actor `watch`) · vault import → `received(vault-import)` + `derived(git-log ×N)` · history import → `received(bookmarks\|browser-history\|feeds\|chat-log)` by export type · praxis backfill → `derived(praxis-backfill)` · mail import (D7) → `received(mailbox)` per source file — one event per run, all persisted idempotently |
| Mail | D7: `src/mails.rs` — `scan_mail_dir()` walks `.eml`/`.msg` + RFC-4155 `.mbox` (indexed sources, garbage skipped); labels `mail:<subject> — <from>`, native Date header parsed. `POST /api/v1/mail/import` (scan-root confined dir). `email` feature now in default features |
| Queries | `POST /api/v1/files/query` (`web/ingest.rs::files_query_handler`, tenant-scoped): facets `query`/`place`/`cause`(mechanism or actor)/`within_days`/`during YYYY-MM` → newest-first matches + `by_place`/`by_period`/`by_mechanism` rollups. CLI: `physis-pro where <q> [--place --cause --within N --during P]` reads the `_default` ledger straight from sled. UI: console Timeline tab Cause-ledger card |
| Purpose links | D6: `POST /api/v1/files/serve` `{object, goal_id}` → `serves(planner)` event (goal validated per tenant, 404 otherwise); `GET /api/v1/files/lineage?path=` → lineage/descendants/purposes; `GET /api/v1/goals/list` for the picker; UI: serve form + click-to-trace on the Cause-ledger card |

## Ask — NL graph queries (PLAN 3.2)

| Piece | Description |
|-------|-------------|
| Endpoint | `POST /api/v1/ask` (`web/search.rs::ask_handler`) — question → `retrieve_nodes()` (shared with `/api/v1/search/nodes`: embedder-consistent cosine over coherence+goal labels, token-packed) → grounded synthesis via cascade (system prompt demands `[n]` citations, 300-token cap). Never errors for lack of provider/hits: returns raw matches + `note`. Response: `question/semantic/answer/provider/note/budget/total_tokens/truncated/results` |
| UI | ui-shell ask bar calls `/api/v1/ask`; answer renders above the matches it cites (`.ask-answer` in physis.css), degradation notes inline; Agent handoff unchanged |
| Tests | pure `ask_prompt()` citation-contract tests + http_surface degradation test + shell-wiring assertion |

## Output-Quality Probe (Phase 10c A6)

| Piece | Description |
|-------|-------------|
| Probe | `tests/output_quality.rs` — `#[ignore]`d like accuracy_floor; run `cargo test --features full --test output_quality -- --ignored --nocapture`. Prints live classify (cells + similarity-ranked exemplars), recall rankings over planted facts, `/ask` synthesis, ledger facets, lineage/purpose chains under the production ONNX cascade |
| Guarantees | classification seeds = all nine domain buckets (928); cell exemplars ranked by per-entry cosine (core 0.1.10); recall top-1 correct on the probe set; ask degrades to matches+note without a provider and cites `[n]` with one |

## Quality Dashboard (PLAN 3.3)

| Piece | Description |
|-------|-------------|
| Endpoint | `GET /api/v1/quality` (`web/coherence.rs::quality_handler`) — enriched: `records` (per-failure views, newest first, embeddings stripped, feedback truncated to 160 chars, cells as `DOMAIN/MODE` display form), `open`/`resolved` counts, `mean_severity`; legacy `penalties`/`boosts`/`failures` unchanged. Pure builder `quality_response()` unit-tested (4 tests: empty ledger, ordering+aggregates, no vectors/NULs over the wire, truncation) |
| UI | Console tab `#tab-quality` ("Quality — feedback loop", nav under *Mean*): KPI tiles, zero-filled 30-day trend bars anchored on the newest report, recent reports with severity/resolved/correction pills, top-10 penalty & decayed-boost bar lists; loader `loadQuality()` fetch-once + manual refresh |

## Pulse Dashboard (`/pulse`)

Overview dashboard (PLAN 9.7) — repowise dashboard *patterns* (adaptation doc:
StatRibbon / attention panel / community grid / execution tour / heatmap) as a
thin read-only page over existing endpoints. No new data writes except dream
replay (archives what it revisits).

| Piece | Description |
|-------|-------------|
| Page | `src/bin/ui/pulse.html` — loads shared `/physis.css` + `ui-shell.js`; handler `pulse_ui_handler`, route `GET /pulse` (protected router, like Console/Demo/Study) |
| Panels | KPI ribbon (`coherence/snapshot`), coherence distribution + PDCA line (`pdca/stats`), attention feed (`POST dream/replay`), dream proposals (`dream/generate`), communities grid (`communities`), wiki execution tour (`wiki/tour`), semiotic heatmap (`semiotic/heatmap`) |
| Communities endpoint | `GET /api/v1/communities` (`web/coherence.rs::communities_handler`) — PLAN 9.4b parity with the physis-core studio endpoint, reusing the Phase-9.4 clean-room detector. `resolved_edges()` yields coherence **ids**, so the handler maps ids→labels before detection; edge-less labeled nodes collapse into one singleton community (full cohesion); response sorted largest-first; read-only invariant; pure `communities_payload()` unit-tested |
| Nav | Registered in `PRODUCT_PAGES` (`ui-shell.js`) — first entry, all pages inherit |

Dev note: rebuilding with a custom vendor key needs
`PHYSIS_LICENSE_PUBKEY=<hex> cargo build` (baked at compile time via
`option_env!`); a licence signed by that key then runs the binary
(`PHYSIS_LICENSE_FILE`). See `docs/business/DEPLOY.md`.
