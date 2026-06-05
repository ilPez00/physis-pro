# Physis

**Standalone Runtime Engine — Ontology, Immunization, Token Economy**

Physis is the central engine of the Ayu Ecosystem. It governs three fundamental
architectural domains in both the local (Aura) and global (Praxis/Praxisweb.xyz)
contexts: the UberWiki, the Compression Function, and the Dream Function. It
communicates via MCP (Model Context Protocol) with AI agents and remote instances.

## Architecture: The Three Domains

```
                         ┌──────────────────────────┐
                         │     PRAXIS / AURA (UI)    │
                         │  Gantt · PDCA · MCP Tool  │
                         └───────────┬──────────────┘
                                     │ prompt / categorization
                                     ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                          PHYSIS (Runtime Core)                            │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │                     1. UBERWIKI (Immutable State)                    │ │
│  │                                                                      │ │
│  │  ┌─────────────────────┐    ┌──────────────────────────────────┐    │ │
│  │  │ DynamicVectorTrie   │    │ CoherenceNode (±1.0 weights)     │    │ │
│  │  │ (geometric causal   │◀──▶│ Success (+1.0) / Inert (0.0) /   │    │ │
│  │  │  structures)        │    │ Failure (-1.0)                   │    │ │
│  │  └─────────────────────┘    └──────────────────────────────────┘    │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
│                                    │                                      │
│                    ┌───────────────┴───────────────┐                      │
│                    ▼                               ▼                      │
│  ┌─────────────────────────────┐  ┌─────────────────────────────────┐   │
│  │ 2. COMPRESSION FUNCTION     │  │ 3. DREAM FUNCTION               │   │
│  │    (Waking Phase)           │  │    (Rest / Async Phase)         │   │
│  │                             │  │                                  │   │
│  │  • filter_context()         │  │  • dream() — predictive         │   │
│  │  • compress_logs()          │  │    collapse simulations         │   │
│  │  • Wenyan-density           │  │  • Collides contingent          │   │
│  │    summarization            │  │    failures with universal      │   │
│  │  • Token economy            │  │    UberWiki myths               │   │
│  │                             │  │  • Generates Effective Actions  │   │
│  │  Input: raw logs,           │  │  • Recalibrates the global      │   │
│  │  commands, telemetry        │  │    Coherence Index              │   │
│  └─────────────────────────────┘  └─────────────────────────────────┘   │
│                                                                           │
│  ┌───────────────────────────────────────────────────────────────────┐   │
│  │                 Linguistic Routing Middleware                       │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐                         │   │
│  │  │  Wenyan  │  │  Pirahã  │  │ Sanskrit │                         │   │
│  │  │ archive  │  │  logging │  │ dreaming │                         │   │
│  │  └──────────┘  └──────────┘  └──────────┘                         │   │
│  └───────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
                          ┌──────────────────────┐
                          │     Aura (sensors)    │
                          │  filesystem · network │
                          └──────────────────────┘
```

## Physis Core: The Three Domains

### 1. UBERWIKI — The Immutable State

The impersonal vector database of geometric causal structures. It maintains the
distinction between positive weights (+1.0, stable flows) and negative weights
(-1.0, frictions/failures). Implemented as a `DynamicVectorTrie` with
`CoherenceNode` nodes.

### 2. COMPRESSION FUNCTION — Waking Phase / Input

Acts as the real-time 'gauntlet' during AURA's local operations or PRAXIS
interactions:
- Strips raw logs (Rust compiler errors, commands, telemetry) of identities and
  syntactic redundancies.
- Reduces the input to the minimal causal tokens before passing them to the Host AI.
- Keeps the working context clean and immune to erroneous statistical inertia.

### 3. DREAM FUNCTION — Rest Phase / Asynchronous Processing

Runs in the background when the user or agents are inactive:
- Takes the failure fragments (0.0 or -1.0) accumulated in the local Wiki during
  the active cycle.
- Runs logical simulations to find the missing link of causality: it collides
  contingent failures with the universal myths of the UberWiki.
- Generates the Effective Actions, recalibrating the Coherence Index and laying
  the rails for the next cycle's Gantt.

## Coherence Ontology

