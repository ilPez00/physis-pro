# Physis Engine — Development Plan

## Overview

Physis is moving from a single-threaded ontological mapper into a **holarchic runtime**
with real-time graph processing, semantic classification, temporal scheduling, and
distributed dream synchronization.

---

## Recommendation: lift accuracy past the MiniLM ceiling

Measured state (nearest-entry scoring, de-hubbed ontology, 2026-06-04):
**CORE 0.872 domain / 0.795 cell**, **HARD 0.444** — on `all-MiniLM-L6-v2`
(384-d), the only model that loads. The remaining errors are two distinct
problems with two distinct fixes; do them in this order.

### ✅ RESULT (2026-06-04): BGE-base swapped in — 0.974 domain / 0.923 cell

Downloaded the models and measured. **BGE-base-en-v1.5 (768-d)** lifts CORE
top-1 from MiniLM's 0.872/0.821 to **0.974 domain / 0.923 cell**, and the HARD
tier (abstract/empty-cell concepts) from 0.444 to **1.000** — past the 0.85
target. It is now the engine's live embedder (self-test-gated selection).

**Jina is downloaded but unusable as-is:** fp16 *loads* (after disabling ort
graph optimization, commit 6a9615f) but outputs **float16** tensors that
`jina.rs` extracts as f32 → silently returns the random fallback (0.154, below
chance). The semantic self-test catches it and the selection skips to BGE.
To use Jina: either handle f16 outputs in `jina.rs` (extract as `half::f16` →
convert), or fetch the fp32 `model.onnx` + `model.onnx_data` (3.4GB external).
Deferred — BGE already clears the target; Jina's only added value is image
embedding (CLIP path).

### 1. Swap the text embedder (biggest lift, least work) 🔴 — done, see above

`all-MiniLM-L6-v2` is a 2021-era 384-d model near the bottom of MTEB. The hard
tier and the abstract core misses ("blog" vs "legacy", "incubate", "dough rest")
are its semantic ceiling, not an ontology bug. **Jina is the intended primary but
its `model_q4f16.onnx` won't load under `ort 2.0.0-rc.12`** (4-bit quant trips the
graph optimizer — P4). Two clean paths:

- **Fastest:** obtain Jina CLIP v2 as **`model_fp16.onnx`** (or full `model.onnx`)
  instead of the q4f16 quant — the loader already prefers those filenames
  (`jina.rs`). Same 1024-d multilingual text+image space, no code change.
- **Strong text-only alternative that loads cleanly in `ort` today:**
  `BAAI/bge-base-en-v1.5` (768-d) or `Alibaba-NLP/gte-base-en-v1.5` — both are
  small, ONNX-exportable, and far above MiniLM on MTEB semantic-similarity.
  Wire as another `EmbedderKind`; centroids/classifier are dimension-agnostic.
  (Loses native image embedding — keep CLIP/Jina for the image path.)

Expected: CORE → 0.92+, HARD materially up. Re-run the floor to confirm, then
raise it.

### 2. Nested (hierarchical) classification — ✅ done (model-agnostic)

Flat 30-way scoring conflates same-domain modes (BOND/CREATE vs BOND/LEARN).
`classify::CellClassifier` now blends a domain prior:
`final = ALPHA·domain_score + (1-ALPHA)·cell_score`, `ALPHA=0.4`, where
`domain_score` is the top-2 mean of the domain's **per-cell** scores.

**Implementation note (two dead ends first):** pooling *all* of a domain's entry
cosines for `domain_score` made it *worse* (0.846) — `max` over more entries
favours entry-rich domains (BOND), a sampling bias. Using the domain's per-cell
scores instead is unbiased. Result: domain top-1 unchanged (0.872), **cell top-1
0.795 → 0.821** — the mode-disambiguation win, no domain regression. The gain is
small on MiniLM because the embedder, not the structure, is the ceiling; expect
more once #1 lands. Floors now gate both domain (≥0.80) and cell (≥0.78).

### 3. Densify toward 25/cell (ongoing)

