# Physis Architectural Evolution Report

## 1. What was borrowed conceptually from each adjacent project

### Semantica (https://github.com/semantica-agi/semantica)

| Concept borrowed | How it appears in Physis |
|-----------------|-------------------------|
| Context Graph | `FitnessContext` with domain/machine/process/environment/operator/material/state/task tags |
| Provenance chains | `ProvenanceChain` + `ProvenanceLink` — ordered, queryable, audit-ready |
| Decision records | `Revision` in `Hypothesis` — every status change is recorded with trigger |
| Conflict detection | `Contradiction` + `ContradictionParty` — explicit objects with authority, confidence, resolution |
| Explainability | `Explanation` struct + `PhysisCore::explain_hypothesis()` |
| Temporal validity | `TemporalValidity` — valid_from/valid_until/trigger on hypotheses and contradictions |

### Cognee (https://github.com/topoteretes/cognee)

| Concept borrowed | How it appears in Physis |
|-----------------|-------------------------|
| Persistent semantic memory | `PhysisCore` nodes + hypotheses persisted via JSON |
| Feedback-driven memory evolution | `ContextualQualityTracker` — fitness evolves per context |
| Connected retrieval | `search_nodes` + hypothesis-evidence links |
| Ontology grounding | OntologyLoader + CellClassifier + Hypothesis::ontology_refs |

### TrustGraph (https://github.com/trustgraph-ai/trustgraph)

| Concept borrowed | How it appears in Physis |
|-----------------|-------------------------|
| Context cores | `CoherenceProfile` with 6 dimensions (semantic, logical, temporal, causal, procedural, empirical) |
| Provenance | `ProvenanceChain` on every hypothesis |
| Explainability | `Explanation::to_markdown()` + `explain_hypothesis()` |
| Portable domain knowledge | Ontology JSON files are standalone, version-controllable |

### Other systems

| Concept | Source influence | Physis implementation |
|---------|-----------------|----------------------|
| Causal reasoning | Pearl, causal inference literature | `CoherenceProfile::causal` dimension + `Hypothesis::predictions` |
| Temporal knowledge graphs | Time-aware ontology work | `TemporalValidity` on hypotheses and contradictions |
| Truth maintenance systems | Doyle's TMS | `HypothesisStatus` state machine + `Revision` history |
| Active learning | feedback-driven classification | `ContextualQualityTracker` + hypothesis fitness |
| Evolutionary computation | fitness landscapes | `Hypothesis::fitness` as evolutionary pressure |

## 2. What was deliberately rejected

| Capability | Reason rejected |
|-----------|----------------|
| Generic RAG pipeline | Physis is not a document retrieval system. Embeddings are one component, not the intelligence. |
| Generic vector databases | In-memory graph + optional sled is sufficient. No need for external DB dependencies. |
| LLM chat interfaces | Not part of the coherence engine. The core stays embeddable and offline-capable. |
| Huge ontology standards | The 5×14 grid is intentionally minimal. Extensions are domain-specific and loaded as JSON. |
| Cloud infrastructure | Must remain runnable on a single machine, offline, without external services. |
| Graph databases | In-memory graph with JSON persistence is the right abstraction for the core. |
| Blind feature parity | Adding features because competitors have them would dilute the Physis thesis. |

## 3. What is now uniquely characteristic of Physis

1. **Multi-dimensional coherence as inspectable dimensions** — not a single score, but semantic + logical + temporal + causal + procedural + empirical, each trackable and explainable.

2. **Context-dependent fitness** — interpretations survive or fail in specific contexts. A failure in context A does not destroy validity in context B.

3. **First-class hypotheses with closed status machines** — candidate → supported → contradicted → confirmed → inert → failed → superseded → isolated → certified. Every transition is recorded with provenance.

4. **Explicit contradiction retention** — conflicting evidence is preserved as structured `Contradiction` objects with authority levels, confidence, and resolution status. Never silently deleted.

5. **Temporal coherence** — belief states are reconstructable at any point. `TemporalValidity` distinguishes "X is true" from "X was true at T".

6. **Causal hypotheses with evidence levels** — correlation, temporal precedence, influence, and causation are distinct dimensions in the coherence profile.

7. **Process semantics as first-class citizens** — goal/plan/task/event/state/transition are represented computationally, bridging PDCA, Lean, and industrial process data.

8. **Dream loop with counterfactual exploration** — generates candidate explanations without self-certifying. All candidates remain candidates until supported.

9. **Ontology evolution with provenance** — every concept introduction can be traced to a gap detection event and human review.

10. **Fitness decomposition** — the system can explain *why* an interpretation has its current fitness, breaking it into semantic, ontological, logical, empirical, predictive, and outcome components.

## 4. What remains technically weak

