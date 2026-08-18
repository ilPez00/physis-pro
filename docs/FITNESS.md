# Fitness in Physis: Evolutionary Epistemology and Contextual Learning

## 1. What is Epistemic Fitness?

In Physis, **fitness** is the formal measure of how well an interpretation or hypothesis has survived empirical interaction with reality, observations, and real-world outcomes.

Unlike static confidence (which comes from the authority of a source) or semantic similarity (which measures distance in vector space), **fitness is earned dynamically through testing, prediction verification, and feedback**.

---

## 2. Decomposed Fitness Formulation

Physis calculates fitness using an extensible, inspectable decomposition:

$$\text{Fitness} = \left( w_{\text{sem}} \cdot F_{\text{sem}} + w_{\text{ont}} \cdot F_{\text{ont}} + w_{\text{log}} \cdot F_{\text{log}} + w_{\text{emp}} \cdot F_{\text{emp}} + w_{\text{pred}} \cdot F_{\text{pred}} + w_{\text{out}} \cdot F_{\text{out}} - P_{\text{contra}} - P_{\text{fail}} \right)_{\text{clamped}[0, 1]}$$

### Component Breakdown

| Component | Symbol | Description |
| :--- | :--- | :--- |
| **Semantic Fit** | $F_{\text{sem}}$ | Lexical and vector alignment with target domain vocabulary. |
| **Ontological Fit** | $F_{\text{ont}}$ | Conformance to ontology constraints, mode definitions, and facet schemas. |
| **Logical Consistency** | $F_{\text{log}}$ | Absence of internal self-contradiction. |
| **Empirical Support** | $F_{\text{emp}}$ | Weighted support from verified observations: $\frac{1}{2} + \frac{1}{2} \cdot \frac{\sum \text{conf}_{\text{supp}} - \sum \text{conf}_{\text{contra}}}{N_{\text{ev}}}$. |
| **Predictive Success** | $F_{\text{pred}}$ | Historical ratio of verified predictions: $\frac{N_{\text{correct}}}{N_{\text{resolved}}}$. |
| **Outcome Success** | $F_{\text{out}}$ | Success rate when acting upon this hypothesis in real workflows / PDCA cycles. |
| **Contradiction Penalty** | $P_{\text{contra}}$ | Explicit deduction for active, unharmonized contradictions with authoritative sources. |
| **Failed Prediction Penalty** | $P_{\text{fail}}$ | Severe penalty scaled by the number and severity of refuted predictions ($15\%$ per failure, capped). |

---

## 3. Context-Dependent Fitness vs Naive Global Penalties

A critical flaw in simplistic feedback systems is **global destructive interference**: if an interpretation fails in one context, penalizing it globally destroys valid knowledge in all other contexts.

Physis solves this by making fitness strictly context-aware:

$$\text{Fitness} = f(\text{Interpretation}, \text{Context})$$

### Context Dimensions (`FitnessContext`)

A context is defined by an orthogonal tuple:
- **Domain**: Engineering, Medical, Procurement, Logistics, Quality, etc.
- **Machine**: Specific machine ID (e.g. `CNC_Heavy_M1`, `LaserCutter_L4`).
- **Process**: Roughing, Finishing, Extrusion, Annealing, Inspection.
- **Environment**: Cleanroom, Shop Floor, Outdoor, High Humidity.
- **Operator**: Experience tier or certification.
- **Material**: Aluminum 6061, Titanium Gr5, PEEK, Cast Iron.
- **State**: High Speed, Startup, Idle, Maintenance.
- **Task**: Calibration, Rapid Prototyping, Volume Production.

### Example: Contextual Independence

- **Interpretation**: `"Aggressive High-Feed Milling"`
- **Context A** (Heavy Cast-Iron Roughing on Rigid CNC):
  - Outcome: High material removal rate, no chatter $\rightarrow$ $\text{Fitness}(\text{HeavyCNC}, \text{AggressiveFeed}) = 1.0$.
- **Context B** (Thin-wall Acrylic Finishing on Desktop Mill):
  - Outcome: Vibration chatter and optical cracking $\rightarrow$ $\text{Fitness}(\text{MicroMill}, \text{AggressiveFeed}) = 0.0$.
- **Result in Physis**: The interpretation is retained as optimal for Context A, but suppressed for Context B.

---

## 4. The Contextual Quality Tracker

The `ContextualQualityTracker` records feedback per $(interpretation, context)$ pair:

```rust
let ctx = FitnessContext {
    machine: Some("CNC_Heavy_M1".to_string()),
    material: Some("CastIron_GG25".to_string()),
    ..Default::default()
};

// Report success in specific context
tracker.report_success_context("HighFeedRate", ctx.clone());

// Raw score is modulated by blended global standing + context fitness
let adjusted = tracker.adjust_score_contextual("HighFeedRate", 0.90, &ctx);
```

When evaluating candidate interpretations during real-time classification, Physis blends:
$$\text{Adjusted Score} = \text{Raw Score} \cdot \left( 0.70 \cdot \text{Global Standing} + 0.30 \cdot \text{Contextual Fitness} \right)$$
ensuring that contextual experience directly guides inference.