Mean is ~9 entries/cell. Add domain-specific entries (human + machine + natural)
per Phase 6F so each cell's nearest-entry has real coverage. Highest value on the
sparse cells that back the hard tier.

**Net recommendation:** ship #1 (fp16 Jina, or bge/gte for text) first — it is the
single highest-leverage change and unblocks the hard tier. Then #2 for structural
robustness, then #3 as continuous improvement.

---

## Phase 0: Hardening (premortem-driven — TOP PRIORITY) 🔴

See `PREMORTEM.md`. These gate everything below: until the engine can prove it
loaded a real model, no roadmap metric is trustworthy.

| # | Risk | Fix | Status |
|---|------|-----|--------|
| P0 | Silent embedder degradation (random projection in prod, no signal) | Loud startup banner + `degraded` flag + `/api/v1/embedder` endpoint; runtime model download from `MODEL_BASE_URL`; drop dead `COPY models` | ✅ Done |
| P1 | Global `Mutex<AppState>` serializes server; lock poisoning | Split read-only embedder/ontology into `Arc<SharedCtx>`; poison-tolerant `AppCtx::lock()` | ✅ Done |
| P6 | Unauthenticated public API + open LLM relay + `/scan` path traversal | `PHYSIS_API_TOKEN` Bearer guard (constant-time), fail-closed on non-loopback bind without token, `/scan` confined to `PHYSIS_SCAN_ROOT` | ✅ Done |
| P3 | No accuracy measurement | `tests/classification.rs`: always-on pipeline invariants (RP, CI) + `#[ignore]` top-1 domain-accuracy floor (real model). Classify logic extracted to lib `classify::CellClassifier` (dedups web_server, testable) | ✅ Done |
| P4 | `ort` rc / opset drift breaks inference at runtime | Pin `ort` exactly; log the real load error; startup semantic self-test marks a loaded-but-broken model degraded | ✅ Done |

**P3 finding + fix (2026-06-04):** first measurement exposed a **scoring bug**, not just a weak model. Mean-centroid scoring gave MiniLM only **0.556 domain / 0.333 cell** with a heavy BOND over-prediction — the mean of a densely-populated cell drifts toward the embedder's common (hub) direction and wins regardless of query. Switching to **nearest-entry scoring** (cell score = mean of its top-2 member-entry cosines, `classify.rs`) lifted the *same* MiniLM to **0.852 domain / 0.741 cell** — at the 0.85 plan target. (Mean-centering the centroids was tried first and made it *worse*, 0.481 — confirming the hub is real, not a fixed offset.) Floor raised 0.45 → **0.75**. Jina (`model_q4f16.onnx`) still won't load under rc.12 (→ P4); expected higher once it does.

**Densification (Phase 6F, 2026-06-04):** added `config/grid_fill_ontology.json` — the 4 empty cells (CONSTRUCT/REST, FABRICATE/REST, FABRICATE/LEARN, STUDY/REST) and 8 thin cells now populated; **all 30 grid cells are reachable** classification targets. Fixtures split into **core** (27 clear cases, gated at 0.75 — still 0.852 domain / 0.741 cell, no regression) and **hard** (9 abstract/empty-cell concepts, informational — MiniLM hits only 0.333; they need a stronger model). Known residual: CONSTRUCT/CREATE is a hub (its "Impact & Legacy" entry has generic write/build/create/launch hints) that absorbs many misses — tighten when revisiting, or let Jina resolve. Net: coverage complete; quality past 0.85 on hard cells awaits Jina (P4).
| P4 | `ort` rc / opset drift breaks inference at runtime | Pin `ort`, startup embed self-test on vector norm/spread | ⬜ Queued |

**Discipline (P2):** freeze new holons (gantt, MCP mesh, graph-dreams, industrial
adapters, SigLIP/ImageBind/Gemma) until the text+image classify path is measured
and the P0/P1/P6 launch-blockers are closed.

---

## Phase 1: Holarchy & Real-time Graph (Current — In Progress)

### Graph Engine (`graph.rs`)
- `DenseSlotMap<NodeKey, NodePayload>` with `Pod`-compatible zero-copy types
- `Edge` with semantic type and weight — `MmappedStorage` for persist
- ONNX worker thread processes graph nodes asynchronously via `crossbeam_channel`
- `RawNodeKey` bridges graph internals with serialization boundaries

