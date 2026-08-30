# Physis Engine — Development Plan

## Overview

Physis is moving from a single-threaded ontological mapper into a **holarchic runtime**
with real-time graph processing, semantic classification, temporal scheduling, and
distributed dream synchronization.
### ✅ DONE (2026-08-15) — semiotic editing of individual data nodes

Gio's vision — "ontology studio semiotics editing via file affinity scan and
adaptation … hotswapping conceptual objects without losing identity (or
function)" — is implemented as the **exchange-parts** edit primitive:

- **`POST /api/v1/coherence/edit`** — swap a node's *content* (label → re-embed)
  while its *structure* (id, `asserted` verdict, holarchy edges, embedder
  provenance, dedupe index) survives. The grid cell is re-derived (auto) from the
  new content; an optional `cell: {domain, mode}` **pin** overrides it and
  persists (hybrid semantics). Response reports `before_cell`/`auto_cell`/
  `pinned_cell` for the delta.
- **`POST /api/v1/coherence/delete`** — remove a node and every reference (index,
  holarchy node + edges, sled row).
- **`POST /api/v1/search/nodes`** — fixed-token-cost search over coherence + goal
  nodes (`total_tokens ≤ token_budget`, the `/rag/retrieve` contract applied to
  the live node store).
- The spiderweb surface was already there (`/api/v1/fs/graph` + `graph_fm.html`):
  scanned files classified into the 70-cell grid, movable nodes. The edit half is
  now the missing piece above.

Design + API detail: `docs/SEMIOTIC_EDITING.md`. Core primitive +
`CoherenceNode.cell_pin` in `src/core.rs` / `src/models.rs`, tests in
`core::tests` + `tests/http_surface.rs`.

> **Correction (2026-08-15).** The last bullet above overstates the result. The
> spiderweb (`/api/v1/fs/graph` + `graph_fm.html`) operates on **files** — its
> move button posts to `/api/v1/fs/move`, which moves a file on disk, and
> `fs_graph` never touches the coherence node store. A scanned file has no node
> id, so the edit endpoints cannot reach anything the spiderweb shows. The edit
> half exists; it is not yet joined to that surface. See the plan below.

---

## Interface that mimics nature (2026-08-15)

### The one bug behind five

Every defect found this session has the same shape: physis keeps a **state
layer** and a **reporting layer**, and they are synchronised by hand.

| Symptom | State | Report |
|---|---|---|
| coherence structurally 0.0 | scored | scored before the node was insertable |
| `/semiotic/grid` showed 27 of 70 cells | 33 ontologies loaded | handler read one map |
| goals carried no text | embedding present | no label field |
| `cell_pin` inert | stored + persisted | no reader anywhere |
| studio facets decorative | `Facets` on `DomainDef` | handler wrote `default()` |

An organism has no reporting layer. Wilting is not a readout of turgor, it *is*
turgor; there is no second place to forget a field. "Interface mimicking nature"
is therefore an architectural constraint, not a visual style: **what is shown
must be derived from state by construction, never assembled alongside it.**

### 1. One projection — make the compiler forbid write-only state

`NodeView`, built by a single `observable()` that destructures `CoherenceNode`
**exhaustively** (no `..`). Adding a field to the node then fails to compile
until someone decides whether it is observable — `cell_pin` could not have been
born unreadable. Every surface (`/coherence/nodes`, `/nodes/all`, TUI heatmap,
`graph_fm`, console) *selects* from `NodeView` instead of hand-assembling its
own DTO, so seven partial views stop drifting apart.

- [ ] `NodeView` + `observable()` in `src/core.rs`, exhaustive destructure
- [ ] Effective cell = `cell_pin` if set, else classifier top hit; classify
      **after** the truncate in `/nodes/all` so it stays O(limit), not O(store)
- [ ] `CoherenceNodeMatch` selects from `NodeView`
- [ ] Test: a pinned node reports its pinned cell, not the auto one (the
      existing pin test asserts only that the edit *response echoes* the pin —
      it would pass if nothing downstream honoured it)

### 2. Pins are testimony — they stick, and disagreement is a signal

A scar survives, a posture does not. A pin is a human assertion about where a
thing belongs, the same species of claim as `asserted` (a human verdict about
whether it worked). Both are testimony, so a content edit must not silently
erase one: **pins persist across edits; clearing is explicit (`cell: null`).**