Three isomorphic states applied identically to machine functions and human behaviors:

| State    | Weight | Machine                                       | Human                                          |
|----------|--------|-----------------------------------------------|------------------------------------------------|
| Success  | +1.0   | Compiled + functional test confirmed          | Action completed with a real, detected effect  |
| Inert    |  0.0   | Compiles but does not produce the expected effect | Task executed without cognitive progress    |
| Failure  | -1.0   | Compiler error or refuted pattern             | Violation of a self-imposed rule (diet slip)   |

## Validation Flow

```
raw_input ──▶ [Phase 1: scrub noise] ──▶ [Phase 2: extract causal constraints]
                                     ──▶ [Phase 3: inject vector history]
                                     ──▶ [Phase 4: check_consistency]
                                          │
                                ┌─────────┴─────────┐
                                ▼                   ▼
                              Clean             Conflict
                                                  │
                                                  ▼
                                      ConstructiveRefutation
                                      (suspension + PDCA notification)
```

If a user categorization conflicts with established Success nodes, the system
suspends execution and generates a Constructive Refutation payload for PDCA
cycle recalibration.

## Behavioral Isomorphism

`register_behavioural_vector(domain, action, outcome, reason)` records a human
action with the same logic as a software function:

```rust
// Diet violation → Failure (-1.0), coherence drops
core.register_behavioural_vector("Body & Fitness", "ate_cake",
    CoherenceRating::Failure, "voluntary_slip");

// Task completed without effect → Inert (0.0)
core.register_behavioural_vector("Intellectual", "studied_3h",
    CoherenceRating::Inert, "no_retention");

// The system will inject these constraints into the next planning cycle
```

## MCP Flows: AURA ↔ PRAXIS

Physis instances communicate over asynchronous MCP connections:

### Upstream (Local → Global)
1. Friction detected in the local Scheduler → failure recorded in the Local UberWiki
2. The Dream Phase isolates the resolving causal string, compresses and immunizes it
3. The package is pushed to the Global UberWiki as a universal Myth

### Downstream (Global → Local)
1. The user queries the Global UberWiki from the unified UI
2. Physis downloads the Global Wiki branch as an MCP resource
3. The Client's Ontological Filter re-clothes the abstract causal bone by injecting
   local parameters, constraints, and variables (hardware, resources, personal Gantt)
4. The adapted guidance becomes a positive constraint (+1.0) in the AI agent's context

## Modules

| Module | Description |
|--------|-------------|
| `core` | **PhysisCore** — UberWiki, Compression, Dream, branch certification, context filtering |
| `models` | Core types: CoherenceRating, CoherenceNode, ConstructiveRefutation, Goal, Experience, Dream |
| `scanner` | Filesystem scanning with hash-based change detection |
| `trie` | DynamicVectorTrie for token-based storage and retrieval |
| `mapper` | Ontological filesystem→goal mapping |
| `config` | Ontology loading (human + machine) |
| `actor` | PDCA (Plan-Do-Check-Act) cycle engine |
| `dream` | DreamEngine — stochastic generation (mutation, graft, prune) |
| `output` | Formatters: Wiki, JSON graph, Mermaid, domain report |
| `network` | Watch directories, network event detection |
| `mcp` | MCP server for external tool integration and Aura↔Praxis communication |
| `linguistic` | Linguistic routing middleware — Wenyan / Pirahã / Sanskrit |
| `cli` | CLI interface via clap |
| `graph` | Holarchic graph engine — `DenseSlotMap`, `NodePayload`, `Edge`, `MmappedStorage`, ONNX worker integration |
| `rachmaninov` | PDCA directive engine — `Focus`/`Expand`/`Prune`/`Synthesize`, state-vs-goal tick cycle |
| `sensory` | `SensorPayload` bridge — raw real-world input (audio, network) → graph node space |
| `gantt` | `GanttTask` scheduler — start/end, dependencies, `RawNodeKey` causal integration |
| `storage` | Zero-copy memory-mapped `MmappedStorage` for Pod types |
| `quantize` | Product quantizer — f32 vectors → byte codes, M centroids per sub-vector |
| `reconstruct` | Nearest-neighbor reconstruction and LLM-assisted interpretation |
| `embed` | `VectorEmbed` trait + `RandomProjectionEmbedder` (deterministic) + `OnnxEmbedder` (MiniLM) |
| `ai` | Provider cascade (OpenAI/Anthropic), tool-using agent loop, episodic memory, ONNX worker |