**Status**: DenseSlotMap, Pod types, Edge, NodePayload, MmappedStorage, ONNX worker channel — done.
**Next**: Graph traversal (BFS/DFS), subgraph extraction, causal path search.

### Rachmaninov Holon (`rachmaninov.rs`)
- PDCA-driven directive engine: `Focus`, `Expand`, `Prune`, `Synthesize` directives
- `tick(state_vector)` compares state vs goal vector, emits directives on drift
- Designed as a control holon that reads graph state and writes directives back

**Status**: Core tick cycle, directive types, similarity threshold gating — done.
**Next**: Wire tick output into graph mutation pipeline, add directive priority sorting.

### Sensory Pipeline (`sensory.rs`)
- Raw sensor payloads as `Pod` structs — `SensorPayload` with timestamp + f32x4 vector
- Bridge between real-world input (filesystem, network, audio) and graph node space
- Packed binary format for zero-copy mmap storage

**Status**: Data types, serialization — done.
**Next**: Ingestion from audio/network sources, real-time node creation on sensor fire.

### ONNX Worker (`ai/onnx_worker.rs`)
- Background thread pool processing embedding requests
- Request/response channel pattern — non-blocking for the web server
- Pool of active `NdTensor` views for batch inference

**Status**: Thread pool, channel plumbing, ndarray shape fix (3D → flat index) — done.
**Next**: Batch queue with priority, streaming inference, GPU fallback detection.

---

## Phase 2: Semantic Embedding & Classification

### ONNX MiniLM (done)
- Production-grade 384-d embeddings via `ort` runtime + `tokenizers` crate
- Falls back to deterministic `RandomProjectionEmbedder` when model absent
- Enabled with `--features embed-onnx`

### Query Classification (done)
- `POST /api/v1/classify` — embed query text → cosine sim against per-cell centroids
- Centroids pre-computed at startup from all 511 ontology entries (name + hints averaged per DOMAIN×MODE cell)
- Returns sorted `ClassifyResult[]` with domain, mode, score, matched ontology entries

---

## Phase 3: Gantt & Temporal Scheduling

### Gantt Engine (`gantt.rs`)
- `GanttTask` with start/end, dependencies, resource allocation
- `RawNodeKey` integration — tasks reference graph nodes
- Critical path analysis and resource leveling

**Status**: Data structures — done (stub).
**Next**: Causal-aware scheduling (failures push dates, successes pull them in), temporal coherence scoring, Gantt → Mermaid renderer.

---

## Phase 4: Distributed Physis

### MCP Mesh
- Global UberWiki sync between instances via MCP
- `physis_ingest_prompt` → local goal → remote broadcast
- Dream pattern sharing across instances

**Status**: MCP server/client stubs — done.
**Next**: Conflict resolution for concurrent writes, partial sync (subscribe to branches), latency-tolerant dream merging.

---

## Phase 5: Advanced Dreaming

### Graph-based Dreams
- Dream generation traverses graph edges instead of linear vector interpolation
- ONNXWorker-assisted dream evaluation (semantic plausibility scoring)
- Dream → subgraph extraction → directive queue

**Status**: Linear stochastic dreams (mutation/graft/prune) — done.
**Next**: Graph-aware mutation (prune failed subgraphs, expand coherent ones), predictive coherence simulation, cross-instance dream comparison.

---

## Phase 6: Multi-Modal Embedding & Ontology Expansion 🔄 IN PROGRESS

This phase is the answer to: *"What if we could classify images, audio, processes — not just text? And what if the ontology was deep enough to distinguish a welding robot from a chef?"*

**Modality status (2026-06-05):**
- **Voice** ✅ — `physis-pro-voice` bin folds in the standalone `physis_voice`
  repo (arecord → whisper-large-v3 → llama3.2 → `OntologicalMap` graph UI). Now
  on the shared cascade + ontology + Dream Function.
