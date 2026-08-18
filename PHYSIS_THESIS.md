# PHYSIS THESIS

## Problem

LLMs optimize plausibility, not necessarily epistemic fitness. Generated text is selected for fluency and contextual fit, not for tracking what has actually worked across time, context, and conflicting evidence.

## Hypothesis

Persistent, multidimensional coherence, combined with outcome feedback, can improve knowledge revision. A system that maintains competing interpretations, grounds them in ontology, evaluates coherence across distinct dimensions, and updates fitness from observed outcomes should revise its beliefs more reliably than similarity alone.

## Formal Model

- **Hypothesis**: structured interpretation with statement, embedding, ontology refs, evidence, predictions, status, confidence, coherence, fitness, provenance, temporal validity, revision history.
- **Evidence**: supporting or contradicting claims with source, confidence, observed_at, context.
- **Coherence**: multi-dimensional profile — semantic, logical, temporal, causal, procedural, empirical — kept as inspectable dimensions rather than collapsed into a single score.
- **Fitness**: historical survival of outcomes, decomposed into semantic fit, ontological fit, logical consistency, empirical support, predictive success, outcome success, minus contradictions and failed predictions.
- **Contradiction**: first-class representation of conflicting claims with source authority, confidence, temporal validity, and resolution status; never silently overwritten.
- **Provenance**: ordered chain of evidence and reasoning steps, enabling queryable audit trails and epistemic reconstruction at any point in time.

## Implementation

Physis-core: a lean Rust engine implementing the model above. Deterministic embedding, ontology lookup, semiotic-grid classification, coherence nodes, quality feedback, hypothesis management, contradiction handling, temporal validity, and explanation generation. No external databases required; `default-features = false` remains viable.

## Baselines

- Embeddings / vector similarity alone.
- RAG retrieval without coherence or feedback.
- An LLM judge evaluating outputs without persistent belief state.

## Experiments

Independent industrial and process datasets demonstrating the loop:

1. SECOM semiconductor manufacturing (pass/fail outcomes).
2. NASA C-MAPSS engine run-to-failure.
3. PHM Society bearing/gearbox prognostics datasets.
4. General process monitoring with PDCA/Lean interventions.

Each experiment preserves the unidirectional flow:

```
observation → embedding → classification → prediction → ground truth → feedback → fitness update → model revision
```

## Results

Quantitative metrics only:

- Classification accuracy before vs. after feedback.
- Top-1 and top-k ranking changes.
- Mean score change.
- Number of penalized / boosted cells.
- Contradiction detection rate.
- Explanation completeness (evidence coverage, provenance depth).
- Calibration under distribution shift.
- Recovery after failure.

No manufactured positive results. If performance degrades, it is reported as a legitimate outcome.

## Limitations

- Hypothesis generation is currently manual or heuristic; automatic candidate generation is partial.
- Causal inference is representational only; no do-calculus or intervention-based updates yet.
- Contradiction detection relies on embedding similarity rather than logical inference.
- Process model is typed but not yet executable as a state machine.
- Temporal reasoning covers points and intervals, not duration calculus or metric temporal logic.
- Single-threaded dream loop; no active learning or targeted evidence gathering.
- Belief revision algorithms are rudimentary; no AGM-style or kernel-based revision.
- Industrial benchmarks use embedded samples for reproducibility; full-dataset scale is unverified.

## Philosophical Implications

- **Truth vs. coherence**: Physis explicitly distinguishes high coherence from empirical truth. A hypothesis can be highly coherent yet unsupported, or empirically successful yet causally unexplained.
- **Belief revision**: The system implements a form of persistent, revisable belief with audit trails, contrasting with stateless generation.
- **Explanation**: Every important assertion traces back to sources, timestamps, methods, and revision triggers. Explanation is structured data, not post-hoc rationalization.
- **Fitness vs. similarity**: Semantic proximity is not epistemic reliability. Physis treats embeddings as one component among ontology, logic, temporal state, causal hypotheses, provenance, and feedback.
- **Contradiction**: Conflicting evidence is preserved and reasoned about, not overwritten or averaged away.
