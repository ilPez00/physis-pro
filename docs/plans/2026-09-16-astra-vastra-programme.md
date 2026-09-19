# ASTRA / VASTRA — intention-relative semantic interface (programme record)

Written 2026-09-16. This is the evaluation record for two external design
prompts, folded into the development plan. It is not a commitment to build
everything they describe; it is the map of what already exists, what is
genuinely new, and the order in which the claims can be falsified.

- **ASTRA** — "Physis as an intention layer over the existing computer": a
  keyboard-first TUI whose whole visible state is generated from
  `S = (q, c, r, d)` — intention, context, semantic radius, direction — with five
  interactions (TYPE / ZOOM / POINT / FOLLOW / ACT).
- **VASTRA** — an extension adding situated salience, temporally persistent
  identity, and an editable visual-semiotic layer (classified text, classified
  edges, legend, pins, provenance).

Neither is accepted as a specification. Both are treated as hypotheses with
prior art in this repository, and the prior art is the most useful thing in this
document.

## 1. Verdict

The conceptual core is right and unusually well matched to what Physis already
is: deterministic-first, provenance-carrying, interpretation inspectable and
reversible, the model an optional operator rather than the interface. Roughly
80% of the engine both prompts need already exists; the missing 20% is the
viewport.

Two hard problems decide whether the interface is possible at all, and neither
prompt's detail addresses them:

1. **Latency.** `pack` costs ~75 s per query over 2939 chunks with a warm
   embed cache. The target is "typing → incremental field movement". That is
   three to four orders of magnitude, and no ANN index exists in the tree.
2. **Evidence recall.** Day 3 measured the shipped packer behind a plain BM25
   baseline at equal token budget (`docs/plans/2026-09-16-day3-equal-budget-retrieval.md`).
   Until that is repaired, "the agent receives the relevant semantic region" is
   true about tokens and false about evidence.

Neither is an interface problem. Build the beautiful terminal semiotics after
them, not before.

## 2. Prior art already in the repository

Three of the prompts' open questions have already been answered here. Re-running
them would be a waste; not citing them would be worse.

