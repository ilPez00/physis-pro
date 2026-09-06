# Epistemic Audit — physis-pro / physis-core

**Date:** 2026-09-06
**Audited:** `physis-pro` @ `13d8cdf` (origin/main, 2026-09-05) with submodule `physis-core` @
`9d43821` (v0.1.14) — i.e. exactly what a fresh clone of main builds.
**Also surveyed:** `physis-core` @ `f4dd2a1` (origin/master, v0.1.15), 28 commits ahead of the pin.
See §8 — it changes the publication verdict.
**Method:** fresh clone, real weights, full build, full test suite, then a source trace of every
claim in `PHYSIS_THESIS.md` down to the line that implements it.
**Question asked:** is the epistemic thesis of this codebase supported by what the codebase does?

Every fact below carries a file and line, measured at the commits named above. Re-measure before
relying on any of it.

---

## 1. Executive summary

The engineering is sound. Clean build, a large green test suite, genuinely good type design, and a
working ONNX path that produces a respectable, measurable classification accuracy.

**On the pinned tree, the epistemic claims are not supported by the implementation.** Four
findings, in severity order:

1. Contradiction detection is cosine similarity. `PHYSIS_THESIS.md` §1.1 names this exact error as
   "The Cosine Fallacy" and presents it as the defect Physis exists to correct;
   `src/ai/harness.rs:199` commits it.
2. Multidimensional coherence is, in practice, three cosine variants, one dimension silently
   discarded by a `match` arm that does not exist, and two dimensions never written.
3. Two of nine fitness fields are hardcoded literals on the product path, including
   `predictive_success`, which carries 0.25 of the composite weight.
4. The thesis's "Empirical Verification Summary" (§8 of that document) rests on a single test
   function using a 64-dimension random-projection hash embedder — no semantic model at all.

**But the research program is real, and it is not in the pinned tree.** `physis-core`
origin/master carries 22 experiment programs implementing 26 numbered iterations with ablation
matrices, cross-embedder ARI controls, blind conditions, and honestly recorded negative results.
That is the experimental discipline the thesis lacks. It is unreachable from physis-pro's
submodule pin and has never been written up. See §8.

**Publication verdict.** The broad epistemic-architecture thesis is not publishable as it stands:
its central claims have no evaluation, and the mechanism that would distinguish Physis from cosine
retrieval is not the mechanism that runs. **The ontology-discovery experiment series is a
different matter** — 26 iterations that mostly failed, recorded honestly, with controls, is a
publishable negative-results paper today and needs writing up, not more code. That is the paper.

---

## 2. Build and test evidence

Fresh clone of main, submodule initialised, real weights placed:

| artefact | source | size | role |
|---|---|---|---|
| `models/bge-base-en-v1.5/onnx/model.onnx` | `BAAI/bge-base-en-v1.5` | 416 MB | primary embedder, 768-d, mean pooling |
| `models/bge-base-en-v1.5/tokenizer.json` | tracked in repo | 695 KB | — |
| `models/model.onnx` | `Xenova/all-MiniLM-L6-v2` | 86 MB | 384-d fallback |

`models/jina-clip-v2` was deliberately **not** placed: it sits above bge-base in the startup
cascade (`src/embed/mod.rs:330`), so its presence alone swaps the primary text embedder and
orphans every persisted node.

| step | result |
|---|---|
| `cargo build --release` (default features) | green, 1 m 20 s, 6 binaries |
| `cargo test --release` (physis-pro) | **956 passed, 1 failed, 3 ignored** |
| `cargo test --release` (physis-core) | **143 passed, 0 failed**, < 1 s total |
| `physis-pro doctor` | `embedder: dim=768`, bge-base active, self-test pass |

The single failure is
`tests/http_surface.rs::content_search_finds_sections_cites_them_and_reuses_its_index`
(assertion at `tests/http_surface.rs:4043`), which runs under the random-projection embedder so
its ranking is noise-driven. This is the repo's own known `PH-015` flake, not a new regression.

Note for anyone scripting the gate: `cargo test | tail -n` **masks the exit code**. The piped run
reported exit 0 while `cargo test` itself exited 101. Use `${PIPESTATUS[0]}` or redirect to a file.

### 2.1 Measured accuracy — the one honest number in the pinned tree

`tests/classification.rs::accuracy_floor`, normally `#[ignore]`d, run with real bge-base weights
against the 79 hand-labelled fixtures in `tests/classification_fixtures.json`
(55 core / 15 italian / 9 hard):

