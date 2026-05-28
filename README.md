# Physis

**Motore di Runtime Standalone — Ontologia, Immunizzazione, Economia dei Token**

Physis è il motore centrale dell'Ecosistema Ayu. Governa tre domini architetturali
fondamentali sia nel contesto locale (Aura) che globale (Praxis/Praxisweb.xyz):
la UberWiki, la Funzione Compressione e la Funzione Sogno. Comunica tramite MCP
(Model Context Protocol) con gli agenti IA e con le istanze remote.

## Architettura: I Tre Domini

```
                         ┌──────────────────────────┐
                         │     PRAXIS / AURA (UI)    │
                         │  Gantt · PDCA · MCP Tool  │
                         └───────────┬──────────────┘
                                     │ prompt / categorizzazione
                                     ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                          PHYSIS (Runtime Core)                            │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │                     1. UBERWIKI (Stato Immutabile)                   │ │
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
│  │ 2. FUNZIONE COMPRESSIONE   │  │ 3. FUNZIONE SOGNO               │   │
│  │    (Fase di Veglia)        │  │    (Fase di Riposo / Async)     │   │
│  │                             │  │                                  │   │
│  │  • filtra_contesto()        │  │  • dream() — simulazioni        │   │
│  │  • compress_logs()          │  │    di collasso predittivo       │   │
│  │  • Wenyan-density           │  │  • Collide fallimenti           │   │
│  │    summarization            │  │    contingenti con miti         │   │
│  │  • Token economy            │  │    universali della UberWiki    │   │
│  │                             │  │  • Genera Azioni Efficaci       │   │
│  │  Input: log grezzi,         │  │  • Ricalibra Indice di          │   │
│  │  comandi, telemetria        │  │    Coerenza globale             │   │
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
                          │     Aura (sensori)    │
                          │  filesystem · network │
                          └──────────────────────┘
```

## Physis Core: I Tre Domini

### 1. UBERWIKI — Lo Stato Immutabile

Il database vettoriale impersonale delle strutture causali geometriche.
Mantiene la distinzione tra i pesi positivi (+1.0, flussi stabili) e
negativi (-1.0, attriti/fallimenti). Implementato come `DynamicVectorTrie`
con nodi `CoherenceNode`.

### 2. FUNZIONE COMPRESSIONE — Fase di Veglia / Input

Agisce come il 'guanto' in tempo reale durante le operazioni locali di AURA
o le interazioni su PRAXIS:
- Sveste i log grezzi (errori del compilatore Rust, comandi, telemetria)
  dalle identità e dalle ridondanze sintattiche.
- Riduce l'input ai minimi token causali prima di passarli all'IA Host.
- Mantiene il contesto di lavoro pulito e immune da inerzie statistiche errate.

### 3. FUNZIONE SOGNO — Fase di Riposo / Elaborazione Asincrona

Agisce in background quando l'utente o gli agenti non sono attivi:
- Prende i frammenti di fallimento (0.0 o -1.0) accumulati nella Wiki locale
  durante il ciclo attivo.
- Esegue simulazioni logiche per trovare l'anello mancante della causalità:
  fa collidere i fallimenti contingenti con i miti universali della UberWiki.
- Genera le Azioni Efficaci, ricalibrando l'Indice di Coerenza e preparando
  i binari per il Gantt del ciclo successivo.

## Ontologia della Coerenza

Tre stati isomorfi applicati identicamente a funzioni macchina e comportamenti umani:

| Stato    | Peso | Macchina                                      | Umano                                         |
|----------|------|-----------------------------------------------|-----------------------------------------------|
| Success  | +1.0 | Compilato + test funzionale confermato        | Azione completata con effetto reale rilevato   |
| Inert    |  0.0 | Compila ma non produce l'effetto atteso       | Task eseguito senza avanzamento cognitivo      |
| Failure  | -1.0 | Errore compilatore o pattern smentito         | Violazione ordine auto-imposto (sgarro dieta)  |

