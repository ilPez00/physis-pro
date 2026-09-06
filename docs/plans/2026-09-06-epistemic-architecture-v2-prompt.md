# Agent brief — Epistemic Architecture v2

**Written:** 2026-09-06
**For:** an agent (or a person) taking on the redesign of Physis's epistemic core.
**Prerequisite reading:** `docs/EPISTEMIC_AUDIT_2026-09-06.md`, `PLAN.md` Phase 22, `packets/PROTOCOL.md`.
**Relationship to the packets:** `PH-017` … `PH-023` fix what is *broken*. This brief addresses what is
*mis-designed*. Do not start it until PH-017, PH-018 and PH-019 have landed — every measurement here
is meaningless while the composite silently drops a dimension, hardcodes two of its inputs, and
detects contradiction by cosine.

---

## 0. Role and standing orders

You are working on an engine whose stated purpose is to maintain competing interpretations of
reality, evaluate their coherence with observations and outcomes, and preferentially retain the ones
that keep working. The engine currently does not do this: it stores interpretations, scores each one
in isolation against its vector neighbourhood, and sorts them. Your job is to close that gap, or to
establish that it cannot be closed and say so.

Six standing orders. They override any instruction below that appears to conflict with them.

1. **Measure before and after every change.** A mechanism that has not been compared against the
   thing it replaces has not been evaluated. The comparison target is always the *simplest* thing
   that could work — usually raw cosine plus a threshold.
2. **A negative result is a successful outcome.** `physis-core` origin/master already contains 26
   iterations in which five signal families were tried and abandoned, with the failures recorded as
   failures. That is the standard. Match it.
3. **Never fabricate a score.** A quantity that cannot be measured must be *representable as
   unmeasured*, never filled with a plausible constant. See `PLAN.md` → Architecture Constraints.
4. **Never invent an API.** Read the source before writing against it. Where an interface does not
   exist, mark your proposal as an adapter or extension and say so explicitly.
5. **Cite the prior art you are reimplementing.** Most of this problem was solved, or proved
   unsolvable, between 1979 and 2006. Section 8 is not optional background; it is a work item.
6. **Comments record the failure that motivated the code.** If you change code a comment describes,
   rewrite the comment to describe the new reason. Never delete it. (Repo rule,
   `packets/README.md`.)

---

## 1. Required reading, in this order

Do not write code until you have read all of it. Budget a full session.

**The engine as it is:**

| file | why |
|---|---|
| `physis-core/src/hypothesis.rs` | `Hypothesis`, `Evidence`, `FitnessBreakdown`, `recompute_fitness` (:370), the composite weights (:412) |
| `physis-core/src/coherence_dimensions.rs` | the six dimensions and `composite()` — the design flaw in §2 lives here |
| `physis-core/src/core.rs` | `update_coherence` (:303), `detect_contradictions` (:378), `dream_hypotheses` (:727) |
| `physis-core/src/contradiction.rs` | the contradiction model — this part is good, preserve it |
| `physis-core/src/relation.rs` | `TypedEdge`, `RelationType::{is_epistemic,is_causal,is_procedural}` — exists, read by nothing |
| `physis-core/src/temporal.rs` | `TemporalValidity` — correct, tested, consulted by no scorer |
| `physis-core/src/delta_engine.rs` | the shadow-frame propagation engine — the strongest component |
| `src/ai/harness.rs:190-300` | where the product path actually computes coherence and fitness |

**The research record — this is not optional:**

```bash
cd physis-core && git fetch origin
git log --oneline 9d43821..origin/master        # 28 commits, the 26-iteration series
git show origin/master:examples/experiment16_ablation_matrix.rs
git show origin/master:examples/experiment24_cross_embedder_split_agreement.rs
```

Iterations 14–15 report "widespread apparent overlap" in the real ontology. Iterations 19, 20, 24,
25 report five signal families as not supported. **Read what has already been ruled out before
proposing anything.** Re-running a failed experiment under a new name is the most likely way for
this brief to waste a month.

---

## 2. Problem 1 — the composite destroys what the profile preserves

**Current state.** `CoherenceProfile` separates six dimensions with care, then `composite()`
collapses them into one scalar by a fixed weighted mean, and `recompute_fitness`
(`hypothesis.rs:412`) does the same for fitness with weights `0.20 semantic / 0.15 ontological /
0.15 logical / 0.25 empirical / 0.25 predictive`.