```
CORE    top-1 domain 0.873 (48/55)   cell 0.727 (40/55)
ITALIAN top-1 domain 0.867 (13/15)   cell 0.800 (12/15)
HARD    top-1 domain 1.000 (9/9)     [informational tier]
```

Both gated floors pass (core domain ≥ 0.85, core cell ≥ 0.72). This is real, and it is the only
place in physis-pro where a labelled dataset meets a numeric threshold. It measures
**classification**, not any epistemic claim.

### 2.2 Measured SECOM benchmark

`physis-pro benchmark secom --embedder onnx`, N = 10 test observations:

```
accuracy before feedback  0.70
accuracy after  feedback  0.70      top1_changed = 0   topk_changed = 0
mean_adjusted_score       0.52 → 0.94
```

The quality feedback loop changed **no decision**; it moved confidence only. The cells selected for
semiconductor-fab observations were `BOND×PLAY`, `HEAL×PLAN`, `HEAL×WORK` — the semiotic grid has
no vocabulary for this domain, which is itself a finding.

---

## 3. Architecture map — claimed pipeline vs. implemented pipeline

```
  CLAIMED                         IMPLEMENTED                                  STATUS
  ─────────────────────────────────────────────────────────────────────────────────────
  raw text / activations          text only                                    partial
        ↓
  tokenizer / model repr          tokenizers 0.21, embed_onnx.rs               exists
        ↓
  hidden-state extraction         last_hidden_state only, embed_onnx.rs:208    FINAL LAYER ONLY
        ↓                         mean/CLS pooled immediately
  SAE feature decomposition       —                                            ABSENT
        ↓
  feature → concept               —                                            ABSENT
        ↓
  ontology grounding              OntologyLoader::load_all(), CellClassifier   exists (5×14 grid)
        ↓
  classification / relations      classify() exists; TypedEdge types exist     NO RELATION EXTRACTOR
        ↓
  competing interpretations       Hypothesis + 9-state machine                 STORAGE ONLY, no generator
        ↓
  multidimensional coherence      CoherenceProfile, 6 dimensions               3 cosine, 1 dropped, 2 constant
        ↓
  prediction / action / RAG       Prediction + TokenFixedRetriever             RAG is cosine + MMR only
        ↓
  observed outcome                resolve_prediction()                         exists; one caller (CLI)
        ↓
  QualityTracker / feedback       QualityTracker, ContextualQualityTracker     real, hand-tuned constants
        ↓
  ontology / model revision       evaluate_mutation + EvaluationContext        REAL — the best component
```

### 3.1 Per-mechanism verdict

| mechanism | primitive | file:line | verdict |
|---|---|---|---|
| embedding | `VectorEmbed` trait | `physis-core/src/embed.rs:16` | real; the correct adapter seam |
| ONNX embedding | `OnnxEmbedder` | `physis-core/src/embed_onnx.rs:45` | real; reads `last_hidden_state`, pools |
| embedder cascade | `pick_embedder` | `src/embed/mod.rs:264` | real; self-test gated, cache-backed |
| ontology load | `OntologyLoader::load_all` | `physis-core/src/ontology.rs:192` | real; 5×14 grid + custom axes |
| classification | `CellClassifier::classify` | `physis-core/src/classify.rs:363` | real; measured 0.727 cell accuracy |
| coherence node scoring | `PhysisCore::update_coherence` | `physis-core/src/core.rs:303` | **mean cosine to 5-NN, clamped ≥ 0** |
| hypotheses | `Hypothesis`, `HypothesisStatus` | `physis-core/src/hypothesis.rs:219` | model excellent; nothing generates competitors |
| multidim coherence | `CoherenceProfile` | `physis-core/src/coherence_dimensions.rs:50` | container only; see §5.2 |
| fitness | `recompute_fitness` | `physis-core/src/hypothesis.rs:370` | real arithmetic; 3 inputs never computed |
| contradiction model | `Contradiction`, `ResolutionStatus` | `physis-core/src/contradiction.rs:31` | excellent type design |
| contradiction detection | `detect_hypothesis_contradiction` | `physis-core/src/core.rs:606` | **no detection; returns `Some` unconditionally** |
| contradiction detection (pro) | inline in harness | `src/ai/harness.rs:199` | **cosine > 0.68 to a failed hypothesis** |
| isolated-branch detection | `detect_contradictions` | `physis-core/src/core.rs:378` | outlier detection (`coherence < 0.3`), renamed |
| temporal validity | `TemporalValidity` | `physis-core/src/temporal.rs:15` | correct and tested; **never consulted in scoring** |
| provenance | `ProvenanceChain`, `ProvenanceLink` | `physis-core/src/provenance.rs` | real, persisted side-map; usable seam |
| audit / replay | `EpistemicAuditTrail`, `reconstruct_status_at` | `physis-core/src/epistemic.rs:107` | real, append-only, tested |
| quality feedback | `QualityTracker` | `physis-core/src/quality.rs:48` | real; hand-tuned constants, no validation |
| contextual fitness | `ContextualQualityTracker`, `FitnessContext` | `physis-core/src/quality.rs:681` | real; per-context records |
| dream loop | `dream_hypotheses` | `physis-core/src/core.rs:727` | **reuses failure's own embedding + template string** |
| RAG | `TokenFixedRetriever::retrieve` | `physis-core/src/rag.rs:152` | cosine + greedy MMR + budget; no epistemic input |
| ontology delta | `evaluate_mutation`, `EvaluationContext` | `physis-core/src/delta_engine.rs:424` | **real mechanism**; γ-attenuated shadow propagation |
| discovery | `discover` | `physis-core/src/discovery.rs:182` | real gap clustering |
| experiments (pinned) | `experiments.rs` | `physis-core/src/experiments.rs` | **0 lines of code** — but see §8 |