| Prompt asks | In-repo result | Consequence |
|---|---|---|
| §VASTRA-16 concept clustering: "these belong together" | `physis-core/src/linkage.rs`: **seven** structurally distinct ways to *discover* stable groupings failed on real data. Re-derived clusters lose ~42% anchor overlap after +25% data; nothing survives an embedder swap (ARI ≈ 0.10). `physis-core/src/coverage.rs`: **nine** mechanisms for certifying discovered structure failed | Cluster discovery is a settled negative. Concepts must be **human-grounded**, as VASTRA already says. Now backed by measurement, not taste |
| §VASTRA-6/7 history changes later behaviour | `benchmarks/conatus/e75_history.py`: "is history storage, or does it constrain the future?" — the same final evidence in a different order must produce a different state, or every stronger word is unsupported | **E75 gates §VASTRA-6 and §VASTRA-7.** Run it before building agent trajectories |
| §VASTRA-1/23 world state vs view | `e76_perturbation.py` separates "nothing moved" from "nothing acted"; `physis-core/src/epistemic.rs` is a replayable belief timeline ("what did Physis believe at T") | The ΔW/ΔV distinction is representable today. Only the visual encoding is new |
| §ASTRA-7 granularity from geometric scale | `benchmarks/conatus/e79_granularity.py` — scans document/section/paragraph/window with position metadata. Result: **NESTED on both files tested** (coarse vector closer to its own pooled children than to another document's pool) | Supports the zoom idea. Does **not** test whether a single scalar radius orders granularities monotonically. That test is still owed |
| §ASTRA-12 named edges | `physis-core/src/relation.rs`: `TypedEdge` + 15 `RelationType`s with provenance, confidence and `TemporalValidity` on every edge | Reuse. Missing every edge the demo journey needs: `CALLS`, `DEFINES`, `IMPLEMENTS`, `CITES`, `REPLIES_TO`, `MENTIONED_IN`, `CHANGED_BY` |
| §ASTRA-14 WHY? | `physis-pro why`, `physis-pro causes`, `physis-core/src/provenance.rs`, `explanation.rs` (`ExplanationReport`, `HistoricalPrecedent`), `coherence_dimensions.rs` | Reuse |
| §ASTRA-20 proposed world changes | `src/ai/harness.rs` — "Epistemic Coherence Harness & Final Arbiter": grid placement, contradiction detection, coherence profiles, decomposed fitness, precedent recall, counterfactual critique | The accept/reject gate already exists. Manual semiotic grounding should pass through it |
| §VASTRA-12/13 pins | `cell_pin` + `PinEdit::Set/Clear` on `CoherenceNode`, with the structure/content invariant in `docs/SEMIOTIC_EDITING.md`: editing a label re-embeds it while `id`, `asserted` verdict, edges, `embedder` provenance and `cell_pin` survive; a pin wins over the auto cell and survives restart | Pins as **non-destructive overlay** are already the shipped semantics |
| §VASTRA-15/27 raw structure → language | `structsim.rs` (node-level structure comparison in one embedding space), `transform.rs` (symbolic homomorphism; E53 analogy top-3 0.500 vs popularity 0.337 vs permuted null 0.209), `extract_code_identifiers` (E60, added after the chunker scored precision 0.224 on this repo's own prose) | Both halves of the bridge exist |
| §VASTRA-17 negative pins (ATTRACT/REPEL) | `src/empedocles.rs` — Eros/Nemesis attraction/repulsion. Its own doc: "a RESEARCH module … wired to nothing on the production classify/web path", to be falsified (self-organised-criticality power law) or dropped | Do **not** reuse as if proven |
| §ASTRA-25/VASTRA-30 falsification harness | `benchmarks/retrieval/equal_budget.py` (Day 3): frozen cases, equal budget, paired deltas, explicit `NOT_MEASURED` | Use as the template for the interaction experiments |

## 2b. External precedent (three papers, 2026-09-16)

Full record: `research/RELATED_WORK_2026-09-16.md` and
`research/literature-2026-09-16.json`. None of the three is about retrieval or
knowledge graphs; the transfer is on **method**, and all three argue for the
discipline this repository already applies — measure a component in isolation
before trusting the pipeline it sits in.

| paper | what it supplies |
|---|---|
| **arXiv:2509.03636** — *CausalARC* (Maasch, Kalantari, Khezeli) | **Generative task sampling.** Each reasoning task is a sample from a fully specified SCM, so difficulty is a parameter and exact test cases were never seen. This is the direct fix for the holdout problem hit on 2026-09-16: four hand-authored retrieval holdouts were spent, and two of them gave opposite verdicts on the same comparison because difficulty was an authoring accident. Also names the PCH rungs (L1 observational / L2 interventional / L3 counterfactual) that the ledger half-implements. |
| **arXiv:2511.23465** — *SmallWorld* (Li et al.) | Isolated-evaluation discipline as external precedent, plus two design factors this repo lacks: an explicit difficulty knob, and reporting over a distribution of instances. Also a testable hypothesis about `hypothesis.rs` (does forecast error grow with revision horizon?). |
| **arXiv:2606.08242** — *Light-WAM* (Li et al.) | "The expensive component is not what produces the benefit" — measured, with cost reported beside quality. Its bottleneck ablation (8 queries 95.4 vs 16 queries 98.2) is the same finding as 3000 tokens over ~2900 chunks. |

Two consequences for this programme:

1. **Before any ASTRA/VASTRA probe is scored, generate its cases from a
   parameterized spec** rather than authoring them. Otherwise the same
   difficulty accident that flipped set #2 against set #4 will decide the
   verdict.
2. **The ledger needs an exogenous-context key for L3.** A counterfactual is
   `Y_x(u) := Y_{M_x}(u)` with `u` held fixed. `dream.rs` retrospective
   proposals replay history but do not pin `u`, so they are not counterfactuals
   in that sense — which matters for §VASTRA-31's provenance ladder and for
   §ASTRA-21's "do not infer causation from chronology".

## 3. REUSE / EXTEND / NEW

### 3.1 Situated salience

VASTRA-2 lists ten signals. Eight have a home; **none is wired to a salience
function**, and everything existing is file- or corpus-level rather than
region-level.

| Signal | Home | State |
|---|---|---|
| intention relevance | `pack`, `find` | shipped (weak — see §5) |
| semantic variance | E79 `scale` metric | benchmark-only, no runtime code |
| structural repetition | `physis-core/src/map.rs` — "The Difference/Repetition structural map" (discovery = what repeats, best-cell similarity = what differs) | shipped, corpus-level |
| repetition vs importance | `ngram_table.rs` (backoff/alpha); `becoming.rs` `deviations()` — a deviation that recurs is variation, one that does not is a mistake | shipped, unused for salience |
| prediction error | `hypothesis.rs` — `Prediction`, `resolve_prediction`, `open_predictions`, `FitnessBreakdown` | shipped |
| change | `indexer.rs` (`+~-` diff), `observe.rs`, `ledger.rs`, `study.rs` | shipped |
| graph position | `communities.rs`, coherence kNN density | shipped |
| ontology relevance | `classify.rs` `CellClassifier` | shipped |
| temporal significance | `temporal.rs`, `epistemic.rs` | shipped |
| structural density | — | **NEW** |

REUSE: everything in the table. NEW: a local (region-level) density/repetition
measure, and the salience function itself.

VASTRA-3's central claim — structural repetition is a cheap baseline for
deciding where model analysis is warranted — is testable with `map.rs` +
`ngram_table.rs` and no new representation. It is the cheapest real experiment
in either prompt.

### 3.2 Persistent identity

NEW where it matters: `indexer.rs` is path-keyed with a content hash, so a
rename is `-old/+new`. No identity across rename, split or merge.

Precedents to reuse:

- `becoming.rs` — order-aware meaning-change vs name-collision, with a null
  (Wald–Wolfowitz runs test, z-scored) and an uptake rule for mistake vs
  variation. This is VASTRA-5's "surprise" instrument, already built.
- `worldstate.rs` (E55) — recover node identity across two representations
  *from structure alone*, identity never shown to the matcher, ARI as the
  continuity column. VASTRA-4's hardest case.
- `ledger.rs` (lineage/descendants/purposes), `address.rs` (`Mechanism`),
  `observe.rs` (source/subject/at → `Evidence`).

VASTRA-6 (agent trajectory) is NEW: `AgentRunRequest` / `AgentToolInfo` /
`AgentAbortRequest` only. Gated on E75.

### 3.3 Editable semiotics

Mostly already present, and the prompts barely notice it.

- Pins: `cell_pin`, non-destructive, survives restart, wins over auto
  classification. REUSE. Missing: node→node typed pins and a pin type registry.
- Overlay vs learned: overlay is already the default (`cell_pin`,
  `quality.rs` penalties/boosts). The learned branch should be refused for now
  on the strength of `linkage.rs`.
- Machine-readable semiotics (VASTRA-28): `CoherenceNode` already carries
  `embedding`, `asserted` verdict, `cell_pin`, `embedder` provenance,
  `previous_coherence_state`, `preference`; `TypedEdge` carries provenance,
  confidence and `TemporalValidity`. What is missing is the pin type registry
  and the style map — not a data model.
- Provenance ladder (VASTRA-31): every rung exists as a field somewhere
  (`embedder` provenance, `cell_pin` = human, `asserted` verdict, quality
  penalties = human feedback, `TypedEdge.confidence`). No single enum. Small
  EXTEND.
- Visual layer (VASTRA-8..12, 21..24): NEW. Nothing renders semantic classes.
  `physis-pro-system-tui` (`src/ported/tui.rs`) already has `EnableMouseCapture`,
  click-to-select, click-to-open, click-to-focus-pane and wheel scroll — reuse
  that plumbing rather than the `physis-pro-tui` four-tab panel.

## 4. Terminal capability, honestly

Verified against the pinned versions (`crossterm 0.27`, `ratatui 0.26`):

| ASTRA asks | Reality |
|---|---|
| click / double click / right click | `MouseEventKind::Down(Button)` — `Left`/`Right`/`Middle`. Double click must be timed by us; no native event |
| wheel → semantic zoom | `ScrollUp` / `ScrollDown` |
| horizontal scroll | `ScrollLeft` / `ScrollRight` exist, and are documented as mostly touchpad |
| drag canvas / drag axis | `Drag(Button)`, `Moved` |
| trackpad pinch-to-zoom | **Does not exist in any terminal protocol.** Do not build it |
| `Δr = f(scroll_delta)` | **No scroll magnitude is exposed.** The analog feel must be an integrator over event rate and time. Buildable, but synthesized — and tmux may coalesce events |
| Kitty keyboard protocol | `KeyboardEnhancementFlags` + `Push`/`PopKeyboardEnhancementFlags` exist in 0.27 |

ASTRA-6 already refuses the pinch hack; the honest addition is that the wheel
carries no delta either, so "analog" is an implementation choice, not a
capability.

## 5. Correction to the Day 3 repair proposal

The Day 3 note proposed "hybrid (lexical ∪ cosine)" as a new candidate. That was
wrong, and the real finding is sharper:

**`src/rag.rs` (411 lines) is a stale fork of `physis-core/src/rag.rs` (734
lines).** It predates the G5 hybrid work and lacks the `bm25: Bm25Index` field
on `RagCorpus`, plus `Bm25Index`, `bm25_terms`, `rank_by_cosine`, `fuse_rrf`,
`rank_hybrid`, `HybridVerdict` and `measure_hybrid_vs_cosine`. `src/pack.rs`
imports `crate::rag`, so **the packer is the one retrieval path built on a copy
that never received the fusion the rest of the codebase already uses** —
`fuse_rrf` is called from `src/ported/act.rs:503` and swept as `rrf@60` in
`act_recall.rs`. `src/ported/studio.rs:1408` already uses `physis_core::rag`
with the BM25 leg built, which is the correct pattern.

The repair is therefore not "add fusion to the packer". It is "stop using the
stale fork", and the repo's own promotion rule
(`HybridVerdict { improvement, ships_fusion }`) decides whether the hybrid
becomes the default.

**Outcome (same day).** The fork is deleted, the hybrid is reachable behind
`pack --mode hybrid`, and it was scored on a fresh frozen holdout:
`docs/plans/2026-09-16-day3-retrieval-repair.md`. The hybrid **ties** cosine
(3/12 vs 3/12, identical on every case) while selecting different files. The
selection-policy candidate (adjacency expansion, behind `--expand`) was then
built and scored on a third holdout: it is a **trade** (+1 cross-border, −1
symbol, −1 paraphrase), also not promoted.

**The headline across three independent holdouts: a plain BM25 baseline over the
same 40-line grid, at the same token budget, beats the shipped cosine pack on
every one** (5/9, 4/12, 7/12 against 3/9, 3/12, 4/12) at 1–2 s against 65–105 s.
Neither the ranking leg nor the selection policy closes the gap, and the next
candidate is `PackMode::Lexical` — rank by the `Bm25Index` already sitting in
`RagCorpus` — on a fourth holdout.

This matters for the programme above: §ASTRA-19's "construct the minimum context
required" and §VASTRA-29's compute routing both assume the packer already
selects the right evidence. It does not yet, and the gap is the leg, not the
model — which is cheaper to fix than the plan assumed, because the leg already
exists.

## 6. Falsification order

Do not build the interface until these come back. Each can fail, and each
failure changes the design.

| # | Probe | Decides | Cost |
|---|---|---|---|
| 1 | Resident index: kill the per-query cache reload and per-process model load. Measure. | whether *anything* interactive is possible | days |
| 2 | Wire the existing `fuse_rrf`/`rank_hybrid` into the pack path; re-measure Day 3 on a **fresh** holdout | whether evidence recall can reach the lexical baseline | days |
| 3 | Radius ordering: 20 queries × 4 granularities, is the radius at which each granularity enters the field monotone across queries? | ASTRA-7/9 (zoom and graph↔text morph) | days |
| 4 | Local PCA over `N(q,c,r)`: top-2 components, can a human name them twice? | VASTRA-10/11 (meaning radar, semantic lenses) | days |
| 5 | E75 (already written) — does history constrain the future? | VASTRA-6/7 (agent trajectory, situated attention) | hours |
| 6 | Pin ablation with a **random-pin null** | VASTRA-13/19/30D (do pins teach, or overfit?) | days |
| 7 | Test G: two sessions, different prior intentions, same repo, same final query. Do their salience fields differ usefully? | the thesis | days |

Test 6's null matters most: a pin is a human asserting "these belong together",
and without a random-pin control "pins work" is indistinguishable from
overfitting the user's own examples.

Already-answered, do not re-run: concept discovery (`linkage.rs`,
`coverage.rs`), cross-representation identity (`worldstate.rs` / E55), the
granularity-nesting half of the zoom question (E79).

## 7. Role of the agent and MCP

VASTRA-28 requires that the human visual layer and the machine-readable layer be
two projections of one state. That is the MCP surface, and it is the right
design — with one blocker.

- The 30 `physis_*` tools are the machine projection: `pack` (bounded region),
  `why`/`causes` (provenance), `recall`/`remember` (outcome memory), `route`
  (the ladder), `serve` (goal linkage), `coherence_query` (typed facets).
- `src/ai/harness.rs` is the arbiter for ASTRA-20 and VASTRA-26: manual
  semiotic grounding should pass through it, not a new gate.
- `src/ai/provider.rs` (`ProviderCascade` + `RouteHint`) is the compute ladder;
  salience-driven *selection* is new, the ladder is not.
- **Blocker:** the Pro MCP server is `#[cfg(feature = "mcp")]`, `mcp` is not in
  `Cargo.toml` default features, the checked-in `target/release/physis-pro`
  answers `error: unrecognized subcommand 'serve'`, and no integration test
  launches the 30-tool server. VASTRA-28 ships on a server that cannot start.
  Fix before anything in §ASTRA-19/20 is built.

## 8. Prerequisites, in order

1. Fix the Pro MCP build and add one protocol test (list 30 tools; round-trip a
   write). Without it the action layer has no transport.
2. Repair the pack path. The fork is gone and three modes exist
   (`cosine`/`hybrid`/`lexical`, plus `--expand`); **none is promoted** — four
   holdouts disagree and the 3–1 split is query-dependent
   (`docs/plans/2026-09-16-day3-retrieval-repair.md` §9). The next candidate is
   identifier-aware leg selection, and it needs a holdout that is *generated*,
   not authored (see 2b and 3).
3. **Replace hand-authored holdout cases with sampled ones** (§2b). Until case
   difficulty is a controlled variable, every verdict in this programme is
   decided by an authoring accident — sets #2 and #4 proved that today by
   returning opposite results on the same comparison.
4. Latency: resident index. Until this lands, no probe below it is meaningful.
5. Then probes 3–7 from §6, in that order.
6. Then, and only then, the viewport: one canvas, TYPE / ZOOM / FOLLOW, over
   code + docs only, reusing `ported/tui.rs` mouse plumbing. No ACT, no editing,
   no legend editor, no heat map.

Deferred without prejudice: web as unmapped territory (ASTRA-17), the physical
computer (ASTRA-22), raw-data zoom (VASTRA-27), semiotic styling (VASTRA-8..12,
21..24), agent compute routing (VASTRA-29), identity trajectories (VASTRA-5).

Every one of them depends on 1–6 above, and none can be evaluated before them.

## 9. What must never be lost

From both prompts, the parts worth protecting:

1. **Human-imposed associations must never look like discovered geometry.** This
   matches `cell_pin`'s shipped semantics and is the difference between a tool
   and a system that lies about what it knows.
2. **Concepts are human-grounded and never auto-accepted** (VASTRA-16).
   `linkage.rs` and `coverage.rs` make this a measured result, not caution.
3. **No consciousness claims** (VASTRA-7, 27, 34). It keeps the claims
   measurable.
4. **Provenance is non-negotiable** (VASTRA-31). The data model already agrees.
5. **Do not infer causation from chronology** (ASTRA-21). `temporal.rs` and
   `ledger.rs` already keep validity separate from causality.

**Actor**: continuation session (OpenCode)
**Date**: 2026-09-16
**Revisions**: Pro `96a98c4`, Core `d65858b`
