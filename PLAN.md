# Physis Pro — Master Plan

## Overview

Physis Pro is an **industrial backoffice intelligence engine** that combines
ontological reasoning, semantic classification, PDCA cycles, and dream-driven
countermeasure generation. It ingests machine data, documents, and personal
history; classifies them on a 70-cell semiotic grid; and closes the loop from
anomaly detection to corrective action.

**Active vertical:** Industrial backoffice intelligence (Lean / Industry 4.0).
**Monetization:** service-led pilots → Industriale/Enterprise licensing (Pro),
plus MIT Core as the agent-directory distribution funnel (see
`docs/plans/2026-08-30-productization.md`).

---

## Current State (2026-08-18)

| Layer | Status |
|-------|--------|
| Engine | Holarchic runtime with real-time graph processing, semantic classification, temporal scheduling; 5 compilation blockers fixed (quality.rs, web/mod.rs, coherence.rs, main.rs, industrial_benchmark/secom.rs); `cargo check --features web` compiles cleanly; 759/759 lib tests pass |
| Industrial | OEE tracking, maintenance baselines, anomaly detection, Italian shift reports |
| Document | Ingest, classify, duplicate detection, admin review queue |
| Personal history | Vault import (Obsidian/git log), browser history, OPML, chat JSONL, praxis backfill |
| Study | Study log with rhythms, Elenchus argument trainer |
| UI | Pulse (vitals), Search (by content), Atlas (organic graph + object explorer), Studio (ontology editor), Files (graph file manager), Console (multi-tab dashboard), Agent, Demo, Study — one shared shell, no external assets; new customer-facing pages: index.html (landing), customer_dashboard.html (usage/jobs/tier), api_tester.html (endpoint experimentation), pricing.html (4-tier: Free Trial/Starter/Professional/Enterprise), about.html (capabilities/contact) |
| Licensing | Offline license enforcement with vendor-signed envelope; tier derived from signed feature list (no forged upgrades); banner shows tier. **2026-08-30:** per-job billing (Free Trial/Starter/Professional/Enterprise, pay-per-job) is superseded by the service-led offer in `docs/COMMERCIAL_MODEL.md` — pilots → Industriale/Enterprise licensing |
| Multi-tenancy | Per-user tenant state in sled, `X-Physis-User` header scoping; two deployment nodes: omo and pansa (shared PostgreSQL + model volume, unique API tokens per node; `RAILWAY_PG_CONNECTION_URL` or direct PostgreSQL; ONNX weights from S3/Supabase via `MODEL_BASE_URL`) |

---

## Research track — epistemic mechanisms (status 2026-09-08)

Full record: **`research/RESEARCH_STATUS.md`**. Per-experiment reports sit with
their code in `research/perspective-discovery/` (E1) and `research/mapper/`
(E2–E18).

### Settled

| question | answer | key evidence |
|---|---|---|
| Can embedding geometry give stable ontology structure? | **NO** | cross-embedder ARI ≈ 0.10 |
| Was the shipped corpus sound? | **NO — fixed** | 43.3% misfiled; regenerated in 0.1.21 |
| Can the machine *filter* discovery candidates? | **YES, shipped** | `coverage`, Welch *t* = +12.11 |
| Can it *rank* them? | **YES, shipped 0.1.20** | precision@25 0.480 vs 0.132 base |
| **Can any tested signal certify a filing?** | **NO — settled E14–E18** | on **real** misfilings every scorer is at chance: cosine **0.519**, CONSTRAINT 0.536, ANCESTRY 0.534. ANCHORVOC's 0.664 was an entry-length artifact that fails replication |
| Does cross-model **divergence** carry signal? | **NO (E1)** | 0.602 vs 0.733; equals its null once conditioned on the stronger support |
| Can Qwen n-grams be a groundable **map**? | **NO (E2)** | no disproportionate sparse gain; propagation worse than ignoring the map |
| Is it a **data** problem? | **NO (E3/E4)** | 24× the text made every level worse |
| Is the **70-cell grid** the constraint? | **NO (E5)** | 57→14 classes gains +0.014 kappa |
| Is the mode axis weak from missing definition? | **PARTLY (E6/E7)** | population explains it, r = +0.718 with log class size |
| Do ontologies propagate **hypertrophically**? | **YES (E8)** | hypertrophy index **−0.644**; WORK sits +0.003 above random |
| Should the grid be **expanded**? | **NOT YET (E8)** | pre-flight STOPs on sparsity at every level |
| Is a discrete **signifier address** worth formalising in Lean? | **NOT YET (E9)** | best address 0.244 held-out vs geometry's 0.447 |
| Does **downstream viability** beat local correctness? | **NO (E10)** | +0.01 identity for −5.8 depth |
| Can the **anchors** be improved? | **NO (E12/E13)** | three attempts refuted; headroom over a probe built from the cell's own entries is **+0.004** |

### Two things that reversed under scrutiny, kept because the process is the point

**Stage 12's certification refutations were reached by an invalid route (E14).**
Its injection moves an entry to a *uniformly random* cell. Cosine leads by 0.207
under that policy and **trails CONSTRAINT by 0.106** under a plausible near-miss
— the scorer detects *distance moved*, not wrongness. The conclusion survives;
the reasoning did not. `FINAL_REPORT.md` and `RESEARCH_LOG.md` now carry the
correction inline at the sentences it invalidates.

**ANCHORVOC looked like the first real certification signal and was not
(E16→E18).** 0.664 on real misfilings, significant — then entry length alone
scored 0.619, no vocabulary effect survived controlling for it (+0.048, ns), and
it failed to replicate on the audit of a filing the rater had not produced.
Four experiments, each correcting the last, ending where the track stood before
E14 for much better reasons.

### The one constraint everything converges on

**The corpus is too small, and that is measured rather than argued.**

- E5: 22 of 57 cells hold fewer than 5 entries; macro-F1 0.318 against accuracy 0.447.
- E7: a well-populated mode cut to 3 training entries scores **F1 0.06** — the
  F1 = 0.00 modes are empty, not ill-defined. **n ≈ 20 → F1 0.26–0.49.**
- E8: the pre-flight STOPs the *current* grid on sparsity — 35 of 57 classes
  below 6 training entries.
- E10: the mechanism under test fired in under 3% of generations.

**Populate first, expand second.** `research/mapper/EXPANSION_SPEC.md` has the
deficit table, forecast and acceptance test;
`research/mapper/E11_CANDIDATE_MINER.md` has a validated worklist (WordNet,
recall@100 = 0.497 against a 0.086 shuffled control).

### Standing rules, each earned

1. **Treat any positive as artifact until a construction-matched control says
   otherwise.** Seven have died to one.
2. **Report kappa, never accuracy against a majority floor**, on unbalanced
   multi-class problems.
3. **Check what the ground truth is made of.** Synthetic perturbations encode a
   difficulty assumption; E14 showed Stage 12's was the entire signal.
4. **Run the pre-flight** (`research/mapper/preflight.py`) and honour it. It was
   overridden once, by its own author, in E10.
5. **Coverage filters and ranks discovery candidates; it does not verify that a
   candidate is correct.** Sell the filter, not the judgement.

### Open, and what each needs

| question | needs |
|---|---|
| Does ANCHORVOC survive a rater who never sees the anchor hints? | one adjudication pass — a person, not compute |
| Will populating the starved modes raise mode kappa? | the entries themselves |
| Does anything detect a real misfiling? | a mechanism nobody has proposed yet |

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

