# Physis Research Program — Publication Portfolio

Derived from the complete audit of the Physis experiment history (2026-06 through 2026-09).
Every paper below is scored against the evidence actually in the repository, not against
the claims in READMEs or commit messages.

---

## Portfolio summary

| # | Paper | Classification | Evidence strength | Time to submission |
|---|-------|---------------|-------------------|--------------------|
| 1 | Transferable operational failure priors for coding agents | **READY TO WRITE** | Strong (4 agents, 75,933 actions, cross-machine) | 2-4 weeks |
| 2 | Semantic trajectories of evolving software objects | **ONE EXPERIMENT AWAY** | Promising (6 objects, 5/6 temporal direction) | 4-8 weeks |
| 3 | Representation instability in induced ontologies | **RESEARCH PROGRAM** | Settled negative (cross-embedder ARI ~0.10) | 8-12 weeks |
| 4 | Propose, do not certify | **ONE EXPERIMENT AWAY** | Strong (top-3 0.712 vs null 0.136) | 4-6 weeks |
| 5 | Failure modes of AI-assisted empirical research | **READY TO WRITE** | Rich (12+ documented corrections) | 2-3 weeks |
| 6 | Routed symbolic/neural contradiction detection | **RESEARCH PROGRAM** | Promising but narrow (precision 1.000, recall 0.840) | 8-12 weeks |
| 7 | Cheap representation / expensive model routing | **RESEARCH PROGRAM** | Interesting (cascade rerank, floor 0.66) | 6-10 weeks |
| 8 | Self-falsifying adaptive systems / research methodology | **READY TO WRITE** | Unique (executable falsification workflow) | 2-4 weeks |
| 9 | Dataset release: multi-agent coding action corpus | **ONE EXPERIMENT AWAY** | Data exists (75,933 actions, redacted) | 4-6 weeks |
| 10 | Empedocles criticality — power-law cluster distribution | **WORKSHOP** | Narrow (300 nodes, 2D, one seed) | 2-4 weeks |
| 11 | The null is part of the instrument | **READY TO WRITE** | Methodological, 12+ cases | 2-3 weeks |

---

## Paper 1 — Transferable operational failure priors for coding agents

**Classification:** READY TO WRITE

**One-sentence claim:** Cross-agent failure priors transfer when target history is thin and coverage is high, with a measurable cold-start crossover point.

**Why it matters:** This is the first demonstrated case where an operational prior (action-kind failure rates) learned from one coding agent predicts another agent's failures above the target's own thin-history baseline, with coverage as the measurable transfer gate.

**Existing evidence:**
- E71: 9,466 actions, 27 sessions, frozen prior AUC 0.645 vs 0.500 base-rate
- E73: Cross-machine transfer feather->pansa 0.644 vs in-corpus 0.642
- E74: Four harnesses (Claude Code, Codex, Cline, OpenCode), 75,933 actions, 402 sessions
- E74: Cold-start rule: borrowed prior beats own when own is thin, loses when thick
- E74: Coverage sets the ceiling: every pair >0.6 has >=0.46 coverage
- Redacted corpus exists: `benchmarks/behaviour/corpus-redacted.json.gz`

**Missing evidence:**
- External replication on public coding-agent datasets
- Leave-one-scaffold-out validation
- Calibration curves (Brier scores exist but calibration plots absent)
- Simple baselines (logistic regression on action-kind dummies)

**Novelty:** HIGH. Prior work on coding-agent failure prediction exists (SWE-bench, Commit0, etc.) but none demonstrate cross-agent transfer with a coverage gate.

**Scientific strength:** Strong within one user's fleet; externally unvalidated.

**Reproducibility:** High — committed redacted corpus + extraction scripts.

**External validity:** LOW until replicated on public datasets from other users.

**Implementation cost:** Low — analysis scripts exist.

**Experiment cost:** Medium — external replication requires public dataset acquisition.

**Time to submission:** 2-4 weeks (with external replication).

**Best venue family:** EMNLP, ICSE, MSR.

**Main reviewer attack:** "This is one user's machines, one fortnight, four harnesses. The 'cross-agent' label is four harnesses but one human."

**Cheapest test that could kill it:** Leave-one-scaffold-out — if the prior collapses when the target scaffold differs from all source scaffolds, the claim is one-fleet-specific.

---

## Paper 2 — Semantic trajectories of evolving software objects

**Classification:** ONE EXPERIMENT AWAY

**One-sentence claim:** Revisions of a single source file form a low-dimensional directed semantic trajectory, with temporal direction recoverable from embeddings alone.

**Why it matters:** If software objects are better represented as trajectories than static points, this changes how we think about code search, clone detection, and version similarity.