**Why this is a design flaw, not a tuning problem.** The separation exists so that "coheres
semantically but fails empirically" is distinguishable from "coheres empirically but is
ontologically ungrounded". The weighted mean makes them identical whenever the arithmetic happens to
match. And no fact about the world fixes the weight of the semantic dimension at 0.20 — the number
is unjustified and *unjustifiable in principle*, because the dimensions are not commensurable
quantities.

**What to build.**

1. Keep `CoherenceProfile` as the ranking object. Do not collapse it by default.
2. Implement **Pareto dominance** over the dimension vector: H₁ dominates H₂ when it is ≥ on every
   dimension and > on at least one. Ranking returns the non-dominated frontier, not a sorted list.
3. Where a single ordering is genuinely required (UI, top-k retrieval), require an explicit
   **policy** naming the weights and the goal it serves, sourced from `FitnessContext`
   (`quality.rs:595`) — never a global default. A caller that wants one number must say for what.
4. Keep `composite()` for backwards compatibility, `#[deprecated]`, with a comment recording that
   its fixed weights were never justified and that this is why.

**How to test.** Construct two hypotheses that the weighted mean scores identically but which differ
on every dimension; assert the new ranking distinguishes them and reports *why*. Assert the
non-dominated frontier is invariant to any monotone rescaling of a single dimension — the weighted
mean is not, which is the point.

**What would falsify this direction.** If, on a real task with real outcomes, the Pareto frontier is
almost always the whole set (no dominance structure), the vector carries no usable ranking signal
and a policy-weighted scalar is the honest answer. Measure this before committing to the redesign.

---

## 3. Problem 2 — nothing propagates, so nothing competes

**This is the deepest problem in the engine. Fix it first if you fix only one thing.**

**Current state.** Evidence attaches to one hypothesis and stops. `TypedEdge` and `RelationType`
exist and no scorer reads them. Each hypothesis's coherence is computed locally and independently,
then the results are sorted. There is no mutual-exclusion construct anywhere in either crate.

**The consequence.** Supporting H₁ does nothing to H₂. "Competing hypotheses" never compete — they
are scored in isolation and ranked. Every claim about preferential retention among competitors is,
at present, a claim about sorting.

**What to build.**

1. **`ExclusionSet`** — an explicit, first-class set of hypotheses that cannot all hold. This is the
   missing primitive; without it competition is not representable. It is *distinct* from
   `Contradiction`, which is about two claims; an exclusion set is about a partition of possibility.
2. **Constraint relaxation over the hypothesis graph.** Follow Thagard's ECHO: excitatory links for
   explanatory support and coherence relations, inhibitory links for exclusion and contradiction,
   then relax to equilibrium and read off acceptance. This is a small algorithm — connectionist
   constraint satisfaction, a few dozen lines — and it is forty years old and well understood. Do
   not invent a new one.
3. **Propagate along the edges that already exist.** `is_causal` and `is_procedural` should mean
   something to the scorer. Start with support propagation along `RelationType` and measure whether
   it helps before adding anything cleverer.
4. Every acceptance decision must remain **auditable**: record the relaxation's inputs and the
   resulting equilibrium in `EpistemicAuditTrail`, so "why is this believed" can still be answered.

**How to test.** The decisive test: attach supporting evidence to H₁ where H₁ and H₂ are in an
exclusion set, and assert **H₂'s standing falls without any evidence being attached to H₂**. Under
the current architecture that is impossible; it is the single clearest before/after demonstration
this brief can produce.

**Prior art you must read first:** Thagard's TEC/ECHO (1989) — competing hypotheses, coherence as
constraint satisfaction, contradiction as inhibitory links, "accepted if it coheres better overall
than its competitors". Also de Kleer's ATMS (1986), which maintains all consistent contexts
simultaneously. Cite both.

---

## 4. Problem 3 — coherence is defined as density, which rewards echo chambers

**Current state.** `core.rs:303` `update_coherence` sets a node's coherence to the mean cosine to
its 5 nearest neighbours, clamped `≥ 0`.

**Why this is wrong by construction.** A cluster of mutually-repeating errors scores maximally
coherent. Ten copies of one source score higher than two independent corroborating sources. And
because of the clamp, negative coherence is unrepresentable — a node cannot be *incoherent*, only
*isolated*.

**What to build.**

1. Weight agreement by **source independence**. Two sensors agreeing is evidence; one sensor quoted
   twice is not. `Evidence.source`, `Evidence.context`, `ContradictionParty.authority` and
   `CoherenceNode.embedder` already carry enough to estimate shared provenance — start there.