### Phase 10v — Space·Time·Cause verified end to end (2026-08-30)

> The second open thread from the plan index. D1–D6 each had their own unit or
> handler test; nothing walked the chain a customer actually walks.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 10v.1 | End-to-end walk over a real scan | ✅ Done | One e2e drives scan → `/api/v1/fs/graph` (every file carries `place`, a 7-char `period` bucket, an fs-derived `modified` and `mechanism: generated`) → the ledger (`cause: generated` with rollups) → time facets (this month hits, `1999-01` misses) → goal → `/files/serve` → `/files/lineage` (chain non-empty, purpose present, and `serves` **absent from the chain** — a final cause is not a production step) → tenant isolation. Passed unmodified: the HTTP half of D1–D6 works as documented |
| 10v.2 | CLI `scan` recorded no cause | ✅ Done | The walk's server-less half did **not** work. `POST /api/v1/scan` recorded `generated(scan)`; `physis-pro scan` recorded nothing, so on an air-gapped install `physis-pro where` answered *"the causal ledger is empty — run a scan, watch a directory, or import a vault first"* to a user who had just run a scan — the one instruction the message gives could never work. Now both paths write the same event, keyed on the **canonical** path so `scan .` and `scan /abs/path` are one history rather than two. Extracted as a free `record_scan_cause(db, dir)` so it is testable without building a whole `PhysisApp`; unit test covers the event shape, the canonical object and that a second scan is a second producing act |

### Phase 13 — `/notebook`: user-composed dashboards (2026-08-30)

> Design: `docs/plans/2026-08-29-notebook-dashboard-editor.md`. Shipped
> complete: N1–N5, plus two defects the build itself surfaced (N6, N7).

Every surface today is a composition *we* authored, so giving a maintenance
lead their screen means editing HTML and shipping a binary; that is the wrong
cost for a layout question, and it is why Console keeps growing tabs. The
proposal is an ordered grid of cells the user adds, removes, reorders and
resizes, persisted per tenant in sled and shareable as JSON.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| N1 | Widget registry + server-side notebook validation | ✅ Done | `src/notebook.rs`: 12 widgets, each a thin reader over an endpoint that already exists. A cell carries a widget **id and params** — never HTML, JS or a URL — and the endpoint lives in the registry, which is code the server ships. That asymmetry is the guarantee: the registry is code, the notebook is data, so a shared notebook cannot make a recipient's browser fetch something of the author's choosing. `validate()` reports **every** problem at once (one round-trip per error is not an editor) and refuses unknown widget ids outright rather than storing them to skip at render — a stored reference nobody checks is how a payload eventually rides in. Undeclared params are refused too, so a cell cannot smuggle a field to the endpoint behind it. Geometry is a closed set (spans 3/4/6/12, height presets, 24-cell cap) and interval refresh has a 15s floor. 12 unit tests |
| N2 | Per-tenant persistence + CRUD | ✅ Done | `src/web/notebook.rs`: `notebook:{uid}:{id}` in sled behind GET list / GET one / POST save / DELETE. `localStorage` was rejected in the design: a notebook that lives in one browser cannot be opened on the machine wired to the projector, handed to a colleague, or survive a cache clear — the three things a composed dashboard is for. The path is the identity (a body claiming another id would write to a key the caller never addressed); persistence-off returns 503 rather than a success with nothing written; a tenant with no notebook gets the **starter composition**, not a blank grid or a 404. e2e covers save/list/read/delete, tenant isolation, both refusal paths, and that markup in a widget id is rejected |
| N3 | `/notebook` page — render an ordered grid | ✅ Done | Seven shared renderers (`kpis`, `bars`, `list`, `table`, `hits`, `feed`, `heat`); a bespoke renderer per widget is how a catalogue stops being cheap to extend. Numeric widgets declare **which fields** they plot — found by looking at the first render, where "verdict distribution" was plotting `cluster_count` and `dream_cycle_count` too, which is a dump, not a distribution. A retired widget renders an explicit card naming it; a failed fetch renders an error distinct from an empty result, because those two looking identical is most of the perceived brokenness in a dashboard. Each cell footer shows the endpoint and its latency |
| N4 | Editor: add / remove / reorder / resize / export / import | ✅ Done | Edit mode adds per-cell move/width/height/remove controls (buttons, not drag-only — drag fails accessibility and fails the shop-floor tablets), a picker that builds the param form from the registry, and rename/new/delete. Export copies the layout as JSON; import keeps the id being edited so someone else's layout cannot silently overwrite a different notebook of yours. Save failures render the server's full error list. One scheduler per interval cell, suppressed while the tab is hidden. Verified live: added a pinned `"no fault found"` search, saved, reloaded — composition persisted and the query answers live |
| N5 | "Add to notebook" from Search and Atlas | ✅ Done | `addToNotebook(widget, params, describe)` lives in `ui-shell.js`, not on each page: the direction that matters is the one nobody plans for — a query someone ran once, kept, and now sees every morning — so any page that can name a widget and its params gets the affordance, including pages not written yet. It read-modify-writes the whole notebook through the same save endpoint the editor uses, so a pinned cell is validated exactly like a hand-built one; there is no second, laxer path into a notebook. Search pins the query **with its active filters** (so the panel answers the same question tomorrow, not a broader one); the Atlas entity panel pins a standing search for the selected node. Verified live from both surfaces; page-serving test pins the wiring |
| N7 | XSS on the import path (found by security review) | ✅ Done | `data-span` interpolated a cell field into an attribute unescaped. Server validation did not cover it: **Import renders a pasted layout before it ever reaches the server**, and a shared layout is precisely the input an attacker controls — a span of `6" onload="…` broke out of the attribute. Two fixes, because either alone is a half-measure: the sink now coerces (`Number(c.span)`) and escapes (`esc(c.height)`), and `sanitizeImported()` **rebuilds** each cell field-by-field from the registry rather than filtering a copy, so anything the format does not describe is dropped by construction rather than by a blacklist. What it drops is reported, never silently repaired. Verified against a hostile document (attribute-breaking span and height, markup as a widget id, undeclared params, markup in the title): nothing executed, no handler attribute reached the DOM. A CI test pins the sink so it cannot regress without a browser in the loop |
| N6 | Quoted phrases mark as one span | ✅ Done | Fallout from N4's first live pinned search: `"no fault found"` highlighted as three separate words. A phrase and each of its words match at the same offset and the renderer keeps the first of an overlapping pair, so ascending order handed the mark to the single word — the phrase the user asked for verbatim, shown as three coincidences. `snippet()` now orders longest-span-first at a shared start; fixes `/search` too |

### Phase 14 — Monitor mode: the phone as a perception device (2026-08-30)