- **Images** ✅ — `embed_image()` (CLIP / Jina v2); `/classify` + `/embed` take
  base64 `image`, text-fallback on failure. Shared vector space with text.
- **Video** ✅ — `src/video.rs`: ffmpeg frame-sample → `embed_image` per frame →
  temporal mean-pool + L2-normalize → one clip vector. `POST /api/v1/embed/video`
  (path or base64; `fps`/`max_frames`); blocking thread; fail-soft `422` when
  ffmpeg/image-embedder absent. Follow-ups: attention pool, `/classify/video`,
  shot-boundary sampling. Audio track via the voice pipeline.
- **Sensory bus** ✅ — `AuraFrame` / `SignalType` (`sensory.rs`): `AudioPCM`,
  `VisionFeature`, `Acceleration`, `Gyroscope`, `ThoughtCapture` over a Unix
  socket into the holarchy.

### 6A. Multi-Embedder Architecture

Replace the single `Box<dyn VectorEmbed>` with a **trait-typed registry** that dispatches by modality and model type.

**Status**: ✅ Done — `EmbedderRegistry`, `EmbedderKindConfig`, `EmbeddersConfig` in `config.rs`; fallback chain primary → Jina → CLIP → MiniLM → RP in both `web_server.rs` and `cli.rs`.

**Design**:

```rust
/// Which model to use for a given modality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmbedderKind {
    RandomProjection,
    MiniLM,
    CLIP,
    JinaV2,
    SigLIP,
    ImageBind,   // Python sidecar; ONNX unavailable
}

/// What kind of input we're embedding
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modality {
    Text,
    Image,
    Audio,
    /// Structured process graph → vector
    Process,
}

/// Registry holds one embedder per modality, each with its own dimension
pub struct EmbedderRegistry {
    pub text: Box<dyn VectorEmbed>,
    pub image: Option<Box<dyn VectorEmbed>>,
    pub audio: Option<Box<dyn VectorEmbed>>,
    pub config: EmbedderConfig,
}
```

**Config** (`PhysisConfig` addition):

```json
{
  "embedders": {
    "text": { "kind": "jina-v2", "model_dir": "./models/jina-v2" },
    "image": { "kind": "clip", "model_dir": "./models/clip-vision" },
    "audio": null
  }
}
```

**At startup**: each configured embedder loads independently. If model files are missing, the embedder is `None` and falls back gracefully. Centroids are computed once per text embedder (for text queries). Image queries skip centroids and go direct to generative classification or use CLIP's shared text-image space.

**Key files**: `src/embed/registry.rs` (new), `src/config.rs` (extend), `src/bin/web_server.rs` (use registry).

### 6B. CLIP Implementation (OpenAI) ✅ Done

**Status**: ✅ `src/embed/clip.rs` implements `ClipEmbedder` with separate ONNX sessions for text and vision, L2-normalized 512-d output, uses Xenova pre-exported models from HuggingFace.

**Download** (user-side, one command):

```bash
# CLIP models from HuggingFace ONNX export
pip install optimum onnx
optimum-cli export onnx --model openai/clip-vit-base-patch32 ./models/clip-vit-base-patch32
```

Produces:
- `models/clip-vit-base-patch32/text_model.onnx` — text encoder (512-d)
- `models/clip-vit-base-patch32/vision_model.onnx` — vision encoder (512-d)
- `models/clip-vit-base-patch32/tokenizer.json`

**Rust implementation** (`src/embed/clip.rs`):

```rust
pub struct ClipEmbedder {
    text_session: Mutex<Session>,
    vision_session: Mutex<Session>,
    tokenizer: tokenizers::Tokenizer,
    dim: usize, // 512
}

impl ClipEmbedder {
    pub fn embed_text(&self, text: &str) -> Vec<f32> { ... }
    pub fn embed_image(&self, bytes: &[u8]) -> Vec<f32> {
        // decode JPEG/PNG → RGB f32 tensor
        // resize to 224×224
        // normalize with CLIP mean/std
        // run vision_model.onnx
        // L2-normalize output
    }
}
```