## Flusso di Validazione

```
input_grezzo ──▶ [Phase 1: scrub rumore] ──▶ [Phase 2: estrai vincoli causali]
                                         ──▶ [Phase 3: inietta storico vettoriale]
                                         ──▶ [Phase 4: check_consistency]
                                              │
                                    ┌─────────┴─────────┐
                                    ▼                   ▼
                                  Clean             Conflict
                                                      │
                                                      ▼
                                          ConstructiveRefutation
                                          (sospensione + notifica PDCA)
```

Se una categorizzazione utente entra in conflitto con nodi Success consolidati,
il sistema sospende l'esecuzione e genera un payload di Smentita Costruttiva
per la ricalibrazione del ciclo PDCA.

## Isomorfismo Comportamentale

`register_behavioural_vector(domain, action, outcome, reason)` registra un'azione
umana con la stessa logica di una funzione software:

```rust
// Violazione dieta → Failure (-1.0), coerenza scende
core.register_behavioural_vector("Body & Fitness", "ate_cake",
    CoherenceRating::Failure, "sgarro_volontario");

// Task completato senza effetto → Inert (0.0)
core.register_behavioural_vector("Intellectual", "studied_3h",
    CoherenceRating::Inert, "nessuna_ritenzione");

// Il sistema inietterà questi vincoli nel prossimo ciclo di pianificazione
```

## Flussi MCP: AURA ↔ PRAXIS

Le istanze di Physis comunicano tramite connessioni MCP asincrone:

### Upstream (Locale → Globale)
1. Rilevamento d'attrito nello Scheduler locale → fallimento registrato in UberWiki Locale
2. Fase Sogno isola la stringa causale risolutiva, la comprime e immunizza
3. Il pacchetto viene spinto sulla UberWiki Globale come Mito universale

### Downstream (Globale → Locale)
1. L'utente interroga la UberWiki Globale dalla UI unificata
2. Physis scarica il ramo della Wiki Globale come risorsa MCP
3. Il Filtro Ontologico del Client riveste l'osso causale astratto iniettando
   parametri, vincoli e variabili locali (hardware, risorse, Gantt personale)
4. La guida adattata diventa vincolo positivo (+1.0) nel contesto dell'agente IA

## Moduli

| Modulo | Descrizione |
|--------|-------------|
| `core` | **PhysisCore** — UberWiki, Compressione, Sogno, certificazione rami, filtraggio contesto |
| `models` | Core types: CoherenceRating, CoherenceNode, ConstructiveRefutation, Goal, Experience, Dream |
| `scanner` | Filesystem scanning con hash-based change detection |
| `trie` | DynamicVectorTrie per storage e retrieval token-based |
| `mapper` | Mapping ontologico filesystem→goal |
| `config` | Caricamento ontologie (umana + macchina) |
| `actor` | PDCA (Plan-Do-Check-Act) cycle engine |
| `dream` | DreamEngine — generazione stocastica (mutation, graft, prune) |
| `output` | Formattatori: Wiki, JSON graph, Mermaid, domain report |
| `network` | Watch directories, network event detection |
| `mcp` | MCP server per integrazione strumenti esterni e comunicazione Aura↔Praxis |
| `linguistic` | Middleware di routing linguistico — Wenyan / Pirahã / Sanskrit |
| `cli` | Interfaccia CLI via clap |
| `graph` | Holarchic graph engine — `DenseSlotMap`, `NodePayload`, `Edge`, `MmappedStorage`, ONNX worker integration |
| `rachmaninov` | PDCA directive engine — `Focus`/`Expand`/`Prune`/`Synthesize`, state-vs-goal tick cycle |
| `sensory` | `SensorPayload` bridge — raw real-world input (audio, network) → graph node space |
| `gantt` | `GanttTask` scheduler — start/end, dependencies, `RawNodeKey` causal integration |
| `storage` | Zero-copy memory-mapped `MmappedStorage` for Pod types |
| `quantize` | Product quantizer — f32 vectors → byte codes, M centroids per sub-vector |
| `reconstruct` | Nearest-neighbor reconstruction and LLM-assisted interpretation |
| `embed` | `VectorEmbed` trait + `RandomProjectionEmbedder` (deterministic) + `OnnxEmbedder` (MiniLM) |
| `ai` | Provider cascade (OpenAI/Anthropic), tool-using agent loop, episodic memory, ONNX worker |