---

## 4. Type / interface map

The API surface an integration may rely on. Nothing invented; every entry read at the pinned
commits.

### 4.1 Embedding

```rust
// physis-core/src/embed.rs:16
pub trait VectorEmbed: Send + Sync {
    fn embed(&self, text: &str) -> Vec<f32>;
    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>>;   // default impl
    fn dimension(&self) -> usize;
}
```

Implementors must be deterministic and return L2-normalised vectors. This is the **only** injection
point for an externally computed or modified vector: `ort` as used here exposes no pre-pooling hook.

Implementations: `RandomProjectionEmbedder` (`embed.rs:69`, SHA-256 n-gram feature hashing),
`OnnxEmbedder` (`embed_onnx.rs:45`), plus pro-side `JinaEmbedder` / `ClipEmbedder`.

### 4.2 Graph and interpretations

```rust
// physis-core/src/models.rs:48
pub struct CoherenceNode {
    pub id: String,
    pub embedding: Vec<f32>,
    pub coherence_score: Score,             // derived; never negative
    pub asserted: Option<Score>,            // outcome in [-1, 1]; None = unjudged
    pub label: Option<String>,
    pub cell_pin: Option<(String, String)>,
    pub embedder: Option<String>,           // model provenance — no layer, no token span
    pub previous_coherence_state: Option<f32>,
    pub self_model_version: u32,
    pub preference: Option<f32>,
}

// physis-core/src/hypothesis.rs:219
pub struct Hypothesis {
    pub id: String,
    pub statement: String,
    pub embedding: Vec<f32>,
    pub ontology_refs: Vec<String>,          // DOMAIN×MODE keys
    pub assumptions: Vec<String>,
    pub supporting_evidence: Vec<Evidence>,
    pub contradicting_evidence: Vec<Evidence>,
    pub predictions: Vec<Prediction>,
    pub expected_outcomes: Vec<String>,
    pub actual_outcomes: Vec<String>,
    pub revision_history: Vec<Revision>,     // append-only
    pub status: HypothesisStatus,            // 9-state closed machine
    pub confidence: Score,
    pub coherence_profile: CoherenceProfile,
    pub coherence: Score,
    pub fitness_breakdown: FitnessBreakdown,
    pub fitness: Score,
    pub created_at: DateTime<Utc>,
    pub revised_at: DateTime<Utc>,
    pub temporal: TemporalValidity,
    pub provenance: Option<String>,          // ONE STRING — see §6
}
```

`HypothesisStatus` = `Candidate | Supported | Contradicted | Confirmed | Inert | Failed |
Superseded | Isolated | Certified`.

```rust
// physis-core/src/hypothesis.rs:63
pub struct Evidence {
    pub source: String,
    pub polarity: EvidencePolarity,          // Supports | Contradicts
    pub confidence: Score,
    pub claim: String,
    pub observed_at: Option<DateTime<Utc>>,
    pub embedding: Vec<f32>,
    pub context: Vec<String>,
}
```

There is **no numeric activation/strength field** on `Evidence` distinct from `confidence`. An
external feature strength has no typed home and would have to be coerced into an epistemic weight.

