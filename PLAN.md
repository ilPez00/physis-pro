# Physis Pro — Master Plan

## Overview

Physis Pro is an **industrial backoffice intelligence engine** that combines
ontological reasoning, semantic classification, PDCA cycles, and dream-driven
countermeasure generation. It ingests machine data, documents, and personal
history; classifies them on a 70-cell semiotic grid; and closes the loop from
anomaly detection to corrective action.

**Active vertical:** Industrial backoffice intelligence (Lean / Industry 4.0).
**Monetization:** Tiered licensing (Core / Industrial SaaS / Enterprise On-Prem).

---

## Current State (2026-08-18)

| Layer | Status |
|-------|--------|
| Engine | Holarchic runtime with real-time graph processing, semantic classification, temporal scheduling |
| Industrial | OEE tracking, maintenance baselines, anomaly detection, Italian shift reports |
| Document | Ingest, classify, duplicate detection, admin review queue |
| Personal history | Vault import (Obsidian/git log), browser history, OPML, chat JSONL, praxis backfill |
| Study | Study log with rhythms, Elenchus argument trainer |
| UI | Pulse (vitals), Search (by content), Atlas (organic graph + object explorer), Studio (ontology editor), Files (graph file manager), Console (multi-tab dashboard), Agent, Demo, Study — one shared shell, no external assets |
| Licensing | Offline license enforcement with vendor-signed envelope |
| Multi-tenancy | Per-user tenant state in sled, `X-Physis-User` header scoping |

---

## Roadmap

### Phase 1 — Technical Perfection (In Progress)

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 1.1 | Scanner overhaul: parallel WalkDir + streaming SHA256 | ✅ Done | — | rayon + 64KB buffer |
| 1.2 | Pipeline dedup: `run_ingest_pipeline()` | ✅ Done | — | 160 lines extracted |
| 1.3 | Disk indexer (`physis-pro index`) | ✅ Done | `src/indexer.rs`: stat fast path (re-hash only changed), sled catalog, `--watch`/`--interval`; e2e verified |
| 1.4 | Critical unwrap elimination | ✅ Done | — | `unwrap_or_default()` in VFS |
| 1.5 | CI/CD: cargo check + clippy + test | ✅ Done | — | 3 feature sets |
| 1.6 | Inventory command (sled DB) | ✅ Done | — | + SMA-30 forecast |
| 1.7 | Process command (TCP checks) | ✅ Done | — | Behind `industrial` feature |
| 1.8 | Real-time MQTT/Modbus adapters | ✅ Done | — | `src/industrial.rs`: `MachineAdapter` trait gains `publish()`; MQTT subscribe (rumqttc) + `publish` to `physis/<id>/cmd`; new `modbus::ModbusTcpAdapter` (tokio-modbus 0.17) with `ModbusMap` register decoding + `publish` writes coils/registers; `AdapterRunOptions` + `run_adapter`/`run_loop` driver emits normalized `OperationalEvent` JSON per poll, dedupes by state fingerprint, flags anomalies via `detect_anomalies`, optionally registers into an engine (`post_event`); CLI `physis-pro connect adapter --protocol mqtt\|modbus --endpoint … [--post-to … --temp-max … --once]`; 3 new tests (ModbusCommand parse, ModbusMap defaults, mock-loop `once`) |
| 1.9 | BGE-base embedder + self-test gate | ✅ Done | — | 768-d, MiniLM→RP fallback |
| 1.10 | Automated ontology discovery | ✅ Done | — | `discover` + `--ai` legacy path |
| 1.11 | Config dedup (`BUILTIN_ONTOLOGIES` table) | ✅ Done | — | 10 fns → 1 lookup |
| 1.12 | GPU support investigation | ✅ Done | — | ROCm dead, MIGraphX/WebGPU deferred |
| 1.13 | Goals carry text + persist | ✅ Done | — | `label` field + sled per tenant |
| 1.14 | HTTP router test suite | ✅ Done | — | 11 in-process tests via oneshot |
| 1.15 | Live product demo (`/demo`) | ✅ Done | — | Seed+watch+classify, 11 HTTP tests |
| 1.16 | Study Log + Elenchus trainer (`/study`) | ✅ Done | — | 12 HTTP tests, per-tenant isolation |
| 1.17 | TUI wiki browser (WIKI tab) | ✅ Done | — | Meaning-ranked, graph walk, heatmap |
| 1.18 | Semiotic node editing (exchange parts) | ✅ Done | — | `POST /coherence/edit` + `/delete` |
| 1.19 | Fixed-token node search | ✅ Done | — | `POST /search/nodes`, budget-bounded |
| 1.20 | Watch-mode auto-ingest | ✅ Done | — | Background thread + `POST /watch` |
| 1.21 | Vault importers (Obsidian, git log) | ✅ Done | — | `src/vault.rs` |
| 1.22 | Personal-history importers | ✅ Done | — | `src/history.rs` |
| 1.23 | Praxis backfill | ✅ Done | — | `src/praxis.rs` |
| 1.24 | Spanner Graph sink | ✅ Done | — | `--features spanner`, off hot path |