| Weakness | Severity | Plan |
|----------|----------|------|
| Hypothesis generation is manual | Medium | Add automatic candidate generation in the dream loop |
| Causal inference is representational only | Medium | Integrate do-calculus or counterfactual reasoning |
| Contradiction detection is heuristic | Medium | Add logical inference engine for explicit A vs not-A |
| Process model is type-only | Medium | Implement state machine execution and simulation |
| Temporal reasoning is point/interval only | Low | Add duration calculus and metric temporal logic |
| No active learning | Low | Use fitness uncertainty to drive targeted evidence gathering |
| Single-threaded dream loop | Low | Parallelize candidate generation and evaluation |
| No belief revision algorithms | Low | Implement AGM-style revision or kernel-based belief change |

## 5. What should be built next

### Immediate (next sprint)

1. **Hypothesis auto-generation in dream loop** — when `PhysisCore::dream()` finds a failure, automatically generate candidate hypotheses from ontology gaps and similar successful cases.

2. **Contradiction detection from hypotheses** — extend `detect_hypothesis_contradiction` to use logical inference, not just heuristic statement comparison.

3. **Context-aware classification** — extend `CellClassifier` to accept `FitnessContext` and use `ContextualQualityTracker::adjust_score_contextual`.

### Short-term (1-2 months)

4. **Process model execution** — implement a lightweight state machine that can run plans and record outcomes.

5. **Active learning** — when fitness is uncertain, generate targeted observations to resolve uncertainty.

6. **Ontology gap → candidate concept pipeline** — connect `discover` module to hypothesis generation.

### Medium-term (3-6 months)

7. **Causal reasoning** — integrate intervention-based fitness updates (A→B tested by intervening on A).

8. **Multi-agent belief revision** — support multiple agents with different authorities and perspectives.

9. **Industrial benchmark expansion** — add NASA C-MAPSS and PHM Society datasets to the benchmark abstraction.

## 6. Which claims can actually be demonstrated experimentally

### Claim 1: Feedback improves future interpretation without destroying previously valid interpretations

**Test**: Context-dependent fitness benchmark.
- Train hypothesis H in context A (successes) and context B (failures).
- Verify H retains high fitness in A and low fitness in B.
- Verify global penalty system would destroy H in both contexts.

**Status**: Implemented in `ContextualQualityTracker`. Needs benchmark expansion.

### Claim 2: Multi-dimensional coherence detects distinct failure modes

**Test**: Synthetic observations with controlled coherence dimensions.
- Low semantic, high empirical → "unexpected but measured"
- High semantic, low empirical → "expected but not observed"
- Verify dimensions remain distinct and explainable.

**Status**: `CoherenceProfile` implemented. Needs systematic benchmark.

### Claim 3: Contradiction retention preserves evidence

**Test**: Inject conflicting observations.
- Verify both claims remain accessible.
- Verify provenance is intact.
- Verify resolution status can be changed without data loss.

**Status**: `Contradiction` + `ContradictionParty` implemented. Needs integration test.

### Claim 4: Temporal validity reconstructs historical belief

**Test**: Record hypothesis status changes over time.
- Query belief state at arbitrary timestamps.
- Verify reconstruction matches actual history.

**Status**: `TemporalValidity` + `reconstruct_belief_at` implemented. Tested in `epistemic_audit_reconstruction`.

### Claim 5: Context-dependent fitness outperforms global penalties

**Test**: SECOM benchmark with context tags (process step, machine, time).
- Compare global QualityTracker vs ContextualQualityTracker.
- Measure accuracy, false positives, and context-specific performance.

**Status**: `ContextualQualityTracker` implemented. Needs benchmark integration.

### Claim 6: Explanation API returns structured, inspectable output

**Test**: Generate hypotheses with known evidence chains.
- Query `explain_hypothesis`.
- Verify supporting evidence, contradicting evidence, provenance, predictions, and revision history are all present.

**Status**: `Explanation` + `explain_hypothesis` implemented. Tested in `explain_returns_structured_output`.

## 7. The Physis thesis, restated

After this evolution, Physis is:

> **A coherence engine: it maintains interpretations of observations, grounds them in an ontology and process model, tracks their evidence and consequences, learns from success and failure in context-specific ways, and can explain how its current beliefs came to be.**

Everything added from other projects strengthens this proposition. Everything rejected would have diluted it.

The implementation is now structurally capable of:

1. Maintaining competing interpretations (Hypothesis)
2. Evaluating coherence across six dimensions (CoherenceProfile)
3. Tracking evidence and provenance (Evidence, ProvenanceChain)
4. Handling contradictions explicitly (Contradiction)
5. Reasoning temporally (TemporalValidity)
6. Learning from outcomes in context (ContextualQualityTracker)
7. Explaining beliefs (Explanation)
8. Evolving ontologies with audit trails (Revision history)
9. Representing process semantics (ProcessEvent, ProcessState, ProcessTransition)
10. Generating and evaluating candidates (Dream loop)

The core remains lean. The default feature set stays minimal. All new capabilities are additive.
