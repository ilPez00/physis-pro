# Architecture Comparison: Physis and Adjacent Knowledge & Memory Systems

This document analyzes Physis in relation to adjacent open-source frameworks in knowledge graphs, semantic memory, context graphs, decision intelligence, and computational epistemology (notably **Semantica**, **Cognee**, and **TrustGraph**).

The objective is not to clone another system, but to sharpen the unique thesis of Physis:

> **An engine that maintains competing interpretations of reality, evaluates their multidimensional coherence with observations, processes, evidence, and outcomes, and preferentially retains interpretations that continue to work.**

---

## 1. Capability Matrix

| Capability | Physis (Evolved) | Semantica | Cognee | TrustGraph | Process Mining / TMS |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Embeddings** | Pluggable (VectorEmbed / ONNX / CLIP / Jina) | Standard vector embeddings | Vector store integrations (Qdrant, pgvector) | Embedded vectors | N/A |
| **Ontology** | Orthogonal 2D semiotic grid (5×14) + N-D Facets | Hierarchical domain ontologies | Pydantic graph schemas | Context cores & entity schemas | Petri-net / BPMN ontologies |
| **Knowledge Graph** | Typed causal, epistemic & procedural edges | Entity-relation context graph | Entity-relation knowledge graph | Knowledge subgraphs | State-transition graphs |
| **Semantic Memory** | Persistent labeled core nodes + Sled / SQLite | Memory store | Hierarchical episodic/semantic memory | Context memory | Case memory |
| **Provenance** | First-class provenance chains on all assertions | Source tracking & entity lineage | Extraction chunk lineage | Document / corpus provenance | Event log provenance (XES) |
| **Conflict Detection** | Explicit non-destructive `Contradiction` tracking | Rule-based inconsistency detection | Overwrite / merge on conflict | Merge / deduplication | Anomaly / deviation detection |
| **Temporal Reasoning** | `TemporalValidity` intervals + historical replay | Temporal graph edges | Timestamped memories | Snapshotting | Temporal event ordering |
| **Causal Reasoning** | Explicit `Causes` / `Influences` vs correlation | Causal graphs | Indirect (graph traversals) | Directed relation edges | Process dependency causality |
| **Decision Records** | Asserted verdicts (+1/0/-1) & PDCA tracking | Architecture decision records | Feedback scores | Policy outputs | Process decision mining |
| **Process Model** | Goal → Plan → Action → Deviation → Intervention | Static context layers | Retrieval workflows | Pipeline tasks | Formal process execution (PDCA / Petri nets) |
| **Feedback Loop** | Contextual fitness $f(\text{ctx}, \text{hyp})$ | User feedback | User retrieval reranking | Prompt optimization | Conformance feedback |
| **Outcome Learning** | Predictive verification & fitness updates | Rule updates | Memory consolidation | Retrieval policy updates | Process optimization |
| **Coherence Scoring** | Multidimensional profile (8 inspectable axes) | Single relevance metric | Vector distance | Cosine similarity | Conformance fitness |
| **Hypothesis Mgmt** | First-class `Hypothesis` with 9-state lifecycle | Implicit interpretation | Entity retrieval candidates | Context entities | Competing hypotheses (TMS) |
| **Structured Explanation** | Multi-factor evidence, precedent, & outcome reports | Graph path traversal | Retrieval source list | Retrieved subgraph | Root cause analysis |
| **Ontology Evolution** | Unsupervised gap discovery → Proposal → Promotion | Manual schema editing | Schema migration | Schema versioning | Process model discovery (Alpha / Inductive) |
| **"Why/How" Tracing** | Epistemic audit trail (`reconstruct_belief_at(t)`) | Lineage graphs | Retrieval logs | Prompt audit trail | Replay logs |

---

## 2. Analysis of Adjacent Projects

### Semantica
- **Conceptual Contributions**:
  - Pioneered the concept of an accountable context graph where decisions and domain boundaries are explicit.
  - Demonstrated the value of domain-specific rule validation over unconstrained LLM generation.
- **Physis Distinction**:
  - Semantica functions primarily as a static/authoritative context provider.
  - Physis treats every representation as an **empirical hypothesis** whose validity and standing are dynamically tested against actual industrial and behavioral outcomes.

### Cognee
- **Conceptual Contributions**:
  - Demonstrated how graph + vector hybrid indexing bridges episodic document chunks and semantic concepts.
  - Illustrated memory evolution driven by user queries and iterative feedback.
- **Physis Distinction**:
  - Cognee primarily optimizes for *document retrieval accuracy* in RAG pipelines.
  - Physis optimizes for *reality testing and epistemic resilience*: distinguishing semantic similarity from empirical correctness, and maintaining active contradictions rather than blending them away.

### TrustGraph
- **Conceptual Contributions**:
  - Modularized portable domain knowledge into discrete "context cores" with embedded schemas.
  - Emphasized explainable retrieval subgraphs.
- **Physis Distinction**:
  - TrustGraph treats context packages as static domain bundles.
  - Physis integrates process execution (PDCA, Gantt, industrial telemetry) directly into the knowledge graph, closing the loop between plan, observation, failure, and revision.

---

## 3. Capabilities Strategically Imported

1. **First-Class Hypothesis Lifecycle** (from Truth Maintenance Systems & Computational Epistemology):
   - Representations are explicit, revisable objects with status (`Candidate`, `Supported`, `Contradicted`, `Certified`, `Failed`, etc.).
2. **Explicit Non-Destructive Contradiction Handling** (from Truth Maintenance Systems):
   - When claim $B$ contradicts claim $A$, neither is deleted; the conflict is recorded with source authority and resolution state.
3. **Multidimensional Coherence Decomposition** (from Epistemology & Coherence Theory):
   - Separating semantic match from logical consistency, temporal validity, causal plausibility, and empirical outcome success.
4. **Context-Dependent Fitness Tracking** (from Active Learning & Evolutionary Systems):
   - Tracking $fitness(\text{interpretation}, \text{context})$ prevents global destruction of domain knowledge when an idea fails in a specific setting.
5. **Process Semantics & PDCA Loop** (from Process Mining & Industrial Engineering):
   - First-class modeling of Plan, Action, Measurement, Deviation, Intervention, and Outcome.
6. **Replayable Epistemic Audit Trail** (from Provenance Architecture):
   - Full timeline allowing Physis to answer: *"What was believed at time $T$, and what specific evidence caused the belief to change?"*

---

## 4. Capabilities Deliberately Rejected (Anti-Patterns for Physis)

1. **Generic RAG Pipeline Reductions**:
   - Refusing to reduce Physis to "Document → Embeddings → Vector DB → Retrival → LLM".
2. **Global Monolithic Penalties**:
   - Refusing global blacklisting of ontology cells on single failures (replaced by contextual fitness).
3. **Equating High Similarity with Truth**:
   - Refusing to assume that a high cosine score implies empirical validity.
4. **Silent Overwriting of Dissenting Evidence**:
   - Refusing deduplication strategies that discard contradictory sensor logs or operator reports.
5. **Hallucinated Self-Certification**:
   - Ensuring the dream loop only generates *Candidate* hypotheses, never certifying its own unverified musings.
6. **Bloated Cloud Infrastructure Dependencies**:
   - Ensuring `physis-core` remains 100% embeddable with `--no-default-features` (pure deterministic Rust, zero external database requirement).