### Phase 2 — Interface Mimicking Nature (Done 2026-08-24)

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 2.1 | `NodeView` + `observable()` exhaustive destructure | ✅ Done | — | `models.rs:244`; no `..` in destructure — new fields force a decision |
| 2.2 | Pins persist across edits; clearing is explicit | ✅ Done | — | Web `"cell": null` → `PinEdit::Clear` (tested); CLI `node-edit --clear-pin` |
| 2.3 | Pin disagreements feed `QualityTracker` automatically | ✅ Done | — | `web/coherence.rs`: dissent penalizes the auto cell, pins reinforce |
| 2.4 | Single projection for all surfaces | ✅ Done | — | Web module dropped its own DTO; TUI parses NodeView subsets |

### Phase 3 — User Experience (Backoffice Polish)

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 3.1 | Web-based Dashboard (React/Next.js) | ✅ Done (superseded) | — | React/Next framing dropped for the no-build Bioluminal Forge shell; the substance ships: `/api/v1/fs/graph` FileThemeGraph over HTTP + force-directed (cose) theme↔file visualization on the console File-map tab and `/ui`, with file preview, moves, similarity edges |
| 3.2 | Natural Language Interface (chat) | ✅ Done | — | `POST /api/v1/ask`: plain-English question → embedder-consistent retrieval over coherence+goal labels (token-packed) → grounded synthesis via the provider cascade with [n] citations, 300-token cap. Degrades gracefully: no provider / no hits / synthesis failure each return the raw ranked matches + an explicit note, never an error. Ask bar on every page renders the answer above the matches it is grounded in; pure `ask_prompt()` unit-tested (citation contract) + http_surface degradation test |
| 3.3 | Quality Dashboard UI | ✅ Done | — | `GET /api/v1/quality` enriched: per-failure records (embeddings stripped, `DOMAIN/MODE` display cells, newest first) + open/resolved/mean-severity aggregates; pure `quality_response()` builder, 4 unit tests + 2 http_surface tests. Console Quality tab (`#tab-quality`, unified Bioluminal Forge shell): KPI tiles, zero-filled 30-day trend bars, recent reports with severity/resolution/correction pills, penalty & boost bar lists |

### Phase 4 — Monetization

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 4.1 | Tiered licensing (Core / Industrial / Enterprise) | ✅ Done | Tier enum derived from the signed feature list (no separate field → no forged upgrades); banner shows tier; vendor tool `--tier core\|industrial\|enterprise` expands bundles; 9 tests incl. cross-tier denial (56341a8); offline enforcement was already done |
| 4.2 | Industry-specific ontology packs | ⬜ Pending | — | Aerospace, Food Processing, … |
| 4.3 | ROI-based pricing | ⬜ Pending | — | Quality failures prevented / inventory optimized |

### Phase 5 — Industrial Wedge

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 5.1 | CSV/JSON shift importer | ✅ Done | — | `src/shifts.rs`, partial acceptance w/ line-level errors |
| 5.2 | Customer-ready report metadata + evidence export | ✅ Done | — | `ReportMetadata` in md/html/json; `/api/v1/shifts/export` evidence bundle |
| 5.3 | Durable tenant/plant-scoped shift history | ✅ Done | — | sled per tenant, `/api/v1/shifts/*` + aggregated report |
| 5.4 | First paid pilot + protocol decision | ⬜ Pending | — | Before adding more adapters |

### Phase 5B — Interest Engine (Personal Vertical)

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 5B.1 | Interest core: observations, decayed profile, sled store | ✅ Done | `src/interest.rs`, 7 tests |
| 5B.2 | Document analyzer: pptx/docx text+metadata, plaintext | ✅ Done | OOXML behind `daemon` feature |
| 5B.3 | Daemon binary: clipboard + watched dirs → loopback API | ✅ Done | `physis-interest-daemon`, port 19878 |
| 5B.4 | Semantic embedder in daemon (BGE via ONNX) | ✅ Done | cascade + self-test + seed cache; `--features daemon,embed-onnx` |
| 5B.5 | Console tab for the interest timeline | ✅ Done | console.html `#tab-interest`: daemon connect, decayed profile, recent events, offline help |

### Phase 8 — QA-Found Defects (2026-08-24 full-feature audit)

> Source: `docs/QA_VERDICT_2026-08-24.md`. Fix order = severity order.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 8.1 | VFS re-ingest crash: dangling-symlink EEXIST (`orchestrator.rs:44` `.exists()` follows links) | ✅ Done | `symlink_metadata()` + stale-link repair/prune; 3 vfs tests incl. rename regression |
| 8.2 | `query` CLI finds nothing: trie process-local (`cli.rs run_query`) | ✅ Done | `run_query_nodes` recalls persisted coherence labels from sled; trie hits still shown first |
| 8.3 | RP-embedder score compression (0.88–1.00 flat, wrong top cells) | ✅ Done | BGE weights validated: CORE top-1 domain 0.891, HARD 1.000; floors recalibrated (2026-08-24 provenance); RP limits documented in README + CLI warning |
| 8.4 | `order` command stub — persists nothing (`main.rs:904`) | ✅ Done | `place_manual_order` w/ cooldown + sled order log; `inventory` shows history |
| 8.5 | `\u0000` literal in `quality` output cell names | ✅ Done | physis-core summary renders `DOMAIN/MODE`; regression test |
| 8.6 | rachmaninov WARN spam every 10s at sim=0.0000 on empty holarchy | ✅ Done | Zero-signal skip + ≥3-node threshold + existing rate-limit; 2 tests |
| 8.7 | timeline/machine input schemas undiscoverable | ✅ Done | `machine <name> --schema` (executable example, pre-licence); timeline long_about |
| 8.8 | Cell-level classify precision regressed: 0.636 vs 0.923 (2026-06-12 calibration) | ✅ Done | Root cause: smoothing diluted anchors post-expansion. Sweep (bge-base) → shipped max scoring: CORE cell 0.636→0.782, Italian 0.667→0.867 dom; `TopKStrategy` seam kept |


