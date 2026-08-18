# First-Class Hypotheses in Physis

## 1. Overview

In conventional semantic engines, an "interpretation" is merely an ephemeral string or label returned by a classifier.

In Physis, an interpretation is a **first-class `Hypothesis`**: a persistent, inspectable, and revisable entity that tracks evidence, makes predictions, undergoes empirical testing, and evolves over time.

---

## 2. Hypothesis Data Model

```rust
pub struct Hypothesis {
    pub id: String,
    pub statement: String,
    pub embedding: Vec<f32>,
    pub ontology_refs: Vec<String>,
    pub assumptions: Vec<String>,
    pub supporting_evidence: Vec<Evidence>,
    pub contradicting_evidence: Vec<Evidence>,
    pub predictions: Vec<Prediction>,
    pub expected_outcomes: Vec<String>,
    pub actual_outcomes: Vec<String>,
    pub revision_history: Vec<Revision>,
    pub status: HypothesisStatus,
    pub confidence: Score,
    pub coherence_profile: CoherenceProfile,
    pub coherence: Score,
    pub fitness_breakdown: FitnessBreakdown,
    pub fitness: Score,
    pub created_at: DateTime<Utc>,
    pub revised_at: DateTime<Utc>,
    pub temporal: TemporalValidity,
    pub provenance: Option<String>,
}
```

---

## 3. Status Lifecycle State Machine

```
                            [ Observation ]
                                   │
                                   ▼
                            ┌──────────────┐
                   ┌───────▶│  Candidate   │◀──────────┐
                   │        └──────────────┘           │
                   │               │                   │
  [ Contradictory  │               │ [ Supporting      │ [ Reopened /
    Evidence ]     │               │   Evidence ]      │   Revised ]
                   │               ▼                   │
            ┌──────────────┐┌──────────────┐           │
            │ Contradicted ││  Supported   │───────────┤
            └──────────────┘└──────────────┘           │
                   │               │                   │
   [ Refuted by    │               │ [ Multi-source    │
     Outcome ]     │               │   Confirmation ]  │
                   ▼               ▼                   │
            ┌──────────────┐┌──────────────┐           │
            │    Failed    ││  Confirmed   │           │
            └──────────────┘└──────────────┘           │
                                   │                   │
                                   │ [ Authoritative   │
                                   │   Sign-off ]      │
                                   ▼                   │
                            ┌──────────────┐           │
                            │  Certified   │───────────┘
                            └──────────────┘
                                   │
                                   │ [ Replaced by
                                   │   Better Theory ]
                                   ▼
                            ┌──────────────┐
                            │  Superseded  │
                            └──────────────┘
```

### State Definitions

1. **Candidate**: Newly formulated interpretation under evaluation; not yet supported or refuted.
2. **Supported**: Corroborated by at least one independent observation or test.
3. **Contradicted**: Challenged by conflicting evidence or observations; retained for non-destructive tracking.
4. **Confirmed**: Validated across multiple independent sources and verified predictions.
5. **Inert**: Inactive or dormant hypothesis with no active observations or dependencies.
6. **Failed**: Empirically refuted by a failed prediction or catastrophic process outcome.
7. **Superseded**: Replaced by a more comprehensive or refined hypothesis.
8. **Isolated**: Outlier hypothesis with low network connectivity and no supporting clusters.
9. **Certified**: Authoritatively verified by human domain expert or certified test standard.

---

## 4. Predictions and Outcome Verification

Hypotheses in Physis are not passive descriptions; they generate testable predictions:

```rust
pub struct Prediction {
    pub statement: String,
    pub expected_outcome: Option<String>,
    pub made_at: DateTime<Utc>,
    pub observed_at: Option<DateTime<Utc>>,
    pub actual_outcome: Option<String>,
    pub correct: Option<bool>,
    pub confidence: Score,
}
```

When a prediction is recorded:
- If `correct == Some(true)`: Hypothesis predictive success increases, boosting overall fitness.
- If `correct == Some(false)`: A severe failed prediction penalty is applied, the failure is appended to `actual_outcomes`, and the status may transition to `Contradicted` or `Failed`.

---

## 5. The Evolved Dream Loop

The background reflection loop ("Dreaming") has evolved from a naive rerun into an active hypothesis generator:

1. **Failure Scan**: Identifies unresolved asserted failures and isolated outlier nodes.
2. **Cluster & Gap Analysis**: Analyzes vector distance from active ontology centroids.
3. **Candidate Synthesis**: Generates candidate hypotheses providing alternative explanations for why the failure occurred.
4. **Coherence Profiling**: Calculates initial multidimensional coherence.
5. **Strict Candidate Retention**: Generated hypotheses are **strictly assigned `Candidate` status** and recorded in the epistemic audit trail. They are presented for empirical testing, **never silently self-certified**.