## Main API

### PhysisCore

```rust
use physis::PhysisCore;
use physis::models::{CoherenceRating, AxisKind};

let mut core = PhysisCore::new();

// Register coherence nodes
core.register_node("exercise:running", CoherenceRating::Success,
    AxisKind::Human, Some("Body & Fitness"));

// Filter a raw input (Compression Function)
let result = core.filter_context(
    "exercise:running is not producing any effect",
    AxisKind::Human,
    &ontology,
);
// result.valid == false
// result.conflict == Some(ConstructiveRefutation { ... })

// Transition Success → Inert
core.mark_inert("exercise:running", "no endurance gain detected");

// Certify stable branches
let certified = core.certify_branches(&ontology);

// Detect contradictions
let isolated = core.detect_contradictions();

// Compress daily logs → dense causal rules (Compression Function)
let compressed = core.compress_logs(&daily_logs);

// Run a predictive dream over Inert nodes (Dream Function)
let dreams = core.dream(&ontology);
// If collapse detected → preventive downgrade to Failure

// Coherence index (Stock Market metric)
let coherence = core.coherence_index(Some(AxisKind::Human));

// Snapshot for UI
let snap = core.snapshot();
```

## CLI

```
physis scan <dir>              # Scan a directory, build the ontological map
physis query <query>           # Query the trie
physis dream [--count N]       # Generate stochastic dreams
physis evaluate <id> <grade>   # Evaluate a dream (0.0-1.0)
physis watch <dirs...>         # Watch directories for changes
physis stats                   # Show engine statistics
physis config                  # Print the current configuration
physis serve                   # Start the MCP server (feature: mcp)
```

## Configuration

```json
{
  "data_dir": "~/.physis",
  "dream_batch_size": 5,
  "pdca_stagnant_threshold": 0.2,
  "pdca_stagnant_window": 5
}
```

Default ontologies: `config/praxis_ontology.json` (human, 14+ domains) and
`config/machine_ontology.json` (machine, 50+ domains).

## Holarchy — Real-time Graph Engine

Physis now includes a **holarchic graph engine** for real-time causal processing:

- **Graph** (`graph.rs`) — `DenseSlotMap<NodeKey, NodePayload>` with zero-copy `Pod` types, edges with semantic weights, `MmappedStorage` for persistence. New nodes/edges flow through an ONNX worker thread for async embedding enrichment.
- **Rachmaninov Holon** (`rachmaninov.rs`) — PDCA-driven directive engine. `tick(state_vector)` compares current state vs goal vector, emits `Focus`, `Expand`, `Prune`, or `Synthesize` directives when coherence drifts past threshold.
- **Sensory Pipeline** (`sensory.rs`) — Packed binary `SensorPayload` structs (timestamp + f32x4 vector) bridging real-world input to graph node space.
- **ONNX Worker** (`ai/onnx_worker.rs`) — Background thread pool processing embedding requests via `crossbeam_channel`. Non-blocking from the web server.
- **Gantt Scheduling** (`gantt.rs`) — `GanttTask` with start/end, dependencies, `RawNodeKey` integration for critical path analysis.

## Semiotic Grid & Ontology Expansion

Physis Pro ships **10 ontologies** (630 domain entries) mapped onto a **5-domain × 6-mode semiotic grid**:

| Ontology | Kind | Entries | Coverage |
|----------|------|---------|----------|
| `praxis_ontology` | human | 54 | Human activities (body, career, social, finance, etc.) |
| `machine_ontology` | machine | ~110 | CNC + 23 industrial process monitoring domains (thermal, pressure, speed, flow, OEE, SPC, etc.) |
| `semiotic_ontology` | semiotic | 44 | Peirce, Saussure, Barthes, Eco, Jakobson, Greimas |
| `category_ontology` | category | 54 | Category theory — objects, morphisms, functors, monads |
| `agent_ontology` | agent | ~69 | AI cognitive architectures + 21 agent workflow domains (prompt pipeline, tool execution, agent lifecycle) |
| `natural_ontology` | natural | 56 | Physics, chemistry, biology, astronomy, ecology |
| `social_ontology` | social | 50 | Sociology, culture, politics, economics, law |
| `abstract_ontology` | abstract | 56 | Math, logic, computation, systems, linguistics, info |
| `engineering_ontology` | engineering | 65 | Software, civil, mechanical, electrical, chemical, etc. |
| `office_ontology` | office | 75 | Communication, Finance, Procurement, People, Projects, Customer, Reporting, IT, Facilities, Legal, Marketing, Travel, R&D |