### Phase 9 — Repowise Pattern Adaptation (2026-08-24)

> Source: `docs/reference/repowise-adaptation.md`. Clean-room pattern
> re-implementations in Rust — repowise is AGPL, physis-pro is proprietary;
> **no code transfer, patterns only**.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 9.1 | Session-transcript adapter registry (`src/sessions/`) | ✅ Done | `discover` + per-line `normalize` → None contract; claude_code/codex adapters; substring prefilter; 11 tests incl. real-transcript ground truth (`c7db9f6`) |
| 9.2 | Anti-hallucination grounding gate (`src/grounding.rs`) | ✅ Done | apply_gate(): ungrounded fields dropped, zero-evidence candidates rejected, Grounded/Partial verdict; 7 tests (5cf0893) |
| 9.3 | Decision mining → embedding dedup | ✅ Done | 7dcf91a core + 53d89b0 embedding dedup (dedup_by_embedding, intra-batch collapse, broken-vector guard) + decisions-mine CLI; 10 tests |
| 9.4 | Holarchy community detection (`label-propagation`, no new deps) | ✅ Done | core: deterministic label-prop + cohesion + read-only invariant, 6 tests (afc2a29); 9.4b: GET /api/v1/communities + Communities studio tab, live-verified on :19877 (5b5d4a7) |
| 9.5 | Wiki generation over the holarchy (`wiki-gen` feature) | ✅ Done | outline core: group_nodes + name_groups (9e86c27); 9.5b prose + tour (b047601): `render_page` deterministic skeleton + LLM summary injection, `execution_tour` Kahn order w/ cycle-safe fallback; 12 tests total |
| 9.6 | HTTP surface for wiki-gen | ✅ Done | POST `/api/v1/wiki/generate` (outline+pages+tour, cascade naming skipped when degraded/deterministic) + GET `/api/v1/wiki/tour`; empty-tenant explicit error; http_surface coverage test (d9b72a6) |
| 9.7 | Overview dashboard (`/pulse`) + main-server communities endpoint | ✅ Done | `GET /api/v1/communities` (`web/coherence.rs`: label-space rollup — resolved_edges ids mapped to labels before detection, edge-less labeled nodes kept as one singleton community, largest-first; pure `communities_payload` + 3 tests) + `/pulse` page (`src/bin/ui/pulse.html`, Bioluminal Forge + ui-shell): KPI ribbon, coherence distribution, dream-replay attention feed, dream proposals, communities grid, wiki execution tour, semiotic heatmap — repowise dashboard patterns per adaptation doc, thin read-only UI over existing endpoints; live-verified on :19899 |

### Phase 10 — Datawork: Space · Time · Cause (2026-08-26)

> Source: `docs/plans/2026-08-26-space-time-cause-datawork.md`. Filing axes the
> meaning grid cannot carry — where/when/why-made/what-for — as one
> `FileAddress` per file plus a causal ledger and two new ontologies.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| D1 | `FileAddress` on FileNode + scan/watch capture | ✅ Done | `src/address.rs`: SpaceRef/TimeRef/CauseRef + fs-metadata derivation (degrades to `now`, never fails ingestion); `FileNode.address` serde-defaulted so old graph JSON still loads; captured in `build_theme_graph`, demo CLI graph builder, `/api/v1/fs/graph` DTO (`place/period/modified/mechanism` fields); 4 unit tests + save/load round-trip incl. legacy-node coexistence |
| D2 | Space + time ontologies (`place`, `period`) | ✅ Done | `config/space_ontology.json` (22 places), `config/time_ontology.json` (16 periods); dedicated `space_domains`/`time_domains` buckets — deliberately NOT classification themes (never join classifier centroids); facet CLI parity via `--kind place\|period`; grid-cell validation tests |
| D3 | Causal ledger (sled, per-tenant) → OperationalEvent | ✅ Done | `src/ledger.rs`: CauseEvent (monotonic seq) + lineage/descendants/purposes walks; projects onto OperationalEvent pipeline under connector `cause` with replay-stable event_keys; sled rows keyed `ledger:{uid}:{seq}` (append-only, idempotent save); RuntimeState field, lazy tenant load + default-tenant startup load; watch thread records Generated/Corrected per file diff; 5 unit tests |
| D4 | Vault/history/praxis importers write addresses | ✅ Done | causal events per import run (not per doc): vault→`received(vault-import)` + `derived(git-log ×N)`, history→`received(bookmarks\|browser-history\|feeds\|chat-log)` by export type, praxis→`derived(praxis-backfill)`; /scan records `generated(scan)`; all persist through the same idempotent sled save |
| D5 | `where` CLI + `/api/v1/files/query` + /ui panels | ✅ Done | tenant-scoped POST endpoint composing space (`place` prefix) · time (`within_days`/`during YYYY-MM`) · cause (`mechanism`\|`actor`) · free-text facets, newest-first matches + by_place/by_period/by_mechanism rollups; `physis-pro where <query> [--place] [--cause] [--within N] [--during]` reads the persisted `_default` ledger serverless with ASCII rollup bars; console Timeline tab gains the Cause-ledger card (rollup pills + recent events, fetch-once + refresh); e2e test covers recording, all four facets, and tenant isolation |
| D6 | Purpose links (`serves` events) file↔goal | ✅ Done | `POST /api/v1/files/serve` validates the goal per tenant and records `serves(planner)` as final cause — ledger history, no parallel truth; `GET /api/v1/files/lineage?path=` returns upstream chain + descendants + purposes (Serves excluded from chains by construction); `GET /api/v1/goals/list` feeds the picker; console Timeline card gains the serve form + click-to-trace lineage; e2e: attach → facet query → lineage → 404 on unknown goal → tenant isolation |
| D7 | Mailbox import (email × datawork) | ✅ Done | `src/mails.rs` (feature `email`, now in default features so `.eml` scans parse instead of registering empty shells): walks export dirs — `.eml`/`.msg` per message, RFC-4155 `.mbox` split on sender lines with indexed sources (`file.mbox#3`), garbage chunks skipped; labels `mail:<subject> — <from>` embed subject+body with native Date parsed. `POST /api/v1/mail/import` (scan-root confined) registers nodes + records `received(mailbox)` per source file; e2e covers registration, cause trail via facet query, confinement |

