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
| UI | Console (9-tab dashboard), Demo (shop-floor seed+watch+classify), Study (log+trainer) |
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
| 1.8 | Real-time MQTT/Modbus adapters | ⬜ Pending | — | Full subscribe/publish loops |
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
| 3.1 | Web-based Dashboard (React/Next.js) | ⬜ Pending | — | FileThemeGraph force-directed |
| 3.2 | Natural Language Interface (chat) | ⬜ Pending | — | Plain-English graph queries |
| 3.3 | Quality Dashboard UI | ⬜ Pending | — | Failure trends + ROI metrics |

### Phase 4 — Monetization

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 4.1 | Tiered licensing (Core / Industrial / Enterprise) | 🔜 In Progress | — | Offline enforcement done |
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
| 9.2 | Anti-hallucination grounding gate (`src/grounding.rs`) | ⬜ Pending | decision/rationale/quote must token-match source span or field is dropped; candidate rejected when no evidence survives; attach verification to node provenance |
| 9.3 | Decision mining → embedding dedup | ⬜ Pending | mine 9.1 episodes for decisions; dedup via existing embedder cascade cosine search (shared store, label-prefix namespace) — no separate index |
| 9.4 | Holarchy community detection (`label-propagation`, no new deps) | ⬜ Pending | oversized-cluster split >30%; cohesion score; curation READS graph, WRITES only its result (regression-tested invariant); studio rollup tab |
| 9.5 | Wiki generation over the holarchy (`wiki-gen` feature) | ⬜ Pending | two-pass outline: deterministic grouping pass + LLM naming-only pass (coverage invariant to LLM failure); pure grouping/layers/tour fns unit-tested, LLM injected via trait |

### Phase 6 — Administration / Datawork

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 6.1 | Fixed demo corpus + Admin Review Desk flow | ⬜ Pending | — | |
| 6.2 | CSV/XLSX batch import with validation | ⬜ Pending | — | + reviewed export |
| 6.3 | Invoice/PO/delivery-note reconciliation | ⬜ Pending | — | No payment automation |
| 6.4 | Weekly management-pack templates | ⬜ Pending | — | Source provenance |

### Phase 7 — Decision Planner

| # | Deliverable | Status | Owner | Notes |
|---|-------------|--------|-------|-------|
| 7.1 | Version ontology/overlay scenarios | ⬜ Pending | — | Without mutating live state |
| 7.2 | Coverage gaps + affected reports/workflows | ⬜ Pending | — | Provenance tracked |
| 7.3 | Dated signifier shifts as reviewable hypotheses | ⬜ Pending | — | Never silent facts |
| 7.4 | Scenario review + promotion + version history | ⬜ Pending | — | Reversible |

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

## Detailed Plans (Archive)

Historical planning documents have been consolidated here. For full detail on
any section, see `docs/archive/`:

| Document | Topics |
|----------|--------|
| `docs/archive/DEVELOPMENT_PLAN.md` | Semiotic editing, interface design, NodeView, pins as testimony |
| `docs/archive/DEVPLAN.md` | Industrial MVP, administration/datawork, decision planner, documentation |
| `docs/archive/LEAN_I40_PLAN.md` | OEE, predictive maintenance, SPC, A3, management dashboard, JIT inventory |
| `docs/archive/INDUSTRIAL_VERTICAL_PLAN.md` | BGE-M3 evaluation, MQTT adapter, shift report, telemetry upsell |
| `docs/archive/ONTOLOGY_EXPANSION_PLAN.md` | Semiotic ontology (Peirce/Eco), 70-cell grid, holonic graph, category theory |

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