**Existing evidence:**
- E78: 6 source files, within-object distance 4-14x smaller than between-object
- E78: Temporal direction for 5/6 files (Spearman rho up to +0.976, beats shuffled-x null)
- E78: 2D spectral concentration (0.569-0.817 variance in first 2 dims)
- E79: Granularity matters — document-level identity survives, paragraph-level dissolves
- E79: Position direction exists in developmental docs but not reference docs

**Missing evidence:**
- Expansion to thousands of files across many public repos
- Multiple encoders (only BGE-base tested)
- Git-revert behavior (do reverts create closed loops?)
- Feature/fix/refactor/revert trajectory taxonomy
- Lexical controls (is this just edit distance in embedding space?)

**Novelty:** MODERATE. Software evolution and representation learning are well-studied, but the specific claim about trajectory directionality from embeddings alone is new.

**Scientific strength:** Promising but narrow (6 objects, 1 repo, 1 embedder).

**Reproducibility:** High — scripts committed.

**External validity:** LOW until expanded to public repos.

**Implementation cost:** Low — analysis scripts exist.

**Experiment cost:** HIGH — needs thousands of files, multiple encoders, public repos.

**Time to submission:** 4-8 weeks.

**Best venue family:** EMNLP, ICSE, FSE.

**Main reviewer attack:** "Six files is anecdotal. The 'temporal direction' could be corpus drift (file gets longer/more complex over time, which correlates with commit order)."

**Cheapest test that could kill it:** Random-walk control — if random walks in embedding space produce the same directionality, the result is geometry not temporal signal.

---

## Paper 3 — Representation instability in induced ontologies

**Classification:** RESEARCH PROGRAM

**One-sentence claim:** Cross-embedder ontology agreement is near chance (ARI ~0.10), and the dominant cause is representation instability rather than corpus properties.

**Why it matters:** This settles a foundational question for any system that induces ontological structure from embeddings.

**Existing evidence:**
- E1-E18: Cross-embedder ARI ~0.10, reproduced across multiple vector sets
- E14: Injection realism — random injection gives AUC 0.880, nearest-cell gives 0.506
- E18: ANCHORVOC closed as entry-length artifact (0.619 of 0.664 explained by length alone)
- E88: Anisotropy measured (0.53-0.56 mean pairwise cosine), slope -0.489 per unit
- E90: Rank intervention fails — anisotropy is a covariate, not a lever
- E15/E16: Real misfilings — every geometric scorer at chance (0.508-0.519)

**Missing evidence:**
- External ontology datasets (WordNet, SUMO, DOLCE)
- More than 3 embedder pairs
- Intervention studies on public taxonomy induction benchmarks

**Novelty:** MODERATE. Embedding instability is known; the specific ontology-induction framing is less studied.

**Scientific strength:** Strong negative result, well-controlled.

**Reproducibility:** High.

**External validity:** MODERATE — needs public ontology datasets.

**Implementation cost:** Medium.

**Experiment cost:** Medium.

**Time to submission:** 8-12 weeks.

**Best venue family:** EMNLP, NeurIPS, ICML.

**Main reviewer attack:** "This is a negative result about one corpus (658 entries, 57 cells). The field has moved on to LLM-based ontology induction."

**Cheapest test that could kill it:** If a simple ensemble of 3+ embedders achieves ARI >0.5 on the same corpus, the "instability" claim becomes "instability is fixable by averaging."

---

## Paper 4 — Propose, do not certify

**Classification:** ONE EXPERIMENT AWAY

**One-sentence claim:** Representations that are unreliable as autonomous judges (AUC 0.41 on real misfilings) can remain highly useful as candidate generators (top-3 0.712 vs permuted null 0.136).

**Why it matters:** This reframes the evaluation of representation quality: a representation need not be a good judge to be a useful proposal mechanism.

**Existing evidence:**
- E21: Top-3 0.712 vs permuted 0.136, learning curve still climbing at 436 decisions
- E24: Halving hints (alpha=0.5) beats shipped vector — top-3 0.705 vs 0.651
- E16: Direct certification at chance (AUC 0.519) — the negative that motivates this
- Contrast: E14-E18 show certification fails while E21 shows proposal succeeds