### 4.3 Scoring

```rust
// physis-core/src/coherence_dimensions.rs:50
pub struct CoherenceProfile {
    pub semantic:   CoherenceDimension,
    pub logical:    CoherenceDimension,
    pub temporal:   CoherenceDimension,
    pub causal:     CoherenceDimension,
    pub procedural: CoherenceDimension,
    pub empirical:  CoherenceDimension,
}
// NOTE: no `ontological` field, though lib.rs:20 and the thesis both name one.
```

```rust
// physis-core/src/hypothesis.rs:163
pub struct FitnessBreakdown {
    pub semantic_fit: Score,                 // set by delta_engine / harness only
    pub ontological_fit: Score,              // set by harness only
    pub logical_consistency: Score,          // set by delta_engine / harness only
    pub empirical_support: Score,            // computed in recompute_fitness
    pub predictive_success: Score,           // computed in recompute_fitness; HARDCODED in harness
    pub outcome_success: Score,              // NEVER computed, and not in the composite
    pub contradiction_penalty: Score,
    pub failed_prediction_penalty: Score,
    pub composite_fitness: Score,
}
```

Composite weighting (`hypothesis.rs:412`):
`0.20·semantic + 0.15·ontological + 0.15·logical + 0.25·empirical + 0.25·predictive
− contradiction_penalty − failed_prediction_penalty`, clamped to `[0,1]`.

### 4.4 Contradiction, temporal, provenance, audit

```rust
// physis-core/src/contradiction.rs:31
pub struct Contradiction { id, claim_a: ContradictionParty, claim_b: ContradictionParty,
                           resolution: ResolutionStatus, explanation: Option<String>,
                           detected_at: DateTime<Utc>, temporal: TemporalValidity }
pub enum ResolutionStatus { APreferred, BPreferred, Open, Contextual, Superseded }
pub struct ContradictionParty { claim, source, authority: Score, confidence: Score,
                                timestamp, embedding: Vec<f32>, context: Vec<String> }

// physis-core/src/temporal.rs:15
pub struct TemporalValidity { valid_from: Option<DateTime<Utc>>,
                              valid_until: Option<DateTime<Utc>>,
                              trigger: Option<String> }
impl TemporalValidity { fn permanent(); fn from(t); fn during(a,b);
                        fn is_valid_at(t) -> bool; fn is_current() -> bool;
                        fn overlaps(&other) -> bool }

// physis-core/src/provenance.rs
pub struct ProvenanceLink { claim, source, timestamp, confidence: Score,
                            raw_reference: Option<String>, method: Option<String> }
pub struct ProvenanceChain { links: Vec<ProvenanceLink> }

// physis-core/src/epistemic.rs:107
pub struct EpistemicAuditTrail { events: Vec<EpistemicEvent> }
impl EpistemicAuditTrail { fn record(e); fn history_for(subject_id) -> Vec<&EpistemicEvent>;
                           fn reconstruct_status_at(subject, t) -> Option<HypothesisStatus> }
```

### 4.5 Engine entry points

```rust
// physis-core/src/core.rs
impl PhysisCore {
    fn register_node_from_text(&mut self, text, &dyn VectorEmbed) -> String;
    fn register_hypothesis(&mut self, Hypothesis) -> String;
    fn hypothesis(&self, id) -> Option<&Hypothesis>;
    fn transition_hypothesis(&mut self, id, HypothesisStatus, desc, trigger);
    fn provenance_chain(&mut self, subject) -> &mut ProvenanceChain;           // :657
    fn record_contradiction(&mut self, Contradiction) -> String;               // :588
    fn detect_hypothesis_contradiction(&self, a, b) -> Option<Contradiction>;  // :606 — see §5.1
    fn add_edge(&mut self, TypedEdge);
    fn edges_for(&self, entity_id) -> Vec<&TypedEdge>;
    fn search_nodes(&self, &[f32], k) -> Vec<(String, Option<String>, f32)>;
    fn asserted_failures(&self) -> Vec<&CoherenceNode>;
    fn dream_hypotheses(&mut self) -> Vec<Hypothesis>;                         // :727 — see §5.5
    fn query_coherence(&self, &EpistemicQuery) -> EpistemicQueryResult;
    fn full_explanation_report(&self, id) -> Option<ExplanationReport>;
    fn coherence_index(&self) -> Score;
    pub epistemic_audit: EpistemicAuditTrail;
    pub provenance_chains: HashMap<String, ProvenanceChain>;
}

// physis-core/src/hypothesis.rs:450
impl Hypothesis { fn resolve_prediction(&mut self, index, actual_outcome, correct: bool) -> bool; }

// physis-core/src/delta_engine.rs
pub fn evaluate_mutation(&mut EvaluationContext, OntologyMutation) -> OntologyDeltaReport;
pub const GAMMA: f32 = 0.85;
pub const DEGRADATION_THRESHOLD: f32 = 0.25;
pub const MAX_PROPAGATION_DEPTH: usize = 5;
pub const MIN_IMPACT: f32 = 0.01;

// physis-core/src/rag.rs
impl TokenFixedRetriever {
    fn new(budget: usize, top_k: usize) -> Self;
    fn with_diversity(self, f32) -> Self;
    fn retrieve(&self, query_emb: &[f32], &RagCorpus) -> RetrievalResult;
}

// physis-core/src/quality.rs
impl ContextualQualityTracker {
    fn report_failure_context(...); fn report_success_context(cell, FitnessContext);
    fn fitness_for(interpretation, &FitnessContext) -> Option<Score>;
    fn adjust_score_contextual(...) -> f32;
}
```

