# Industrial Know-How Maintenance

**Status:** design proposal, 2026-09-05. Nothing below is built yet.
**Depends on:** `physis-core` ≥ 0.1.13 (`resolve_prediction`, `hypothesis query`, `open_predictions`).

---

## The gap

`src/maintenance.rs` is condition-based monitoring and it is good at what it does:
`HealthBaseline` keeps per-feature running statistics, `AnomalyScore` reports
sigma deviations with `FeatureDeviation` breakdowns, `project_time_to_limit`
gives a remaining-useful-life estimate, and `MaintenanceMonitor` persists to
sled. Given a `MachineReading`, it will tell you *that* something is off and *how
far* off.

It has no representation of the only thing that fixes machines: what the
technician knows.

> "When spindle 3 whines like that on a cold start it's the coolant line, not the
> bearing. Bleed it — ten minutes. If you don't, the bearing eats itself inside a
> week."

That sentence contains a symptom pattern, a differential diagnosis, a rejected
alternative, an intervention, a cost, and a falsifiable prediction with a
deadline. `maintenance.rs` can represent none of it. Today it lives in one
person's head and leaves the company when they do.

The other half of the gap is that **physis-pro already knows the answer**.
`oee.rs` records `StopEvent` with `StopReason` and `LossFactor`; `industrial.rs`
streams `MachineReading`. If someone bleeds the coolant line on Tuesday, the
system can see for itself whether the whine came back. Nobody has to remember to
write it down.

## Why this is a hypothesis problem, not a document problem

Every CMMS and maintenance wiki ever built is **write-only**. Knowledge goes in;
nothing ever comes back out to say whether it was *true*. Advice accumulates,
contradicts itself, goes stale when a supplier changes a part, and is arbitrated
by seniority because there is no other arbiter available.

Physis already has the machinery for exactly this. The engine can hold competing
interpretations, attest evidence with polarity and weight, track contradictions
non-destructively, and — as of 0.1.13 — resolve a prediction and let fitness move
on the outcome.

physis-pro already reaches for part of it: `src/core.rs`, `src/ai/harness.rs` and
`src/web/coherence.rs` all call `register_hypothesis`. But **nothing in
physis-pro has ever called `add_prediction` or `resolve_prediction`** — the
engine's outcome machinery is untouched, and the industrial side
(`maintenance.rs`, `industrial.rs`, `oee.rs`) does not reference hypotheses at
all. Claims go in; no claim is ever graded.

A piece of maintenance know-how *is* a hypothesis:

| Know-how | `physis_core::hypothesis` |
|---|---|
| "cold-start whine on spindle 3" | the symptom pattern that retrieves the claim |
| "it's the coolant line, not the bearing" | the `Hypothesis` statement |
| "bleed the line, 10 min" | the intervention the claim recommends |
| "assumes the line was bled at last service" | `assumptions` |
| "three times last winter, worked every time" | `Evidence` (`Supports`, weighted) |
| "bearing failed within a week when ignored" | `Evidence` on the *rival* claim |
| "whine will not recur within 200 operating hours" | `Prediction` with `expected_outcome` |
| what actually happened | `resolve_prediction(...)` ← **the new part** |
| how much to trust this tech's advice next time | `fitness`, measured not asserted |

## Design

### 1. `src/knowhow.rs` — the claim

```rust
/// A piece of operator knowledge, stated so it can be wrong.
pub struct KnowHowClaim {
    pub id: String,
    /// The physis-core hypothesis this claim is stored as. Fitness, evidence,
    /// predictions, revision history and contradiction tracking all live there;
    /// this struct only adds the shop-floor fields the engine has no opinion on.
    pub hypothesis_id: String,

    pub trigger: SymptomPattern,
    pub intervention: Intervention,
    pub scope: ClaimScope,        // this machine / this model / this whole class
    pub contributor: String,
    pub captured_at: DateTime<Utc>,
}

/// What must be true of the telemetry (and optionally of a human observation)
/// for this claim to be worth surfacing.
pub struct SymptomPattern {
    pub machine_selector: ClaimScope,
    /// Matched against `AnomalyScore::deviations` — the feature and the
    /// direction/magnitude of its sigma excursion.
    pub deviations: Vec<DeviationMatcher>,
    /// Conditions maintenance.rs cannot see: "cold start", "after a filament
    /// change", "only on the night shift". Embedded and matched semantically.
    pub context: Vec<String>,
}

pub struct Intervention {
    pub summary: String,
    pub steps: Vec<String>,
    pub estimated_minutes: u32,
    pub parts: Vec<String>,
    /// What this claim says happens if you do nothing. This is the field that
    /// makes know-how worth money, and the one no CMMS has.
    pub if_ignored: Option<String>,
}
```

### 2. The loop — and the part that makes it different

```text
  anomaly fires                 MaintenanceMonitor::check() → AnomalyScore
        │
        ▼
  knowhow::recall(&score, &ctx) → ranked candidate claims, each with its
        │                          MEASURED track record:
        │                          "bleed coolant line — 7/8 correct, fitness 0.81"
        │                          "replace bearing — 1/4 correct, fitness 0.29"
        ▼
  technician acts               → knowhow::log_intervention(claim_id, ...)
        │                          which calls hypothesis predict, registering
        │                          "symptom does not recur within N op-hours"
        ▼
  ┌─────────────────────────────────────────────────────────────┐
  │  N operating hours later, NO HUMAN INVOLVED:                │
  │                                                             │
  │  symptom recurred?  ── no ──▶ resolve_prediction(.., true)  │
  │         │                     fitness rises                 │
  │         └────── yes ────────▶ resolve_prediction(.., false) │
  │                               fitness falls                 │
  └─────────────────────────────────────────────────────────────┘
```