> Full design: `docs/plans/2026-08-30-monitor-mode.md`. A day leaves traces
> nobody writes down — the accelerometer knows there was a workout, the GPS
> knows it was the gym, the clock knows the fifth Tuesday did not happen. None
> of it reaches the graph, so the engine reasons about documents while the life
> those documents are *for* stays invisible.
>
> This is small because the split it needs is the one Phase 8b closed: **device
> perceives, engine means**. Every reducer already exists — `kinesis`→`soma` for
> motion and effort, `topos` for recurring places, `chronos` for rhythms and the
> ones that are slipping, `empedocles` for readings against a baseline. The
> engine work is a mapping layer and widgets, not new concepts.
>
> **Pro adds synthesis, never capture.** Normal mode is the deterministic
> reducers (they are the product); Pro is the provider cascade phrasing them. A
> Pro tier that can *see* more than Normal is two products and a user can trust
> neither.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| M1 | `monitor` module — sensor batch → machine primitives | ✅ Done | Pure and tested: the sensor→machine mapping is a product decision and belongs in the engine, not re-implemented in every client |
| M2 | `POST /api/v1/monitor/ingest` | ✅ Done | One call per sync, not one per machine — a phone on mobile data should not make four round trips to record one morning. Routes to the reducers, records causes |
| M2b | Observations are remembered, not only rhythms | ✅ Done | Found by the first e2e: the graph held the workout, the place and the readings but **no lesson**. `chronos` keeps *rhythms* — three Tuesdays become "lesson every 7d" and one Tuesday becomes nothing — which is right for chronos and wrong for a monitor, where "there was a 50-minute conversation at 14:00" is the fact the day was worth noticing. Each observation is now also remembered on its own, with the timestamp **in the label** because registration dedupes by label: without it Tuesday's lesson and Wednesday's collapse into one node and the second day looks like it never happened. The bare label stays bare so chronos can still match it across days |
| M3 | Notebook widgets over what M2 produced | ✅ Done | One `coherence.facet` **graph-query** widget (label / verdict / limit) rather than four bespoke monitor panels. Each of `workout`, `place:`, `rhythm:`, `conversation` would have been a thin filter over the same endpoint, and four registry entries to keep in step with label formats that will change; one widget plus a saved param is the same panel with none of that. Widgets, not a page: "show everything" is a wall, and a wall is not *derived from state by construction* — the notebook already lets each person compose the panels their life actually has |
| M4 | Android foreground service: sensors → batched events → M2 | ✅ Done | `clients/physis-monitor/` — Kotlin, **builds** (3.1 MB debug APK, verified: zero Play Services or Firebase classes linked). A foreground service and not a background job because the honest version of this app is the one the system shows is running. `TYPE_STEP_COUNTER` (hardware, already counting) rather than polling the accelerometer; `LocationManager` at 5 min / 50 m rather than the fused provider; first step read establishes a baseline and reports nothing, so a fresh install cannot claim the day's steps as one window's workout. Samples queue in memory and are dropped once the engine confirms them — the phone is a sensor, not an archive, and an archive on the phone is a second copy of a life to lose. A failed sync **keeps** its batch, which is safe precisely because the engine's translation is pure and registration dedupes by label. Every source is off until switched on and asks its own permission at that moment; the reboot receiver restores only what the user had chosen. Dependencies: `androidx.core` + `androidx.appcompat`, nothing else — `HttpURLConnection`, `org.json` and `LocationManager` are AOSP |
| M5 | Audio features with **no retention** | ✅ Done | The one piece with real legal exposure. Recording yourself is your business; a lesson or a call captures *other people*, which leaves the GDPR household exemption and engages art. 615-bis / 617 c.p. The liability is in storing audio and the value is in the inference, which does not need it: on-device RMS / spectral flux / VAD / speaker-count on a rolling buffer discarded in the same pass, never PCM on disk. The event is "conversation, 2+ voices, 50 minutes, at the university" — the audio was never the point. Shipped as `AudioProbe.kt`: one `ShortArray` allocated once and overwritten per ~62 ms frame, loudness + zero-crossing rate + an adaptive noise floor, recorder released *before* the result is returned. The guarantee is structural — the file contains no `File`, no `OutputStream`, no `Uri` and no network call, verified by grep over the source. Voice count comes from turn-taking and returns **null when the window gives no honest basis for a guess**, because "you met someone" recorded on no evidence is worse than a gap |
| M6 | Distribution: Play + direct APK from git | ✅ Done | **Owner decision:** F-Droid dropped because it requires every dependency to be free software. Recorded correction: F-Droid's constraint is *licensing*, Play's is *review*, and Play reviews background location and microphone — this app's two most sensitive permissions — far more harshly. The useful consequence is that **none of this needs Play Services** (`LocationManager`, `SensorManager`, `AudioRecord` are AOSP), so building without them costs nothing, ships on Play, ships from git, and leaves F-Droid open if review turns out to be the harder road. Pro mode points at the user's own instance, which is the honest answer to any store's "where does the data go". Shipped: `PRIVACY.md` (what leaves the phone per source, and why the microphone design is what it is), `RELEASE.md` (build, signing, direct-APK checksums, the **complete Play data-safety answers**, the background-location justification, the microphone disclosure, and the review friction to expect), `README.md`. Release builds are deliberately **not minified** so an APK published from git can be diffed against the source that claims to produce it |

### Phase 15 — Monetization v2 (2026-08-30)

> Source: `docs/plans/2026-08-30-productization.md` + `docs/COMMERCIAL_MODEL.md`.
> The repo was trying to be three products at once. Split into two — MIT **Core**
> (distribution funnel via MCP/agents) and EULA **Pro** (service-led pilots →
> Industriale/Enterprise licensing) — and drop the stale 4-tier SaaS / per-job
> metering (`BUSINESS.md`, the old `pricing.html`) that describes a product this
> repo is not.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| 15.1 | SKU → feature-slug entitlement table + order-form mapping | ✅ Done | `Tier::sku()/from_sku()/ALL` + `ENTITLEMENTS` table + `entitlement_for()` in `src/license.rs`; vendor tool `physis-license list-skus` prints the order-form table; `/settings` and `physis-pro license` expose the SKU. The licence still stores only features — the SKU is presentation, so a forged upgrade stays impossible |
| 15.2 | License `inspect` / `info` + trial-key generator | ✅ Done | `physis-pro license [--json]` (instant — dispatched before `PhysisApp::new`); vendor tool `sign --trial` mints a 14-day eval (TRIAL- prefixed id, maintenance included, traceable) |
| 15.3 | Key rotation + revocation | ✅ Done | `LicenseError::Revoked` + `check_revoked()` enforced in `license::load()` against a drop-in `revocations.json` (`PHYSIS_REVOCATIONS_FILE` override); vendor tool `revoke --id X [--list]` writes the sorted list to ship to air-gapped hosts. Malformed lists fail open (a vendor typo must not brick installs); key **rotation** stays out of scope — the module documents that rotating the vendor key reissues every licence |
| 15.4 | Customer/plant/host identity in report metadata | ✅ Done | `ReportMetadata` gains `customer` (from licence org, else `unlicensed`), `plant` (`PHYSIS_PLANT`), `host` (`PHYSIS_HOST`/`/etc/hostname`), `version` (build). `identity()` fills every report via `new`; rendered as an evidence line in shift-report markdown + HTML. 1 unit + 1 e2e test |
| 15.5 | Pricing page rewritten to the offer ladder (Pilot €1.5–2.5k → Industriale €1,500/mo → Enterprise €5,000/mo) | ✅ Done | `/` landing + `/pricing` + `/about` rewritten to the service-led model and routed publicly; supersedes the per-job SaaS tiers |

### Phase 16 — Integration spine (2026-08-30)