**Missing evidence:**
- Human-in-the-loop study (does the proposer actually help a human filer?)
- Second rater (is the learned policy generalizable or rater-specific?)
- External task (does this generalize beyond physis's filing task?)

**Novelty:** MODERATE. The propose-vs-judge distinction is known in IR (retrieval vs ranking) but not empirically demonstrated for ontology filing.

**Scientific strength:** Strong within its domain; needs external validation.

**Reproducibility:** High.

**External validity:** LOW — single rater, single corpus.

**Implementation cost:** Low.

**Experiment cost:** Medium — needs human study or second rater.

**Time to submission:** 4-6 weeks.

**Best venue family:** EMNLP, CHI, UIST.

**Main reviewer attack:** "One rater (Claude) produced both the training decisions and the evaluation labels. The 'proposer' learned the rater's filing policy, not the corpus structure."

**Cheapest test that could kill it:** Second rater on a sample — if the proposer's top-3 doesn't transfer to a different rater's decisions, the result is rater-specific.

---

## Paper 5 — Failure modes of AI-assisted empirical research

**Classification:** READY TO WRITE

**One-sentence claim:** AI-assisted empirical research produces systematic, identifiable failure modes that executable falsification can catch.

**Why it matters:** As AI agents increasingly design and run their own experiments, the failure modes of that process become a first-order research question.

**Existing evidence (12+ documented cases):**
1. E14: Wrong injection distribution (random vs nearest-cell changes AUC from 0.880 to 0.506)
2. E83: Generated fixture circularity (antonym rule scored 0.998 on self-generated fixtures)
3. E85: Adversarial collapse (E84 rules went from precision 1.000 to 0.000 on adversarial input)
4. E75: Order-shuffle failing to change predictor (path dependence refuted at f32 precision)
5. E76: Measurement bug — parser picked up node count instead of coherence
6. E77: Degenerate null (all drift values identical -> permutation band collapsed)
7. E78: First 1200 bytes only — module header barely changes, within-object distance read 0.0000
8. E17: Confound check that stopped at first reassuring answer (ignored Test 3 failure)
9. Claims audit: 10 documentation errors in one session, 5 by the author
10. E19b: Control arm validity depends on model scale (B5c fires at 4B, valid at 9B)
11. E81: Static rules — single-word cues at chance under shuffle (0.92 ratio)
12. E10: Pre-flight returned STOP on power, run proceeded anyway

**Novelty:** HIGH. No systematic catalog of AI-assisted research failure modes exists.

**Scientific strength:** Strong — every case has a documented experiment and verdict.

**Reproducibility:** High — all cases have committed code and results.

**External validity:** HIGH — these failure modes are generic to AI-assisted research.

**Implementation cost:** Low — writing only.

**Experiment cost:** Low — all evidence exists.

**Time to submission:** 2-3 weeks.

**Best venue family:** NeurIPS, ICML, EMNLP.

**Main reviewer attack:** "This is a list of mistakes, not a scientific result. Where's the generalizable finding?"

**Cheapest test that could kill it:** If the failure modes are not reproducible by other AI research assistants, they are author-specific.

---

## Paper 8 — Self-falsifying adaptive systems / research methodology

**Classification:** READY TO WRITE

**One-sentence claim:** Physis's claim/verdict/history architecture constitutes an executable falsification workflow that retains failed branches as research evidence.

**Why it matters:** This is a methodological contribution about how to build AI systems that can falsify their own hypotheses.

**Existing evidence:**
- Claim ledger with verdicts (supported/contradicted/partial)
- Construction-matched nulls (permuted labels, shuffled orders, random injections)
- Failed branches preserved (E1-E18 all remain in the repo)
- 360-degree audit (E75-E77) that tested the system's own claims
- No-deletion policy for failed experiments

**Missing evidence:**
- Comparison with other autonomous-science agents (e.g., Sakana AI Scientist)
- Formal characterization of the falsification workflow
- Does the workflow catch errors that human review would miss?

**Novelty:** MODERATE. Autonomous science agents exist, but the explicit claim/verdict/history architecture with retained failures is distinctive.

**Scientific strength:** Moderate — the architecture exists and works, but the formal comparison is missing.

**Reproducibility:** High.

**External validity:** LOW — single system, single domain.

**Implementation cost:** Low.

**Experiment cost:** Low.

**Time to submission:** 2-4 weeks.

**Best venue family:** NeurIPS, ICML, AAAI.

**Main reviewer attack:** "This is a software architecture description, not a scientific contribution. Where's the empirical evidence that this workflow produces better science?"

**Cheapest test that could kill it:** If the workflow does not catch any errors that a careful human researcher would not also catch, its value is ceremonial.

---

## Paper 9 — Dataset release: multi-agent coding action corpus

**Classification:** ONE EXPERIMENT AWAY

**One-sentence claim:** A carefully anonymized corpus of 75,933 coding-agent actions from four harnesses across three machines enables cross-agent failure prediction research.

**Why it matters:** Public coding-agent trajectory datasets are rare; a multi-harness corpus with outcome labels would be a valuable community resource.

**Existing evidence:**
- Redacted corpus exists: `benchmarks/behaviour/corpus-redacted.json.gz` (796 KB)
- 402 sessions, 75,933 actions, 4 harnesses, 3 machines
- Targets replaced by salted digests
- Action kinds normalized
- Session boundaries preserved

**Missing evidence:**
- Ethics/privacy review (the audit found an API key and bearer token in raw commands)
- License clarification
- Community demand assessment
- Comparison with existing public datasets

**Novelty:** MODERATE. SWE-bench, Commit0, and others exist, but a multi-harness cross-machine corpus is distinctive.

**Scientific strength:** N/A (dataset contribution).

**Reproducibility:** High — corpus is committed.

**External validity:** HIGH — community can validate.

**Implementation cost:** Low — corpus exists.

**Experiment cost:** Medium — needs ethics review, documentation, license.

**Time to submission:** 4-6 weeks.

**Best venue family:** EMNLP, ICSE, MSR.

**Main reviewer attack:** "The corpus is one user's machines, one fortnight. The 'cross-agent' label is four harnesses but one human."

**Cheapest test that could kill it:** If a simple baseline (global failure rate) achieves comparable prediction to the full model, the dataset's value is limited.

---

## Paper 10 — Empedocles criticality

**Classification:** WORKSHOP

**One-sentence claim:** A deterministic node-field with coupled attraction/repulsion forces exhibits a power-law cluster-size distribution at a critical parameter band.

**Existing evidence:**
- 300 nodes, 2D, 400 ticks, sweep of nemesis 0.001-0.160
- Critical band at nemesis ~0.01, eros=0.10
- Power-law-like distribution (59 32 9 9 8 8 7 6 6 5)
- Gate 2 (no regression) still pending

**Missing evidence:**
- Multiple seeds
- Finite-size analysis
- Distribution fitting (power-law vs lognormal/exponential with goodness-of-fit)
- Sensitivity to dimensionality, node count, timestep/update rule

**Novelty:** LOW. Self-organized criticality is well-studied (sandpile, neuronal avalanches). The Empedoclean dyad is a minor variant.

**Scientific strength:** Weak — one configuration, no seeds, no distribution fitting.

**Reproducibility:** High.

**External validity:** N/A — toy system.

**Implementation cost:** None.

**Experiment cost:** Low.

**Time to submission:** 2-4 weeks (as workshop paper).

**Best venue family:** Complex Systems workshop, NeurIPS workshop.

**Main reviewer attack:** "This is a 300-node particle simulation. Where is the connection to actual neuro-symbolic AI?"

**Cheapest test that could kill it:** If the power-law fit is not significantly better than lognormal, the "criticality" claim is unfounded.

---

## Paper 11 — The null is part of the instrument

**Classification:** READY TO WRITE

**One-sentence claim:** In AI-assisted experimental software research, the construction-matched null is not a formality but the primary instrument, and its design determines what can be concluded.

**Why it matters:** This methodological paper would establish null-design as a first-class research concern.

**Existing evidence:**
- E14: Null design changed the verdict (random injection AUC 0.880 vs nearest-cell 0.506)
- E15/E16: Null injection policy changed which scorer "won"
- E18: Length-only control captured 0.619 of ANCHORVOC's 0.664
- E83: Generated fixture circularity (null was the generator)
- E85: Adversarial fixtures changed precision from 1.000 to 0.000
- E19b: Control validity depends on model scale

**Novelty:** HIGH. No prior work treats null design as the primary instrument in AI-assisted research.

**Scientific strength:** Strong — multiple independent cases.

**Reproducibility:** High.

**External validity:** HIGH — generic to all experimental research.

**Implementation cost:** Low — writing only.

**Experiment cost:** Low.

**Time to submission:** 2-3 weeks.

**Best venue family:** NeurIPS, ICML, EMNLP.

**Main reviewer attack:** "This is a philosophy of science paper, not a machine learning paper."

**Cheapest test that could kill it:** If the examples are all from one project, the generalizability is unproven.

---

## What should NOT consume more time

1. **Cross-domain structural convergence (E19/E19b)** — NOT SUPPORTED, and the field (abstraction learning) is crowded.
2. **Mode axis discovery (E22-E29)** — Every computational route closed; mode is absent from the representation. No paper until new data sources emerge.
3. **Mapper hypothesis / Qwen n-grams (E2-E4)** — REFUTED. The map's topology does not carry labels. No paper.
4. **Taxonomy constraint (E5-E8)** — NOT SUPPORTED. Granularity is not the binding constraint. No paper.
5. **Conatus as implemented (E75-E77)** — Three of four ingredients are inert fields. No paper until the fields are wired and tested.
6. **Empedocles as a "neuro-symbolic AI" contribution** — It is a toy physics simulation. Only publishable as a workshop paper at best.
7. **E63-E65 retrieval experiments** — The "2400x build cost" claim needs external validation. The cascade rerank result is interesting but BM25 reranking is well-known.
8. **E53 structural analogy** — Likely reduces to classical link prediction. Kill it if it reduces to a standard method.
9. **E9/E10 signifier address** — Best address 0.244 held-out vs geometry's 0.447. Not competitive. No paper.
10. **PH-037 NeuralSubstrate** — Feature-gated biological backends with no demonstrated advantage over deterministic backends.