## API Principale

### PhysisCore

```rust
use physis::PhysisCore;
use physis::models::{CoherenceRating, AxisKind};

let mut core = PhysisCore::new();

// Registra nodi di coerenza
core.register_node("exercise:running", CoherenceRating::Success,
    AxisKind::Human, Some("Body & Fitness"));

// Filtra un input grezzo (Funzione Compressione)
let result = core.filtra_contesto(
    "exercise:running is not producing any effect",
    AxisKind::Human,
    &ontology,
);
// result.valid == false
// result.conflict == Some(ConstructiveRefutation { ... })

// Transizione Success → Inert
core.mark_inert("exercise:running", "no endurance gain detected");

// Certifica rami stabili
let certified = core.certify_branches(&ontology);

// Rileva contraddizioni
let isolated = core.detect_contradictions();

// Comprimi log giornalieri → regole causali dense (Funzione Compressione)
let compressed = core.compress_logs(&daily_logs);

// Esegui sogno predittivo su nodi Inert (Funzione Sogno)
let dreams = core.dream(&ontology);
// Se collasso rilevato → downgrade preventivo a Failure

// Indice di coerenza (Stock Market metric)
let coherence = core.coherence_index(Some(AxisKind::Human));

// Snapshot per UI
let snap = core.snapshot();
```

## CLI

```
physis scan <dir>              # Scansiona directory, costruisce mappa ontologica
physis query <query>           # Interroga il trie
physis dream [--count N]       # Genera sogni stocastici
physis evaluate <id> <grade>   # Valuta un sogno (0.0-1.0)
physis watch <dirs...>         # Monitora directory per cambiamenti
physis stats                   # Mostra statistiche motore
physis config                  # Stampa configurazione corrente
physis serve                   # Avvia server MCP (feature: mcp)
```

## Configurazione

```json
{
  "data_dir": "~/.physis",
  "dream_batch_size": 5,
  "pdca_stagnant_threshold": 0.2,
  "pdca_stagnant_window": 5
}
```

Ontologie predefinite: `config/praxis_ontology.json` (umana, 14+ domini) e
`config/machine_ontology.json` (macchina, 50+ domini).

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
| `office_ontology` | office | 75 | **NEW** — Communication, Finance, Procurement, People, Projects, Customer, Reporting, IT, Facilities, Legal, Marketing, Travel, R&D |

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
| `/api/v1/classify` | POST | Embed query text → classify against 22 populated DOMAIN×MODE centroids. Returns sorted `{domain, mode, score, entries}[]` |

### Query Classification

```bash
curl -X POST http://127.0.0.1:19876/api/v1/classify \
  -H "Content-Type: application/json" \
  -d '{"text":"a serene landscape painting with soft brush strokes"}'
# → top: FABRICATE×CREATE (0.89) — Visual Arts, Storytelling, Music & Performance
```

At startup, all 630 ontology entries are embedded (name + hints concatenated) and averaged per DOMAIN×MODE cell. At query time, the same embedder scores input text against all centroids via cosine similarity.

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
cargo build --features mcp     # Con supporto server MCP
cargo build --features network # Con directory watching
cargo build --features full    # Tutte le feature
cargo test                     # Esegue la test suite
cargo test core                # Test del Physis Core engine
```

## CI/CD

Il progetto compila automaticamente su GitHub Actions a ogni push e PR.
Vedi `.github/workflows/ci.yml`.
