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
