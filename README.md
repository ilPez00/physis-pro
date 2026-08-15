# Physis Pro

**A Rust runtime engine for ontological classification, coherence tracking, and predictive ("dream") planning.**

Physis is the reasoning core of the Ayu / Praxis ecosystem. It reduces any input —
text, image, video, audio, a code log, a human action — to a vector in one shared
embedding space, classifies it against a 70-cell semiotic grid, and tracks each
thing's *coherence* (does it work? +1 / 0 / −1). A background "dream" loop replays
the failures to propose corrective actions.

It runs as:
- a **CLI** (`physis-pro`) for scanning, querying, and one-off dreams,
- an **HTTP server** (`physis-pro-web`, default `127.0.0.1:19876`) that Praxis and
  Aura clients call,
- an **MCP server** (feature `mcp`) for agent tool-use,
- a **TUI** and a **voice** binary for live ontological mapping.

> Crate: `physis_pro` v0.1.0 · edition 2021 · the Praxis backend talks to it through
> `src/lib/physisProClient.ts`.

---

## Core model — three functions over one state

```
                         ┌──────────────────────────┐
                         │   PRAXIS / AURA clients   │
                         │  classify · pdca · dream  │
                         └───────────┬──────────────┘
                                     │ HTTP / MCP
                                     ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                          PHYSIS (runtime core)                            │
│                                                                           │
│   UBERWIKI (state):  DynamicVectorTrie + CoherenceNode (+1.0 / 0 / -1.0)  │
│                                    │                                       │
│            ┌───────────────────────┴───────────────────────┐             │
│            ▼                                                ▼             │
│   COMPRESSION (waking):                       DREAM (rest/async):         │
│   strip noise from raw input,                 replay Inert/Failure nodes, │
│   keep minimal causal tokens                  simulate fixes, recalibrate │
│                                               the coherence index         │
└──────────────────────────────────────────────────────────────────────────┘
```

### 1. UberWiki — the state
A vector store of causal nodes. Each node carries a **coherence weight**: stable
flows are `+1.0`, frictions/failures are `-1.0`. Backed by a `DynamicVectorTrie`
of `CoherenceNode`s.

### 2. Compression — the waking/input phase
Strips raw input (logs, commands, telemetry, prose) of identity and redundancy,
leaving the minimal causal tokens before they reach the host AI. Keeps the working
context clean and cheap. `filter_context()`, `compress_logs()`.

### 3. Dream — the rest/async phase
Runs in the background. Takes accumulated Inert (`0.0`) and Failure (`-1.0`)
fragments, simulates the missing causal link, and emits corrective **Effective
Actions** while recalibrating the global coherence index. `dream()`.

> Waking-compression and rest-dream are the two phases of one metabolism —
> discrimination (separate signal from noise) and integration (replay,
> recombine, synthesise). `docs/EMPEDOCLES_DYAD.md` develops this as the
> Empedoclean dyad (Strife/Neikos vs Love/Philotes) and tests it.

### Coherence ontology — two axes

A node carries **two** coherence numbers, and they answer different questions:

| Axis | Field | Range | Who sets it | Question |
|------|-------|-------|-------------|----------|
| **Derived** | `coherence_score` | `0 … 1` | the engine (mean cosine to the 5 nearest neighbours) | *how typical is this?* |
| **Asserted** | `asserted` | `-1 … 1`, `None` until judged | the caller | *did it work?* |

Only the asserted axis can express failure — derived density is clamped at `>= 0`
by construction. Keeping them separate is deliberate: a node can be perfectly
typical (density `1.0`) and still be a reported disaster (`-1.0`).

The three asserted states apply identically to machine functions and human behaviour:

| State   | Weight | Machine                                  | Human                                      |
|---------|:------:|------------------------------------------|--------------------------------------------|
| Success |  +1.0  | Compiles + functional test passes        | Action completed with a real, detected effect |
| Inert   |   0.0  | Compiles but produces no expected effect | Task done without cognitive progress       |
| Failure |  −1.0  | Compiler error / refuted pattern         | Self-imposed rule violated (e.g. diet slip)|