> Normalize every source into one typed, timestamped `OperationalEvent`, promote
> the substrate to a typed property graph, add the temporal/audit layer, and bind
> dream output to outbound actions. Each step is additive — the embed/coherence/
> dream engine only reads `.coherence`/`.embedding`, so nothing breaks.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| I1 | Canonical `OperationalEvent` (connector, mechanism, actor, ts, payload) | ✅ Done | `src/operational.rs`: kind/status/provenance + stable `event_key` (Sha-256) for dedupe; `From<&MachineReading>` + `From<&IngestedDoc>`; ledger-mechanism mapping |
| I2 | Adapters as `SourceAdapter` emitting `OperationalEvent` | ✅ Done | industrial adapter `post_event` now emits the full structured event (was a flattened register); machine/doc adapters normalize via `From` impls |
| I3 | One `post_event` ingestion path | ✅ Done | `POST /api/v1/events` (`src/web/operational.rs`): dedupe by `event_key` (idempotent), bounded per-tenant ring (`op_events:{uid}`, ≤10k, oldest dropped), causal-ledger `record_at`, optional coherence-node registration. Adapter change confirmed the "100 lines" claim |
| I4 | Provenance survives every projection (timeline, ledger, report, lineage) | ✅ Done | events persisted per tenant and rehydrated on startup/`with_user`; `GET /api/v1/events` (newest first) + `GET /api/v1/events/timeline` (the shared `OperationalTimeline::from_events` pipeline); ledger CauseEvent carries actor/object/observed_at into lineage/purposes |
| I13 | **Scan v2 — hierarchy tree + subtree-skipping diff + one node per doc** | ✅ Done | `src/tree.rs`: persistent per-root `DirTree` (dir layout + per-document mtime/size/content-hash/labels) with Merkle subtree fingerprints; `meta_walk` is stat-only (no content reads) and `diff` skips whole unchanged subtrees. `DirTree::scan_run` (shared by `/scan` and **watch-mode**) loads the previous tree → stat-walks → diffs → hashes+extracts ONLY changed/new docs (`process_file`, downgrading mtime-bumped-but-same-content to unchanged) → embeds one node **per document** (path-independent structural text → identical content anywhere is computed once by the content-addressed cache) → deletes removed. A document is scanned at most once. Watch-mode now uses the tree too (per-doc Generated/Corrected cause events preserved; old per-dir hash cache removed). 25 tree unit tests + 2 e2e (idempotent re-scan, one-node-per-doc) |
| I5 | `CoherenceNode` gains `type` + `properties` (default-empty, back-compat) | ✅ Done | `src/models.rs::CoherenceNode` + `NodeView` gain `kind` + `properties` (`serde(default)` → legacy rows load as untyped). Exhaustive `observable()` destructure forces the projection to carry them. `CoherenceNode::with_kind/with_properties/property` builders. `/coherence/register` accepts `kind` + `properties` (dedupe-by-label updates them); the event ingestion stamps kind=source-family + properties from the event. 2 unit + 1 e2e test |
| I6 | Named link types + inverse traversal (replace bare `semantic_type: u32`) | ✅ Done (coherence-graph layer) | `PhysisCore::add_typed_edge(source, RelationType, target, conf, w)` (named vocabulary: Supports/Contradicts/Causes/Influences/Precedes/Follows) + `edges_from` (forward) + `edges_to` (inverse traversal). HTTP: `POST /api/v1/coherence/add-edge`, `GET /api/v1/coherence/edges?node_id=` (both directions, tenant-scoped, unknown relation/endpoint → 400). The low-level repr(C) holarchy `Edge.semantic_type: u32` is left intact — it is serialization-critical and its callers pass a generic `1`; the named vocabulary lives on the `TypedEdge` layer where I5's typed axis sits. 1 e2e test |
| I7 | 70-cell grid → object types (Machine, WorkOrder, SensorReading, Invoice) | ⬜ Pending | |
| I7 | 70-cell grid → object types (Machine, WorkOrder, SensorReading, Invoice) | ✅ Done | `models::object_type_for(domain, mode)` — a **total** resolver over the real 5×14 grid (HEAL/CONSTRUCT/FABRICATE/BOND/STUDY × 14 modes): mode refinements beat the per-domain default (e.g. FABRICATE·SENSE → `sensor_reading`, FABRICATE·PLAN → `work_order`, FABRICATE·WORK → `machine`). `OBJECT_TYPES` vocabulary + `is_valid_object_kind`. Surfaced: `/classify` response gains `object_type`; `/coherence/register` auto-stamps `kind` from the winning cell when untyped (no second embedding). 3 unit + 1 e2e test |
| I8 | Dream output → typed actions (open WorkOrder, schedule maintenance) | ✅ Done (decide half) | `models::ActionKind` + `TypedAction` + `typed_action_for(node_kind, id, outcome, prevented)` — a replayed failure node proposes one concrete action: machine/asset → `schedule_maintenance`, work_order/sensor → `open_work_order`, document → `review_document`, person/goal → `escalate`, resolved → `acknowledge`. `/dream/replay` entries now carry `typed_action {kind, object_id, object_kind, summary}` — agent-dispatchable, not free text. Act half (dispatching the action) is I11/I12. 3 unit + 1 e2e test |
| I9 | Append-only mutation log (node/link/verdict) | ✅ Done | `src/audit.rs` `MutationLog` — sealed per-tenant rows `{seq, at, op, subject_id, subject_kind, actor, detail}`, capped ≤10k, sled `audit:{uid}` (whole-list, resumes seq across restarts). Hooked: register/assert, edit, delete, edge:add. `GET /api/v1/audit` newest-first + `?node_id=` change history + `?as_of=RFC3339` time-travel slice. 5 unit + 1 e2e test |
| I10 | Time-travel + audit view | ✅ Done | audit slice by `as_of` (`GET /api/v1/audit?as_of=`) answers "what changed when OEE dropped"; point-in-time node *existence* already served by `/coherence/nodes/all?as_of=` (`created_at <= cut`). Deeper full-state reconstruction from the log is future work — the log is the replayable source for it |
| I11 | Dream countermeasures → webhook/CSV export | ✅ Done | `POST /api/v1/actions/dispatch` — the **act** half of I8: delivers typed actions to `PHYSIS_ACTIONS_WEBHOOK` (air-gapped site leaves it unset → honest `configured:false` + note, never a silent failure); per-action delivery status + audit `action:dispatch`. `GET /api/v1/dream/replay.csv` exports the same replay as RFC-4180 CSV for spreadsheets/workflows. 3 e2e tests |
| I12 | ERP/MES/ticketing bindings | ⬜ Pending | only when a named customer names their system |

### Phase 17 — UI/UX honesty (2026-08-30)

