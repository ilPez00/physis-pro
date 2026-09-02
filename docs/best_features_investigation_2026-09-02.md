# Physis Pro — Best Features Investigation (2026-09-02)

## Inspection Summary

SSH'd into pansa, inspected `/home/gio/dev/physis-pro/` (canonical repo location,
confirmed by skill physis-pro-dev). Running binary is `/home/gio/physis-runtime/physis-pro-web`
(PID 6135, started Sep 1 17:40, built Aug 30 — STALE vs source). BGE-base embedder
is live and healthy (`/api/v1/embedder` returns `kind:"bge-base", dim:768, semantic:true`).

## Top 5 Best Features (ranked by uniqueness + impact)

### 1. Semiotic Grid + QualityTracker Feedback Loop (Phase 19 B5)
**Where:** `physis-core/src/quality.rs`, `src/quality.rs`, `src/web/coherence.rs:566-585`
**Why it's best:** This is the single most unique feature. Pin disagreements between
human and classifier automatically penalize the auto-cell and boost the pinned cell
via `QualityTracker`. `PIN_DISSENT_SEVERITY` controls the penalty. The quality
dashboard (`/api/v1/quality`) returns per-failure records (embeddings stripped,
`DOMAIN/MODE` display cells, newest first) + aggregates. This is "ontology that
learns from correction" — a human pins "this is HEAL/SENSE, not FABRICATE/CREATE"
and the system recalibrates the cell centroid. No other open-source embedding engine
does this. Verified live: quality endpoint returns `{"penalties":{},"boosts":{},"failures":0}`.

### 2. Three-Axis Coherence (Derived + Asserted)
**Where:** `src/models.rs:176` (CoherenceNode), `src/models.rs:328` (NodeView)
**Why it's best:** Each node carries TWO coherence numbers. `coherence_score` (0..1,
derived density — mean cosine to 5 nearest neighbours) answers "how typical is this?"
`asserted` (-1..1, human-set) answers "did it work?" The `CoherenceRating` (Success/Inert/Failure)
is derived from `asserted`. The QA verdict confirms: "report --shift demo-shift.json"
produces Italian shift reports with OEE 78.5%, per-machine tables, loss Pareto,
anomaly 4.4σ, RUL ~2h10m — customer-ready output that scores coherence per machine.
This two-axis model (statistical + judgmental) is philosophically sound and
practically powerful.

### 3. Causal Ledger with Four Causes (Phase 10, D3)
**Where:** `src/ledger.rs`, `src/address.rs`
**Why it's best:** Every file carries a `FileAddress` with `SpaceRef` (where),
`TimeRef` (when), `CauseRef` (why-made). `CausalLedger` records `CauseEvent`
with monotonic seq numbers (deterministic, reproducible). `Mechanism` enum:
Generated/Received/Derived/Corrected. The `where` CLI and `POST /api/v1/files/query`
compose space·time·cause facets in plain language. Lineage traced via
`GET /api/v1/files/lineage?path=` — returns upstream chain + descendants + purposes.
This is Palantir Gotham's "provenance" but as a single `FileAddress` struct on
every node, not a bolt-on. Verified: 5 unit tests, e2e covers recording, all four
facets, tenant isolation.

### 4. Dream-Driven Planning (Compression/Dream Dyad)
**Where:** `src/dream.rs`, README "Core model" section
**Why it's best:** The `dream()` loop runs in the background, takes accumulated
Inert (0.0) and Failure (-1.0) nodes, simulates the missing causal link, and
emits corrective Effective Actions while recalibrating the global coherence index.
`POST /api/v1/dream/generate` correctly REFUSES while PDCA is active unless
`force:true`. Dreams return interpolate/extrapolate proposals with IDs for
evaluation. `POST /api/v1/dream/evaluate` grades them 0.0-1.0. The dream replay
(`/api/v1/dream/replay`) shows attention feeds on the Pulse dashboard. This is
active inference / predictive coding implemented as an engineering tool.

### 5. Atlas — Organic Graph Visualization
**Where:** `src/web/atlas.rs`, `src/bin/ui/atlas.html`
**Why it's best:** `GET /api/v1/atlas/graph` returns a single payload with nodes,
recorded holarchy edges (solid cyan, pulsing) + similarity edges (soft violet,
cosine >= 0.55), communities, and degrees. The browser draws it with a physics
simulation — "positions are a physical simulation, not a stored layout: strong
links pull nodes together, so distance on screen is a fact about the graph."
The object explorer (`openEntity()`) shows cell, verdict, links, lineage (if path),
goal-attachment, and "In documents" search. Communities detected via label
propagation (Phase 9.4, no new deps). This is the closest thing to a "living
ontology" UI — the graph is an organism, not a static diagram.

## Secondary standout features

- **Multi-modal embedding**: CLIP for images (text+image in 512-d shared space),
  Jina CLIP v2 (single ONNX model), BGE-base for text (768-d). All share the
  `VectorEmbed` trait. Voice daemon (whisper -> llama3.2 -> graph).
- **Hypotheses + Epistemic audit**: `POST /api/v1/hypotheses` with evidence
  tracking, `GET /api/v1/epistemic/audit` (who believed what, when),
  `GET /api/v1/coherence/query` (typed question enum: WhyIsBelieved...).
- **Structured output extraction**: `POST /api/v1/extract {text, schema}` —
  tools-first, JSON-fallback, explicit `note` on failure (never fabricates).
- **Corpus comprehension**: Phase 11 community summaries, doctree heading parsing
  (sections carry their ancestor heading chain as context), `physis-pro themes` CLI.
- **Agent surface**: 23 MCP tools, declarative route table (Phase 19 B1),
  `POST /api/v1/agent/run` SSE streaming, quality-failure trace
  (`GET /api/v1/trace?failure_id=`).

## Key finding for Phase 20 (node viz citations)

The citation/fragment machinery ALREADY EXISTS in `src/content.rs` (`IndexedSection::citation()`,
`ContentHit.citation`, `snippet`, `highlights: Vec<[usize; 2]>`) and `src/doctree.rs`
(`Section::path()` returns "Heading1 > Heading2 > Heading3"). But it is NOT
attached to ontology nodes. `NodeView` has no `source_fragment` or `instances`
field. The plan correctly targets the `observable()` exhaustively-destructure to
force every surface to adopt the new fields.

## Key finding for Phase 21 (multimodal ingestion)

- OCR: completely absent. `pdf-extract` only reads text layers.
- Audio: Whisper transcription exists in `ai/provider.rs` (`ProviderCascade::transcribe`)
  but is NOT wired into `ingest.rs`.
- Images: CLIP embedder exists but `embed-onnx` feature is BROKEN (ort 0.13 API drift).
- Video: explicitly planned, not built.
- Docx/pptx: handled in `interest.rs` (daemon feature), not in core ingest.