Once pins persist, every pin either agrees or disagrees with the classifier, and
a disagreement is the system reporting its own misclassification *with the
correct answer attached*. `QualityTracker` already receives exactly this shape:

```rust
quality.report_failure(feedback, &centroids, None)  // pin disagrees with auto
quality.report_success(&cell)                       // pin confirms auto
```

Sender and receiver both exist; there is no wire between them. Connecting them
makes the ontology improve as a by-product of use — no labelling task, no review
queue. `discovery.rs` finds texts the ontology *cannot name* (unlabelled gaps);
pin disagreements are *labelled corrections*, which are strictly more valuable.

**Weighting decision:** a pin is **locally authoritative, globally advisory** —
it always wins for its own node, but contributes only one low-severity sample to
the cell's quality score, so a single stray click cannot reshape classification
for everyone while repetition still accumulates.

- [ ] Pins survive edits; `cell: null` clears (update `test_edit_node_pin_roundtrips_and_clears`)
- [ ] Pin↔auto delta → `QualityTracker`, low severity per event
- [ ] Test: repeated disagreement on one cell moves its penalty; one does not

### 3. The grid shows a headcount; it should show a condition

The studio heatmap colours cells by entry `count`. But per cell the engine
already knows population, activation, `cell_penalties`, `cell_boosts`, and the
asserted verdicts of nodes landing there — and `adjust_score` *already applies*
those penalties to live classification. So a cell can be actively demoted on
every query and still render identical to a healthy cell of the same size.

- [x] Colour by condition (population × mean asserted × recent penalty)
- [x] Same treatment for the TUI heatmap pane

**Done (2026-08-17).** The engine already had the answer: `adjust_score`
computes `1.0 - penalty + boost` and applies it to every live classification.
Writing a second health formula next to it would have recreated exactly the
state-vs-report split §1 exists to kill, so that expression is now named
(`cell_standing`) and both grids *select* from it.

The combining rule became a **slider rather than a decision**, because `count`
can be read two ways and both are defensible:

- *responsive* — count is mere size, every cell reacts at full strength, new
  problems surface immediately, a stray click is visible;
- *trusting* — count is a confidence weight, thin cells hold near neutral until
  evidence accumulates, colours are slower but worth acting on.

`cell_condition(count, standing, mean_asserted, confidence)` shrinks a cell's
reading toward neutral by `n / (n + k)`, `k = confidence * 16`; `ConditionLens`
names the positions. Exposed as `?confidence=` on both grid endpoints, a range
input in the studio, `[`/`]` in the TUI map. Three rules are load-bearing and
tested: an empty cell reads **neutral, not bad** (absence of evidence is not bad
news); `standing` is squashed `s/(s+1)` not clamped, so unbounded boosts stay
ordered; junk input falls back to the default instead of pinning to an extreme.

### 4. Dreams are a rhythm, not a request

`dream()` already replays asserted failures and skips successes, but only when
someone calls `/dream/generate` — which in practice means never. Run it when
request rate drops and surface the result as the first thing the console shows.

- [x] Idle-triggered dream pass
- [x] "What it chewed on" panel

**Done (2026-08-17).** The rate-limit layer was already universal, so it now
stamps the time of every request — including throttled ones, since a rejected
request is still someone asking and must postpone the pass. A background tick
notices when the gap exceeds `PHYSIS_IDLE_DREAM_SECS` (default 60) and replays
whatever has not been dreamt over yet. Surfaced at `GET /api/v1/dream/rest` and
as the first panel on the console overview.

Two states are deliberately kept apart: a pass that finds nothing new leaves the
previous one standing, and `rested: false` means *no pass has ever produced
material* — collapsing "found nothing new" into "never ran" would make an idle
engine look like a broken one.

One trap worth recording: the threshold must be read at build time, not inside
the spawned block. A `tokio::spawn`ed future does not run until first polled, so
reading the environment in there picks up whatever the caller set afterwards
rather than the configuration the app was built with. The first version did
exactly that and silently used the default.

### 5. Cross-project nerves carry sensation, not scalars

physis is the sensorium, aion the motor loop, praxis the social body. Messages
between them must carry *what happened and how it felt*, not identifiers:
`checkinController.ts:337` embeds `user:<uuid>`, which has no smell — nothing
downstream can classify it, no cell lights up, no dream can replay it. The aion
client's `Recalled` dataclass already gets this right by carrying `.outcome`.

- [x] praxis: embed check-in content, not the user id
- [ ] Make `.outcome`-style verdict carriage the contract on every edge