**Image pipeline**: The `image` crate is already a dependency. Decode → resize with `image::DynamicImage::resize_exact(224, 224, ...)` → normalize per CLIP's ImageNet stats (mean=[0.481, 0.457, 0.408], std=[0.268, 0.261, 0.275]) → f32 tensor → ONNX session → 512-d vector.

**CLIP's superpower**: text and image live in the same vector space. You can classify an image without any image-specific centroid — just embed the photo and compare against the same text centroids. A photo of a gym gets HEAL×LIFT 0.91 automatically.

### 6C. Jina CLIP v2 Implementation ✅ Done

**Status**: ✅ `src/embed/jina.rs` implements `JinaEmbedder` with fused ONNX session for text+vision, pre-normalized 1024-d outputs, dummy tensors for single-modality runs.

**Download**:

```bash
# Jina CLIP v2 — 1024-d, multilingual, open weights
optimum-cli export onnx --model jinaai/jina-clip-v2 ./models/jina-clip-v2
```

**Why Jina v2 over CLIP**:
- 1024-d vs 512-d — richer representation
- Multilingual (handles Italian prompts natively)
- Beats CLIP on zero-shot classification benchmarks
- Same text+image shared space architecture

**Rust implementation** (`src/embed/jina.rs`): mirrors CLIP but with 1024-d output and Jina's tokenizer config.

### 6D. SigLIP (Google) — Optional

SigLIP uses a sigmoid loss instead of CLIP's contrastive, yielding better separation. ONNX export same pattern. 768-d. Drop-in replacement for CLIP.

### 6E. ImageBind (Meta) — Optional Python Sidecar

ImageBind embeds **6 modalities** into one space: text, image, audio, depth, thermal, IMU. No ONNX export exists; runs in Python.

**Sidecar design**: lightweight Python process (FastAPI or stdio) that:
- Listens on a local port or stdin
- Accepts base64-encoded bytes + modality label
- Returns 1024-d vector
- Physis calls it as a subprocess or HTTP sidecar

**Worth it only if you need audio embedding natively** (no Whisper STT). For audio, the simpler path is Whisper STT → text embed, which avoids the sidecar.

### 6F. Ontology Expansion — 5 Domains × 6 Modes, Deep

The current 511 entries cover 22 populated cells. Goal: fill all 30 cells with **at least 30 entries each** (900+ total), for both human and machine, with rich multi-lingual hints.

**Strategy**: expand each domain×mode cell by adding new entries derived from:
- **Human activities**: sports, crafts, professions, relationships, learning methods
- **Machine operations**: sensors, actuators, control loops, manufacturing, transport
- **Cross-domain**: activities that span human+machine (e.g. "welding" appears in both human FABRICATE×WORK and machine FABRICATE×WORK)

**Example expansion for HEAL×LIFT** (currently 1 entry: Body & Fitness):

| Entry | Hints | Kind |
|-------|-------|------|
| Body & Fitness | gym, lift, workout, run, yoga | human |
| Physical Therapy | rehab, physio, stretch, mobility | human |
| Heavy Lifting | crane, hoist, jack, forklift | machine |
| Structural Load | beam, column, load-bearing, stress-test | machine |
| Immune Response | antibody, lymphocyte, inflammation, fever | natural |
| Muscular Exertion | sprint, jump, throw, push, pull | human |
| Hydraulic Pressure | pump, cylinder, psi, actuator | machine |

**Each cell gets this treatment**: systematic domain analysis to find 20-40 distinct entries covering human, machine, natural, abstract, social, engineering, agent, semiotic, and category kinds.

**Impact**: centroids become denser, classification becomes sharper — a cooking photo won't accidentally score high on HEAL×LIFT because that cell now has 30 specific entries pulling its centroid toward actual lifting activities.

### 6G. Gemma as Ontology Mapper

Once the embedder architecture and expanded ontology are in place, Gemma enters as a **re-ranking / generative mapping layer** above the vector classification.

**Architecture**:

```
User input (text/image/audio)
       │
       ▼
┌──────────────────────────────┐
│  Fast Path: Vector Classify  │  ← 5ms, all 30 cells scored
│  (CLIP/Jina/MiniLM centroids)│
└──────────┬───────────────────┘
           │
     confidence > 0.75?
      ┌────┴────┐
      yes       no
      │         │
      ▼         ▼
   return    ┌──────────────────────────────┐
   result    │  Slow Path: Gemma Re-rank    │  ← 2-5s, top-3 cells
             │  "Given text X, which of     │     reasoned classification
             │   these 3 DOMAIN×MODE cells  │
             │   best fits? Explain why."   │
             └──────────┬───────────────────┘
                        ▼
                   return result
```

**Gemma as pure ontology mapper** (no centroids at all): remove the vector path entirely and prompt:

```
Classify the following text into one of these DOMAIN×MODE pairs.
Consider both the domain (HEAL, CONSTRUCT, FABRICATE, BOND, STUDY)
and the mode (LIFT, REST, WALK, WORK, CREATE, LEARN).

Text: "chopping vegetables for a hearty soup"
→ FABRICATE×CREATE (Culinary Arts, Cooking & Meal Prep)  — confidence: 0.94
→ HEAL×LIFT (Nutrition & Diet)                           — confidence: 0.82
→ ...
```

**Advantages of Gemma mapping**:
- No centroids to pre-compute or maintain
- Can reason about novel or ambiguous cases
- Understands context ("fixing a leaky pipe" = CONSTRUCT×WORK, not FABRICATE×WALK)
- Multimodal natively (Gemma 4 Vision)

**Disadvantages**:
- 2-5s per query on GPU (vs 5ms for vector)
- Can't be embedded in vector-space operations (dreams, PDCA, coherence scoring)
- Higher hardware requirements (8GB+ VRAM for 4B, 20GB+ for 9B)
- Temperature sensitivity — same input can map differently across runs

**Hybrid recommendation**: use vector centroids as the primary path (fast, deterministic, vector-compatible), and run Gemma as an optional **re-ranker for edge cases** and an **ontology expansion assistant** (Gemma suggests new entries and hints for cells).

### 6H. Implementation Order

| Step | What | Depends On | Effort | Status |
|------|------|-----------|--------|--------|
| 1 | `EmbedderRegistry` struct + config | — | 1 day | ✅ Done |
| 2 | CLIP text embedder (`src/embed/clip.rs`) | Steps 1 | 2 days | ✅ Done |
| 3 | CLIP vision embedder | Steps 2 | 2 days | ✅ Done |
| 4 | Jina v2 text+vision embedder (`src/embed/jina.rs`) | Steps 1 | 2 days | ✅ Done |
| 5 | SigLIP embedder (drop-in) | Steps 1 | 1 day | ⬜ Pending |
| 6 | ImageBind Python sidecar (optional) | Steps 1 | 2 days | ⬜ Pending |
| 7 | Ontology expansion — fill 30 cells to 30+ entries each | — | 3 days | 🔄 Cells: 0 empty (was 4). `config/grid_fill_ontology.json` fills empties + thin cells; all 30 reachable. Mean ~9/cell, target 25 |
| 8 | Multi-modal classify endpoint — accepts text + base64 image | Steps 3, 4 | 1 day | ✅ Done |
| 9 | Gemma re-ranker — confidence-gated generative path | Steps 8 | 2 days | ⬜ Pending |
| 10 | Process embedding — task graph → vector via Gemma structural summarization | Steps 9 | 2 days | ⬜ Pending |

### 6I. Key Decisions

- **All ONNX embedders share the `ort` runtime** — no additional C++ deps. The `image` crate (already in Cargo.toml) handles decode/resize.
- **Dimension mismatch** is handled by projecting to the embedder's native dim at embed time, and pooling centroids to the query embedder's dim at classify time. This means you can mix MiniLM centroids (384-d) with CLIP queries (512-d) via a learned linear projection or simple mean pooling.
- **Jina v2 is the recommended primary embedder** for both text and image: best accuracy, multilingual, open weights, ONNX-exportable.
- **CLIP is the recommended fallback**: smaller, faster, battle-tested.
- **Gemma is not an embedder replacement** — it's a complementary reasoning layer. The vector space stays the source of truth for PDCA, dreaming, and coherence.