### 4.6 What does not exist — adapters required, not existing API

Named explicitly so no integration pretends otherwise:

- no SAE, sparse decomposition, or feature dictionary;
- no layer index, token span, or attention signal in either crate. The `activation` fields at
  `src/models.rs:1123` and `src/graph.rs:39` are semiotic-grid heat values, unrelated to neural
  activations;
- no logits or probability distributions;
- no steering hook inside the forward pass;
- no `ontological` field on `CoherenceProfile`;
- no temporal, causal, or procedural coherence scorer;
- no contradiction detector based on polarity, negation, or entailment;
- no baseline/experiment runner **in physis-pro** (physis-core master has one — §8).

---

## 5. Findings on the pinned tree

### 5.1 Contradiction detection is cosine similarity — SEVERITY: blocking

`PHYSIS_THESIS.md` §1.1, "The Cosine Fallacy", states that a claim *A* and its negation *¬A*
"frequently share high cosine similarity (≥ 0.85) due to shared lexical and semantic context", and
presents this as the defect Physis exists to correct.

`src/ai/harness.rs:196`:

```rust
for h in core.hypotheses.values() {
    if h.status == HypothesisStatus::Contradicted || h.status == HypothesisStatus::Failed {
        let sim = cosine_sim(&embedding, &h.embedding);
        if sim > 0.68 { contradictions.push(ContradictionSummary { severity: sim, .. }); }
    }
}
```

Similarity to a claim already labelled failed **is** the contradiction signal. A second block at
`harness.rs:216` flags a candidate that is cosine > 0.70 to *both* sides of an open contradiction —
exactly the symptom §1.1 predicts, used here as the signal.

`physis-core/src/core.rs:606` is worse — no comparison of any kind; any two hypotheses that exist
are "contradictory", and the only `None` is a missing id.

`physis-core/src/core.rs:378` `detect_contradictions()` returns nodes with `coherence_score < 0.3`:
vector-space outlier detection under a contradiction name.

Operational consequence: a maintenance note that *agrees* with a known-failed hypothesis is flagged
as contradicting it. The system raises conflicts on corroboration and stays silent on negation.

### 5.2 Multidimensional coherence collapses to cosine — SEVERITY: blocking

`src/ai/harness.rs:234–279`:

| dimension | actual source | line |
|---|---|---|
| semantic | mean cosine over top-5 recalled nodes | `harness.rs:238` |
| ontological | best ontology-entry cosine | `harness.rs:247` |
| logical | `1 − 0.25 × count(contradictions)` — a counter | `harness.rs:254` |
| empirical | cosine-weighted ratio of asserted verdicts | `harness.rs:262` |
| temporal | never written → stays 0.5 | — |
| causal | never written → stays 0.5 | — |
| procedural | never written → stays 0.5 | — |

The ontological score is then **silently discarded**. `harness.rs:252` calls
`profile.set("ontological", ontological_score, 1.0)`, but `CoherenceProfile::set`
(`coherence_dimensions.rs:96`) matches only `semantic | logical | temporal | causal | procedural |
empirical` and falls through to `_ => {}`. There is no `ontological` field. The one dimension that
is not a plain text-similarity measure is written to nothing.

Net: the composite is (three cosine-derived values + three constants) / six.

