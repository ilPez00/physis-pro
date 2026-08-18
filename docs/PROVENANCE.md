# Provenance and Epistemic Audit in Physis

## 1. Traceability as an Architectural Foundation

In Physis, provenance is not an optional metadata annotation. It is a fundamental property of the reasoning and evaluation engine.

Every important assertion must be capable of answering four questions:
1. **Who/what originated this claim?** (Sensor, operator log, maintenance report, PDF, API caller)
2. **When was it observed or asserted?** (Timestamp, temporal validity interval)
3. **What evidence supports or contradicts it?** (Sensor readings, inspection photos, spectrometry)
4. **Why was the interpretation revised?** (Feedback ID, failed prediction, contradiction detection)

---

## 2. Provenance Chains

Every node and hypothesis links to a `ProvenanceChain` representing its lineage:

```rust
pub struct ProvenanceLink {
    pub claim: String,
    pub source: String,
    pub timestamp: DateTime<Utc>,
    pub confidence: Score,
    pub raw_reference: Option<String>,
    pub method: Option<String>,
}

pub struct ProvenanceChain {
    pub links: Vec<ProvenanceLink>,
}
```

### Traceability Example
```
Hypothesis H17: "Nozzle cavitation due to sub-cooled coolant"
   │
   ├── supported by ProvenanceLink:
   │   claim: "Cavitation spectral peak at 3.2 kHz"
   │   source: "maintenance_report_2026_08_12.txt" (line 42)
   │   timestamp: 2026-08-12T14:32:00Z
   │
   └── revised because:
       trigger: "Failed prediction #4: Seal leak observed at hour 14"
       source: "shift_log_OP4.json"
```

---

## 3. Explicit Contradiction Handling (Non-Destructive)

When two sources present conflicting evidence:

$$\text{Source } A \implies X \quad \text{vs} \quad \text{Source } B \implies \neg X$$

Physis **never silently overwrites or discards either claim**. Instead, it generates a first-class `Contradiction` object:

```rust
pub struct Contradiction {
    pub id: String,
    pub claim_a: ContradictionParty,
    pub claim_b: ContradictionParty,
    pub resolution: ResolutionStatus,
    pub explanation: Option<String>,
    pub detected_at: DateTime<Utc>,
    pub temporal: TemporalValidity,
}
```

### Resolution Strategies (`ResolutionStatus`)
- **`APreferred`**: Claim A currently preferred (e.g. calibrated sensor > tactile estimate), but Claim B retained as contradictory evidence.
- **`BPreferred`**: Claim B currently preferred.
- **`Open`**: Both claims held in active unresolved tension.
- **`Contextual`**: Both claims valid under differing operational contexts (e.g. valid at high vs low pressure).
- **`Superseded`**: A third unifying hypothesis explains why both observations occurred.

---

## 4. The Epistemic Audit Trail & Temporal Reconstructability

Physis maintains an append-only `EpistemicAuditTrail` recording all state shifts:

```rust
pub struct EpistemicEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: EpistemicEventType,
    pub subject_id: String,
    pub description: String,
    pub prior_state: Option<String>,
    pub posterior_state: Option<String>,
    pub source: Option<String>,
    pub metric_value: Option<Score>,
}
```

### Historical State Reconstruction

By querying the audit trail, Physis can deterministically reconstruct:
1. **"What did Physis believe at time $T$?"**
   $$\text{reconstruct\_belief\_at}(\text{subject\_id}, T) \rightarrow \text{HypothesisStatus}$$
2. **"What specific event caused belief $A$ to transition to belief $B$?"**
   Returns the exact `EpistemicEvent` with trigger ID, source log, and delta.

```
t0: Physis believed "Nozzle Nominal" [Status: Candidate]
t1: Observed sensor reading 95C from sensor S1
t2: Believed "Nozzle Overheating" [Status: Supported]
t3: Predicted: "Cooling fan increase will lower temp to 70C in 5 min"
t4: Observed: Temp remained at 96C (Heater relay stuck)
t5: Hypothesis fitness decreased from 0.82 -> 0.35 [Status: Contradicted]
t6: New Hypothesis generated: "Heater Relay Welded Shut" [Status: Candidate]
```