---

## Phase 7: Content Recall — Ingestion & TUI Wiki Browser 🔜 NEXT

Goal: make the UberWiki **fillable without ceremony** and **browsable by content**
when you don't remember a thing's name or type. Leans entirely on shipped
primitives (shared vector space, `classify`, `reconstruct`, holarchy, coherence).
Full rationale + UX in `PHYSIS_PRO_USECASES.md`.

### Status (2026-06-05)

- ✅ 7A TUI Wiki Browser shipped (`src/bin/wiki.rs`): live-as-you-type re-rank,
  coherence-colored results, preview, F2 heatmap / F3 dreams overlays.
- ✅ Node labels: `CoherenceNode.label` retains source text; `coherence/register`
  + `register_node_from_text` keep it; flows through `Neighbor` → API → TUI.
- ✅ 7B scan-labelling: register scanned files as labeled core nodes using the
  **shared** embedder so they're recallable in the wiki.
- ✅ Structural-line nodes: `map_filesystem_labeled` now also emits one label per
  structural line (`path → fn/struct …`), so a scan registers fine-grained nodes
  and you can recall an inner function/struct by content, not just the file.
  (Cost: a scan embeds file + all structural-line labels with the shared embedder;
  acceptable for a one-shot scan, revisit a cap if scans get huge.)
- ✅ 7A.1 background-task the live search: `dispatch_search` spawns a tokio task
  per query, seq-tagged results return over an mpsc channel, `run` applies only
  the newest seq (stale keystroke responses dropped). UI no longer blocks on the
  reconstruct+classify round-trip — kills remote-engine lag.
