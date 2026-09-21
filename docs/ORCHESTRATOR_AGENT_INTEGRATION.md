# Orchestrator Agent Integration — physis-core

**Status:** Ready (no code changes required)
**Related:** ADR-0020 in opencode-orchestrator

## Overview

physis-core provides a neurosemantic layer via MCP. The opencode-orchestrator
integrates physis-core as a 5th agent (`PhysisAgent`) that Commander can spawn
for neurosemantic analysis tasks.

**No changes to physis-core are needed.** The MCP server is already
feature-complete behind the `mcp` feature flag.

## MCP Server

### Start Command

```
cargo build --release --features mcp,cli --bin physis-pro   # once
./target/release/physis-pro serve --stdio
```

### Required Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `PHYSIS_SCAN_ROOT` | Path confinement for file operations (`physis_pack`, `physis_find`, `physis_scan`, `physis_workflow`) | Required (set to project root) |
| `PHYSIS_DATA_DIR` | State directory for `.physis/` data (ontology, cache, memory, ledger) | Required (set to `.physis`) |
| `PHYSIS_ORACLE_MODEL` | LLM provider for synthesis (optional) | `openai/gpt-4o` |
| `PHYSIS_ALLOW_DEV_LICENCE` | Required by a build with no vendor key stamped in. The gate still verifies signatures, so a licence signed by a different key is rejected — mint one with `cd physis-dev-license && cargo run -q > <data-dir>/license.key`. Local development only: a dev build's private key is public. | unset |
| `PHYSIS_EMBEDDER` | `random-projection` forces the offline, **non-semantic** embedder. Fast plumbing, useless retrieval — measured, it returns the wrong file with higher confidence than the right one. | unset (ONNX cascade) |

### Portabilities

The MCP server path must be portable. Strategies:
1. Use `PHYSIS_CORE_PATH` environment variable to locate the physis-pro checkout
2. Resolve from workspace root via relative path
3. Make configurable in plugin options

## Tool Catalog (30 tools, verified live 2026-09-12)

### Health & Stats
- `physis_health` — server health, embedder status, classifier cell count
- `physis_stats` — mapper stats, goal count, dream count

### Classification & Routing
- `physis_classify` — classify text against 5×14 = 70 semiotic grid cells
- `physis_embed` — embed text into vector
- `physis_route` — classify + return cell-bound route actions

### Context Packing
- `physis_pack` — pack highest-value context for a query within a hard token budget

### Search & Discovery
- `physis_find` — search inside documents (BM25 + cosine hybrid)
- `physis_query` — query the ontology mapper
- `physis_wiki_search` — search physis wiki documentation (Tantivy BM25)
- `physis_scan` — scan directory, populate ontology trie
- `physis_discover` — ontology-gap discovery over a corpus

### Memory & Coherence
- `physis_remember` — register labeled memory into shared coherence graph
- `physis_recall` — semantic recall over remembered nodes + goals
- `physis_coherence_query` — typed facet query over live coherence graph
- `physis_themes` — recurring themes across the remembered graph

### Goals & PDCA
- `physis_goals_list` — list all current goals with progress
- `physis_goals_add` — add a new goal from a prompt
- `physis_goals_remove` — remove a goal by ID
- `physis_pdca_plan` — get PDCA plan (which goals need action)
- `physis_pdca_act` — execute a PDCA action on a goal
- `physis_pdca_stats` — get PDCA cycle statistics

### Dreams
- `physis_dream_generate` — generate novel dream vectors from goals
- `physis_dream_evaluate` — evaluate (grade) a dream by ID
- `physis_dream_advanced` — generate advanced dreams with schedule context

### Scheduling
- `physis_gantt_schedule` — manage Gantt schedule (add, recompute, status)
- `physis_gantt_cross_correlate` — cross-correlate PDCA progress with schedule

### Workflow & Analysis
- `physis_workflow` — workflow audit (documentation coverage, authors)
- `physis_why` — why-made chain (lineage, descendants, purposes)
- `physis_serve` — record artifact serves a goal

## Data Model

### Semiotic Grid

> **Corrected 2026-09-21.** This section described the **pre-rework** axis
> (5 domains × 14 modes = 70 cells). The mode axis was reworked on 2026-09-19 to
> **9 modes**, each owning one WordNet verb supersense, so the default pack is
> **5 × 9 = 45 cells** — and `PHYSIS_DOMAINS` replaces the domain list per
> deployment. `docs/WHAT_PHYSIS_IS.md` §2 is canonical; the unchanged mode list
> below is retained only as the historical record of this integration's
> assumptions.

5 HumanDomains × 14 HumanModes (pre-rework):

| Domain | Modes |
|--------|-------|
| HEAL | LIFT, REST, WALK, WORK, CREATE, LEARN, DESTROY, SENSE, GUIDE, PLAY, BRAINSTORM, MAINTAIN, MOVE, PLAN |
| CONSTRUCT | (same 14 modes) |
| FABRICATE | (same 14 modes) |
| BOND | (same 14 modes) |
| STUDY | (same 14 modes) |

### Core Types

- **PhysisCore** — knowledge graph with coherence nodes, hypotheses, edges
- **CellClassifier** — nearest-centroid classification against 70 cells
- **CoherenceGraph** — mutable graph of CoherenceNode + edges
- **OntologyLoader** — loads from `.physis/ontology.json` and built-in configs
- **CausalLedger** — append-only causality record (Space·Time·Cause addressing)
- **PDCActor** — PDCA (Plan-Do-Check-Act) cycle actor with goals
- **DreamEngine** — generates and evaluates dream vectors
- **GanttScheduler** — CPM schedule computation

## Integration Contract

### What the Orchestrator Provides
- Workspace directory as `PHYSIS_SCAN_ROOT`
- `.physis/` data directory path
- MCP stdio transport via `.mcp.json`

### What physis-core Provides
- Tool palette of 31 MCP tools accessible to PhysisAgent
- Semiotic classification over 70 cells
- Token-budgeted context packing (`physis_pack`)
- Verdict-aware recall (`physis_recall` with −1/0/+1 verdicts)
- Coherence graph query (`physis_coherence_query`)
- Goals + PDCA cycle management
- Dream generation for creative exploration

## Startup Sequence

`PhysisMcpServer::new(config)` performs:
1. Load all ontologies (`OntologyLoader::load_all`)
2. Build `OntologyMapper` with embedder
3. Create `PDCActor` for PDCA cycles
4. Initialize `DreamEngine`
5. Select embedder via `select_embedder(&config)` — respects `PHYSIS_ORACLE_MODEL`
6. Load/pack cache from data dir, build `CellClassifier`
7. Open sled database at `{data_dir}/state.sled` — degrades to session-only on failure
8. Rehydrate `PhysisCore` from default tenant nodes
9. Load `CausalLedger` for why-made tracing
10. Load route table from `{data_dir}/routes.json`

Startup timing is printed to stderr (not stdout — stdout is the MCP protocol).

## Verification

Test the MCP connection:
```bash
cd /home/gio/dev/physis-pro
PHYSIS_SCAN_ROOT=. PHYSIS_DATA_DIR=.physis cargo run --features mcp --bin physis-pro -- serve --stdio
```

Then send an MCP `initialize` + `tools/list` request. All 31 tools should appear.

Health check: call `physis_health` → returns `{"status":"ok","classifier_cells":70}`.

## ADR-0019 Alignment

ADR-0019 retired passive RAG injection. physis-core tools are **explicitly invoked** —
available in the agent's tool palette but only called when the agent requests them.
No automatic prompt injection occurs.
