# Coherence in Physis: Multi-Dimensional Evaluation vs Semantic Similarity

## 1. The Core Conceptual Separation

A foundational failure in modern AI systems is conflating semantic similarity with truth, logical consistency, or operational validity.

Physis enforces an explicit distinction between these concepts:

$$\text{coherence} \neq \text{consistency} \neq \text{truth} \neq \text{confidence} \neq \text{similarity} \neq \text{fitness}$$

| Concept | Definition in Physis | Question Answered |
| :--- | :--- | :--- |
| **Semantic Similarity** | Cosine distance between vector embeddings in shared space. | *"Does this text sound like that text?"* |
| **Logical Consistency** | Absence of contradictory propositions within a belief set. | *"Can statement A and statement B both hold without contradiction?"* |
| **Ontological Validity** | Conformance to the schema, categories, modes, and facets. | *"Is this concept properly categorized within the action space?"* |
| **Confidence** | Estimated certainty of a specific source or extraction mechanism. | *"How reliable is the sensor, operator, or parser making this claim?"* |
| **Coherence** | Degree of mutual support and harmonious integration across multiple evidence dimensions. | *"How well does this interpretation fit together with everything else we know?"* |
| **Fitness** | Historical survival and predictive track record in interaction with real-world outcomes. | *"Has acting on this interpretation historically produced successful results?"* |
| **Truth** | The actual state of reality (accessible only through empirical observation and testing). | *"What is actually happening in the physical system?"* |

---

## 2. Pathological Cases Handled by the Distinction

1. **High Similarity, Empirically False**:
   - A hallucinated failure report may share 95% cosine similarity with real maintenance logs while describing a motor that does not exist.
   - *Physis Diagnosis*: High semantic similarity, high semantic coherence, **zero empirical coherence, zero fitness**.
2. **Empirically Successful, Poorly Explained**:
   - An operator adjusts nozzle pressure by intuition; it consistently fixes a print defect, but no formal physics formula is yet attached.
   - *Physis Diagnosis*: High empirical fitness, high outcome success, **low causal coherence**.
3. **Internally Consistent, Empirically Refuted**:
   - A beautifully logical theory predicting that higher temperature will decrease viscosity, but an unmodeled crystallization reaction occurs at 120°C.
   - *Physis Diagnosis*: High logical consistency, **contradicted by empirical outcome**.

---

## 3. The Multi-Dimensional Coherence Profile

Physis models coherence as a structured `CoherenceProfile` containing explicit, inspectable dimensions:

```rust
pub struct CoherenceProfile {
    pub semantic: CoherenceDimension,
    pub logical: CoherenceDimension,
    pub temporal: CoherenceDimension,
    pub causal: CoherenceDimension,
    pub procedural: CoherenceDimension,
    pub empirical: CoherenceDimension,
}
```

### Dimensional Definitions

1. **Semantic Coherence ($S_{\text{sem}}$)**:
   - Measures vector alignment between the hypothesis embedding and the centroids of relevant ontology domains.
2. **Logical Coherence ($S_{\text{log}}$)**:
   - Deductive consistency with active assertions, penalizing unresolved contradictions:
     $$S_{\text{log}} = 1.0 - \min(1.0, 0.2 \cdot |\text{contradictions}|)$$
3. **Temporal Coherence ($S_{\text{tmp}}$)**:
   - Validates that state sequences and causality respect time ordering ($t_{\text{cause}} \le t_{\text{effect}}$) and fall within the entity's `TemporalValidity` interval.
4. **Causal Coherence ($S_{\text{cau}}$)**:
   - Evaluates whether known mechanisms or typed causal edges (`CAUSES`, `INFLUENCES`) support the explanation.
5. **Procedural Coherence ($S_{\text{pro}}$)**:
   - Checks conformance to active process plans, workflows, or PDCA cycles (e.g. required prerequisites completed).
6. **Empirical Coherence ($S_{\text{emp}}$)**:
   - Measures quantitative corroboration by sensor readings, measurements, and physical observations.

---

## 4. Composite Ranking with Transparent Decomposition

While a composite scalar score is calculated for ranking competing candidate interpretations:

$$\text{Composite Coherence} = \frac{\sum_{d} w_d \cdot S_d}{\sum_d w_d}$$

Physis **never collapses these dimensions internally**. The full decomposition is preserved and returned in all API responses and explanation reports.

This allows operators and downstream AI agents to inspect *why* an interpretation is ranked as it is, distinguishing an interpretation supported by rigorous physics from one that merely sounds convincing.