> The 10 product pages (Pulse…Demo) share the Bioluminal Forge shell and are
> strong. The customer-facing pages are orphans — unrouted, using CSS classes
> that no longer exist, describing a "pay-per-job API platform", with broken JS
> and dead `/api/v1/my/*` endpoints. This phase makes the customer surface stop
> lying about the product.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| U1 | Landing (`/`), pricing, about rewritten to the real model + design system | 🔨 In progress | this change |
| U2 | `customer_dashboard.html` → license/deployment status page over real endpoints | ⬜ Pending | no billing backend exists; stop faking one |
| U3 | Remove or fold `api_tester.html` into Agent/Studio | ⬜ Pending | duplicates Notebook + Search affordances |
| U4 | Route `/`, `/pricing`, `/about` publicly; keep `/pulse` as product home | 🔨 In progress | |
| U5 | 10-minute demo: `prospect.csv → shift report + ontology studio` | ✅ Done | `physis-pro demo <dir> [--demo]`: reads every shift `*.csv` → `report.html`+`report.md` (Italian OEE report, per-machine losses, evidence-grade identity line) and runs the ontology studio (`studio.html`) over the same folder. `--demo` seeds a sample `shifts.csv` + demo corpus idempotently (real data never overwritten). Verified end-to-end against a vendor-stamped build + trial license |
| U6 | `--screenshot` mode / per-release capture pipeline | ✅ Done | `scripts/screenshots.sh`: boots the web server (real ONNX model required — marketing shots need semantics; fail-loud otherwise), captures `/`, `/pricing`, `/about`, `/deploy`, `/settings`, `/pulse`, `/console`, `/search`, `/studio`, `/demo` with headless chromium at 1440×900 into `shots/`, and writes a reviewable `shots/index.html` contact sheet. Verified end-to-end (10 valid PNGs). Local loopback capture opts into `PHYSIS_DEV=1` |
| U7 | Re-audit pages against the offline/WCAG floors in `physis.css` | ✅ Done | Audit found the system already compliant: palette contrast ≥ ~6.5:1 (AA+), `:focus-visible` on every interactive element, `prefers-reduced-motion` kills all animation, nav/ask-bar carry `aria-label`, nav sets `aria-current`. Added the one real gap: **skip-to-content** (WCAG 2.4.1) — `ui-shell.js` injects a `.skip-link` on every page (product + marketing) targeting the first `<main>` |

### Phase 18 — Feature testing (2026-08-30)

> The engine is unit-tested (759 lib tests) but tests are per-phase, not
> funnel-shaped. Add cross-surface tests for the journey a customer actually
> walks, and prove the air-gapped moat in CI.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| T1 | E2E funnel: scan → shift report → quality → dream → `where` → lineage | ✅ Done | `funnel_scan_report_quality_dream_where_lineage_is_tenant_isolated` — the whole customer journey in one test under the offline embedder: scan (file → labeled node + ledger `generated` event) → shift report (`/api/v1/shifts/report?format=md` renders OEE + provenance) → quality (hot reading vs thresholds → anomaly + audit) → dream (failing typed node → typed action) → where (`/api/v1/files/query?cause=scan` → `by_mechanism.generated`) → lineage (`/api/v1/files/lineage?path=` shows the scan cause) — plus tenant-b sees an empty ledger/events/audit. Also corrected the 15.4 test to hit the shift-history report (`/shifts/report`) instead of the live-board one |
| T2 | Adapter-contract test: stub `OperationalEvent` inherits dedup+anomaly+ledger | ✅ Done | `POST /api/v1/events` now takes `thresholds` and anomaly-checks accepted **machine** events (temperature/vibration/rpm from event properties via `reading_from_event` → `detect_anomalies`); each hit is audited (`anomaly` op) and returned in the response. e2e proves the I4 "new adapter ≈ 100 lines inherits dedup + ledger + anomaly + timeline" claim end-to-end (hot reading flagged + audited; cool reading not) |
| T3 | Typed-object round-trip (type+props survive persist/edit/delete) | ⬜ Pending | verifies I5 back-compat |
| T4 | Temporal-log replay + audit view | ⬜ Pending | verifies I9–I10 |
| T5 | Marketing-page smoke: `/`, `/pricing`, `/about` 200 + no external assets | ✅ Done | `marketing_pages_serve_expected_copy_without_external_assets`: `/`, `/pricing`, `/about`, `/deploy` serve 200 with the real product copy (not the stale "API platform" framing), load the shared `/physis.css`, and fetch no external CSS/JS; `/settings` serves its own copy. The air-gap rule the product obeys is asserted on the marketing pages too |
| T6 | Entitlement matrix (each SKU = exactly its slugs, cross-tier denial) | ✅ Done | `entitlement_matrix_enables_exactly_its_bundle` iterates all 3 tiers against the full feature universe: every bundle feature granted, every non-bundle feature denied (no over-grant), SKU↔tier consistent with the signed bundles |
| T7 | Air-gapped test: no weights, no network → lexical path, no 500, no CDN | ✅ Done | `air_gapped_path_degrades_honestly_without_weights_or_network`: classify 200 + names `random-projection` model; embed 200 with dim; extract 200 with slots OR an explicit degradation note — never 500, never fabricated. (The whole http_surface suite runs under the random-projection embedder as the air-gap worst case; `/ask` degradation already covered separately) |
| T8 | Un-ignore `output_quality` on a scheduled (non-push) job | ✅ Done | `.github/workflows/output-quality.yml`: daily `cron` + `workflow_dispatch` job that caches/fetches BGE-base (same contract as the accuracy-floor job) and runs `cargo test --features full --test output_quality -- --ignored --nocapture`. The heavy review-oriented probe stays off the push path; the accuracy floor remains per-push |

### Phase 19 — Agent surface (2026-08-30)

> **Thesis:** Physis is not "LangChain + Semantic Router" — it is the layer both
> are missing: routing that learns, budget-bounded grounded retrieval, and
> memory that carries verdicts. The harness owns the loop; Physis owns the
> semantics, served over MCP + HTTP. This phase closes the four gaps agents
> expect (route table, structured output, streaming/tool schemas, tracing) and
> ships a thin SDK. **Non-goals:** no LCEL/chains, no integration catalog, no
> vector-store zoo — composition stays data, and the harness stays out of repo.

| # | Deliverable | Status | Notes |
|---|-------------|--------|-------|
| P1 | Declarative route table (`src/routes.rs` + `src/web/routes.rs` + MCP `physis_route`) | ✅ Done | `Route { name, cell, action, examples }` as JSON data in `{data_dir}/routes.json` (composition-as-data, same rule as the notebook registry). `GET/POST/DELETE /api/v1/routes`, `POST /api/v1/routes/match` (classify → cell → routes, deterministic), MCP tool. Validation reports every problem at once; routes persist before `ok`. 6 unit tests |
| P2 | Structured output / slot extraction (`src/extract.rs` + `src/web/extract.rs`) | ✅ Done | `POST /api/v1/extract {text, schema}` — schema-constrained extraction through the cascade; tools-first, plain JSON prompt fallback, explicit `note` on failure (never a fabricated object). 4 unit tests (fenced/prose JSON, garbage, schema in message) |
| P3 | Streaming + tool schemas | ✅ Verified | `run_agent_stream` + `POST /api/v1/agent/run` → SSE of `AgentEvent`s already shipped (web/agent.rs); `ProviderCascade.complete(.., tools, ..)` already takes `ToolDefinition[]`. Nothing to build — recorded so the claim is checked, not assumed |
| P4 | Thin SDK (`clients/physis_client.ts`) | ✅ Done | Zero-dep fetch client mirroring the MCP surface: health/settings, classify/ask, routes CRUD + `route`, `extract`, `recall`. Field names match the wire (`token_budget`/`top_k`) |
| P5 | Quality-failure trace (`GET /api/v1/trace?failure_id=`) | ✅ Done | Wired the previously-dead scaffold: resolves the failure from `QualityTracker`, joins the tenant's causal-ledger events within ±1h (tenant-scoped), returns failure + ledger trail. Removed the dead `display_cell`/`TraceFailureRequest` |

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
| `docs/plans/2026-08-29-notebook-dashboard-editor.md` | Phase 13 — `/notebook` composable dashboards | ✅ Shipped | N1–N5 built 2026-08-30; registry-as-references, per-tenant persistence, pinned queries |
| `docs/plans/2026-08-30-monitor-mode.md` | Phase 14 — monitor mode (mobile perception) | 🔨 In progress | Engine slice M1–M3 here; Android client M4–M6 is a separate repo over HTTP |
| `docs/plans/2026-08-30-productization.md` | Phase 15–18 — monetization v2 · integration · UI/UX · feature testing | 🔨 In progress | Pro/Core split, offer ladder, saleability gates, `OperationalEvent` spine, temporal log, customer-page rewrite, funnel tests |
| `PLAN.md` Phase 19 | Agent surface (route table, extract, SSE, SDK, trace) | ✅ Done (P1–P5) | Harness out of repo; MCP + HTTP are the contract |
| `docs/plans/2026-08-26-space-time-cause-datawork.md` | Phase 10 — Space·Time·Cause | ✅ Shipped + verified | D1–D6 (+ D7 mail) built; see `AGENTS.md` Space·Time·Cause section. End-to-end verified 2026-08-30 (Phase 10v) — HTTP chain passed as documented, CLI `scan` was found recording no cause and fixed |
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
- ~~**Space·Time·Cause verification**~~ — **closed 2026-08-30** (Phase 10v
  above). The HTTP chain passed unchanged; the check earned its keep on the
  CLI side, where `scan` had never recorded a causal event, leaving `where`
  permanently empty on a server-less install.