**Phase 10 complete (D1–D7).**

### Phase 10b — Datawork surfaces (2026-08-26)

> Follow-ups found while landing D1–D7. All buildable now; no external deps.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| T1 | Timeline pipeline consumes the ledger | ✅ Done | `GET /api/v1/study/session` merges `to_op_events()` into the assembled timeline (chronologically interleaved with study events; connector-tagged via `properties.mechanism`), exposes a `causes` count, tallies/rhythms stay study-scoped; empty-ledger tenants unchanged (back-compat asserted); /study page renders merged view for free + mechanism chip in entry meta; e2e covers interleaving, counts, scoping |
| T2 | /ui place+time filters | ✅ Done | fs/graph already ships `place`/`period` per file (D1) — two facet selects on the graph FM header populated from the scan, combined with text search into one dimming pass, status line shows visible count; selections survive re-scans when still valid |
| T3 | Lineage as edges | ✅ Done | selecting a file node draws its why-made chain into the cose graph: rose round-diamond cause nodes (`mechanism·actor`) chained input→artifact with dotted arrow edges + lime `serves·goal` nodes for purposes; cleared on reselect/re-render, best-effort (never breaks selection) |
| T4 | Ontology pack mechanism (4.2-lite) | ✅ Done | `load_all` scans `{data_dir}/packs/*.json` (sorted, deterministic): entries join `custom_domains` keyed `pack-stem:name`, interpretants attributed `pack:<stem>`, broken packs skipped with a warning — packs are data, no rebuild. Example pack `docs/examples/logistics_pack.json` (9 entries); unit test covers load + skip + classification enrichment |

### Phase 10c — Adoption track (2026-08-26)

> Source: `docs/business/ADOPTION.md`. Make the agent-facing surface match
> what the product can actually do.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| A1 | MCP surface parity | ✅ Done | `src/mcp.rs`: 18 → 23 tools. `remember` (register labeled node + verdict into the shared sled graph), `recall` (token-packed semantic retrieval over nodes+goals, same corpus contract as /search/nodes), `why`/`serve`/`causes` (causal ledger: lineage+purposes, purpose links validated against agent goals, facet browse). Startup opens `{data_dir}/state.sled`, rehydrates default tenant, degrades to session-only memory if sled unavailable |
| A2 | llms.txt served per instance | ✅ Done | repo root → public route `/llms.txt`, capability coverage tested |
| A3 | Registry submission pack | ✅ Done | `docs/business/mcp-server-submission.md`; owner runs topics command (CI token lacks scope) |
| A4 | Crate published | ✅ Done | `physis_pro v0.1.10` live on crates.io |
| A5 | Classification reach fix | ✅ Done | root cause: `classification_domains()` chained only `human+custom`, so machine/agent/engineering/office/semiotic/category/natural/social/abstract entries (≈576) were authored and displayed but never scoreable — industrial text fell back to human cells. All nine domain buckets now chained (seeds 352 → 928); `place`/`period` stay excluded by design; regression test pins OEE/Error-Recovery reachability + filing-axis exclusion; accuracy_floor passes under the real embedder. physis-core's studio copy needs the same treatment in its own release |
| A6 | Output-quality validation | ✅ Done | `tests/output_quality.rs` (ignored probe, real bge embedder, production ONNX cascade) prints live output for classify/recall/ask/ledger/lineage/serve. Surfaced and fixed a second defect: cell exemplar entries returned in insertion order — core 0.1.10 now ranks them by their own similarity. Verified: OEE query fronts [OEE Metrics], agent-failure fronts [Agent Loop/Tool Invocation/Error Recovery], recall top-1 correct with wide margins, ask synthesizes correct `[1]`-cited answers via groq; accuracy_floor re-passes. Shipped as physis-core 0.1.10 + physis_pro 0.1.13 |

