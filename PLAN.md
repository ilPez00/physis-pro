# Physis Pro Evolution & Monetization Strategy

## Goal
To evolve Physis Pro from an internal intelligence engine into a commercially viable, industry-standard "Industrial Backoffice" platform, and to define a clear path to monetization.

## Implementation Plan

### Phase 1: Technical Perfection (The "Intelligence" Upgrade)

#### 1A — Scanner & Ingestion Pipeline
- [x] **Scanner overhaul**: `WalkDir` directory traversal + `rayon::par_iter()` parallel file processing + streaming SHA256 (64KB buffer).
- [x] **Pipeline dedup**: Extracted 160-line duplicate Ingest/RebuildGraph code into `run_ingest_pipeline()`.
- [x] **Regex caching**: 4 `once_cell::sync::Lazy` statics replacing repeated `Regex::new()` calls.
- [ ] **Disk indexer**: `physis-pro index <mount-point> [--output F]` — generate a structured file catalog (tree depth, sizes, dates, type breakdown, largest files, total/human-readable). Outputs Markdown, JSON, or plain-text. Reuses scanner's `WalkDir` + `rayon` infra but skips hash/embedding (fast path). `--watch` mode: track changes across runs via a persisted manifest. Covers all mounted volumes in one invocation with `physis-pro index --all`.

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

#### 1F — GPU support (investigated 2026-08-12 — **the `rocm` feature is dead configuration**)

**Finding: ONNX Runtime removed the ROCm execution provider upstream.** ORT 1.24.2
(the version `ort` 2.0.0-rc.12 links) has no ROCm EP: `build.py` offers
`--use_migraphx` but no `--use_rocm`, and `onnxruntime_providers_migraphx.cmake`
is the only AMD provider file in the tree. The `ort` crate still exports a
`rocm` feature and a `ROCMExecutionProvider` type, so our `rocm` feature and the
`physis-pro-web-rocm` binary **compile and silently produce a CPU build**:

```
ERROR ort::ep] register `ROCMExecutionProvider`: ROCM execution provider is not enabled in this build.
WARN  ort::ep] No execution providers from session options registered successfully; may fall back to CPU.
```

Confirmed empirically: full suite under `--features rocm` ran `1451% cpu` (~14.5
cores) for 333s with `rocm-smi --showpids` reporting no KFD processes throughout.
Every run since the "enable ROCm ONNX prebuilt download" commit has been on CPU.

Ruled out along the way:
- **Host ROCm SDK build** — ORT's AMD EP needs MIOpen, which Debian does not
  package at all (`apt-cache search miopen` = 0 hits); the host stack is also an
  incoherent HIP 5.7 / rocBLAS 5.5 / rocm-cmake 6.3 mix.
- **AMD `onnxruntime-rocm` wheel** — only published at 1.22.x; `ort` rc.12 needs
  the 1.24 C API, so it cannot initialise against it.
- **Container build** (`rocm/dev-ubuntu-22.04`, ROCm 7.14, MIOpen present, both
  GPUs visible via `/dev/kfd`) — got as far as a clean ORT 1.24.2 checkout before
  hitting the upstream removal above. MIGraphX is not in that image's repos
  either (773 ROCm packages, none named migraphx).

Open options, none started:
- [ ] **MIGraphX EP** — needs AMD's separate `rocm/migraphx` image, an ORT build
  with `--use_migraphx`, and an EP-dispatch change in `src/embed/mod.rs` behind a
  new `migraphx` feature. Risk: unsupported ops silently partition back to CPU.
- [ ] **WebGPU EP** (`--use_webgpu`, Dawn/Vulkan) — vendor-neutral, needs no ROCm
  userspace, and `ort` rc.12 exposes a `webgpu` feature. Most portable option;
  op coverage for BERT-shaped graphs is unproven.
- [ ] **Decide the fate of the `rocm` feature** — it currently promises
  acceleration it cannot deliver. Either delete it with the
  `physis-pro-web-rocm` binary, or keep it strictly as the seam for a future EP.

**Payoff check before spending more on this:** the embedder serialises inference
behind a Mutex (`src/embed/mod.rs`), so a GPU cuts per-embed latency, not suite
throughput — realistic gain on the test suite is 333s → ~200s. Low priority
against the TUI browser and the industrial wedge.

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

#### 1G — TUI wiki browser (shipped 2026-08-12)
- [x] **WIKI tab in `physis-pro-tui`** — browse the UberWiki by *meaning*: describe
  what a thing was about, get cosine-ranked nodes back. Built as a fourth tab
  rather than the standalone binary in PHYSIS_PRO_USECASES.md §2, reusing the
  existing HTTP/event-loop scaffolding; extract it later if it outgrows the tab.
  Uses only endpoints that already existed (`/coherence/nodes`, `/reconstruct`).
  Each row shows the **asserted verdict** as a colour dot (green worked / red
  blocked / yellow inert / dim unjudged) with derived density as a number beside
  it — one colour cannot honestly carry both axes. Respects `PHYSIS_USER`, since
  coherence state is per-tenant and `_default` is empty when nodes were written
  under `aion` / `praxis`.
- [x] **Live re-rank** — results follow the query as you type, debounced on a
  350ms pause with a 3-char floor. Per-keystroke would cost an HTTP hop plus an
  ONNX embed per letter; a pause gives the same feel for one request.
- [ ] **Still open from the §2 spec**: semiotic
  heatmap pane with cell filtering, graph-walk (`g`), dream view (`d`).

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