---

## Hardening Backlog — External Evaluation (2026-08-31)

An end-to-end evaluation was run against the built binaries rather than the
documentation: licence gate, embedder cascade, classification, the SECOM
benchmark, `pack` retrieval, the HTTP surface, and CI configuration. What
follows is ordered by commercial risk, not by effort. Each item names the
evidence that produced it so it can be re-checked.

### H1 — Fail closed when the licence public key was not stamped in

`src/license.rs` falls back to a hardcoded development public key when
`PHYSIS_LICENSE_PUBKEY` is absent at build time. That key
(`3b6a27bc…8b59da29`) is the Ed25519 public key of the all-zeros seed, so its
private half is guessable by anyone without repository access. A licence signed
with a 32-byte zero key was minted and accepted by a `cargo build --release`
binary as "Enterprise tier, 1 seat, expires 2027-12-31".

`scripts/package.sh` does require the variable, so official packages are not
affected — but any binary built the ordinary way carries no enforceable gate,
and one such binary reaching a customer or a demo host removes the premise of
the Pro layer.

Fix: when `option_env!("PHYSIS_LICENSE_PUBKEY")` is `None`, compile a build
that refuses to start unless `PHYSIS_ALLOW_DEV_LICENCE=1` is also set, and
print an unmistakable "DEVELOPMENT BUILD — licence gate disabled" banner on
every run. Keep the dev key usable for tests; make it impossible to ship by
accident.