Plus **20 extra domain ontologies** (`software_dev`, `devops`, `data_science`, `cybersecurity`, `health_fitness`, `nutrition_cooking`, `personal_finance`, `learning_education`, `creative_writing`, `music`, `visual_arts`, `relationships`, `productivity`, `travel`, `home_maintenance`, `mindfulness_spirituality`, `science_research`, `business_entrepreneurship`, `language_learning`, `outdoors_sports`) baked in via `EXTRA_ONTOLOGIES` and used by `classify`.

Each entry maps to one of 30 grid cells: **5 domains** (HEAL, CONSTRUCT, FABRICATE, BOND, STUDY) × **6 modes** (LIFT, REST, WALK, WORK, CREATE, LEARN).

### Semiotic API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/ontology/list` | GET | Domain counts per ontology |
| `/api/v1/semiotic/grid` | GET | 30-cell grid with entries per cell |
| `/api/v1/semiotic/triangle` | GET | Peircean semiotic triangle (Mermaid) |
| `/api/v1/semiotic/square` | GET | Greimas semiotic square (Mermaid) |
| `/api/v1/semiotic/heatmap` | GET | Activation heatmap table + matrix |
| `/api/v1/category/diagram` | POST | Custom category diagram (objects + morphisms) |
| `/api/v1/classify` | POST | Embed query text → classify against the populated DOMAIN×MODE centroids. Returns sorted `{domain, mode, score, entries}[]` |
| `/api/v1/embed` | POST | Raw deterministic embedding vector + dimension |

### Query Classification

```bash
curl -X POST http://127.0.0.1:19876/api/v1/classify \
  -H "Content-Type: application/json" \
  -d '{"text":"a serene landscape painting with soft brush strokes"}'
# → top: FABRICATE×CREATE (0.89) — Visual Arts, Storytelling, Music & Performance
```

At startup, all ontology entries are embedded (name + hints concatenated) and
averaged per DOMAIN×MODE cell. At query time, the same embedder scores input
text against all centroids via cosine similarity.

## Business Backoffice Modules

Physis Pro adds 5 new modules for industrial backoffice automation on top of the core engine:

### Quality (`src/quality.rs`)
`QualityTracker` implements a feedback loop: classify an output as "wrong" → embed against the semiotic grid → identify top cell → apply penalty → persist → adjust future scores.
```rust
let mut qt = QualityTracker::load_or_new()?;
qt.report_failure(&output_text, &ontology);
qt.save()?;  // persists to ~/.physis/quality_failures.json
```

### Industrial (`src/industrial.rs`)
Machine adapter trait with pluggable protocol backends (MQTT, Modbus, serial, REST).
```rust
let adapter = MqttAdapter::new("tcp://localhost:1883");
let reading: MachineReading = adapter.read()?;
let anomalies = detect_anomalies(&readings);
```

### Ingest (`src/ingest.rs`)
Document classification and ingestion pipeline — classifies files, structures them, and routes by kind (Invoice, Report, Contract, Manual, etc.).

### Inventory (`src/inventory.rs`)
Stock tracker with SMA-30 moving average forecast and human-in-loop ordering with cooldown:
```rust
inv.upsert("widget", 100.0);
let should_order = inv.check_reorder("widget", reorder_point)?;
inv.record_order("widget", 500, "manual")?;
```

### Wiki (`src/wiki.rs`)
`llmwiki` CLI fallback with recursive grep fallback for offline vector retrieval.

## Multimodal Sensory (Voice · Images · Video)

