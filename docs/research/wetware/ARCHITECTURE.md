# PHYSIS × Neural Substrate — Architecture

## Layer cake

```text
PHYSIS Semantic State        (coherence nodes, hypotheses, ontology)
        ↓  encoder
semantic encoder             (Embedding → StimulusPattern)
        ↓
substrate adapter            (stimulus → channel activations)
        ↓
computation / dynamics       (backend: deterministic | reservoir | spiking | …)
        ↓
substrate observation        (NeuralObservation)
        ↓  decoder
decoder                      (observation → OntologyMutation)
        ↓
delta_engine                 (evaluate_mutation → projected next state)
        ↓
PHYSIS Semantic State
```

The semantic layer **never calls into a backend directly** — the decoder translates `NeuralObservation` → `OntologyMutation`, and `delta_engine::evaluate_mutation` resolves it against the existing graph + cell constraints + hypothesis priors. This is how "the underlying substrate changes while the semantic layer remains coherent."

## Reuse decisions (no new object model)

| Research type | Reused Physis type | File |
|---|---|---|
| PopulationState.values | `CoherenceNode.embedding` (Vec<f32>) | `physis-core/src/models.rs` |
| Confidence | `CoherenceNode::asserted` ∈ [-1,1]; `Hypothesis::confidence` | `models.rs` / `hypothesis.rs` |
| Cell/uncertainty | `CoherenceNode::cell_pin` (domain × mode) | `models.rs` |
| Semantic delta | `delta_engine::{OntologyMutation, MutationOp, evaluate_mutation}` | `delta_engine.rs` |
| Distribution over labels | `Hypothesis` fitness profile (not winner-take-all) | `hypothesis.rs` |
| Encoder projection | `embed::{VectorEmbed, RandomProjectionEmbedder}` | `embed.rs` |
| Domain semantics | `ontology::OntologyLoader` (9 ontologies) | `ontology.rs` |

## NeuralSubstrate trait

Location: `src/neural/substrate.rs`. Always available (no feature gate). Exposes `capabilities`, `configure`, `stimulate`, `observe`, `reset`, `health`. Backends implement it.

## Backend seam

Location: `src/neural/backends/`. Feature-gated on `neural` except the deterministic pass-through, which ships in `substrate.rs` so the core builds without any biological dependency.

## Decoder seam

Location: `src/neural/delta.rs`. `decode_semantic_delta(prev, obs, id, threshold) -> Option<OntologyMutation>` is the **delta-first** decoder. It is deliberately weak (drift detector) — the heavy lifting (constraint repair, cell enforcement) goes to `delta_engine::evaluate_mutation`.

## Invariant

The invariant that survives a substrate swap is: **a `StimulusPattern` encoding a semantic delta produces an `OntologyMutation` whose projected next state lands on the expected concept.** The *path* (reservoir dynamics, spiking threshold) varies; the delta signal and the `evaluate_mutation` repair do not.