`physis-core/src/core.rs:303` `update_coherence` — the score for every `CoherenceNode` — is mean
cosine to its 5 nearest neighbours, clamped `≥ 0`. Negative coherence is unrepresentable, so a node
cannot be *incoherent*, only *isolated*.

### 5.3 Fabricated fitness constants — SEVERITY: blocking

`src/ai/harness.rs:290`, the only place in the product path that populates a full breakdown:

```rust
predictive_success: 0.70,      // literal
outcome_success: 0.70,         // literal
```

`predictive_success` carries 0.25 of the composite. `resolve_prediction`
(`physis-core/src/hypothesis.rs:450`) exists specifically to compute it; its only caller is the CLI
(`physis-core/src/main.rs:1130`). The product path never calls it — so whether an interpretation
worked never enters its score, which makes the engine's headline claim literally false on the path
customers run.

`outcome_success` is worse: nothing computes it anywhere, and `recompute_fitness`
(`hypothesis.rs:412`) does not include it in the composite at all. It is displayed, never computed,
never used.

Adjacent fabrications: `physis-core/src/core.rs:750` hardcodes
`coherence_profile.empirical.score = 0.2` in the dream loop; `delta_engine.rs:407` sets
`hyp.coherence = new_semantic_fit`, overwriting the composite with one dimension; `harness.rs:277`
uses `0.60` as the "no evidence" neutral while `hypothesis.rs:378` uses `0.5` for the same
condition in the same pipeline.

### 5.4 The thesis's empirical verification is one test with a hash embedder — SEVERITY: high

`PHYSIS_THESIS.md` §8 is a seven-row table, every row marked **VERIFIED**, citing
`physis-core/tests/e2e_verification.rs`. That file is 252 lines and contains **one** `#[test]`,
which builds its state with `RandomProjectionEmbedder::new(64)` (`e2e_verification.rs:22`) —
SHA-256 n-gram feature hashing into 64 dimensions, no trained model. Every row is N = 1 and
hand-constructed.

- The table reports `Cosine = 1.088`. Cosine similarity cannot exceed 1.0; this is a nearest-entry
  score labelled a cosine.
- "Quality Penalty ... `1.0884 → 1.0000`" is the `clamp(0.0, 1.0)` at `quality.rs:189`, not a
  learning effect.
- "Truth Maintenance ... contextually resolved as `BPreferred`" — the test *assigns*
  `resolution = ResolutionStatus::BPreferred` and asserts it was stored.

Same pattern in the suite: `tests/epistemic_evolution.rs::test_peculiarity_benchmark_comparison` is
named as a baseline comparison, but at lines 564–566 it writes `semantic = 0.85`, `logical = 0.90`,
`empirical = 0.30` by hand and then asserts a relation over the composite of those constants. It
compares nothing. `test_1_contradiction_preservation` constructs the contradiction, sets its
resolution and explanation, and asserts the store retained it — a persistence test named as an
epistemology test.

`PHYSIS_THESIS.md:174` links `file:///home/gio/dev/physis-pro/...` — an absolute path on one
developer's machine, in the flagship document.

### 5.5 The dream loop does not generate — SEVERITY: medium

`physis-core/src/core.rs:727`:

```rust
let statement = format!("Candidate hypothesis explaining failure in node: {}", ..);
let mut hyp = Hypothesis::new(statement, femb);   // femb = the FAILURE's own embedding
hyp.coherence_profile.empirical.score = 0.2;      // literal
```

The "candidate explanation" is a template string carrying the failure's own vector. It proposes
nothing; it relabels.

### 5.6 RAG has no epistemic input — SEVERITY: high

`physis-core/src/rag.rs:152` is cosine ranking, greedy MMR with a diversity discount, and a
token-budget cutoff — textbook and correctly implemented. No ontology relation, contradiction
state, provenance quality, or temporal validity enters the ranking. The "epistemically constrained
retrieval" claim is not implemented; what exists is a competent standard retriever.

### 5.7 SECOM benchmark measures the wrong thing — SEVERITY: high

The skeleton is good: seeded split, feedback from **train** only (`secom.rs:556`), evaluation on
test, `--no-feedback` ablation. No leakage. Then `secom.rs:541`:

```rust
correct: obs.outcome.map(|gt| if gt { top.score > 0.5 } else { top.score <= 0.5 }),
```

`top.score` is the classifier's confidence in whichever cell it picked; `predicted_cell` is written
on the line above and never compared to anything. A passing wafer counts as "correct" when the
classifier was *unconfident* about an unrelated cell.

