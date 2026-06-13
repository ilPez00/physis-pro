# Physis Pro Evolution & Monetization Strategy

## Goal
To evolve Physis Pro from an internal intelligence engine into a commercially viable, industry-standard "Industrial Backoffice" platform, and to define a clear path to monetization.

## Implementation Plan

### Phase 1: Technical Perfection (The "Intelligence" Upgrade)

#### 1A — Scanner & Ingestion Pipeline
- [x] **Scanner overhaul**: `WalkDir` directory traversal + `rayon::par_iter()` parallel file processing + streaming SHA256 (64KB buffer).
- [x] **Pipeline dedup**: Extracted 160-line duplicate Ingest/RebuildGraph code into `run_ingest_pipeline()`.
- [x] **Regex caching**: 4 `once_cell::sync::Lazy` statics replacing repeated `Regex::new()` calls.

#### 1B — Error Handling & Code Quality
- [x] **Critical unwrap elimination**: `file_name().unwrap()` → `unwrap_or_default()` in `vfs/orchestrator.rs`.
- [x] **CI/CD pipeline**: Rust CI with `cargo check` (3 feature sets), `clippy` (deny warnings), `cargo test`.

#### 1C — Feature Completeness
- [x] **Inventory command**: Real sled DB access showing item count + first 20 key/value pairs.
- [x] **Process command**: TCP connectivity checks behind `#[cfg(feature = "industrial")]`.
- [ ] **Real-time Industrial Adapters**: Move from TCP pings to full MQTT/Modbus subscribe/publish loops feeding `HolonicGraph`.

#### 1D — Embeddings & Ontology
- [x] **Upgrade Embeddings**: BGE-base-en-v1.5 (768-d) ships as the production embedder, self-test-gated with MiniLM → RandomProjection fallback (see PREMORTEM P0/P4). Accuracy 0.974 domain / 0.923 cell on the curated set.
- [x] **Automated Ontology Discovery**: `discover` mode (`src/discovery.rs`) embeds a corpus, flags texts the grid covers poorly (best-entry cosine below an operator-tunable threshold), clusters the gaps, and emits a ready-to-edit ontology JSON draft. Surfaces the best-entry cosine distribution so the threshold is calibrated from real numbers (a fixed default is useless — a dense ontology scores ~0.64 against out-of-domain text). CLI `physis-pro discover <dir> [--threshold T] [--json-out F]`, HTTP `POST /api/v1/discover`; the legacy LLM-synthesis path is kept behind `--ai`.
- [x] **Config dedup**: the 10 copy-paste `load_builtin_*` functions collapsed into a `BUILTIN_ONTOLOGIES` table + one `load_builtin(name)` lookup.

#### 1E — Packaging
- [x] **`setup.py` + `install.sh`**: pip-installable packaging with binary installation.
- [x] **`monitor_resource.sh`**: cgroup-based CPU throttling for heat-throttled environments.

### Phase 1.5 — Session 2026-06-12/13 architecture changes

Shipped this arc (all on `main`, classify hot path untouched unless noted):
- [x] **P1 lock split finished** — last two handlers (`context_filter`,
  `maintenance_check`) embed off the global state lock (`spawn_blocking`).
- [x] **Config dedup** — 10 `load_builtin_*` fns → `BUILTIN_ONTOLOGIES` table.
- [x] **Spanner P7** — hard deletes + `sync_full` mirror sync (orphan pruning,
  edges-before-nodes).
- [x] **Embedder cascade hardened** — `embed::semantic_self_test` shared by web +
  CLI + tests; CLI was ungated (Jina fp16 noise → 0.06 verdicts) and used
  pre-P3 mean-centroid scoring. Both fixed; CLI prints embedder provenance.
- [x] **BGE-M3** — measured, **rejected** (CORE 0.923/0.769 vs bge-base
  1.000/0.949, Italian tie, 2.5× slower). Opt-in only. `PoolingStrategy` knob
  (mean|cls) + `token_type_ids` signature detection kept.
- [x] **Italian classification** — same-language ontology anchors lifted the
  Italian tier 0.733/0.600 → **0.867/0.867**, CORE unchanged. Gated at 0.80.
- [x] **Shift report** (`report.rs`) — Italian OEE/anomaly/RUL/countermeasure
  report, Markdown + self-contained HTML; `report --shift`, `POST/GET
  /api/v1/report/shift?format=html`. The sellable artifact.
- [x] **Ontology discovery** (`discovery.rs`) — embedding-gap detector with
  operator-tunable threshold + surfaced sim distribution (a dense ontology
  scores ~0.64 vs out-of-domain, so a fixed threshold is useless); clusters
  gaps → draft ontology JSON. CLI `discover`, `POST /api/v1/discover`; legacy
  LLM path behind `--ai`. Closed Phase 1D.
- [x] **Empedoclean dyad** (`empedocles.rs`, `docs/EMPEDOCLES_DYAD.md`) —
  RESEARCH module, P2-quarantined, wired to nothing. A node field under
  Eros (attraction + synthesis) vs Nemesis (repulsion + decay). The
  `empedocles_criticality` falsifier **confirmed a phase transition**: a
  critical band (heavy-tailed cluster sizes) sits between Love-collapse and
  Strife-scatter. Open: it is *tunable* criticality, not *self-organised* —
  coupling decay/synthesis back into the ratio to self-tune to the edge is the
  next experiment. Promotion gate: a mechanism enters the live coherence core
  only after it shows organising work AND a test proves no classify regression.

### Phase 2: User Experience (The "Backoffice" Polish)
- **Web-based Dashboard**: Build a React/Next.js dashboard (using the existing `web` feature) that visualizes the `FileThemeGraph` as an interactive force-directed graph.
- **Natural Language Interface**: Implement a chat-based interface (using the `ai` module) that allows users to query the graph in plain English (e.g., "Show me all invoices related to the Q4 budget").
- **Quality Dashboard**: Create a visual UI for the `QualityTracker` to show failure trends and ROI metrics for the business.

### Phase 3: Monetization Path (The "Business" Model)
- **Tiered Licensing**:
    - **Physis Core (Open Source)**: The engine and basic ontology.
    - **Physis Industrial (SaaS)**: Includes the web dashboard, industrial adapters, and automated quality feedback loops.
    - **Physis Enterprise (On-Prem)**: Dedicated support, custom ontology development, and air-gapped deployment for high-security manufacturing.
- **Industry-Specific "Ontology Packs"**: Sell pre-configured ontology packs (e.g., "Physis for Aerospace," "Physis for Food Processing") that include domain-specific machine health states and regulatory compliance checks.
- **ROI-Based Pricing**: Charge based on the number of "Quality Failures" prevented or the volume of inventory optimized, aligning the cost directly with the value provided.