```rust
// A human action is judged with the same logic as a software function.
let id = core.register_node_from_text("ate cake at midnight", &embedder);
core.assert_coherence(&id, CoherenceRating::Failure.weight());   // -1.0
core.assert_coherence(&study_id, CoherenceRating::Inert.weight());  // 0.0

core.asserted_index();     // mean verdict over JUDGED nodes only (None if none)
core.asserted_failures();  // what to replay, worst first
core.coherence_index();    // unrelated: mean derived density over every node
```

`dream()` replays a reported failure however dense its neighbourhood, and skips a
reported success however isolated it is — a caller's verdict outranks a guess
drawn from geometry. Nodes nobody has judged fall back to the density proxy.

When a new categorization conflicts with an established Success node, the engine
suspends it and returns a **Constructive Refutation** payload for the PDCA loop to
recalibrate, instead of silently overwriting state.

---

## HTTP API

Default bind `127.0.0.1:19876` (`PHYSIS_PORT`, `PHYSIS_HOST`). All `/api/v1/*`
routes below the line are **protected** (see [Auth](#auth)); `/health` and
`/api/v1/embedder` are public for healthchecks/ops.

**Per-user state** is selected by the `X-Physis-User: <id>` header. Each id gets an
isolated `RuntimeState` (its own goals / PDCA / dreams / coherence). A missing or
`_default` id routes to the shared global tenant that the holarchy threads drive.
Stateless routes (classify, embed, semiotic, ontology) ignore the header.

| Method | Route | Tenant-aware | Purpose |
|--------|-------|:---:|---------|
| GET  | `/health` | — | Liveness (public) |
| GET  | `/api/v1/embedder` | — | Active embedder provenance (public) |
| POST | `/api/v1/classify` | — | Embed text (or base64 `image`) → ranked `{domain, mode, score, entries}[]` |
| POST | `/api/v1/discover` | — | Ontology-gap discovery over `{texts[]}` → coverage report + draft ontology JSON for the texts the grid covers poorly |
| POST | `/api/v1/embed` | — | Raw embedding vector + dimension (text or image) |
| POST | `/api/v1/embed/video` | — | ffmpeg-sampled clip vector → `{embedding, dim, frames}` |
| GET  | `/api/v1/ontology/list` | — | Domain counts per loaded ontology |
| GET  | `/api/v1/semiotic/grid` | — | 70-cell DOMAIN×MODE grid with entries |
| GET  | `/api/v1/semiotic/triangle` | — | Peircean triangle (Mermaid) |
| GET  | `/api/v1/semiotic/square` | — | Greimas square (Mermaid) |
| GET  | `/api/v1/semiotic/heatmap` | — | Activation heatmap table + matrix |
| POST | `/api/v1/category/diagram` | — | Custom category diagram (objects + morphisms) |
| GET  | `/api/v1/coherence/snapshot` | ✓ | Current coherence index + nodes |
| POST | `/api/v1/coherence/register` | ✓ | Register a node `{input, coherence_score?}` — `input` is embedded **and** kept as the label; optional `coherence_score` (−1…1) is the asserted verdict. Deduped by label, so re-posting the same `input` returns the same id and **updates** its verdict |
| POST | `/api/v1/coherence/edit` | ✓ | **Exchange-parts edit** `{id, label, cell?}` — swap content while preserving structure (id, verdict, edges); `cell: {domain, mode}` pins the semiotic cell (auto re-derivation otherwise). Returns `before_cell`/`auto_cell`/`pinned_cell` |
| POST | `/api/v1/coherence/delete` | ✓ | Delete a node + every reference (dedupe index, holarchy node/edges, sled row) `{id}` |
| POST | `/api/v1/search/nodes` | ✓ | **Fixed-token search** over coherence + goal nodes `{query, token_budget?, top_k?}` — response `total_tokens ≤ token_budget` (same contract as `/rag/retrieve`) |
| GET  | `/api/v1/pdca/plan` | ✓ | Current Plan-Do-Check-Act plan |
| POST | `/api/v1/pdca/act` | ✓ | Apply an action `{action, goal_id?}` |
| GET  | `/api/v1/pdca/stats` | ✓ | PDCA cycle stats |
| POST | `/api/v1/oee/machine` | plant | Register a machine `{machine_id, shift_length_secs, ideal_cycle_time_secs}` |
| POST | `/api/v1/oee/event` | plant | Record a shop-floor event: `{kind:"stop", machine_id, reason, duration_secs, note?}` (reason ∈ six big losses), `{kind:"production", machine_id, good, scrap?}`, or `{kind:"start_shift", machine_id}` |
| GET  | `/api/v1/oee/:machine` | plant | OEE report (A×P×Q + six-big-losses Pareto) for one machine |
| GET  | `/api/v1/oee/plant` | plant | Planned-time-weighted plant OEE rollup + merged Pareto |
| POST | `/api/v1/maintenance/observe` | plant | Fold a healthy reading into a machine's learned baseline |
| POST | `/api/v1/maintenance/check` | plant | Score a reading (σ from baseline) + optional RUL; anomalies auto-register as coherence nodes |
| GET  | `/api/v1/maintenance/:machine` | plant | Health baseline summary for one machine |
| GET  | `/api/v1/report/shift` | plant | Italian shift report (Markdown, or HTML via `?format=html`) from the live OEE board + graded dream countermeasures |
| POST | `/api/v1/report/shift` | — | Shift report from a supplied `ShiftReportInput` (OEE + anomalies + RUL + countermeasures); stateless, `?format=html` for HTML |
| POST | `/api/v1/config/reload` | ✓ | Re-read `config.json`; apply PDCA knobs + scan_root live (embedder/embed_dim need restart — dim change corrupts vectors) |
| POST | `/api/v1/goals` | ✓ | Ingest a prompt as a goal → `{goal:{id}}` |
| POST | `/api/v1/dream/generate` | ✓ | Generate dreams `{count, force, embeddings?}` → `{id, kind, proposal, grade}`; vectors only with `embeddings:true` |
| POST | `/api/v1/dream/replay` | ✓ | Nodes the dream loop is replaying, **named**: `{node_id, label, asserted, density, outcome, prevented_failure}` — reported failures first, unjudged nodes by sparse neighbourhood |
| POST | `/api/v1/dream/evaluate` | ✓ | Grade a dream `{id, grade}` (0.0–1.0) |
| GET  | `/api/v1/stats` | ✓ | Engine statistics |
| POST | `/api/v1/context/filter` | ✓ | Compression: filter a raw input |
| POST | `/api/v1/compress/logs` | ✓ | Compression: logs → dense causal rules |
| POST | `/api/v1/reconstruct` | ✓ | Nearest-neighbour reconstruction + LLM interpretation |
| POST | `/api/v1/translate` | ✓ | Linguistic routing (Wenyan / Pirahã / Sanskrit) |
| GET  | `/api/v1/query` | ✓ | Query the trie |
| POST | `/api/v1/scan` | ✓ | Scan a dir (confined to `PHYSIS_SCAN_ROOT`) |
| POST | `/api/v1/holarchy/ingest` | global | Push a node/edge into the holarchy graph |
| POST | `/api/v1/holarchy/edge` | ✓ | Assert a relation between two coherence nodes `{source, target, weight?, semantic_type?}` (bridges both) |
| GET  | `/api/v1/holarchy/snapshot` | ✓ | Holarchy node/edge counts for the tenant |
| GET  | `/api/v1/holarchy/trajectory` | ✓ | State-vs-goal trajectory |
| GET  | `/api/v1/holarchy/graph` | ✓ | Full graph export for the tenant |
| GET  | `/api/v1/holarchy/neighbors` | ✓ | Labelled neighbours of a coherence node `?id=<node_id>` — the walkable view (`/graph` returns opaque `RawNodeKey`s) |
| POST | `/api/v1/chat` | ✓ | LLM provider-cascade chat proxy |
| GET  | `/api/v1/providers` | — | Configured LLM providers |

### Example — classify

```bash
curl -X POST http://127.0.0.1:19876/api/v1/classify \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"text":"a serene landscape painting with soft brush strokes"}'
# → top: FABRICATE×CREATE (0.89) — Visual Arts, Storytelling, Music & Performance
```

At startup every ontology entry (name + hints) is embedded and averaged per
DOMAIN×MODE cell. At query time the same embedder scores the input against all
centroids; each cell's score is the **max cosine to any member entry**.

### Auth

Protected routes require `Authorization: Bearer <PHYSIS_API_TOKEN>` (constant-time
compared).

- **Token unset** + loopback bind → auth is skipped (local dev convenience).
- **Token unset** + non-loopback bind → the server **refuses to start** (it would
  publish an unauthenticated API that also proxies paid LLM providers — an open
  relay). Set the token or bind `127.0.0.1`.

### Semiotic node editing ("exchange parts") & fixed-token search

A coherence node is **structure** (id, `asserted` verdict, holarchy edges,
embedder provenance, grid position) + **content** (label + embedding). Because
the two are compatible, an edit swaps the content while the function survives:

```bash
# Exchange a node's content; the id/verdict/edges never change.
curl -X POST http://127.0.0.1:19876/api/v1/coherence/edit \
  -H "Content-Type: application/json" -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"id":"<node_id>","label":"welding robot joint recalibration",
       "cell":{"domain":"FABRICATE","mode":"MAINTAIN"}}'
# → { ok, node_id, label, auto_cell, pinned_cell, before_cell, asserted, coherence_score }

# Fixed-token search over coherence + goal nodes (cost is bounded up front).
curl -X POST http://127.0.0.1:19876/api/v1/search/nodes \
  -H "Content-Type: application/json" -H "Authorization: Bearer $PHYSIS_API_TOKEN" \
  -d '{"query":"spindle bearing","token_budget":600}'
# → { query, budget, total_tokens ≤ 600, truncated, results:[{kind,id,label,score,tokens}] }
```

See [docs/SEMIOTIC_EDITING.md](docs/SEMIOTIC_EDITING.md) for the invariant, the
hybrid pin semantics, and the full API contract.

---

## Run

```bash
# CLI
cargo run --bin physis-pro -- scan <dir>
cargo run --bin physis-pro -- query "<text>"
cargo run --bin physis-pro -- classify <file.txt>          # semiotic-grid map of a text
cargo run --bin physis-pro -- dream --count 5
cargo run --bin physis-pro -- discover <dir> [--threshold T] [--json-out F]  # ontology gaps in a corpus → draft entries
cargo run --bin physis-pro -- report --shift <input.json> [--html]           # Italian shift report (Markdown or HTML)
cargo run --bin physis-pro -- stats

# Experiment — Empedoclean dyad criticality (research; see docs/EMPEDOCLES_DYAD.md)
cargo run --release --example empedocles_criticality

# HTTP server (feature: web, on by default)
cargo run --bin physis-pro-web          # binds 127.0.0.1:19876

# Voice — live ontological mapping (features: voice,web), UI on :3000
cargo run --bin physis-pro-voice

# TUI (features: voice,tui)
cargo run --bin physis-pro-tui
```

### Environment

| Var | Default | Purpose |
|-----|---------|---------|
| `PHYSIS_PORT` | `19876` | HTTP port |
| `PHYSIS_HOST` | `127.0.0.1` | Bind host (non-loopback requires a token) |
| `PHYSIS_API_TOKEN` | — | Bearer token for protected routes |
| `PHYSIS_SCAN_ROOT` | data dir | Confines `/api/v1/scan` (path-traversal guard) |
| `PHYSIS_WATCH_DIR` | — | Poll this dir (within scan root) and auto-register new/changed files as labeled nodes |
| `PHYSIS_WATCH_INTERVAL_SECS` | `30` | Watch-mode poll interval |
| `PHYSIS_RERANK` | unset | Enable LLM re-rank on `/classify` for ambiguous cells (Gemma/provider cascade); needs provider keys |
| `PHYSIS_RERANK_MARGIN` | `0.05` | Top-2 score gap below which re-rank fires — only ambiguous queries hit the LLM |

**BYOK re-rank**: a caller can activate the re-ranker per request without any server
key by passing `llm_key` (and optional `llm_base_url`, `llm_model`) in the `/classify`
body — physis builds a one-off OpenAI-compatible provider for that call. Defaults to
Gemini (`gemini-2.5-flash`). The caller's key is used only for that request and never
stored. Lets each user bring (and pay for) their own model.

### Config (`~/.physis`)

```json
{
  "data_dir": "~/.physis",
  "dream_batch_size": 5,
  "pdca_stagnant_threshold": 0.2,
  "pdca_stagnant_window": 5
}
```

Default ontologies live in `config/` (`praxis_ontology.json`, human;
`machine_ontology.json`, machine; plus the semiotic set below).

---

## Semiotic grid & ontologies

Every entry maps to one of **70 cells** — **5 domains** (HEAL, CONSTRUCT,
FABRICATE, BOND, STUDY) × **14 modes** (LIFT, REST, WALK, WORK, CREATE, LEARN,
DESTROY, SENSE, GUIDE, PLAY, BRAINSTORM, MAINTAIN, MOVE, PLAN). The last eight
extend the original activity-energy axis: DESTROY completes the Greimas opposition
with CREATE, SENSE is perception/measurement, GUIDE is leading/mentoring others
(vs LEARN, self-study), PLAY is low-stakes generative, BRAINSTORM is divergent
ideation, MAINTAIN is upkeep/repair/preserve, MOVE is relocate/transport/logistics,
PLAN is convergent sequencing/organizing (the converge to BRAINSTORM's diverge).

Ships **10 ontologies** (~630 entries):

| Ontology | Kind | ~Entries | Coverage |
|----------|------|:---:|----------|
| `praxis_ontology` | human | 54 | Body, career, social, finance, … |
| `machine_ontology` | machine | 110 | CNC + 23 industrial monitoring domains |
| `semiotic_ontology` | semiotic | 44 | Peirce, Saussure, Barthes, Eco, Jakobson, Greimas |
| `category_ontology` | category | 54 | Objects, morphisms, functors, monads |
| `agent_ontology` | agent | 69 | AI cognitive architectures + agent workflows |
| `natural_ontology` | natural | 56 | Physics, chemistry, biology, astronomy, ecology |
| `social_ontology` | social | 50 | Sociology, culture, politics, economics, law |
| `abstract_ontology` | abstract | 56 | Math, logic, computation, systems, linguistics |
| `engineering_ontology` | engineering | 65 | Software, civil, mechanical, electrical, chemical |
| `office_ontology` | office | 75 | Comms, finance, procurement, people, projects, … |

Plus **20 extra domain ontologies** baked in via `EXTRA_ONTOLOGIES` and used by
`classify` (`software_dev`, `devops`, `data_science`, `cybersecurity`,
`health_fitness`, `nutrition_cooking`, `personal_finance`, `learning_education`,
`creative_writing`, `music`, `visual_arts`, `relationships`, `productivity`,
`travel`, `home_maintenance`, `mindfulness_spirituality`, `science_research`,
`business_entrepreneurship`, `language_learning`, `outdoors_sports`).

---

## Multimodal — one shared vector space

Text, images, and video land in the **same** embedding space, so a photo and a
sentence are directly comparable and classifiable through the same core.

- **Images** — `embed_image(bytes)` on **CLIP** (`embed/clip.rs`) or **Jina v2**
  (`embed/jina.rs`); `/classify` and `/embed` accept an optional base64 `image`
  and fall back to `text` on failure (never 500).
- **Video** (`src/video.rs`) — ffmpeg frame-sample → `embed_image` per frame →
  temporal mean-pool + L2-norm → one clip vector. `POST /api/v1/embed/video` takes
  a server-side `path` (confined to scan root) or base64 `video_base64`, plus
  optional `fps` (1.0) / `max_frames` (32). Fail-soft → `422`, never a crash;
  needs an image-capable embedder.
- **Voice** (`physis-pro-voice`) — `arecord` 15s chunks → whisper-large-v3
  (provider cascade) → llama3.2 concept extraction → live `OntologicalMap` graph
  UI on `:3000`. Supersedes the standalone `physis_voice` repo.
- **Sensory bus** (`src/sensory.rs`) — `AuraFrame` wire format (`'AURA'` magic,
  µs timestamp, `SignalType`, payload) over a Unix socket. `SignalType`:
  `AudioPCM`, `VisionFeature`, `Acceleration`, `Gyroscope`, `ThoughtCapture`. Any
  source that speaks `AuraFrame` streams into the same engine.

### Embedders

Selection is **self-test-gated**: at startup each candidate must pass a semantic
probe (dog≈puppy must beat dog≈tax-spreadsheet) or it is skipped. The chain:

1. **BGE-base-en-v1.5** (768-d, production default) — `models/bge-base-en-v1.5/`
2. **ONNX MiniLM** (384-d fallback) — `models/model.onnx` + `models/tokenizer.json`
3. **Random projection** (deterministic, no model files) — last resort; the
   server marks itself `degraded:true` on `/api/v1/embedder` and logs at `error`.

Model weights are git-ignored. Locally, export with `optimum-cli`:

```bash
pip install optimum onnx
optimum-cli export onnx -m BAAI/bge-base-en-v1.5 models/bge-base-en-v1.5/
cargo build --features embed-onnx     # (on by default; falls back down the chain)
```

In production, `docker-entrypoint.sh` fetches weights from `MODEL_BASE_URL`
at boot (see PREMORTEM.md P0). Check what's live with `GET /api/v1/embedder`.

### Docker (self-contained)

```bash
# Without Lean (default, smaller image)
docker build -t physis-pro .

# With Lean 4 toolchain (~+500 MB, enables apodeixis machine)
docker build --build-arg WITH_LEAN=1 -t physis-pro:lean .
```

The runtime image is Debian slim + the Rust binary + optional Lean. Models are
downloaded at boot from `MODEL_BASE_URL` unless baked in at build time via S3.

---

## Modules

| Module | Role |
|--------|------|
| `core` | `PhysisCore` — UberWiki, compression, dream, branch certification, context filter |
| `models` | `CoherenceRating`, `CoherenceNode`, `ConstructiveRefutation`, `Goal`, `Experience`, `Dream` |
| `trie` | `DynamicVectorTrie` token storage/retrieval |
| `classify` | Semiotic-grid classification (nearest-entry max-cosine over DOMAIN×MODE cells) |
| `discovery` | Ontology-gap discovery — embeds a corpus, flags texts the grid covers poorly (tunable threshold + surfaced sim distribution), clusters gaps → draft ontology JSON |
| `ontology` / `ontology_nonhuman` | Human + machine/non-human ontology loading |
| `config` | Config + ontology bootstrap |
| `scanner` | Filesystem scan with hash-based change detection |
| `mapper` | Filesystem → goal ontological mapping |
| `vfs` | Read-only ontology-organized view of files via symlinks |
| `actor` | PDCA (Plan-Do-Check-Act) cycle engine |
| `rachmaninov` | PDCA directive engine — `Focus`/`Expand`/`Prune`/`Synthesize`, state-vs-goal tick |
| `dream` | `DreamEngine` — stochastic generation (mutate / graft / prune). Reframed as the Philotes (integration) phase of one metabolism — see `docs/EMPEDOCLES_DYAD.md` |
| `empedocles` | **Research, quarantined.** `NodeField` under Eros (attraction + synthesis) vs Nemesis (repulsion + decay); the `empedocles_criticality` example confirms a phase transition. Wired to nothing on the product path |
| `graph` | Holarchic graph — `DenseSlotMap`, `NodePayload`, `Edge`, mmap storage, ONNX worker |
| `gantt` | `GanttTask` scheduler — start/end, deps, critical path over `RawNodeKey` |
| `sensory` | `AuraFrame` multimodal ingestion bus |
| `video` | ffmpeg frame-sample → clip vector |
| `embed` | `VectorEmbed` trait — BGE-base / ONNX-MiniLM / RandomProjection / CLIP / Jina |
| `quantize` | Product quantizer (f32 → byte codes) |
| `reconstruct` | Nearest-neighbour reconstruction + LLM interpretation |
| `storage` | Zero-copy mmap storage for `Pod` types |
| `output` | Formatters — Wiki / JSON graph / Mermaid / domain report |
| `linguistic` | Routing middleware — Wenyan / Pirahã / Sanskrit |
| `network` | Directory watch + network event detection (feature `network`) |
| `mcp` | MCP server for agent tool-use (feature `mcp`) |
| `ai` | Provider cascade (OpenAI/Anthropic/…), tool-using agent loop, episodic memory, ONNX worker |
| `life_os` | `LifeOSTracker` — goal/habit tracking layer |
| `social` | `SocialAlgorithm` — engagement metrics × coherence scoring |
| `quality` | `QualityTracker` — classify-failure feedback loop, persisted |
| `industrial` | Machine adapters (MQTT / Modbus / serial / REST) + anomaly detection (feature `industrial`) |
| `ingest` | Document classification/ingest pipeline (feature `ingest`) |
| `inventory` | Stock tracker — SMA-30 forecast + human-in-loop reorder |
| `oee` | Overall Equipment Effectiveness — `OeeShift`/`OeeBoard`, A×P×Q, six-big-losses Pareto, plant rollup |
| `maintenance` | Per-machine learned health baselines, σ-anomaly scoring, least-squares RUL projection |
| `report` | Italian shift report — composes OEE + anomalies + RUL + dream countermeasures into Markdown or self-contained HTML |
| `spanner` | Optional Google Spanner sink (feature `spanner`) — durable holarchy mirror: chunked upsert, hard deletes, `sync_full`, SA-JWT auth |
| `wiki` | `llmwiki` CLI fallback with recursive-grep offline retrieval |
| `cli` | clap CLI (feature `cli`) |

---

## Core API (Rust)

```rust
use physis_pro::core::PhysisCore;
use physis_pro::models::CoherenceRating;

let mut core = PhysisCore::new();
core.set_embedder_id("bge-base");   // vector provenance, stamped on every node

// Register by text — the text is kept as the label, so the node is browsable
// by content. Labeled registrations are deduped: same text ⇒ same id.
let id = core.register_node_from_text("exercise: running", &embedder);

// Report the outcome (the asserted axis). Re-asserting corrects a verdict.
core.assert_coherence(&id, CoherenceRating::Success.weight());

// Compression — validate a raw input against state
let result = core.filter_context_embedded(embedder.embed("running does nothing"), 4);
// result.valid == false; result.conflict == Some(ConstructiveRefutation { .. })

let certified  = core.certify_branches();
let isolated   = core.detect_contradictions();
let compressed = core.compress_logs(&daily_logs);   // Compression
let dreams     = core.dream();                      // Dream — replays failures
let density    = core.coherence_index();            // derived, over all nodes
let verdicts   = core.asserted_index();             // reported, judged nodes only
let snap       = core.snapshot();                   // for UI (both axes)

// Durability — nodes (and their verdicts) survive a restart.
core.persist_node(&db, "tenant-a", &id)?;
let (loaded, skipped) = core.load_nodes(&db, "tenant-a");  // skipped = embedder mismatch
```

---

## Build & test

```bash
# Quick build (default features: cli, voice, web, tui, embed-onnx)
cargo build

# Feature matrix
cargo build --features mcp        # + MCP server
cargo build --features network    # + directory watching
cargo build --features full       # everything (industrial, ingest, pdf, email)

# One-shot dependency installer (probes + installs rust, lean, voice deps, models)
./scripts/setup-physis.sh --full

# Then deploy (systemd --user, no root required)
./scripts/install-physis.sh --port 8080
```

> **Deploys:** Railway auto-deploys on green CI. When CI is unavailable, deploy
> by hand with `./scripts/predeploy.sh && railway up --service physis --detach`
> — the script mirrors the CI gate (clippy `-D warnings`, full tests, the
> accuracy floor) and refuses a dirty/unpushed tree, so prod always equals a
> pushed, gated commit (PREMORTEM P9).

### Features

`cli` · `mcp` · `network` · `voice` · `web` · `tui` · `embed-onnx` · `industrial`
· `ingest` · `spanner` · `wiki-embedded` · `full`. Default = `cli, voice, web, tui, embed-onnx`.

### Binaries

`physis-pro` (CLI) · `physis-pro-web` (HTTP) · `physis-pro-tui` · `physis-pro-voice`.

---

## See also

- `AGENTS.md` — multi-tenancy contract + agent integration notes
- `DEVELOPMENT_PLAN.md` — roadmap (Phase 6: multi-embedder registry, Gemma re-ranker)
- `PREMORTEM.md` — risk register (P4 ONNX pin, P6 open-relay gate)
- `MANUAL_CONFIG.md` — deploy/config steps
- CI: `.github/workflows/ci.yml` (builds on every push/PR)