Also: no external baseline — real SECOM's ≈ 6.6 % fail rate means majority-class scores ≈ 0.934, so
an unbaselined accuracy figure is uninterpretable; and the "optional download" promised at
`secom.rs:4` does not exist in code. The embedded CSV is **21 synthetic rows**, 10 in the test
split.

### 5.8 Documentation drift — SEVERITY: low, but it is what a reviewer reads first

- `physis-core/src/lib.rs:20` names five coherence dimensions (`Semantic, Ontological, Logical,
  Empirical, Process`); the code has six and shares only three of those names.
- `AGENTS.md:12` states all development happens on `pansa` via ssh; this audit built and tested on
  a different host without incident.

---

## 6. Information-loss boundaries

| # | boundary | file:line | what is lost | recorded? |
|---|---|---|---|---|
| 1 | **pooling** | `embed_onnx.rs:208` | token positions, layer depth, per-dimension structure. One 768-d unit vector survives; everything downstream is cosine over it | no |
| 2 | **node coherence** | `core.rs:303` | a node's entire relation to the graph → one mean cosine over 5-NN, clamped ≥ 0. Incoherence unrepresentable | no |
| 3 | **composite overwrite** | `delta_engine.rs:407` | six dimensions → one | no |
| 4 | **`set("ontological")`** | `coherence_dimensions.rs:96` + `harness.rs:252` | the ontological score, entirely, silently | **no — this is a bug** |
| 5 | **hypothesis provenance** | `hypothesis.rs:265` | `provenance: Option<String>` — one string. Structured chains live in the side-map `core.provenance_chains`, not on the object | partial |
| 6 | **evidence strength** | `hypothesis.rs:63` | no numeric strength distinct from `confidence`; an external signal becomes an epistemic weight on contact | no |
| 7 | **temporal flattening** | throughout scoring | `TemporalValidity` is stored and never read by any scorer | no |

Boundary 4 is a defect. Boundaries 1–3 and 7 are design choices that must be *stated* in any paper,
because each is a place where the thesis's distinction between similarity and coherence stops being
maintained.

---

## 7. Conflations to correct

Places where vector similarity is treated as an epistemic quantity, against the project's own
stated position:

1. `harness.rs:199` — similarity to a failed claim treated as contradiction.
2. `harness.rs:216` — similarity to *both* sides of an open contradiction treated as conflict.
3. `core.rs:606` — two hypotheses existing treated as a contradiction between them.
4. `core.rs:378` — vector-space outlier treated as contradiction.
5. `core.rs:303` — neighbourhood density treated as coherence.
6. `delta_engine.rs:407` — semantic fit assigned to `coherence`, the composite.
7. `harness.rs:295–296` — predictive and outcome success asserted as constants.
8. `secom.rs:541` — classifier confidence treated as classification correctness.
9. `rag.rs:152` — retrieval relevance treated as sufficient for epistemic packing.

---

## 8. The correction: physis-core master carries a real experiment program

**This section revises the verdict and must be read before acting on §1–§7.**

The pinned submodule (`9d43821`, v0.1.14) has `src/experiments.rs` as 62 lines of doc comment and
zero lines of code — five experiments specified, none implemented. That is true of the tree
physis-pro builds, and it is why §5 reads as it does.

`physis-core` **origin/master** (`f4dd2a1`, v0.1.15) is 28 commits ahead and contains 22 experiment
programs in `examples/`, implementing 26 numbered iterations:

```
Iteration  1  reference-conditioned discovery vs. naive baselines
Iteration  2  deterministic k-means baseline + fine-grained ground truth
Iteration  3  JEPA-style predictive latent, with a decisive control
Iteration  4  same predictor on real ontology data; a NaN bug caught before it lied
Iteration  5  contextual sentences vs bare terms, isolating the effect
Iteration  6  replicate Iteration 5 on Dataset A (taxonomy) — effect vanishes
Iteration  7  recursion (mission Steps 8-9), the piece never attempted
Iteration  8  Dataset D (cross-cutting concepts) — first unambiguous positive result
Iteration  9  certification gate validation — silhouette scoring backwards
Iteration 10  close gaps 1-3 (calibration, integration, DAG protection)
Iteration 11  hunt for a certification signal robust to balance
Iteration 12  integrated end-to-end synthesis mission
Iteration 13  local calibration (works) + ensemble gate (still doesn't)
Iterations 14-15  certification-signal family exhausted; real ontology shows widespread overlap
Iteration 16  the formal ablation matrix (Section 18) — the mission's capstone test
Iteration 17  genuine JEPA-style ontology binding
Iteration 18  multi-embedder comparison, BGE vs MiniLM on real ontology
Iteration 19  cross-embedder corroboration for multi-membership — not supported
Iteration 20  fixed-point discovery via density peaks — decisively unstable
Iterations 21-22  linkage layer over fixed cells; word discovery vs WordNet
Iteration 23  WordNet fix; language detection works, disambiguation partially
Iteration 24  cross-embedder split agreement — not supported, 5th signal family to fail
Iteration 25  capacity-constrained training loss — still not supported, sharper diagnosis
Iteration 26  persistent registry (PermaTrack's insight), and what it does/doesn't fix
```