**The sensor stream is the oracle.** This is the whole idea. Every maintenance
knowledge system dies at the resolve step, because it asks a busy human to come
back weeks later and grade their own advice. Nobody does it. But physis-pro is
already ingesting the readings that settle the question, so the claim can be
graded by the machine that the claim is about.

A `knowhow::reconcile()` pass runs on the telemetry tick: for every open
prediction whose window has elapsed, ask `MaintenanceMonitor` whether the
triggering `SymptomPattern` fired again on that machine inside the window, and
resolve accordingly. Human effort to keep the ledger honest: **zero**.

### 3. Capture without burden

Tacit knowledge is not written down because writing costs the technician time
they do not have, in the moment their hands are dirty. Three routes, cheapest
first:

- **Retroactive, from what you already have.** `ingest` already parses xlsx, csv,
  PDF and email; `oee.rs` already has categorised `StopEvent`s. Mine the existing
  maintenance log for `(symptom, action, downtime)` triples and propose them as
  `Candidate` claims for a human to confirm. This seeds the graph on day one
  instead of starting empty — and an empty graph is why these systems get
  abandoned in month two.
- **Voice, at the machine.** The `voice` feature and `sensory.rs` already exist.
  A technician says the sentence at the top of this document into a headset; it
  becomes a `Candidate` claim with the transcript as provenance. Confirmation
  happens later, at a desk, in bulk.
- **One prompt at close-out.** When a work order closes, ask exactly one
  question: *"what would you tell someone who hit this next week?"* Free text.
  Structure it later, or never — an unstructured claim that gets resolved beats
  a beautifully structured one that does not.

### 4. Disagreement is a feature

Two technicians will contradict each other. The correct behaviour is **not** to
pick one, and not to let the senior one win. Register both as competing
hypotheses, let both be recalled when the symptom fires, and let the outcome
record settle it over a season. `contradiction.rs` already tracks conflicts
non-destructively, so the dissenting claim is never deleted — which matters,
because the minority claim is sometimes the one that is right about the edge
case, and it will show up as *high fitness on a narrow `ClaimScope`*.

### 5. Know-how goes stale, and staleness must be visible

A claim that was true before the bearing supplier changed is now false, and
nothing about it looks different. Two mechanisms, both cheap:

- **Evidence recency.** A claim whose supporting evidence all predates the last
  overhaul of that machine gets its confidence discounted, not deleted. The
  existing `revision_history` already timestamps everything needed.
- **`knowhow open`, the staleness surface.** The engine already ships
  `hypothesis open --older-than N` for exactly this. A claim that has been
  recalled and acted on but never resolved is a claim nobody is checking, and it
  should be visible as such.

### 6. Honest limits

- **Retrieval is the weak link, not the ledger.** The default embedder is
  `RandomProjectionEmbedder`; unrelated text scores >0.8 cosine against it
  routinely. `SymptomPattern` matching must be driven by the **structured**
  `DeviationMatcher` predicates, with the free-text `context` as a tiebreak only.
  Semantic recall over `context` needs `--features embed-onnx` and real weights
  before it can carry any weight. This is tracked as physis-core hypothesis
  `f8fcf49e`.
- **Auto-resolution can be wrong.** "Symptom did not recur" is not proof the
  intervention worked — the machine may have been idle. Gate resolution on
  actual operating hours from `oee.rs`, not wall-clock, and mark auto-resolved
  predictions distinctly from human-resolved ones so the two can be compared.
- **A claim with no prediction is a note, not know-how.** It should be storable,
  but it must never accrue fitness, and it must never be ranked above a claim
  that has actually been tested.

## Build order

| Step | Deliverable | Gate before moving on |
|---|---|---|
| K0 | `KnowHowClaim` + storage on top of `register_hypothesis`; `knowhow add/list/show` | round-trips through sled and survives restart |
| K1 | `knowhow recall <machine>` — structured `DeviationMatcher` only, no embeddings | on a replayed real anomaly, the right claim is in the top 3 |
| K2 | `log_intervention` → auto `predict` with an operating-hour window | a logged intervention shows up in `hypothesis open` |
| K3 | `reconcile()` on the telemetry tick — **the auto-resolve** | a seeded recurrence flips a claim to `--wrong` with no human input |
| K4 | Retroactive miner over the existing maintenance log | ≥20 candidate claims proposed from real history |
| K5 | Voice + close-out capture | — |

K3 is the one that matters. K0–K2 are a database; K3 is the part that makes the
thing true. If K3 is not built, do not build K4 or K5 — an unresolved ledger
growing faster is worse than no ledger.

## The measurement

The same question the development loop asks, pointed at the shop floor: **how
often does recalling prior know-how change what the technician actually does?**
Log it per recall — claim surfaced, claim followed, outcome. If after a season
that number is near zero, the honest conclusion is that the know-how was already
in the room and the ledger is ceremony. Say so plainly rather than reporting that
the graph grew.