### Phase 11 — Corpus comprehension: summaries · structure (2026-08-27)

> Two limits top-k retrieval cannot fix by ranking harder. A question about the
> whole corpus ("what are the recurring themes", "what do we keep failing at")
> has no single node holding the answer, so no similarity ranking can surface
> it — rank *community summaries* instead. And flat chunking throws away the
> heading chain that makes a chunk mean anything — parse the document's own
> structure and make each section carry its ancestors.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| C1 | `corpus` module — community summaries + global packing | ✅ Done | `src/corpus.rs`: `MemberFacts` → `CommunitySummary` (TF-IDF keywords distinctive *across* communities, majority-vote facets with deterministic tie-break, representatives by keyword coverage, win/failure tally, template digest). Default path is offline and model-free, preserving the air-gapped guarantee; `pack_global` mirrors `rag::TokenFixedRetriever` (skip-not-truncate, so half a digest is never cited); `global_prompt` states the unit is a cluster, not a document. 15 unit tests |
| C2 | `doctree` module — hierarchical document index | ✅ Done | `src/doctree.rs`: parses ATX + setext headings (fenced code blocks skipped, so `# comment` in a shell snippet cannot shred the tree), YAML frontmatter skipped, every line lands in exactly one section, headingless documents degrade to one whole-document section. `Section::retrieval_text()` = breadcrumb + body, so the embedder sees the context the author implied but never repeated; `outline`/`max_depth`/`children` for navigation. 19 unit tests |
| C3 | Modules registered + surfaced over HTTP | ✅ Done | both modules were written but never declared in `lib.rs` (dead files, zero coverage). Now registered; `src/web/corpus.rs` adds `GET /api/v1/corpus/summaries` (offline, no provider needed) and `POST /api/v1/corpus/ask` (rank digests through the content-addressed embed cache → `pack_global` → grounded synthesis, degrading to ranked communities when no provider is configured). Facets come from where the graph records them: cell from `cell_pin`, verdict from `asserted`, mechanism from the causal ledger; `place`/`period` live on `FileAddress` and stay absent rather than guessed. Isolated nodes join one singleton community, same convention as the communities rollup, so nothing is invisible to a corpus-level question. 3 unit tests + e2e (coverage invariant, budget bound, citation resolution, tenant isolation) |
| C4 | doctree wired into retrieval (additive) | ✅ Done | `POST /api/v1/rag/retrieve` gains `unit: "chunk"\|"section"`. `"section"` parses each ingested document into its heading tree and embeds `breadcrumb + body` per section; the response gains `unit` plus a `sources` array parallel to `chunks` (file path + section path + heading level), so a hit cites "Spindle Assembly > Reassembly > Bearing preload" instead of an offset. Additive by decision: the default flat path keeps the same corpus, ids and vectors, so no cached embedding is invalidated. e2e pins the breadcrumb inside the embedded text, hit↔source alignment, and flat-path parity |
| C5 | CLI + MCP surface for corpus questions | ✅ Done | `physis-pro themes [--min-size N] [--json]` clusters the `_default` graph into communities and prints each summary (offline, lib `corpus::gather_facts`+`build_summaries` reused by the web handler); MCP `physis_themes` tool returns the same summaries (min_size filter) for agents |
| C6 | Console tab for summaries | ✅ Done | `console.html` `#tab-themes` under **Mean**: load-on-activate `GET /api/v1/corpus/summaries`, KPI tiles + one card per community (title, keywords, dominant facets, outcome pills, digest, representatives); refresh button; `.chips`/`.pill` styles added to `physis.css` |

### Phase 12 — Demo-grade GUI: content search · organic graph · ontology studio (2026-08-29)