**Check-in edge fixed (2026-08-17)**, on praxis branch
`fix/checkin-carries-sensation` — *not pushed*: praxis deploys on push to main,
so landing it is a deliberate act, not a side effect of this work.

`checkinNarrative` renders the check-in as a sentence — the user's own words
first, then mood, energy, recovery, streak. The user id is gone entirely:
tenancy already travels in the `X-Physis-User` header, so it bought no isolation
and only polluted the embedding.

The sharper half of the bug was dedupe. physis keys nodes by this text, so
`user:<uuid>` collapsed every check-in a person ever made onto **one** node
whose verdict was overwritten daily — the history was not lost in transit, it
was never distinct. Including the date turns dedupe into the feature it should
be: one node per day, and a same-day re-check-in updates rather than duplicates.

Audit result: the notebook (`data.content`) and goal (`newNode.name`) edges
already sent real content. The check-in was the last one sending an identifier.

The second bullet stays open on purpose — it is a standing contract for future
edges, not a discrete task, and nothing enforces it mechanically yet.

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
| 9 | Gemma re-ranker — confidence-gated generative path | Steps 8 | 2 days | ✅ Shipped (`rerank_cell`; margin-gated; server `PHYSIS_RERANK` **or** per-request **BYOK** `llm_key`/`llm_model` on `/classify` → one-off provider, no server key needed) |
| 10 | Process embedding — task graph → vector via Gemma structural summarization | Steps 9 | 2 days | ⬜ Pending |
| 11 | Brainstorm post-trained model (Gemma LoRA reranker via unsloth) — LoRA on quality_failures.json → JSONL {"Activity":"…","completion":"3"} same prompt as rerank_cell; evaluate with output_quality probe; if accuracy_floor +3pts → ship GGUF variant on OLLAMA_FLOOR_MODEL=gemma-uncensored, else revert to centroid/boost tuning | — | 3 days | ⬜ Pending |
| 12 | UI/UX polish phase — unified shell + per-user nav + quality traces + ask bar + incoherence viz | — | 5 days ⬜⟳ 3d done | ⬜ Pending |

### 6I. Key Decisions

- **All ONNX embedders share the `ort` runtime** — no additional C++ deps. The `image` crate (already in Cargo.toml) handles decode/resize.
- **Dimension mismatch** is handled by projecting to the embedder's native dim at embed time, and pooling centroids to the query embedder's dim at classify time. This means you can mix MiniLM centroids (384-d) with CLIP queries (512-d) via a learned linear projection or simple mean pooling.
- **Jina v2 is the recommended primary embedder** for both text and image: best accuracy, multilingual, open weights, ONNX-exportable.
- **CLIP is the recommended fallback**: smaller, faster, battle-tested.
- **Gemma is not an embedder replacement** — it's a complementary reasoning layer. The vector space stays the source of truth for PDCA, dreaming, and coherence.

---

## 6J. UI/UX polish phase

Goal: turn the functional but utilitarian console/web UI into a unified,
per-user decision-support shell. Three deliverables:

**1. Unified nav + per-user state** (`src/bin/ui/` + `ui-shell.js`)
   - Single top nav bar (Home / Console / Quality / Pulse / Study / Net / Ask)
   - `X-Physis-User` header auto-injected per-session; each user gets isolated
     `RuntimeState` (goals, PDCA, dreams, coherence) without config changes.
   - KPI ribbon shared across panels; panel toggling via route `GET /console`
     etc. instead of separate html files (`pulse.html`, `study.html`).
   - Net workspace (key `9`, icon `🌐`) shows remote nodes with live status,
     last-seen age, auto-refresh every 30 ticks.

**2. "Why did this happen?" traces** (`web/coherence.rs` + `quality.rs`)
   - From a `QualityFailure` cell penalty spike → one-click trace to last
     `watch` diff event → file change → ontology hint → centroid delta.
   - `POST /api/v1/quality/trace?failure_id={id}` returns chain:
     `failure → watch event → file ingest → classification score change`
   - Summary pill in quality dashboard: "penalty ↑ 0.32 after file X changed
     on 2026-08-20."

**3. AI co-pilot ask bar** (integrated into unified shell)
   - Persistent ask input calling `POST /api/v1/ask` with grounded synthesis.
   - Answers render above matches with `[n]` citations.
   - Prompt suggestions from current context: "why is cell WORK/LIFT penalized?",
     "which files contributed to this quality drop?", "what does MODE=REST mean
     for machine X?"
   - Degradation: without provider → raw matches + note, still functional.

