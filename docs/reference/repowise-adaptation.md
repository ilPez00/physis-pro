# Repowise → Physis-Pro Adaptation Compendium

Source: `/home/gio/dev/repowise` (repowise-dev/repowise, https://github.com/repowise-dev)
Audit date: 2026-08-24. Audit verdict: HIGH conceptual overlap, LOW mechanical reuse.

## License wall (READ FIRST)

Repowise is **AGPL-3.0-or-commercial**. Physis Pro is proprietary EULA
(Ed25519-gated, `src/license.rs`).

- **Never copy repowise code into physis-pro** — not even snippets, not even
  renamed. Doing so obligates open-sourcing physis-pro or buying their
  commercial license.
- Everything below is a **clean-room pattern extraction**: we read their
  module docstrings to understand the *design*, then re-implement the idea in
  Rust with our own idioms, names, and data shapes.
- Their Python deps (networkx, graspologic, structlog) do not transfer; Rust
  equivalents are trivial for the subset we need.

## What each side is

| | repowise | physis-pro |
|---|---|---|
| Domain | code-repo intelligence (AST+git graph) | semantic coherence engine (embeddings+verdicts) |
| Shape | index → graph → analyze → wiki/UI → MCP | ingest → holarchy → classify/cohere/dream → web/MCP |
| Lang | Python, AGPL | Rust, EULA |

Same architecture shape, different domain and language ⇒ port *patterns*, not code.

## Portable patterns (ranked)

### P1 — Wiki generation pipeline over the holarchy 🥇
Their stack: `generation/page_generator/` (681-line core + levels + orchestrate +
validation), `generation/concept_tree/` (planner/naming/grouping/vocabulary),
`generation/layers.py`. Today physis `wiki.rs` is BM25 search only; generated
prose over coherence nodes is the biggest missing demo/sales surface.

Key designs worth stealing (concept-level):
- **Two-pass outline planning** (`concept_tree/planner.py`): pass 1 = grouping
  (guarantees coverage deterministically), pass 2 = LLM naming ONLY. A single
  combined call dropped directories and invented paths at 9× the rate. Repair
  pass sees only what failed — never hand back the whole tree.
- **Bounded LLM naming** (`naming.py`): model gets opaque group ids, returns
  title+scope per id; forgotten ids keep deterministic names, invented ids are
  discarded. Coverage is invariant to LLM failure. Never rank input by
  centrality (hub files are plumbing — 12.7% vs 91.7% coverage).
- **Deterministic-first, LLM-decorates**: grouping/layers/tour are pure
  functions; the LLM only names and summarizes. Maps directly onto
  provider-cascade calls with cheap local fallback.
- Layer inference from dependency direction (fan-in/fan-out ordering) → for us:
  order holarchy layers by edge direction between label prefixes.

Rust shape: `src/wiki_gen.rs` behind feature `wiki-gen`; pure functions
(grouping, layering, tour) fully unit-tested; LLM steps take
`&dyn Fn(&str) -> anyhow::Result<String>` so tests inject canned responses.

### P2 — Session-transcript adapter registry
Their stack: `sessions/adapters/{base,registry,claude_code,codex}.py`.
Contract: adapter knows (1) where transcripts live (`discover`) and (2) how one
raw line becomes a normalized Event (`normalize`, returns None on unparseable —
never raises). Registry keyed by name, lookup per call, decorator registration.
Consumers ask by INTENT (`shell_calls` / `tool_calls` / `turns`), harness
vocabulary never leaks into miners. Cheap substring prefilter before JSON parse
(transcript lines run hundreds of KB).

For physis: extend `history.rs` chat-JSONL importer into a registry of
per-agent transcript formats (Hermes JSONL, Claude Code, Codex, OpenCode) →
normalized episodes → labeled core nodes with provenance. Feeds P3.

### P3 — Anti-hallucination grounding gate
Their stack: `analysis/decisions/gate.py` + `provenance.py`. The product
guarantee: any produced `decision`/`rationale`/`source_quote` that does not
substring- or token-match its claimed verbatim source span is DROPPED;
candidates with no surviving evidence are REJECTED. Duck-typed candidate shape
so extractor and generator both use it without cycles.

For physis: mandatory gate on EVERY LLM-produced node content (dream outputs,
decision mining, wiki prose). Store verification verdict on the node
(`cell_pin`/provenance already exist as attachment points). This is a
trust primitive for the whole engine, not just decisions.

### P4 — Decision extraction + embedding dedup
Their stack: `analysis/decisions/extractor.py` (multi-source mining),
`semantic_match.py`: decisions embedded into the SHARED vector store under a
`decision:` namespace — dedup reuses the store's cosine search instead of a
separate index, and decisions become searchable alongside regular content
("why Redis?" returns the decision record).

For physis: mine sessions (P2) for decisions → assert verdicts → dedup via the
existing embedder cascade against existing node vectors (label-prefix
namespace instead of page-id namespace). No new index needed — same trick maps 1:1.

### P5 — Community detection on the holarchy
Their stack: `analysis/communities.py`: Leiden→Louvain fallback, oversized-
community splitting (>30% of graph triggers second pass), cohesion scoring,
heuristic labeling from dominant segments. Plus `kg_curation.py` hard invariant:
**curation reads the graph but only writes its returned result — never mutates
the source graph** (regression-tested). Feature-flagged, defaults on.

For physis: label-propagation over coherence edges (no external crate needed,
~150 lines) → concept rollups for dream/report summaries + studio tab.
Adopt the read-only curation invariant verbatim as an engineering rule.

### P6 — KG curation review workflow
Their `kg_curation`: bounded dependency-ordered layers, capped ranked entry
points, canonical execution-flow tour, never-empty summaries, feature flag,
acceptance gate = cross-repo validation matrix. For physis: curation queue for
coherence nodes slots into the existing edition tab (`/api/v1/coherence/edit`
+ `/delete` already shipped).

## NOT portable (do not attempt)

- Code ingestion machinery (AST extractors, `ingestion/languages/specs`,
  resolvers) — wrong domain, huge, Python-only value.
- Health biomarkers, blast radius, test impact, coupling — git/code-specific;
  meaningless over semantic nodes.
- Their React dashboard (`packages/ui`, ~30 panels) — framework+domain
  mismatch. Steal UX layout ideas only; if we want a product-grade web UI,
  build thin React over OUR existing 135 `/api/v1/*` endpoints.
- Typed SQL graph-record persistence layering — sled + Spanner mirror already
  cover this; adds nothing.
- MCP server surface — `src/mcp.rs` already exists.

## Landing plan

Phase 9 in `PLAN.md` tracks execution. Order: P2 (adapters, pure Rust, TDD
friendly) → P3 (gate, small + high leverage) → P4 (decisions, depends on P2/P3)
→ P5 (communities) → P1 (wiki gen, biggest surface, lands last with UI).