> Three gaps the dashboards could not close by polish. Every search surface
> ranked **labels** — the names the graph already knows — so the paragraph
> nobody labeled was unreachable. Every graph loaded cytoscape from a CDN, so
> the air-gapped install this engine is sold into drew nothing. And the
> dictionary could be *read* but not *written*, which meant "add your own
> vocabulary" was a filesystem chore, not a product.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 12.1 | `content` module + `POST /api/v1/search/content` | ✅ Done | `src/content.rs`: hybrid retrieval over **doctree sections** — BM25 (k1=1.2, b=0.75, idf never negative) fused with cosine, each channel max-normalized over the candidate set then weighted 0.55 semantic. Two channels because an industrial corpus is half codes and half prose: vectors treat `E4412`/`E4413` as near-identical, terms miss "cuscinetto grippato" for "seized bearing". Quoted phrases are required verbatim; snippet picks the densest window and returns **character** offsets (byte offsets land mid-codepoint on the first accented word). Facets (place/period/kind/file) count the whole match set, not the page. Index is built once and held on `AppCtx`; staleness is a metadata-only walk (size+mtime), so freshness is checked per request while read+embed happens once; embeddings served from the content-addressed `EmbedCache`. Without a semantic model the index is built lexical-only and the response says `semantic_index: false` rather than ranking random projections confidently. Place facet falls back to the folder relative to the scan root — the address's own `place` defaults to the root itself, which would be one value covering everything. 12 unit tests + 2 e2e |
| 12.2 | `/search` — investigation surface | ✅ Done | `src/bin/ui/search.html`: facet rail with proportion bars, ranked sections citing the heading chain, `<mark>`ed snippets, preview pane reading the section back off disk (with outline, and an explicit note when the file changed since indexing), mode segmented control, `j`/`k` navigation, stale-response guard. Status line states what actually ran — requested vs effective mode, index rebuild cost, missing semantic model. Ask bar now offers content search on every page instead of implying an empty corpus |
| 12.3 | Organic renderer + `/atlas` | ✅ Done | `src/bin/ui/organic.js` (served at `/organic.js`, **no CDN**): force simulation on a plain canvas — breathing amplitude from coherence, edge sway from weight, community blobs as radial fields rather than hulls (a hull asserts a boundary the clustering does not have), travelling pulses on recorded edges only. Layout seeded from a hash of the node id, never `Math.random`, so an unchanged graph lands in the same place and "did it change?" stays answerable. `GET /api/v1/atlas/graph` (`src/web/atlas.rs`) joins nodes + edges + communities server-side: recorded holarchy edges plus derived kNN similarity edges, **tagged and never merged**; label-propagation over both layers so register-only tenants are not a starfield; unconnected nodes share one community, matching the communities rollup. 5 unit tests + 1 e2e |
| 12.4 | Ontology studio write path | ✅ Done | `src/web/ontology_studio.rs`: list / read / save / delete packs under `{data_dir}/packs`, names validated (rejected, never sanitized — a `..` join escapes a sandbox and a silent rename loses a customer's pack), cells validated against the 70-cell grid, **all** errors returned at once. Writes `OntologyEntry`, not `DomainDef`: the loaded shape serializes `"axis_kind": null`, which the loader rejects — the studio would have written packs the engine skipped at startup, and the round-trip test caught it. Atomic write. `/studio` page: pack list with live/pending state, 70-cell density picker, hint editor, and a **before/after** panel that classifies real samples through `/api/v1/planner/simulate`. Deliberately no hot-swap of the live classifier (ontology + centroids are immutable in `SharedCtx`, read lock-free on the hot path); the surface says a pack goes live on restart instead of pretending. 5 unit tests + 1 e2e |
| 12.5 | Object explorer (Palantir parity) | ✅ Done | Atlas right pane: entity card (label, cell with auto-vs-pinned provenance, verdict, coherence, links, community, first seen), neighbour list with edge kind + weight, closest-by-meaning recall, causal lineage + purposes for file-backed nodes, and the actions that write back — assert succeeded/failed (idempotent re-register), pin/unpin a cell, attach a goal, hand the label to Ask or to content search |
| 12.8 | CLI honored `PHYSIS_EMBEDDER` | ✅ Done | Found by using 12.7: `physis-pro find` with `PHYSIS_EMBEDDER=random-projection` set still loaded the whole ONNX cascade and spent ~25 minutes embedding the ontology the operator had just asked it to skip — the variable was read by the web server and the interest daemon but never by `select_embedder`. An ignored instruction is worse than an unsupported one: the only symptom was the wait. One `wants_random_projection()` predicate now serves all three call sites (CLI selection, server selection, the server's DEGRADED decision), so the spelling of the request cannot drift between them; unit-tested on both documented spellings |
| 12.7 | Agent + CLI parity for content search | ✅ Done | `physis-pro find <query> [--dir --mode --place --period --kind --top-k]` (serverless: ingests, embeds through the persistent `EmbedCache`, prints citation + facets + snippet) and MCP `physis_find` (same corpus contract, `PHYSIS_SCAN_ROOT`-confined like `physis_pack`). Two honesty fixes fell out of using it: a vectorless index now reports `lexical` whatever was requested — saying "hybrid" would let a caller read paraphrase matching into a pure keyword result — and `citation()` drops the leading crumb that only repeats the filename (`policy.md › policy › Escalation`). The preview endpoint therefore resolves a section by exact path **or unique trailing suffix**, ambiguous suffixes matching nothing rather than guessing between two sections that both end in "Notes". 14 unit tests |
| 12.6 | `--features full` build repair | ✅ Done | Pre-existing break at HEAD: `mcp.rs` called `web::coherence::query_coherence_nodes` through a private module and read a `verdict` field `NodeView` does not have. Module is now `pub(crate)` (both surfaces must project through one function or they answer differently) and the tool returns `rating` + `asserted` |

### Phase 8b — Engine-side read-back wired to the boundary (2026-08-30)

> Closes the open thread flagged in the plan index: `reduce()` existed in Rust
> and no surface could reach it, so the machines' whole point — perceive there,
> mean here — stopped at the boundary.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 8b.1 | `bridge_node` was being handed the label, not the id | ✅ Done | Found while wiring the endpoint. `chronos`, `euclid` and `empedocles` `reduce()` all called `core.bridge_node(&text)`; `bridge_node` looks the argument up in `core.nodes` and returns `None` for anything that is not an id, so **every node these reducers registered stayed unbridged** and the read-back loop silently never closed — the `Option` return made it a no-op with no error. All three now bridge the returned id, with a regression test asserting every registered node has a holon |
| 8b.2 | `machines::run::reduce` — one dispatcher for both surfaces | ✅ Done | `run()` is pure (JSON → JSON, touches nothing); `reduce()` is the other half (registers, bridges, reports). Both live in `run.rs` so HTTP and CLI cannot drift on what a machine *means*. Returns `ReduceOutcome { score, registered, nodes_before, nodes_after }` — the id set is diffed rather than guessed, so a repeat run reporting zero new nodes is legible as dedup rather than as failure. `REDUCERS` names the three machines with an engine-side half; asking any other to write is an **error, not a silent no-op**, because a caller who got `{"score": 0}` back could not tell nothing had been written. 2 unit tests |
| 8b.3 | `POST /api/v1/machines/:name/reduce` | ✅ Done | Tenant-scoped (a machine writes into the caller's graph, never a shared one), persists each new node through sled and **reports** the persisted count and any persist error rather than swallowing it — a reduce that looked like it worked and vanished on restart is the worst of both. Unknown machine → 404, stateless machine → 400 naming the ones that do write. `GET /api/v1/machines` now carries `writes` per machine so a client need not discover it from a 400. e2e covers registration, holarchy bridging via recall, dedup on repeat, tenant isolation, both refusals and the registry flag |
| 8b.4 | `physis-pro machine <name> --reduce` | ✅ Done | Same dispatcher, serverless: loads the persisted `_default` graph, reduces, writes back. The payload is parsed before app init because stdin can only be read once. Verified live: first run registers and persists, second reports "nothing new" with the dedup reason, stateless machines refuse with the same message as HTTP |

### Phase 13 — `/notebook`: user-composed dashboards (proposed)

> Full design: `docs/plans/2026-08-29-notebook-dashboard-editor.md`. **Not
> started** — proposal only, at the user's request.

Every surface today is a composition *we* authored, so giving a maintenance
lead their screen means editing HTML and shipping a binary; that is the wrong
cost for a layout question, and it is why Console keeps growing tabs. The
proposal is an ordered grid of cells the user adds, removes, reorders and
resizes, persisted per tenant in sled and shareable as JSON.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| N1 | Widget registry + server-side notebook validation | ⬜ Proposed | A notebook holds widget **ids and params**, never HTML/JS/URLs — it is inert by construction, because a document meant to be shared would otherwise run requests inside the recipient's authenticated session. Unknown widget → 400 naming it |
| N2 | Per-tenant persistence + CRUD endpoints | ⬜ Proposed | `notebook:{uid}:{id}` in sled; `localStorage` alone cannot be opened on the demo machine or handed to a colleague |
| N3 | `/notebook` page — render an ordered grid | ⬜ Proposed | Shared renderers (`kpis`/`bars`/`list`/`table`/`heat`/`graph`/`feed`); per-widget empty **and** error states, since panels that look identical when empty and when broken are most of the perceived brokenness today; a retired widget renders an explicit card, never a silent gap |
| N4 | Editor: add / remove / reorder / resize / export | ⬜ Proposed | Drag **and** move-up/down buttons — drag-only fails accessibility and fails the shop-floor tablets |
| N5 | "Add to notebook" from Search, Atlas, Console + one scheduler | ⬜ Proposed | Pinned queries (a saved content search as a live cell) are what make this more than rearranging; one page-level scheduler, hidden-tab suppression, no per-widget polling storm |

### Phase 6 — Administration / Datawork

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 6.1 | Fixed demo corpus + Admin Review Desk flow | ✅ Done (core) | demo corpus existed (studio --demo); review_desk.rs: run_desk() exceptions queue (missing fields, low confidence, fingerprint duplicates) + markdown report, 4 tests (ac62f29); HTTP surfacing = 6.1b done (f78c372 POST /api/v1/admin/desk) |
| 6.2 | CSV/XLSX batch import with validation | ✅ Done (CSV core) | batch_csv.rs: per-row validation (missing fields, ISO dates, EU decimal amounts), status/error codes, reviewed export round-trip; 4 tests (a05267a); HTTP surfacing done (f78c372 POST /api/v1/admin/batch); XLSX via existing calamine = remainder of 6.2b |
| 6.3 | Invoice/PO/delivery-note reconciliation | ✅ Done (core) | reconcile.rs: 3-way match keyed on vendor+PO, qty/amount tolerance ±2%/€0.05, duplicate-invoice flagging, multi-note delivery accumulation, markdown report; 10 tests (1965427); no payment automation per plan |
| 6.4 | Weekly management-pack templates | ✅ Done (core) | management_pack.rs: render_weekly_pack() — headline metrics w/ provenance, exceptions table, reconciliation status + actionable detail, notes; deterministic output; 3 tests (348b7b5); HTTP surfacing done (f78c372 POST /api/v1/admin/pack); HTML variant = 6.4b if needed |

### Phase 7 — Decision Planner

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 7.1 | Version ontology/overlay scenarios | ✅ Done | — | `/planner/simulate` compares candidate ontologies without mutating live state |
| 7.2 | Coverage gaps + affected reports/workflows | ✅ Done | `ImpactAnalysis::derive`: newly covered/uncovered, per-cell deltas (affected aggregates), mean confidence delta; stored on every version |
| 7.3 | Dated signifier shifts as reviewable hypotheses | ✅ Done | — | `web/scenarios.rs`: changed items saved as dated `hypothesis:true` shifts |
| 7.4 | Scenario review + promotion + version history | ✅ Done | — | save/list/promote per tenant; reversible; console planner tab renders versions + coverage ± with promote buttons |

---

## Key Decisions

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-18 | `data_dir` defaults to `./.physis` (CWD) | Portable across dev/prod |
| 2026-08-18 | Loopback auth requires `PHYSIS_DEV=1` | Prevent accidental exposure |
| 2026-08-13 | ROCm feature deleted | ONNX Runtime removed ROCm EP upstream; dead config |
| 2026-08-13 | BGE-base stays production | BGE-M3 lost on CORE accuracy, 2.5× slower |
| 2026-06-12 | Active vertical = industrial backoffice | Only vertical with a named prospect |
| 2026-06-12 | Edge adapters wait for customer | No adapter guessing; customer names machines |

---

## Key Metrics

| Area | Metric | Target |
|------|--------|--------|
| Classification (text) | Top-1 accuracy on curated test set | >0.85 |
| Classification (image) | Top-1 zero-shot on held-out photos | >0.80 |
| Graph throughput | Node mutations/sec | >10,000 |
| Dream quality | Human eval acceptance rate | >0.6 |
| Sync latency | P95 MCP broadcast to N instances | <500ms |
| Embedding latency | P99 ONNX embed (text) | <50ms |
| Embedding latency | P99 CLIP image embed | <200ms |
| Embedding latency | P99 Jina embed (text) | <100ms |
| Ontology density | Entries per cell (mean) | >25 |
| Content recall (TUI) | Forgotten-item found in top-5 | >0.85 |
| Recall latency | Keystroke → re-ranked results | <150ms |

---

## Plan Index & Reconciled Status

Single entry point for *what* each planning document covers and *where it
stands. Detail docs are not duplicated here — they are linked. Status is
reconciled against the live code (verified 2026-08-28).

| Plan document | Area | Status | Notes |
|---------------|------|--------|-------|
| `PLAN.md` (this file) | Master roadmap | Living | Phase tables 1–7 + 10/11; authoritative status for shipped work |
| `docs/plans/2026-08-29-notebook-dashboard-editor.md` | Phase 13 — `/notebook` composable dashboards | 📝 Proposed | Design only, nothing built; registry-as-references, per-tenant persistence, pinned queries |
| `docs/plans/2026-08-26-space-time-cause-datawork.md` | Phase 10 — Space·Time·Cause | ✅ Shipped | D1–D6 (+ D7 mail) built; see `AGENTS.md` Space·Time·Cause section. End-to-end verify on next pass |
| `docs/plans/2026-07-15-phase8-engine-readback.md` | Phase 8 — engine-side reduce | ✅ Done | Tasks 1–6 done (2026-07-16); the residual boundary wiring shipped 2026-08-30 as Phase 8b — `POST /api/v1/machines/:name/reduce` + `machine --reduce`, and the bridge-the-id fix that made the loop actually close. `cargo test --lib machines::` = 56 pass |
| `docs/plans/2026-07-16-ecosystem-next.md` | Ecosystem rescue + Praxis/Agora | ✅ Done (2026-07-16) | A–E complete; Python `Machine` base verified pure-perception; Praxis→physis recall wired; Agora ToS pivot documented |
| `docs/archive/DEVELOPMENT_PLAN.md` | Semiotic editing / NodeView / pins | 📚 Archive | historical |
| `docs/archive/DEVPLAN.md` | Industrial MVP / admin / decision planner | 📚 Archive | historical |
| `docs/archive/LEAN_I40_PLAN.md` | OEE / SPC / A3 / JIT | 📚 Archive | historical |
| `docs/archive/INDUSTRIAL_VERTICAL_PLAN.md` | BGE-M3 eval / MQTT / shift report | 📚 Archive | historical |
| `docs/archive/ONTOLOGY_EXPANSION_PLAN.md` | 70-cell grid / holonic graph | 📚 Archive | historical |

### Open threads (reconciled)
- ~~**Phase 8 boundary wiring**~~ — **closed 2026-08-30** (Phase 8b above).
  Both surfaces now reach the engine-side `reduce()`. Wiring it surfaced a
  defect that had made the loop a no-op even in Rust: all three reducers
  bridged the node's *label* instead of its id, so nothing they registered ever
  entered the holarchy.
- **Space·Time·Cause verification** — D1–D6 built per `AGENTS.md`; an
  end-to-end check (importer addresses + `where` + purpose links) is the
  remaining confidence step.

---

## Architecture Constraints

> **Interface mimicking nature:** what is shown must be derived from state by
> construction, never assembled alongside it. An organism has no reporting
> layer — wilting *is* turgor, not a readout of it.

> **Embedder consistency:** nodes must be embedded with the **same** embedder
> that embeds queries, or cosine recall is meaningless.

> **Tenant isolation:** tenant-scoped handlers use `AppCtx::with_user(uid, |rt| …)`.
> Shared immutable infra stays global in `SharedCtx`.

---

## Milestone Dependencies

```
Phase 1 ──┬──> Phase 2 ──> Phase 3
            │        │
            │        └──> Phase 7 (Decision Planner)
            └──> Phase 4 ──> Phase 5
                               │
                               ▼
                           Phase 6 (Admin/Datawork)
```

Phase 6 is mostly independent of Phases 1–5. Phase 7 depends on classify/reconstruct/holarchy endpoints.