This is the experimental discipline the thesis lacks, and it is of a different quality from
anything in physis-pro. Sampled directly:

- `examples/experiment16_ablation_matrix.rs` is a real ablation: six semantically distinct
  configurations over the same ontology, coarse assignment held **identical across all six** so
  each mechanism's marginal contribution is isolated, scored at coarse or fine level depending on
  whether the config recursed, averaged over 20 items, run for both bare and contextual
  representations. Its header quotes the governing fairness rule: *"if raw cosine plus thresholding
  performs equally well, say so."*
- `examples/experiment24_cross_embedder_split_agreement.rs` compares a given partition against a
  second, architecturally different embedding space's **independently discovered** partition via
  Adjusted Rand Index — the second embedder is "never told what MiniLM's given partition looked
  like". It distinguishes itself explicitly from the adjacent question tested in Iteration 19 and
  reports the result as not supported.

Most of these iterations **failed**, and the failures are recorded as failures, with diagnosis,
in sequence, each referencing what the previous one ruled out. Five signal families were tried and
abandoned. That is what a research program looks like.

Two things follow:

1. **§5's "no experimental discipline" conclusion is wrong about the project and right about the
   pinned tree.** The work exists; physis-pro cannot see it, and no document points to it.
2. **This is the publishable artefact.** A negative-results paper — *"Five signal families for
   unsupervised ontology certification, and why none of them worked"* — is defensible today. It
   needs a write-up, a results table, and reproducible run instructions; it does not need more
   code. That is a rarer and more useful contribution than the epistemic-architecture paper the
   thesis is aiming at.

What is missing to publish: the results are in commit messages and program stdout, not in a
results file. Nothing records the numbers each iteration produced, so the series cannot currently
be read without re-running 22 programs against models that must be fetched separately.

---

## 9. Recommendation

Two tracks, in this order.

**Track A — make the shipped engine honest (PH-017 … PH-019, PH-021, PH-022).**
Fix the ontological drop, the fabricated constants, the cosine contradiction detector, the SECOM
metric, and the thesis's claim table. Two of these are one-line changes. Until they land, no
measurement of physis-pro means anything, and the thesis document actively misrepresents the
engine.

**Track B — write up the experiment series (PH-020, PH-023).**
Capture the 26 iterations' results into a machine-readable record, port the discipline into
physis-pro's own path so its claims can be tested the same way, and write the paper. On current
evidence this is the honest publication and it is close to ready.

The strongest thing this repository can publish is a negative result, honestly measured. It already
has the measurements — in `examples/` on a branch its own product cannot reach.

Work packets: `packets/PH-017` … `packets/PH-023`. Plan: `PLAN.md` Phase 22.

---

## 10. Reproducing this audit

```bash
git clone git@github.com:ilPez00/physis-pro.git && cd physis-pro
git submodule update --init --recursive

# weights (do NOT place models/jina-clip-v2)
mkdir -p models/bge-base-en-v1.5/onnx
# fetch BAAI/bge-base-en-v1.5   onnx/model.onnx -> models/bge-base-en-v1.5/onnx/model.onnx
# fetch Xenova/all-MiniLM-L6-v2 onnx/model.onnx -> models/model.onnx

cargo build --release
cargo test --release > /tmp/tests.txt 2>&1; echo "exit=$?"      # NOT `| tail` — that masks it
./target/release/physis-pro doctor                              # expect dim=768

# the one real accuracy measurement (~11 min)
cargo test --release --test classification -- --ignored --nocapture

# the one dataset benchmark
./target/release/physis-pro benchmark secom --embedder onnx --output /tmp/secom

# §8: the experiment series, NOT reachable from the submodule pin
cd physis-core && git fetch origin && git log --oneline 9d43821..origin/master
git show origin/master:examples/experiment16_ablation_matrix.rs
```
