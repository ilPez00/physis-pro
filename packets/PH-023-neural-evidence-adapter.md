---
id: PH-023
track: physis-pro
phase: 2
status: todo
rung: 4
depends: [PH-017, PH-018, PH-019, PH-020]
gate: "cd ~/dev/physis-pro && cargo test --release --test neural_evidence && cargo run --release --bin physis-pro -- neural-evidence probe --input tests/fixtures/neural_probe.json && cargo clippy --all-targets -- -D warnings"
commit: "branch agent/PH-023; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
---

# Smallest viable neural→Physis evidence adapter, with provenance before optimisation

## Why (the failure this prevents)

The obvious way to connect a neural model to Physis is to embed text, take the top SAE features,
call their labels "concepts", and write their activations into coherence weights. That produces a
system that looks multilayered, reports confident numbers, and is a strictly worse vector store:
activation strength becomes epistemic weight, feature labels become guaranteed semantics, and every
distinction the engine's type system was built to maintain is erased on the way in.

This packet exists to make the honest version cheap and the dishonest version hard. Its deliverable
is **instrumentation and provenance, not performance**. If it ends with "neural evidence adds
nothing measurable over the existing embedding path", that is a successful outcome and the series
in PH-020 shows how to report it.

It is phase 2 and rung 4 deliberately: attaching new evidence to a coherence score that silently
drops a dimension (PH-017), hardcodes two of its inputs (PH-018), and detects contradiction by
cosine (PH-019) would measure nothing. Do not start it early.

## Context (measured 2026-09-06 on physis-pro `13d8cdf` + physis-core `9d43821`; verify first)

**What exists and may be used.** These are real; nothing else is.

```
VectorEmbed { embed(&str)->Vec<f32>, embed_batch, dimension }   physis-core/src/embed.rs:16
PhysisCore::register_hypothesis / hypothesis / transition_hypothesis
PhysisCore::provenance_chain(subject) -> &mut ProvenanceChain   physis-core/src/core.rs:657
PhysisCore::record_contradiction(Contradiction) -> String       physis-core/src/core.rs:588
PhysisCore::add_edge(TypedEdge) / edges_for / search_nodes
Hypothesis::new(statement, embedding), .ontology_refs, .assumptions,
    .supporting_evidence, .contradicting_evidence, .temporal, .provenance: Option<String>
Hypothesis::resolve_prediction(index, actual, correct) -> bool  physis-core/src/hypothesis.rs:450
ProvenanceLink::new(claim, source) + .raw_reference + .method + .confidence
EpistemicAuditTrail::record(EpistemicEvent)  via core.epistemic_audit
CellClassifier::classify(&[f32]) -> Vec<CellScore> / classify_text
ContextualQualityTracker::{report_failure_context, report_success_context,
    fitness_for, adjust_score_contextual} + FitnessContext        physis-core/src/quality.rs:681
TokenFixedRetriever::new(budget, top_k).with_diversity(d).retrieve(&q, &RagCorpus)
TypedEdge::new(RelationType, src, tgt); RelationType::{is_epistemic,is_causal,is_procedural}
TemporalValidity::{permanent, from, during, is_valid_at, overlaps}
evaluate_mutation(&mut EvaluationContext, OntologyMutation) -> OntologyDeltaReport
```

**What does NOT exist.** Every item here is an adapter or extension. Do not write code that implies
otherwise, and do not claim any of it in a doc:

- no SAE, sparse decomposition, or feature dictionary;
- no layer index, token span, attention signal, or logits anywhere in either crate. The
  `activation` fields at `src/models.rs:1123` and `src/graph.rs:39` are semiotic-grid heat values
  and have nothing to do with neural activations — do not reuse them;
- no steering hook inside the forward pass. `physis-core/src/embed_onnx.rs:208` reads
  `last_hidden_state` and pools immediately; `ort` as used here exposes nothing earlier;
- `Evidence` (`hypothesis.rs:63`) has **no numeric strength field distinct from `confidence`**.
  Writing an activation into `confidence` converts a measurement into an epistemic weight — the
  precise error this packet exists to prevent;
- `Hypothesis.provenance` is `Option<String>`, one string. Structured chains live in the side-map
  `core.provenance_chains`, keyed by subject id, and *are* persisted (`core.rs:91`, `:515`).

**The information-loss boundary that governs the whole design:** pooling at `embed_onnx.rs:208`
destroys token position, layer depth, and per-dimension structure. One 768-d unit vector survives,
and everything downstream is cosine over it. Any claim that this adapter adds a representational
layer must be measured against that, not asserted.

## Steps

1. **`NeuralEvidence` — adapter-side type, new module `src/neural/mod.rs`.** Fields:
   `model`, `model_version`, `layer`, `token_span`, `feature_id`, `activation`,
   `concept_label: Option<String>`, `interpretation_confidence: Option<f32>`, `observed_at`.
   `concept_label` is `Option` on purpose: a label is attached only when an external validated
   mapping supplies one, never inferred from the feature id.
2. **Serialise it into provenance, not into scores.** One `ProvenanceLink` per feature:
   `raw_reference` = the `NeuralEvidence` as JSON, `method` = `"sae/<model>@<version>/L<layer>"`,
   `confidence` = `interpretation_confidence` **if and only if** a validated mapping exists, else
   leave the link's default and record the activation only inside `raw_reference`. This is the one
   place raw feature evidence survives; it must survive whether or not it was interpreted.