**Status — CLOSED (2026-09-07).** `license.rs` compiles `DEV_KEY_BUILD`
(`option_env!`-based) and `enforce()` exits 2 on a dev-key build unless
`PHYSIS_ALLOW_DEV_LICENCE=1` (exact string; `true`/`yes` don't count), printing
the dev banner on every allowed run. Licences are still signature-verified
against the embedded key in dev mode — a minted-with-foreign-key licence is
refused either way. Verified end-to-end with a local binary: zeros-key-signed
"Enterprise" licence without the env → exit 2; with the env → runs with the
dev banner; foreign-key licence → signature refused in both modes.

### H2 — Make the semantic self-test actually discriminate

`embed::semantic_self_test` compares "a photograph of a dog" against "a
photograph of a puppy" and "quarterly corporate tax accounting spreadsheet".
The related pair shares four of its five tokens, so any bag-of-words hash
passes. The random-projection embedder passes it, and `/health` and
`/api/v1/embedder` consequently report `"semantic": true, "degraded": false`
while the CLI simultaneously warns that the same embedder is non-semantic.

Measured through `/api/v1/embed` on the random-projection embedder:

| pair | cosine |
|------|--------|
| `car` / `automobile` | 0.401 |
| `car` / `banana` | 0.525 |
| "the spindle motor overheated and tripped" / "thermal fault shut down the drive" | 0.843 |
| "the spindle motor overheated and tripped" / "invoice payment terms net thirty days" | 0.823 |

Fix: replace the probe with lexically disjoint synonym pairs and require a
margin over a distractor set — for example `car`/`automobile` must beat
`car`/`banana` by a fixed margin, over several pairs. Any embedder that fails
must set `degraded: true` on every health surface. This is the guard PREMORTEM
P4 was written for and it currently does not guard.

**Status — CLOSED (2026-09-07).** The probe is now three lexically disjoint
pairs (`car`/`automobile`, `dog`/`puppy`, spindle-overheat/thermal-fault) each
required to beat its distractor by 0.05 (`SEMANTIC_PROBES`, embed/mod.rs).
Unit tests pin both directions: the random-projection embedder must fail the
probe, and a guard test keeps the pairs disjoint so the overlap bug can't
return. The `SelfTestCache` key now carries a probe version
(`SEMANTIC_PROBE_VERSION`), so verdicts recorded under the lax probe are
invalidated. Verified against the real BGE model: the accuracy floor still
passes (CORE 0.873 / cell 0.727 / ITALIAN 0.867, model=bge-base), and the web
surfaces' existing `semantic`/`degraded` logic keeps its contract
(offline-by-request → semantic=false, degraded=false).

### H3 — Never degrade the embedder silently

`OnnxConfig::model_dir` defaults to the relative path `./models`, so the same
command classifies correctly from the repository root and falls back to
random-projection from any other working directory, with no error — only the
one-line warning. Same input, from two directories:

- from repo root (bge-base, dim 768): `FABRICATE×MAINTAIN — 0.840`, nearest
  entry "Machine Maintenance" — correct.
- from elsewhere (random-projection, dim 384): `BOND×GUIDE — 1.000`, nearest
  entries "Mentor and Lead", with the next four cells at 0.997–0.995.

Fix: resolve the model directory against the binary location and the data
directory, not the working directory; add `PHYSIS_MODEL_DIR`; and make a
downgrade to random-projection an error unless `PHYSIS_EMBEDDER` explicitly
requested it. A silent fallback that produces confident nonsense is worse than
a refusal.

**Status — MOSTLY CLOSED.** `embed::model_dir` already searches
`PHYSIS_MODELS` → `MODEL_DIR` → `./models/<name>` → `<data_dir>/models/<name>`
(embed/mod.rs), and the web server aborts startup on an implicit RP downgrade
unless `MODEL_BASE_URL` is set or the operator asked for offline mode
(web/mod.rs). Remaining gap: the CLI still degrades to RP with only a warning
instead of refusing; `PHYSIS_MODEL_DIR` is spelled `PHYSIS_MODELS` here.

### H4 — Stop the feedback loop inflating confidence without accuracy

`QualityTracker::adjust_score` computes `raw × (1 − penalty + boost)` clamped
to `[0,1]`. Boosts are per-cell and input-independent, so a boosted cell is
promoted for every subsequent input; the clamp then destroys ordering once
several cells saturate at 1.0. The SECOM run with the ONNX embedder shows the
effect exactly: mean adjusted score rises 0.523 → 0.939 while accuracy stays
at 0.700 and `top1_changed` is 0.

In an industrial QA setting this is the worst possible failure shape — the
system becomes more confident and no more correct. Fix: cap `cell_standing` so
boosts cannot saturate (for example bound the multiplier to `[0.5, 1.2]`),
and report raw and adjusted scores separately everywhere so a boost is visible
as a boost rather than as evidence.

### H5 — Replace the SECOM benchmark or stop citing it

The embedded sample is 21 observations with 10 features (real UCI SECOM is
1567 × 590). The default run splits to a 10-row test set: 60% accuracy with the
random embedder, 70% with ONNX, against a 61.9% majority-class baseline. At
that size the number carries no information, and the report's own
"Interpretation" section says feedback had no measurable effect.

Fix: either wire `--dataset <path>` to the full UCI CSV and report accuracy,
precision/recall on the rare FAIL class, and a majority-class baseline
alongside; or remove the benchmark from any customer-facing claim. A weak
benchmark that is quoted is a liability; one that is not quoted is dead code.

### H6 — Gate Italian accuracy on the model that actually ships

`tests/classification.rs` gates Italian top-1 domain accuracy at 0.80 only for
multilingual models (`jina-v2`, `bge-m3`). Jina is deliberately excluded from
the auto-cascade and bge-m3 is 2.2 GB, so the model that actually loads is
`bge-base-en-v1.5`, which is English-only and ungated. The test's own comments
record its Italian history: 0.733 → 0.867 after Italian anchors → **0.667**
after the 2026-08-14 ontology expansion diluted nearest-entry scoring.

The product is sold in Italian, to Italian plants, with an Italian shift
report as the pilot deliverable. Two of every three Italian queries landing in
the wrong domain is a first-meeting-ending defect. Fix: pick one multilingual
model as the shipped default and gate on it, or add Italian anchor entries
back until bge-base clears a stated floor — and gate at that floor regardless
of model family. Also make `accuracy_floor` **fail** rather than `return` when
no model is on disk, so a skipped download cannot produce a green CI run.

### H7 — Give `pack` an incremental index

A cold `physis-pro pack src "…"` over 189 files / 2238 chunks took 9m59s wall
and 5164s CPU. Retrieval quality was excellent — the top three chunks were
`bin/license_tool.rs:81-120`, `license.rs:641-680`, `bin/license_tool.rs:41-80`
for a query about Ed25519 licence verification — but no agent will wait ten
minutes, and the pitch for this tool is that it runs locally with no model API.

The warm path depends on `EmbedCache`, which is a single JSON file loaded in
full and rewritten in full on every invocation, with no eviction. It reached
33 MB after two runs on `src/`. At 768 f32 per chunk in JSON text (~15 KB), a
100k-chunk customer corpus is a 1.5 GB parse per invocation.

Fix: move chunk vectors into the existing sled store keyed by content hash,
mmap or stream them, evict by age, and reuse the `index` catalog so unchanged
files are never re-chunked. Then publish a cold and warm number.

### H8 — Repair the front-door install path

`install.sh` copies `target/release/physis-web`, which is not a binary this
workspace produces (the web binary is `physis-pro-web`), and wraps it in
`gateway/physis_mcp_gateway.py`. It cannot succeed as written. Fix or delete
it, and make `scripts/package.sh` the only documented install route.

### H9 — Reconcile the licence claims on customer-facing pages

`pricing.html`, `about.html`, `index.html` and `customer_dashboard.html` state
that the core is MIT and that `physis_pro` is "dual-licensed — MIT for the
crate API". `physis-core/Cargo.toml` declares `license = "Apache-2.0"`, and
the root `Cargo.toml` carries a comment explaining that `physis_pro` has no
SPDX id because `LICENSE.txt` is a proprietary EULA. Any enterprise legal
review will find this before it finds anything else. Pick the true statement
and make all four pages say it.

### H10 — Small correctness debt

- `/health` reports `"version":"0.1.17"`; `Cargo.toml` is at `0.1.19`. Read the
  version from `env!("CARGO_PKG_VERSION")`.
- `.env` sets `PHICS_LOG_LEVEL`; nothing reads that name. It is a typo for
  `PHYSIS_LOG_LEVEL`, so log level is not configurable through the documented
  variable.
- The `default` feature set excludes both `mcp` and `embed-onnx` — the two
  features that carry the value proposition. A default build has no MCP server
  and no semantic embedder. Decide deliberately whether that is the intended
  shipping default.

### H11 — Scope discipline

The surface is 181 distinct HTTP routes, 46 CLI subcommands, 29 MCP tools and
18 named machine reducers, built by one person in 101 days. The reducers are
real algorithms — `chronos` is a median inter-arrival rhythm detector with
next-due prediction and a regularity×evidence confidence, which is exactly
maintenance-interval mining — but nothing in the naming reaches a plant
manager. Roughly 15.3k lines sit in industrially sellable modules (OEE, shifts,
maintenance, monitor, inventory, reconcile, report, machines, ingest) against
75.3k in `src/` overall.

Fix: pick the pilot path — ingest → OEE → anomaly → Italian shift report — and
freeze everything outside it. Rename the reducers on every customer-facing
surface to what they compute (`chronos` → "recurring-event detector"). Keep the
Greek names internally if they help; they cost sales externally.

### What the evaluation confirmed works

Recorded so the backlog above is not mistaken for a verdict on the whole system.

- OEE decomposition in `src/oee.rs` is textbook-correct (availability =
  run/planned, performance = ideal_cycle × total / run capped at 1, quality =
  good/total, world-class at ≥0.85) and depends on no embedder at all. The
  pilot deliverable is the part that does not carry model risk.
- With bge-base loaded, classification is genuinely good on real industrial
  text, and `pack` retrieval put the right three chunks first.
- Security defaults fail closed: the web server refuses to bind loopback with
  no token unless `PHYSIS_DEV=1`, RBAC is three-tier with method-derived
  defaults, and filesystem handlers canonicalize under a scan root.
- CI runs `clippy --all-targets -D warnings`, downloads the real model under
  `set -euo pipefail` for the accuracy job, and exercises the Spanner sink
  against the official emulator.
- The licensing infrastructure is real work: Ed25519 envelopes, tier derived
  from the signed feature list, revocation lists, artifact signing,
  clock-rollback detection.
- Comments throughout record the failure that motivated the code rather than
  restating it. That is the single strongest maintainability signal in the
  repository and it should survive any refactor.

### Sequencing

H1, H2 and H3 are one focused block on trust in the engine's own status
reporting and should land together — each is a case of the system claiming
health it does not have. H6 and H4 are the accuracy block and gate the first
pilot. H7 gates any agent-facing sale. H5, H8, H9 and H10 are pre-sales
hygiene. H11 is a standing constraint, not a task.

---


### Phase 20 — Ontology node visualization: fragments, instances, origin links (2026-09-02)

> **Goal:** every ontology node in the Atlas/Studio visualizations must cite the
> exact text fragment it was derived from, list ALL instances of that concept
> (conceptual and concrete), and provide hyperlinked-clickable references back to
> each origin so the user can "search for them in their origin."

**Current state:** `NodeView` (the projection every surface uses) carries `id`,
`label`, `score`, `asserted`, `cell`, `pinned`, `created_at`, `kind`, `properties`
— but NO source fragment, NO instance list, NO origin URI. The `properties` bag
is a `serde_json::Map<String, Value>` (Phase 16 I5) and is the natural place to
store this, but it is not populated with provenance references during ingestion.
`OntologyNode` (the typed graph) has `props: HashMap<String, String>` but also
lacks fragment/instance/origin fields. `ContentHit` (content search) already
carries `citation` + `path` + `snippet` — the fragment-citation machinery exists
in the search layer but is NOT surfaced at the ontology-node level.

**Gaps:**
1. `CoherenceNode.observable()` -> `NodeView` does not include `source_fragment`,
   `instances`, or `origin_url`. Adding fields here forces every surface to
   decide (the exhaustive-destructure rule), which is the point.
2. `OntologyNode` (ontology_graph.rs) has no `instances` list — concept nodes
   from `merge_discovery` carry `count`/`samples` strings, not structured
   instance references.
3. No origin URIs (file paths, line ranges, section paths) are attached to
   nodes at registration time.
4. The Atlas UI (`atlas.html`) object explorer shows label, cell, score, links,
   and "In documents" — but that search uses `node.label`, not the actual
   fragment the node maps to. It does not list instances of a concept.

**Plan:**

| #  | Deliverable | Est. Effort | Notes |
|----|-------------|-------------|-------|
| 20.1 | `NodeView` gains `source_fragment: Option<String>` (most relevant text snippet, <=256 chars), `instances: Vec<InstanceRef>`, `origin_url: Option<String>` (clickable URI) | Med | Exhaustive destructure in `observable()` — every surface must opt-in |
| 20.2 | `InstanceRef` struct: `{ id, label, kind: "conceptual"|"concrete", origin: String (file:line or URI), fragment: String (<=128 chars) }` | Small | Conceptual instances are other ontology nodes; concrete are coherence nodes pointing here |
| 20.3 | `OntologyNode` gains same fields — `instances` populated during `merge_discovery` from `samples`/`hints`, each sample becomes an `InstanceRef` | Small | |
| 20.4 | Ingestion path writes `origin_url` + `source_fragment` into `CoherenceNode.properties` at registration | Med | `properties["origin_url"]`, `properties["source_fragment"]` |
| 20.5 | Atlas UI explorer: node detail panel gains "Source fragment" + "All instances" sections with hyperlinks | Med | JS: `openEntity()` already exists; extend the panel HTML |
| 20.6 | `CoherenceNode` backfill: one-time migration populates `source_fragment` from `label` for nodes where missing | Small | `properties` already persisted; backfill is additive |
| 20.7 | MCP tool `physis_node_instances` — given node label, returns all InstanceRefs | Small | Reuse MCP surface pattern from Phase 19 |

---

### Phase 21 — Multimodal ingestion pipeline: OCR, MP3, WAV, image, video (2026-09-02)

> **Goal:** extend the ingestion pipeline (`src/ingest.rs`, `src/content.rs`) to
> accept OCR'd PDFs/documents, MP3/WAV audio, images (CLIP embedding), and video
> (frame-sampled CLIP), reducing all to the shared text+vector space that feeds
> classification, the holarchy, and causal ledger.

**Current state:**
- Images: `src/embed/clip.rs` has `ClipEmbedder` with `embed_image()` (ONNX CLIP
  vision encoder + tokenizer), gated behind `--features embed-onnx`. `embed/mod.rs`
  wires it as the `image` embedder in `EmbedderRegistry`. The `/classify` and
  `/embed` endpoints accept base64 `image` in the request body (per AGENTS.md
  "Multimodal & Sensory"). **BUT** `embed-onnx` is documented as BROKEN
  (ort 0.13 API drift) — the CLIP embedder compiles but the semantic path is
  not buildable in this tree (physis-pro-dev SKILL, "embed-onnx feature is BROKEN").
- Audio: `src/bin/voice.rs` (`physis-pro-voice`) does arecord -> whisper ->
  llama3.2 extraction -> `OntologicalMap`. `ai/provider.rs` has
  `ProviderCascade::transcribe()` (Whisper via Groq/OpenAI). `monitor.rs`
  has `SignalType::AudioPCM` and records speech *duration* as an event, but
  explicitly does NOT store the audio (privacy by design). The interest daemon
  (`physis-interest-daemon`) has clipboard + watched-dir observation but no
  audio ingestion.
- Video: AGENTS.md says "planned: frame-sample -> embed_image -> temporal
  pool. Not built." `embed_onnx.rs` has no video path.
- OCR: `ingest.rs` has `extract_pdf()` using `pdf-extract` (text-layer only).
  For scanned PDFs (image-only), `pdf_extract::extract_text` returns empty and
  the code falls back to raw bytes as lossy UTF-8 — **no OCR pass**. No
  `tesseract`, `pdfium`, `mupdf`, or OCR crate dependency exists in Cargo.toml.
- Docx/pptx: `interest.rs` handles via `ooxml_analysis` (zip-of-XML). This is
  feature-gated behind `daemon` feature, not in the default ingest path.

**Gaps:**
1. No OCR for scanned PDFs — only text-layer extraction. Scanned docs silently
   produce empty/garbage bodies.
2. No audio ingestion pipeline — `voice.rs` is a streaming daemon, not the
   batch ingestion path. `ingest.rs` has no `DocKind::Audio`.
3. CLIP image embedder exists but is unbuildable (ort 0.13 API drift).
4. No video ingestion — frame sampling + temporal pooling not implemented.
5. No unified multimodal type in `ingest.rs` — `DocKind` enum is text-centric
  (Invoice, Email, Spreadsheet, VoiceTranscript, Report, Unknown).

**Plan:**

| #  | Deliverable | Est. Effort | Notes |
|----|-------------|-------------|-------|
| 21.1 | Fix `embed-onnx` feature so CLIP vision encoder builds — either pin `ort` to a compatible version or update API calls | Med-High | Blocked on ort 0.13 migration; see PREMORTEM.md P0 |
| 21.2 | `DocKind` adds `Image`, `Audio`, `Video`, `ScannedPDF` variants | Small | Update `classify_file()` extension matching |
| 21.3 | OCR backend: integrate `tesseract` or `pdfium-render` for scanned PDFs | Med | `pdfium-render` has a pure-Rust path; `tesseract` is a system dep. Pick one. |
| 21.4 | Audio ingestion: `extract_audio_transcript()` using `ProviderCascade::transcribe()` — MP3/WAV -> Whisper -> text | Med | Reuse existing Whisper path in `ai/provider.rs`; `DocKind::Audio` |
| 21.5 | Image ingestion: frame-extract from video -> CLIP embed -> average pool | Med | Depends on 21.1; sample 1 frame/5s, temporal average |
| 21.6 | Video ingestion: `extract_video_frames()` -> CLIP embed each -> temporal pooling | Med | Depends on 21.1; use `ffmpeg` CLI or `image`/`ndarray` |
| 21.7 | All multimodal bodies flow through the same `doctree` heading parser + `ContentIndex` so content search covers them | Small | Additive — `section_units()` already calls `doctree::parse(&d.title, &d.body)` |
| 21.8 | `Interest engine` (Phase 5B) gains multimodal observation types — `ObservationKind::Image/Audio/Video` | Low | `interest.rs` already has `ObservationKind`; extend enum |

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