- ✅ Scan dedup: `register_node_vec_labeled` keys a `label_index` (label → id);
  re-registering a seen label returns the existing id, so re-scanning a dir is
  idempotent instead of duplicating every file/line node. Unlabeled registrations
  are never deduped. In-memory index (core isn't persisted), O(1) per insert.
- ✅ Watch mode (auto-feed): `PHYSIS_WATCH_DIR` (confined to scan_root) is polled
  every `PHYSIS_WATCH_INTERVAL_SECS` (default 30) by a std::thread; new/changed
  files (hash-diffed via `NetworkScanner`) are registered as labeled core nodes
  with the shared embedder. Embeds outside the lock; dedup keeps repeated polls
  cheap. The wiki stays fed without a manual `/scan`.
- ✅ Praxis backfill (praxis_webapp repo): `scripts/backfill-physis-wiki.ts`
  pages `notebook_entries` → `/coherence/register` per user tenant, so the wiki
  recalls a user's whole journal. Also fixed `registerCoherence` to send the text
  as `input` (was sent only as `label`, which the engine ignored → coherence
  nodes embedded empty strings; repaired goal/checkin registration too).
- ✅ Id bridge: `core::IdBridge` joins coherence id ↔ holarchy `RawNodeKey`.
  `bridge_node(coh_id)` creates+links a holarchy node (idempotent);
  `resolved_edges()` maps holarchy edges back to coherence ids. Unblocks edge
  export, durable shared graph, and a label-walkable TUI graph. Tested.
- 🟡 7C Spanner Graph sink — **scaffold only** (`src/spanner.rs`, `--features
  spanner`, default off). `export_nodes` + (now) `export_edges` via the bridge →
  Spanner `insertOrUpdate`. Compiles + unit-tested; NOT run against live Spanner.
  Blockers before prod in PREMORTEM P7: untested REST flow, env-token auth (no
  refresh), no mutation chunking, no deletes, no `WikiEdge` upsert yet.
- 📋 New machines: build plan in `MACHINES_BUILD_PLAN.md` (chronos, techne, soma,
  elenchus, lex, harmony, polis) on the shared core + id bridge.
- ⬜ Vault / personal-history importers (Obsidian, browser, email…).

### 7A. TUI Wiki Browser (`physis-pro-wiki`) — ✅ shipped (no engine change)

A new `ratatui` binary (dep already present) that drives the HTTP API over
`reqwest`, so it works against a local or remote engine.

- **Semantic palette** — type remembered *content*; embed the query and live
  re-rank by cosine. Recall without name/type.
- **Panes** — cosine-ranked results (DOMAIN×MODE chip + coherence color) · preview
  (content + neighbors + `reconstruct` LLM interpretation) · 30-cell heatmap to
  scope by cell.
- **Graph walk** (`g`) — traverse holarchy edges like a file browser (vim keys).
- **Dream** (`d`) — show corrective actions for an Inert/Failure node.
- Endpoints (all exist): `/classify`, `/query`, `/reconstruct`,
  `/holarchy/snapshot`, `/holarchy/graph`, `/semiotic/heatmap`, `/stats`.
  Bearer auth + `X-Physis-User` tenant.
- New bin: `[[bin]] name = "physis-pro-wiki"`, `required-features = ["tui"]`.

### 7B. Ingestion connectors — keep the wiki fed

Each connector is thin: `source → embed (shared embedder) → register labeled node`.

- **Scan-labelling** (🔜 this iteration) — `/api/v1/scan` mapped files into
  `mapper`/`s.goals` only (RandomProjection-embedded, outside the reconstruct
  index → unsearchable). Now also register each file as a labeled core node via
  `mapper.map_filesystem_labeled()` + the **shared** embedder, embedded outside
  the lock, so scanned data is recallable by content in the wiki.
- **Watch mode** — `network`/`notify`: auto-ingest on file create/modify.
- **Vault importers** — Obsidian/markdown, plain notes, `git log`, code repos.
- **Personal-history importers** — browser history+bookmarks, email/chat exports,
  RSS/read-later, photo library (EXIF + CLIP embed).
- **Praxis backfill** — `notebook_entries` + trackers → `register_behavioural_vector`
  (Success/Inert/Failure) to seed coherence from the real life-log.

> Embedder consistency rule: nodes must be embedded with the **same** embedder
> that embeds queries (the shared `SharedCtx.embedder`), or cosine recall is
> meaningless. `mapper`'s internal RandomProjection embedder is for trie tokens
> only — never register its vectors into the searchable core index.

### 7C. Spanner Graph sink — durable, multi-device memory

Persist holarchy nodes/edges into Google **Spanner Graph** (property graph over
Spanner SQL). physis = local reasoning; Spanner Graph = durable, GQL-queryable,
cross-device memory. Optional vector column for server-side ANN. Re-hydrate a
local tenant from Spanner on a new device.

**Order:** 7A (browser) → 7B (auto-fill) → Praxis backfill → 7C (durability).
7A/7B need no engine changes; 7C is a new optional sink module.

---

## Milestone Dependencies

```
Phase 1 ──┬──> Phase 2 ──> Phase 3
           │        │
           │        └──> Phase 7 (Content recall: TUI + ingest + Spanner sink)
           └──> Phase 4 ──> Phase 5
                              │
                              ▼
                          Phase 6 (Multi-modal)
```

Phase 6 is mostly independent of Phases 1-5 (new embedder module, new endpoints).
Only depends on Phase 2's centroid + classify infrastructure (already built).
Phase 7 depends only on the existing classify/reconstruct/holarchy endpoints
(7A/7B are client-side); 7C adds an optional Spanner sink.

---

## Key Metrics

| Area | Metric | Target |
|------|--------|--------|
| Classification (text) | Top-1 accuracy on curated test set | >0.85 |
| Classification (image) | Top-1 zero-shot on held-out photos | >0.80 |
| Graph throughput | Node mutations/sec | >10_000 |
| Dream quality | Human eval acceptance rate | >0.6 |
| Sync latency | P95 MCP broadcast to N instances | <500ms |
| Embedding latency | P99 ONNX embed (text) | <50ms |
| Embedding latency | P99 CLIP image embed | <200ms |
| Embedding latency | P99 Jina embed (text) | <100ms |
| Ontology density | Entries per cell (mean) | >25 |
| Content recall (TUI) | Forgotten-item found in top-5 by content query | >0.85 |
| Recall latency | Keystroke → re-ranked results (local engine) | <150ms |