---

## Embedder scaling (ready for a heavier model)

- ✅ Embedding is dim-agnostic at startup: `embed_dim = embedder.dimension()`,
  centroids rebuilt from the embedder, PQ retrains — drop in a better model freely.
- ✅ Off-thread embeds: all request-path embeds go through `spawn_blocking`
  (`embed_text`/`embed_input` helpers) so a slow/GPU/remote embedder never blocks
  a tokio worker (ONNX is serialized behind a session mutex). Bulk paths (scan,
  watch) route through `embed_batch` in one hop.
- ✅ ONNX `embed_batch` override: single `[N, max_length]` session pass + per-row
  masked mean-pool matching `embed`; lock dropped before any fallback. Tested:
  fallback-equality (no model, always runs) + `#[ignore]` real-model batched-vs-
  per-text within 1e-4.
- ✅ Vector provenance: `CoherenceNode.embedder: Option<String>` (serde default),
  stamped via `PhysisCore.set_embedder_id` (set at startup from the loaded
  embedder kind — default tenant + per-tenant). Flows into the Spanner
  `NodeRow`/schema (`embedder` column). A loader can detect a model swap
  (mismatched id ⇒ re-embed) instead of silently mixing vector spaces.

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
- 🟢 7C Spanner Graph sink (`src/spanner.rs`, `--features spanner`, default off).
  Persistence + re-embed: chunked `upsert_nodes` (≤500/commit), `fetch_nodes`
  read-back, `export_nodes`/`export_edges` via the bridge, and re-embed migration
  (`hydrate_into` regenerates vectors from labels under the live embedder,
  `stale_nodes` flags swap targets, `migrate` = fetch→re-embed→write-back).
  Wired: `POST /api/v1/spanner/sync|hydrate` + `physis-pro spanner sync|hydrate`;
  Vector provenance (`embedder` column). `WikiEdge` upsert/fetch + service-account
  JWT auth (RS256, cached/refreshing). REST round-trip (nodes + edges) **verified
  against the Spanner emulator in CI**; JWT signing unit-tested. Library-only, not
  on the hot path. Remaining (PREMORTEM P7): deletes/tombstones + a live-Spanner
  OAuth-exchange smoke.
- ✅ New machines (ALL built): `src/machines/` — chronos, techne, soma, elenchus,
  lex, harmony, polis. Each pure + unit-tested + an `examples/<m>_demo.rs` + a
  captured `docs/machines/samples/<m>.md`. Docs grouped in `docs/machines/`
  (PHILOSOPHICAL_MACHINES, MACHINES_BUILD_PLAN, PIKA_HERMES_PLAN). CI builds them
  (default `cargo check` + `clippy --all-targets` covers examples) + a `spanner`
  feature check.
- ✅ physis-pro-tui = control panel (`src/bin/tui.rs`, was the voice mapper):
  3 tabs — STATUS (poll engine /health + /api/v1/embedder; **s start / x stop /
  r reload-config** the engine), MACHINES (run any of the 7 in-process), CONFIG
  (edit + save ~/.physis/config.json). required-features just `tui`. Live reload:
  `POST /api/v1/config/reload` re-reads config.json + applies PDCA knobs to the
  tenant actor (structural params need restart).
- ✅ chronos (temporal machine): `src/machines/chronos.rs` — `detect_rhythms`
  (recurrence primitives from timestamped events), `upcoming`/`slipping`
  (predict next / flag about-to-break), `register_rhythms` (→ labeled bridged
  nodes). Pure + tested. Demo + sample: `examples/chronos_demo.rs`,
  `docs/machines/samples/chronos.md`.