3. **Ground candidates through the ontology, preserving ambiguity.** For each candidate concept,
   `CellClassifier::classify` its label text, keep the top-k cells as `ontology_refs`, and emit
   **one `Hypothesis` per candidate mapping**, all at `HypothesisStatus::Candidate`. Never collapse
   to a best interpretation in the adapter. Link competitors with `TypedEdge` so the alternatives
   are recoverable.
4. **Contradiction via PH-019's detector only.** Neural evidence may *nominate* pairs to test; it
   may never decide. There is no validated mapping from any activation to deception or
   hallucination in this codebase, so do not write one.
5. **Temporal from the observation, not from now.** Set `Hypothesis.temporal` from the observation
   time and any known validity window. `TemporalValidity` is currently read by no scorer (audit §6,
   boundary 7) — this packet is not obliged to fix that, but it must not add another producer that
   nothing consumes without saying so in the report.
6. **`SteerableEmbed`: a `VectorEmbed` wrapper** over `OnnxEmbedder` applying an optional direction
   to the pooled vector. This is the only available injection point. It must record baseline and
   steered vectors both, and every steered run must have a matched control run. Steering is an
   intervention, never a fact — label it as such in the audit trail via `EpistemicEvent`.
7. **Measure it, in PH-020's style, or do not ship it.** Minimum conditions on one task with a
   held-out split: (A) existing embedding path alone — the baseline that must be beaten;
   (B) A + neural evidence attached as provenance only, no scoring change; (C) B + ontology-grounded
   competing candidates; (D) C + steering, against its control. Plus the controls the series
   establishes as mandatory: shuffled feature ids, random directions of matched norm, and feature
   ablation. Report mean ± sd over ≥ 5 seeds with the trivial baseline beside every number.
   **If B–D do not beat A, say so and stop.**
8. **CLI + fixture:** `physis-pro neural-evidence probe --input <json>` ingests a fixture of
   feature observations and prints the resulting hypotheses, their ontology refs, and their
   provenance chains. This is what makes the adapter auditable by hand, and it is the gate.
9. **Document the boundaries** in `docs/NEURAL_EVIDENCE.md`: what is measured, what is asserted,
   what is lost at pooling, and which of §4.6's absent mechanisms this adapter does and does not
   supply.

## Tests you must add

New file `tests/neural_evidence.rs`.

- `activation_never_becomes_confidence` — ingest evidence with a high activation and no validated
  mapping; assert the resulting `Evidence.confidence` and every coherence dimension are unchanged
  from the no-evidence baseline, and that the activation is nonetheless recoverable from the
  provenance chain. **This is the packet's core assertion**; if it cannot be written, the design is
  wrong.
- `raw_feature_survives_without_a_label` — evidence with `concept_label: None` still produces a
  provenance link carrying `feature_id`, `layer`, `token_span` and `activation`.
- `ambiguous_feature_yields_competing_hypotheses` — one feature with three plausible concept
  mappings produces three `Candidate` hypotheses, none preferred, all linked.
- `steering_records_baseline_and_control` — a steered run without a matched control is refused, not
  silently accepted.
- `provenance_round_trips_through_persistence` — save and reload; the `NeuralEvidence` JSON is
  still readable from `core.provenance_chains`.
- `neural_evidence_does_not_mutate_the_ontology` — ingest never adds or edits an ontology entry.
  Ontology change goes through `evaluate_mutation` and an explicit commit, never as a side effect
  of observation.
- A mechanism without a measured comparison against condition A is not done.

## Gate

```bash
cd ~/dev/physis-pro
cargo test --release --test neural_evidence
cargo run --release --bin physis-pro -- neural-evidence probe --input tests/fixtures/neural_probe.json
cargo clippy --all-targets -- -D warnings
```

## Commit strategy

Land in order, one commit each: `NeuralEvidence` + provenance serialisation; ontology grounding and
competing candidates; the CLI probe and fixture; `SteerableEmbed`; the measurement from step 7;
the docs. Steps 1–3 are useful alone and reviewable alone — steering and optimisation come last, on
purpose.

Prefer adapter-side code in physis-pro. Touch physis-core only if a genuinely missing primitive
blocks the design, and if so, say in the report which primitive and why an adapter could not carry
it.

## Report

Append `packets/reports/PH-023.md` per PROTOCOL §5. Must include the step 7 condition table
(A–D plus controls, mean ± sd over seeds, trivial baseline alongside) and a plain statement of
whether neural evidence improved anything measurable. A negative result closes this packet
successfully.

## Never

- Never write an activation into a coherence weight, a confidence, or a fitness term.
- Never treat an SAE feature label as guaranteed semantics, or a feature id as monosemantic.
- Never map any activation to deception or hallucination. No validated classifier here establishes
  such a mapping.
- Never let neural evidence mutate the ontology, resolve a contradiction, or select among competing
  hypotheses. It is evidence entering the machinery, not a verdict from outside it.
- Never report a steered result without its matched control.
- Never claim a layer "adds information" without measuring it against condition A.