2. Make incoherence representable. Either allow the score to go negative, or carry a separate
   `tension` term. Whichever you choose, record in the comment that the clamp at `core.rs:333` was
   the previous behaviour and why it changed.
3. Add a **duplicate/near-duplicate detector** on the ingest path so the same document arriving
   twice does not manufacture corroboration.

**How to test.** Construct a corpus of ten paraphrases of one claim from one source, and a corpus of
two independent claims from two sources. Assert the second scores higher. Under the current
implementation the first wins by a wide margin — that is the regression test.

---

## 5. Problem 4 — scores in [0,1] that are not probabilities

**Current state.** Every score is an `f32` in `[0,1]` combined by hand-chosen arithmetic. There is
no way to ask P(H|E), to combine independent evidence correctly, or to express that two sources
agreeing means less when they share an origin.

**What to build.** Pick *one* and commit to it, in a design note before any code:

- **Dempster–Shafer** — keeps the non-probabilistic "mass" intuition the engine already has, gives a
  real combination rule, and represents ignorance distinctly from disbelief (which maps well onto
  the `Option<Score>` "unmeasured" representation PH-018 introduces). Probably the best fit.
- **A Bayes net over hypotheses** — stronger guarantees, gives conditioning, but requires priors the
  engine does not currently have and does not obviously want.

Whatever you choose, the existing `Score` values must map into it explicitly and the mapping must be
documented. Do not add a calculus alongside the arithmetic and leave both running.

**How to test.** Evidence combination must be associative and commutative where the chosen calculus
says it should be. The current arithmetic is not; assert the new one is.

---

## 6. Problem 5 — nothing generates hypotheses

**Current state.** `dream_hypotheses` (`core.rs:727`) builds a template string
(`"Candidate hypothesis explaining failure in node: {}"`) carrying **the failure node's own
embedding**, sets `empirical.score = 0.2` as a literal, and stores it. It proposes nothing; it
relabels.

**Why this matters.** The engine's story is evolutionary — variation, selection, retention. There is
no variation operator. Selection over a population supplied entirely by the caller is a sorting
algorithm with extra steps.

**What to build.** A real variation operator, in ascending order of ambition — implement and measure
(1) before attempting (3):

1. **Recombination**: propose hypotheses by combining ontology-adjacent concepts from failed
   hypotheses' `ontology_refs`. Cheap, deterministic, testable.
2. **Abductive templates** over `RelationType::is_causal` edges: given an observed effect, propose
   the causes the ontology says can produce it.
3. **LLM proposal, epistemically gated**: generate candidate explanations, then require each to
   enter as a normal `Candidate` hypothesis with provenance, competing under §3's machinery like
   any other. The gate is what makes this acceptable — a generated hypothesis gets no privilege.

**How to test.** Generated candidates must have embeddings *distinct from* the failure they explain
(the current implementation fails this trivially) and must be able to lose to an existing hypothesis
under §3's relaxation.

---

## 7. Problem 6 — the fixed 5×14 grid is a large unargued commitment

**Current state.** 70 canonical cells, treated as universal. The measured symptom: SECOM
semiconductor-fab observations classify into `BOND×PLAY`, `HEAL×PLAN`, `HEAL×WORK`.

**Evidence from your own lab.** Iterations 14–15 report "widespread apparent overlap" in the real
ontology. Iteration 20 found fixed-point discovery "decisively unstable". Five signal families for
unsupervised certification failed. **These results bear directly on this problem and may already
constrain the answer** — read them before proposing anything here.

**What to build.**

1. Treat the grid as **one axis among several**, not the frame. The `axis_kind` machinery
   (`office`, `machine`, `agent`) already exists — measure whether domain-appropriate axes fix the
   SECOM mismatch before designing anything new.
2. Report classification **out-of-domain** honestly: when the best cell's score is near the
   distribution of an arbitrary cell's score, the answer is "this ontology cannot name this", not a
   confident cell. That is a small change with a large honesty payoff.
3. Only then consider learned or plural ontologies — and check Iteration 21's "linkage layer over
   fixed cells" first, since that is the same idea and it has already been tried.

---

## 8. Prior art — a work item, not background

The coherence/contradiction/audit/provenance machinery is well-trodden. Two of these are *threats*
to the thesis and must be engaged in writing, in `docs/PRIOR_ART.md`, before the paper.

**Read and cite:**

