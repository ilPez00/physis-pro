# physis_pro — Re-inspection & Product Rethink

> Working notes from a deep code re-inspection (2026-07-12). Goal: understand
> what physis_pro actually is, where a Palantir-like ontology layer fits, and
> how to turn the crate into a sellable product. NOT a plan yet — a map.

## 1. What physis_pro actually is (grounded in code)

A Rust runtime engine that is the reasoning core of the Ayu/Praxis ecosystem.

Core loop (from README + core.rs + graph.rs):
- **One shared embedding space.** Text, image (CLIP/Jina), audio (whisper→text),
  and machine telemetry all reduce to a vector in the same space.
- **Semiotic grid classification** (classify.rs): ~370 DOMAIN×MODE cells, each
  backed by member-entry embeddings. Score = nearest-entry max cosine (+ a
  domain-prior blend, ALPHA=0.4). This is nearest-neighbour, not a model — fast,
  offline, deterministic.
- **Coherence weight** (+1.0 / 0.0 / −1.0): every node carries whether it "works".
  Machine functions and human behaviours are scored with the *same* logic
  (core.rs `register_behavioural_vector`) — that symmetry is the philosophical
  spine (Empedoclean dyad: Strife/discrimination vs Love/integration).
- **Dream loop** (dream.rs): background replay of Inert/Failure nodes → proposes
  corrective Effective Actions + recalibrates the global coherence index.
- **Holonic graph** (graph.rs): typed-node (`NodePayload`: activation/decay/prune
  flags) + weighted-edge (`Edge`: source/target/weight/`semantic_type: u32`)
  graph, mmap-backed, with a `dream_cycle` that prunes low-weight edges and
  decays activation. This is the *knowledge-graph substrate*.
- **Discovery / axes** (discovery.rs, axes.rs): propose new cells (domain×mode
  intersections) and new axes (whole new dimensions) from clusters the grid
  can't name. Operator-in-the-loop (samples + suggestions, not auto-commit).

Surfaces: CLI, HTTP server (`physis-pro-web`, 98 routes, X-Physis-User tenancy),
MCP server, TUI, voice binary. Multi-tenant by header.

## 2. What's already genuinely sellable (the under-sold half)

The crate is framed as "the reasoning core," but the *valuable product* is the
industrial-backoffice layer that's already built:

- **OEE** (oee.rs): plant/shift Overall Equipment Effectiveness, stop reasons.
- **Predictive maintenance** (maintenance.rs): RUL estimate, health baseline,
  anomaly score (sigma threshold). Real condition-monitoring.
- **Inventory** (inventory.rs): SMA-30 forecast + human-in-loop order cooldown.
- **Document ingest/classification** (ingest.rs): xlsx/csv/xml → classified.
- **Quality feedback loop** (quality.rs): embed failures, persist, penalise.

These are *operational* capabilities a factory or backoffice would pay for. The
ontology/classification/dream machinery is the *differentiator* that makes them
adaptive (the grid learns new domains from your data via discovery), but the
buyer is buying OEE + maintenance + inventory + document intelligence, not
"a coherence engine."

**Insight:** physis_pro is currently a brilliant substrate wearing the wrong
coat. The product is "adaptive industrial intelligence" — not "an ontology
engine for the Ayu ecosystem."

## 3. Where a Palantir-like ontology layer fits

Palantir Ontology = typed, property-bearing **Objects** connected by typed,
bidirectional **Links**, with **Actions** that mutate objects, all on a
**temporal** (versioned) layer, surfaced to analysts as a navigable graph.

physis_pro today has the *substrate* but not the *type system*:

| Palantir concept      | physis_pro today                          | Gap |
|-----------------------|-------------------------------------------|-----|
| Object (typed)        | `CoherenceNode` (id + weight + embedding) | No type, no properties, no schema |
| Link (typed, bidir)   | `Edge {weight, semantic_type: u32}`       | u32 enum only, no properties, no inverse |
| Link/object properties| —                                         | Absent (nodes hold 1 f32 weight) |
| Actions on objects    | `dream()` emits free-text actions         | Not typed, not bound to object mutation |
| Temporal / versioned  | `decay` only (float decay)                | No event log / time-travel |
| Analyst-facing graph  | TUI graph-walk, wiki export                | No query language, no API for "objects" |

**The opportunity:** make the HolonicGraph a real *typed ontology* without
throwing away the embedding/coherence/dream engine that is physis_pro's actual
edge. Specifically:

1. **Typed nodes.** Promote `CoherenceNode` → `OntologyObject { type: String,
   id, properties: Json, coherence: f32, embedding }`. The ~370 grid cells become
   *object types* (not just classification buckets). A "Machine", "WorkOrder",
   "SensorReading", "Invoice" are object types, each with a schema.
