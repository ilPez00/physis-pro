# Build Plan — Physis Philosophical Machines

Concrete plan to build the machines sketched in `PHILOSOPHICAL_MACHINES.md`. Each
is a thin client over the physis core; the core now exposes everything a machine
needs, including the **id bridge** (coherence id ↔ holarchy key) so every machine
reads/writes one shared graph.

> Naming note: the dialogue machine is **elenchus** (Socratic cross-examination),
> NOT "agora" — `agora` is already the social network.

---

## Shared substrate (already shipped)

| Need | Provided by |
|------|-------------|
| Reduce text/image/audio/video → vector | `/api/v1/embed`, `/api/v1/classify` (5×6 grid) |
| Register a primitive as a recallable node | `/api/v1/coherence/register { input }` (embeds + labels + dedups) |
| Join a node into the shared graph | `PhysisCore::bridge_node(coh_id)` → holarchy key |
| Edges with stable ids | `PhysisCore::resolved_edges()` / `spanner::export_edges` |
| Recall by content | `/api/v1/reconstruct`, `physis-pro-wiki` TUI |
| Coherence (+1/0/−1) + dream | `register_behavioural_vector`, `/api/v1/dream/*`, PDCA |
| Per-user isolation | `X-Physis-User` tenant header |
| Durable mirror | `spanner` feature (scaffold) |

## The machine skeleton (every machine implements two things)

1. **Reducer** — `modality input → primitives`. A primitive is the smallest
   *elaborable* unit (euclid: a polygon, not a circle; harmony: an interval, not
   a note). Each primitive → `/coherence/register` (label = its text form) → then
   `bridge_node` + an edge to its neighbours.
2. **Elaborator** — `(primitive, generation) → richer form`. On re-encounter,
   generation++ and the form deepens (euclid's fractal SVG is the template).

Everything else — recall, coherence, dream, durability — is inherited. A new
machine is ~a reducer + an elaborator + a perceive loop.

Recommended shared crate: `machine-kit` (Rust) or a small TS client — a `Reducer`
trait + a physis HTTP client + the perceive→reduce→compose→elaborate→remember
loop, so each machine is just the two impls.

---

## Per-machine plans

Ordered by leverage × ease. ✦ = reuses an already-shipped piece heavily.

### 1. chronos — temporal machine ✦
- **Source:** Praxis `notebook_entries` (waypoints already have time+place) + tracker cadence.
- **Primitives:** recurrence units — rhythm (weekly/monthly), streak, anniversary, deadline.
- **Compose:** temporal graph; edges = "precedes / co-occurs / breaks".
- **Elaborate:** predict next instance; generation = confidence from history depth.
- **Dream:** surface a rhythm about to break before it does.
- **MVP:** read notebook+tracker → register recurrence nodes → a "what's due / what's slipping" view. Small; data already exists.

### 2. soma — body machine ✦
- **Source:** wearable signals (HR, motion, sleep) — or manual tracker logs today.
- **Primitives:** effort / recovery / drift episodes.
- **Coherence:** a protocol is `+1` only if it produced a detected effect — this is
  literally `register_behavioural_vector(Success/Inert/Failure)`, already built.
- **Elaborate:** propose the next training tweak; dream over Inert protocols.
- **MVP:** map tracker logs → behavioural vectors → coherence dashboard "what works for me".

### 3. techne — codebase machine ✦
- **Source:** repo scan (`/api/v1/scan` already registers file + structural-line nodes).
- **Primitives:** beyond paths — capability ("auth", "embedding"), dependency.
- **Compose:** capability graph via `bridge_node` + edges between co-changed files.
- **Elaborate:** trace "what breaks if this changes"; dream flags an architectural smell.
- **MVP:** extend scan-labelling with capability tags (classify each file) + edges from import graph. Builds directly on Phase 7B.

### 4. elenchus — dialogue / dialectic machine
- **Source:** a conversation (chat, transcript via `physis-pro-voice`).
- **Primitives:** claim / warrant / rebuttal (Toulmin).
- **Compose:** argument graph; edges = "supports / refutes / qualifies".
- **Coherence:** a claim is `+1` only if it ever produced a real effect; conflicts
  trigger the existing **ConstructiveRefutation** path (suspend + PDCA).
- **Elaborate:** find the missing premise; deepen on re-argued topics.
- **MVP:** parse a transcript → claim nodes → contradiction detection via `detect_contradictions()`.

### 5. lex — legal / contract machine ✦
- **Source:** documents (`ingest` feature already classifies invoices/contracts).
- **Primitives:** obligation / right / condition.
- **Compose:** obligations graph; edges = "triggers / blocks / satisfies".
- **Elaborate:** simulate breach paths; dream finds the unhandled contingency.
- **MVP:** sits on `src/ingest.rs` — add the obligation reducer + a breach-path walk.

### 6. harmony — music machine ✦
- **Source:** audio (`physis-pro-voice` already does audio→graph).
- **Primitives:** intervals / motifs (the elaborable unit — not raw notes).
- **Compose:** motif graph; edges = "transposes / inverts / resolves".
- **Elaborate:** fractal variation; memory = a personal style that deepens.
- **MVP:** intervals from a melody → motif nodes → variation generator.

### 7. polis — social-graph machine ✦
- **Source:** Praxis posts/matches; `social::SocialAlgorithm` already scores engagement × coherence.
- **Primitives:** affinity / reciprocity ties.
- **Compose:** relational graph; edges weighted by coherence.
- **Elaborate:** predict which ties strengthen; dream surfaces a fading tie.
- **MVP:** wrap `SocialAlgorithm` outputs as bridged nodes/edges → a coherence-weighted network view.

---

## Build order

```
machine-kit (shared Reducer trait + client + loop)
   │
   ├─ chronos ✦   (data exists; smallest)        ← start here
   ├─ techne  ✦   (extends Phase 7B scan)
   ├─ soma    ✦   (behavioural-vector model exists)
   ├─ lex / harmony / polis  (reuse ingest / voice / social)
   └─ elenchus    (new reducer; uses ConstructiveRefutation)
```

**Prereq for multi-machine sharing:** the id bridge (done) + auto-bridging on
register (follow-up) so machines populate one graph without each calling
`bridge_node` by hand. Then edges + Spanner export + a unified graph-walk across
all machines come for free.

## Open core work these depend on
- **Auto-bridge on register** (opt-in flag) so reducers don't have to bridge by hand.
- **Edge-write API** (`/api/v1/holarchy/edge` or via register) so reducers can
  assert relations, not just nodes.
- **Spanner `WikiEdge` + `upsert_edges`** (PREMORTEM P7) for durable shared graph.