Physis is a single multimodal engine: every modality is reduced to a vector in
the **shared embedding space**, then classified / coheres / dreams through the
same core. The former standalone **`physis_voice`** project is folded in here
(see `src/bin/voice.rs` — keep the old repo for reference only).

### Voice — `physis-pro-voice`

Real-time ontological mapping for live talks/lectures. Pipeline:

```
arecord (44.1 kHz) ─15s chunks─▶ whisper-large-v3 (provider cascade)
  ─transcript─▶ llama3.2 concept extraction ─▶ OntologicalMap (entities + relations)
  ─▶ live graph UI  http://localhost:3000   (+ optional Dream provocations)
```

- Bin: `cargo run --bin physis-pro-voice` (features `voice,web`). UI routes:
  `GET /`, `GET /api/data`, `POST /api/settings`.
- Settings: `record_secs` (15), `transcription_model` (`whisper-large-v3`),
  `extraction_model` (`llama3.2`), `show_dreams`.
- Supersedes standalone `physis_voice` (arecord → Groq/Deepgram → Ollama →
  Cytoscape). Same `OntologicalMap` model, now sharing this engine's cascade,
  ontology classifier, and Dream Function.

### Images

Real today — image vectors land in the **same space** as text, so a photo and a
sentence are directly comparable / classifiable.

- `embed_image(bytes)` implemented on **CLIP** (`embed/clip.rs`) and **Jina v2**
  (`embed/jina.rs`); the `VectorEmbed` trait default is text-fallback.
- HTTP: `POST /api/v1/classify` and `POST /api/v1/embed` accept an optional
  base64 `image` (PNG/JPEG) → image embedding; on decode/availability failure
  they fall back to embedding `text` (logged), never 500.
- Sensory bus carries images as `SignalType::VisionFeature` frames.

### Video

**Path (partial — not yet a built pipeline):** sample frames (via the `image`
crate / ffmpeg) → `embed_image` per frame → temporal pool (mean / attention)
into one clip vector → classify/cohere like any other. Tracked in
`DEVELOPMENT_PLAN.md`. Audio track reuses the voice pipeline above.

### Sensory protocol — `AuraFrame` (`src/sensory.rs`)

The multimodal ingestion bus (Unix socket) that feeds the holarchy. Wire format
is a fixed `AuraFrameHeader` (`'AURA'` magic, µs timestamp, signal type, payload
len) + payload, with `SignalType`:

| Variant | Modality |
|---------|----------|
| `AudioPCM` | raw audio (voice) |
| `VisionFeature` | image / video-frame features |
| `Acceleration`, `Gyroscope` | motion (wearable / Aura) |
| `ThoughtCapture` | text / note capture |

Any source that speaks `AuraFrame` (the voice bin, a camera client, the Aura
wearable) streams into the same engine.

## Future: Multi-Modal & Gemma Re-ranking

See `DEVELOPMENT_PLAN.md` Phase 6 for the full roadmap. Short version:

- **Multi-embedder registry** — CLIP, Jina v2, SigLIP for text+image in shared vector space
- **Jina v2** recommended primary (1024-d, multilingual, beats CLIP)
- **Ontology expansion** — fill all 30 cells to 30+ entries each (900+ total), human+machine
- **Gemma re-ranker** — vector classify first (5ms), fall back to Gemma for ambiguous cases
- **Gemma as pure mapper** — optional mode: skip centroids, use Gemma prompt for all classification. Trade: 5ms→5s per query, but with reasoning and context awareness

### ONNX MiniLM

For production-grade semantic embeddings (instead of deterministic random projection):

```bash
pip install optimum onnx
optimum-cli export sentence-transformers models/sentence-transformers/all-MiniLM-L6-v2
# Creates models/model.onnx and models/tokenizer.json
```

Enable with `--features embed-onnx`. Falls back to RP when model files are absent.

## Build

```bash
cargo build                    # Default (CLI + voice + web + TUI)
cargo build --features mcp     # With MCP server support
cargo build --features network # With directory watching
cargo build --features full    # All features
cargo test                     # Run the test suite
cargo test core                # Test the Physis Core engine
```

## CI/CD

The project compiles automatically on GitHub Actions on every push and PR.
See `.github/workflows/ci.yml`.