2. **Typed, bidirectional links.** `Edge` → `OntologyLink { type, source, target,
   props, weight, inverse? }`. e.g. `WorkOrder --runsOn--> Machine`,
   `SensorReading --measures--> Machine`. The existing `semantic_type: u32`
   becomes a named, schema'd link type. Inverse traversal for "what depends on
   this machine."
3. **The embedding + coherence layer sits ON TOP of the typed graph.** This is
   the differentiation: every object/link also has a vector, so you can do
   *semantic* queries ("show me everything vaguely like this failure") AND
   *structural* queries ("critical-path machines upstream of line 3") in one
   engine. Palantir does structural + property; physis_pro can add semantic.
4. **Dream as Actions.** The dream loop already proposes corrective actions —
   bind them to typed object mutations ("open WorkOrder X", "schedule
   maintenance on Machine Y") so the ontology is actionable, not just analytic.
   This is the "decide + act" half Palantir's Action framework covers.
5. **Temporal.** Add an append-only event log of object/link mutations (the
   coherence weight changes, dream actions) → time-travel, audit, "what did the
   plant graph look like when OEE dropped." This is the missing trust layer for
   an industrial buyer.

## 4. How to make it a sellable product (concrete path)

**Positioning:** "Physis — adaptive industrial intelligence. One engine that
classifies your shop-floor text, telemetry, and documents into a live typed
ontology, predicts failures, and proposes the next action — on-prem, no
black-box LLM required for the core."

**Packaging (3 tiers):**
- **Edge runtime** (the Rust binary): OEE + maintenance + inventory + document
  ingest, runs on a factory box / a Pi-class machine. The differentiator vs
  fixtures like a PLC historian: it *learns your vocabulary* from documents via
  discovery, no ontology-engineering services engagement.
- **Ontology Studio** (the `discovery`/`axes` + graph browser): operators review
  proposed cells/axes, browse the typed graph, export to JSON. This is the
  wedge — it makes the customer's data *legible* in week 1.
- **Action layer** (dream → typed actions → integrations): push to ERP/MES/
  ticketing. This is the recurring-value + lock-in layer.

**Why it's defensible (not another RAG/LLM wrapper):**
- Core classification + coherence + dream are deterministic, offline, embeddable
  — works air-gapped on a shop floor with no internet. That's a hard requirement
  for many industrial buyers and most "AI for manufacturing" startups fail it.
- The adaptive ontology (discovery proposes cells/axes from *your* data) replaces
  the expensive ontology-modelling consulting that Palantir/Foundry engagements
  bill for. That's the wedge against Palantir: same typed-ontology end-state, but
  the ontology *grows itself* from your documents instead of being hand-modelled
  by a deployment team.

**What's missing to be saleable (honest gap list):**
1. No real authn/z on the web server beyond a header + 37 token mentions — need
   real tenant isolation + RBAC before any multi-customer SaaS.
2. The ontology is code/JSON-baked, not a live, queryable property graph API.
3. No integrations (ERP/MES/ticketing) — dream actions are free text.
4. No event log / temporal layer.
5. No deployment story beyond Railway + a 155KB web_server.rs monolith; need a
   clean distro (single binary + config + models) and an on-prem install path.
6. No pricing/packaging doc, no demo scenario, no one-click dataset→ontology
   flow a prospect can run in 10 minutes.

## 5. Suggested sequence (not yet a plan)

A. Ship the *Ontology Studio* wedge first: discovery + graph browser + export,
   on a prospect's own documents, in one sitting. (Highest "wow", lowest build.)
B. Promote `CoherenceNode`/`Edge` to typed `OntologyObject`/`OntologyLink` with
   property bags + named link types — without breaking the embed/coherence/dream
   engine (it just reads `.coherence`/`.embedding`).
C. Bind `dream()` outputs to typed actions; add an event log for temporal.
D. Then harden: authn/RBAC, integrations, on-prem installer, pricing.

## 6. Concrete demo and connector boundary

See [DEMO_AND_CONNECTORS.md](DEMO_AND_CONNECTORS.md) for a product demo built
around a Klipper 3D printer, local workflow files, email threads, and explicit
human verdicts. The printer is a safe validation environment for the ingestion
and feedback loop; it is not evidence that Physis already performs industrial
predictive maintenance.

The important architectural step is to normalize all sources into a typed,
timestamped `OperationalEvent` with provenance. Moonraker, system activity, and
mailbox synchronization should be adapters around that model rather than new
special cases inside the ontology engine.

The first printer adapter now lives in `src/moonraker.rs`; the remaining work
is the live demo surface and the incident/timeline views that consume the
normalized event stream.

The engine is genuinely good. The work is *productisation*, not more research.