- **Doyle (1979) JTMS; de Kleer (1986) ATMS.** Justification tracking, non-destructive multiple
  contexts. ATMS maintains all consistent contexts simultaneously — competing interpretations
  coexisting, in 1986. State what Physis adds.
- **Thagard (1989), *Explanatory Coherence*, BBS; the ECHO program.** The closest ancestor by a wide
  margin: competing hypotheses, coherence as constraint satisfaction, contradiction as inhibitory
  links, acceptance by coherence relative to competitors. This is `PHYSIS_THESIS.md`'s thesis
  statement with a working algorithm behind it, from 1989. **Not citing this is not survivable.**
- **Bovens & Hartmann (2003); Olsson (2005) — the impossibility results. THIS IS A THREAT.** There
  are no non-trivial probabilistic coherence measures that are truth-conducive, even in a weak
  ceteris-paribus sense. Physis's core claim is that coherence tracks what works. You must state
  explicitly why the engine escapes this — the strongest available argument is that Physis measures
  *predictive success against recorded outcomes*, not truth, which puts it outside the scope of the
  impossibility proofs. **Write that argument down and check it holds.** If it does not, restate the
  thesis.
- **Ontology change propagation** (OntoRipple and the change-impact-analysis line). Good news:
  propagating ontology changes to dependent artifacts is described there as an open challenge, still
  largely manual. `delta_engine`'s shadow-frame simulation is a live contribution — position it
  against this literature specifically.
- **2025–26 LLM agent memory — crowded, and some of it is ahead of you formally.** Kumiho
  (graph-native versioned memory, formal AGM correspondence, principled rejection of the Recovery
  postulate, explicit avoidance of the Flouris impossibility results); *Belief Memory* (partial
  observability); *EvoSCM* (scientific belief revision via causal model evolution); MMA-Bench
  (belief dynamics under conflict with controlled source-reliability priors); reliability-conditional
  updating with provenance-capped poisoning defence.

**The strategic conclusion, which the paper must reflect:** Physis's differentiators are *not* the
coherence/contradiction/audit machinery. They are (a) the shadow-frame ontology-mutation delta
engine, (b) the industrial operational-outcome feedback loop, (c) the 26-iteration negative-results
series. Lead with those.

---

## 9. Evaluation — use other people's yardsticks

The engine's largest gap is that it has never been measured against anything external. That gap is
now cheap to close, because the 2025–26 agent-memory literature has built benchmarks.

1. **Adopt an external benchmark** for contradiction handling and staleness — MMA-Bench and the
   contradiction/staleness suites referenced in that literature are the obvious candidates. Survey
   what is actually available and runnable before committing.
2. **Report the trivial baseline beside every number.** Last-write-wins for memory tasks;
   majority-class for classification; raw cosine plus threshold for retrieval. If Physis does not
   beat these, that is the result.
3. **Report mean ± sd over ≥ 5 seeds**, with N and the physis-core commit. Note that origin/master
   commit `041b401` fixed non-deterministic `classification_domains()` ordering — results from
   before it may not reproduce.
4. **Ablate every mechanism this brief adds.** Propagation off, exclusion sets off, independence
   weighting off, generation off. A mechanism that survives ablation without changing the metric is
   complexity you should delete.

---

## 10. Deliverables

In order. Each is independently reviewable; do not batch them.

1. `docs/PRIOR_ART.md` — §8, including the written argument on the impossibility results.
2. A design note per §2–§6 decision, committed *before* its implementation, stating what was chosen,
   what was rejected, and what would falsify the choice.
3. The implementations, one per commit, each with its before/after measurement.
4. `docs/benchmarks/` entries for every external benchmark run, with baselines.
5. An ablation table covering every mechanism added.
6. A revision of `PHYSIS_THESIS.md` that matches what the engine now does, coordinated with PH-022's
   `docs/CLAIMS.md`.

## 11. Never

- Never re-run an experiment the 26-iteration series already ruled out, under a new name.
- Never collapse the coherence vector into a scalar by default.
- Never let a cosine threshold produce a `Contradiction` — it may only prefilter candidates.
- Never delete a losing hypothesis. It loses; it stays. Contradictions are information.
- Never claim a mechanism improves epistemic performance without an ablation and a trivial baseline.
- Never present a hand-constructed scenario as an empirical result. That is the defect PH-022 exists
  to fix; do not recreate it here.
- Never soften a negative result. Five exhausted signal families are the most credible thing this
  project has produced.