- ✅ techne (codebase machine): `src/machines/techne.rs` — `classify_capabilities`
  (cluster files by purpose), `dependency_edges` (module graph from imports,
  module-root → parent-dir resolution), `impact_of` (transitive "what breaks if
  this changes"), `register_capabilities` (→ bridged nodes + dependency edges).
  Pure + tested. Demo + sample: `examples/techne_demo.rs`,
  `docs/machines/samples/techne.md`.
- 📦 Sibling machines committed to the agora monorepo: euclid (geometric, demo +
  SVG output samples) + empedocles (physical-machine, four-elements reducer).
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
- **Watch mode** — ✅ done. Auto-ingest on file create/modify:
  - Web background thread (`PHYSIS_WATCH_DIR` + `PHYSIS_WATCH_INTERVAL_SECS` env,
    confined to scan_root) registers new/changed files as labeled core nodes.
  - `POST /api/v1/watch` — on-demand sweep (same ingestion, scan-root confined);
    returns `{new, changed, deleted, registered, removed, total_nodes}`.
  - CLI `physis-pro watch <dirs> [--once]` — loop (or one sweep) ingesting diffs,
    dropping nodes for removed files, advancing the hash cache.
  - Labels come from `FileInfo::labels()` (shared by /scan, watch, and the CLI);
    removal uses `PhysisCore::delete_nodes_by_label_prefix`.
- **Vault importers** — ✅ done. Obsidian/markdown, plain notes, and `git log`
  reduced to labeled core nodes (`src/vault.rs`):
  - `VaultDoc` (title from H1 / Obsidian `title:` frontmatter / file stem,
    headings `h1:`…`h3:`, full body) with `labels()` = title + `title → hN:…`.
  - CLI `physis-pro vault <dir> [--git N]` and web `POST /api/v1/vault/import`
    (scan-root confined) register them with the shared embedder; re-import
    dedupes by label. `parse_git_log` is unit-testable without a git binary.
- **Personal-history importers** — ✅ done. Browser bookmarks (Netscape HTML),
  browser history (JSON), OPML feed lists, and chat/message exports (JSONL)
  reduced to labeled core nodes (`src/history.rs`, pure parsers):
  - CLI `physis-pro history <file>` and web `POST /api/v1/history/import`
    (scan-root confined) register each item with the shared embedder; re-import
    dedupes by label. `importer_for` dispatches by extension.
- **Praxis backfill** — ✅ done. Notebook entries + trackers → labeled
  behavioural nodes with **asserted** verdicts (`src/praxis.rs`):
  - Mood ≥ 7 → success, ≤ 3 → failure, else inert; tracker value/target (or
    `progress`) ≥ 0.7 → success, < 0.3 → failure, none → inert.
  - CLI `physis-pro praxis <file>` and web `POST /api/v1/praxis/backfill`
    (scan-root confined) register each record via `register_behavioural_vector`
    semantics (labeled + `assert_coherence`), seeding the coherence graph from
    the real life-log. Re-backfill dedupes by `praxis:<kind>:<title>`.

> Embedder consistency rule: nodes must be embedded with the **same** embedder
> that embeds queries (the shared `SharedCtx.embedder`), or cosine recall is
> meaningless. `mapper`'s internal RandomProjection embedder is for trie tokens
> only — never register its vectors into the searchable core index.

### 7C. Spanner Graph sink — durable, multi-device memory

Persist holarchy nodes/edges into Google **Spanner Graph** (property graph over
Spanner SQL). physis = local reasoning; Spanner Graph = durable, GQL-queryable,
cross-device memory. Optional vector column for server-side ANN. Re-hydrate a
local tenant from Spanner on a new device.

**Status: ✅ wired.** The sink scaffold (`src/spanner.rs`, `--features spanner`)
is now reachable from both surfaces (previously library-only):

- **Web** — `POST /api/v1/spanner/sync` (upsert nodes + edges; `mirror:true` also
  hard-deletes remote orphans) and `POST /api/v1/spanner/hydrate` (pull durable
  rows → re-embed under the live embedder → persist into the tenant core). Both
  tenant-aware, error 502 with a clear reason when Spanner env is missing.
- **CLI** — `physis-pro spanner sync [--mirror]` / `physis-pro spanner hydrate`
  against the persisted `_default` tenant.
- Verified: emulator round-trip (`spanner_emulator_roundtrip`, CI) + a routing
  test asserting the endpoints reject unconfigured env with 502.
- Off the hot path by design: cycles are explicit (cron / user-invoked), never
  per-mutation.

**Order:** 7A (browser) → 7B (auto-fill) → Praxis backfill → 7C (durability).
7A/7B need no engine changes; 7C is an optional sink module gated behind
`--features spanner`.

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
# Classifier productization (2026-08-17)

The classifier work is now organized around a stable 70-cell grid plus evidence
layers. See [docs/CLASSIFIER_PLAN.md](docs/CLASSIFIER_PLAN.md). The MVP work is
implemented in `src/semiotic_grid.rs`, `src/classify.rs`, `src/context.rs`,
`src/discovery.rs`, and the `/api/v1/classify` response.
