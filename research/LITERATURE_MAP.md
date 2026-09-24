# Literature Map — Closest Prior Work and Novelty Gaps

---

## Paper 1 — Coding-Agent Failure Priors

**Closest prior work:**
- SWE-bench (Jimenez et al., 2024) — coding-agent evaluation, but no failure prediction
- Commit0 (Li et al., 2024) — commit message generation, not failure prediction
- LLM-based fault localization (Yang et al., 2025) — predicts faulty code locations, not action outcomes
- Self-Repair (Chen et al., 2024) — error correction, not prediction

**What is already known:**
- Coding agents fail in predictable ways (SWE-bench failure analysis)
- Transfer learning across code tasks is effective

**What would NOT be novel:**
- "Coding agents can fail" — known
- "Cross-task transfer helps" — known

**Specific gap:**
- No work demonstrates cross-agent transfer of operational failure priors with a coverage gate
- No work shows a cold-start crossover point where borrowed > own prior

**Strongest defensible novelty claim:**
> First demonstration that action-kind failure priors transfer across coding-agent harnesses with a measurable coverage-gated crossover point.

**Risk of being scooped:** MODERATE — SWE-bench trajectory data is growing rapidly

**External datasets available:** SWE-bench trajectories, Commit0 logs, OpenHands traces

**External baselines available:** Global failure rate, per-kind global rate, simple logistic regression

**Potential venue:** EMNLP, ICSE, MSR

---

## Paper 2 — Semantic Trajectories

**Closest prior work:**
- Code2Vec (Alon et al., 2019) — code embeddings, static not temporal
- CodeBERT (Feng et al., 2020) — pre-trained code representations
- Representation drift in NLP (Bolukbasi et al., 2016; Ethayarajh et al., 2019) — embedding drift over time, but not code objects specifically
- Software evolution models (Nguyen et al., 2010) — change prediction, not trajectory geometry

**What is already known:**
- Code embeddings capture semantic similarity
- Software evolves through predictable change patterns

**What would NOT be novel:**
- "Code has semantic similarity" — known
- "Revisions change code" — known

**Specific gap:**
- No work shows that revisions of a single file form a low-dimensional directed trajectory in embedding space
- No work demonstrates temporal direction recoverable from embeddings alone (without temporal features)

**Strongest defensible novelty claim:**
> Software objects are better represented as low-dimensional directed trajectories than static points, with temporal direction recoverable from embeddings alone.

**Risk of being scooped:** LOW — this specific formulation is novel

**External datasets available:** Public git repos (GitHub), CodeSearchNet, BigQuery GitHub

**External baselines available:** Lexical edit distance, TF-IDF trajectory, static embedding

**Potential venue:** EMNLP, ICSE, FSE

---

## Paper 3 — Ontology Instability

**Closest prior work:**
- Bolukbasi et al. (2016) — gender stereotypes in word embeddings
- Ethayarajh et al. (2019) — anisotropy measurement
- Cai et al. (2021) — isotropy interventions
- Ontology alignment (OAEI) — but uses symbolic methods, not embeddings

**What is already known:**
- Embeddings are anisotropic
- Embedding geometry varies across models
- Ontology induction from embeddings is unreliable

**What would NOT be novel:**
- "Embeddings are anisotropic" — known
- "Different models give different embeddings" — known

**Specific gap:**
- No work quantifies cross-embedder ontology agreement (ARI) on induced clusterings
- No work traces the full arc from "geometry certifies" to "geometry proposes"

**Strongest defensible novelty claim:**
> Cross-embedder ontology agreement on induced clusterings is near chance (ARI ~0.10), with anisotropy as a measured covariate, and rank intervention fails to recover geometry.

**Risk of being scooped:** LOW — negative results are rarely scooped

**External datasets available:** WordNet, SUMO, DOLCE, schema.org

**External baselines available:** k-means, spectral clustering, hierarchical clustering

**Potential venue:** EMNLP, NeurIPS, ICML

---

## Paper 4 — Propose, Do Not Certify

**Closest prior work:**
- Retrieval vs ranking separation (Nogueira et al., 2019) — IR distinction
- Human-in-the-loop taxonomy construction (Chuang et al., 2020)
- LLM-supported ontology design (Frontiers in Big Data, 2025)

**What is already known:**
- Retrieval and ranking are different problems
- Humans are better than machines at some classification tasks

**What would NOT be novel:**
- "Retrieval != ranking" — known in IR
- "Humans help with classification" — known

**Specific gap:**
- No work empirically demonstrates that a representation can be a good proposer while being a bad judge on the same task
- No work quantifies the propose/certify gap with construction-matched nulls

**Strongest defensible novelty claim:**
> Representations unreliable as autonomous judges (AUC 0.41) remain highly useful as candidate generators (top-3 0.712 vs null 0.136), with the gap between proposal and certification as the measurable finding.

**Risk of being scooped:** LOW — specific to ontology filing task

**External datasets available:** OAEI benchmarks, WordNet, domain taxonomies

**External baselines available:** Random ranking, majority class, lookup table

**Potential venue:** EMNLP, CHI, UIST

---

## Paper 5 — Failure Modes of AI-Assisted Research

**Closest prior work:**
- Sakana AI Scientist (Sakana, 2024) — autonomous research agent
- AI Scientist-v2 (Sakana, 2025) — improved version
- ML experiment tracking (Weights & Biases) — but not failure mode analysis
- Reproducibility crisis in ML (Pineau et al., 2021) — general concerns

**What is already known:**
- AI agents can make mistakes in research
- Reproducibility is a problem in ML

**What would NOT be novel:**
- "AI makes mistakes" — known
- "Reproducibility matters" — known

**Specific gap:**
- No systematic catalog of failure modes specific to AI-assisted empirical software research
- No work shows that executable falsification can catch these failure modes

**Strongest defensible novelty claim:**
> AI-assisted empirical software research produces systematic, identifiable failure modes (12+ documented cases) that executable falsification can catch, establishing null-design as a first-class research concern.

**Risk of being scooped:** LOW — this specific catalog is unique

**External datasets available:** N/A (methodological contribution)

**External baselines available:** N/A

**Potential venue:** NeurIPS, ICML, EMNLP

---

## Paper 8 — Self-Falsifying Systems

**Closest prior work:**
- Sakana AI Scientist — autonomous research loop
- AGM belief revision (Alchourron et al., 1985) — formal belief change
- Doyle's TMS (1979) — truth maintenance systems
- Hypothesis testing in automated science (King et al., 2009 — Robot Scientist)

**What is already known:**
- Belief revision is well-studied formally
- Autonomous research agents exist
- Truth maintenance systems exist

**What would NOT be novel:**
- "Beliefs can be revised" — known
- "AI can do science" — known

**Specific gap:**
- No work combines claim/verdict/history architecture with construction-matched nulls and retained failed branches
- No work stress-tests the system against its own claims (360 audit)

**Strongest defensible novelty claim:**
> An executable falsification workflow (claim -> null -> verdict -> revision -> retained failed branch) that catches its own errors, demonstrated on a 200+ module system over 3+ months.

**Risk of being scooped:** MODERATE — autonomous science is active area

**External datasets available:** N/A (methodological)

**External baselines available:** Sakana AI Scientist, Robot Scientist

**Potential venue:** NeurIPS, ICML, AAAI

---

## Paper 9 — Dataset Release

**Closest prior work:**
- SWE-bench — 2,294 task instances from GitHub issues
- Commit0 — commit messages from public repos
- OpenHands — 13K+ agent trajectories
- CodeAct — code generation trajectories

**What is already known:**
- Public coding-agent datasets exist
- Trajectory data is valuable

**What would NOT be novel:**
- "Here is coding-agent data" — known contribution type

**Specific gap:**
- No public dataset combines 4 harnesses, 3 machines, with outcome labels and cross-agent transfer evaluation
- No dataset includes a coverage-gated transfer protocol

**Strongest defensible novelty claim:**
> The first multi-harness (4), multi-machine (3), coding-agent action corpus (75,933 actions) with outcome labels and a coverage-gated cross-agent transfer protocol.

**Risk of being scooped:** MODERATE — dataset contributions are time-sensitive

**External datasets available:** SWE-bench, OpenHands, CodeAct

**External baselines available:** Global failure rate, per-kind rate

**Potential venue:** EMNLP, ICSE, MSR

---

## Cross-Cutting Literature

**Embedding anisotropy:**
- Ethayarajh et al. (2019) — How Contextual are Contextualized Word Representations
- Cai et al. (2021) — IsoScore — isotropic score
- Parupudi et al. (2024) — rank-based metrics under anisotropy

**Ontology alignment:**
- OAEI (Ontology Alignment Evaluation Initiative) — annual benchmark
- OAEI-LLM (arXiv 2409.14038) — LLM-based ontology matching
- DeepOnto (arXiv 2307.03067) — ontology engineering with deep learning

**Contradiction detection:**
- NLI models (RoBERTa-large-MNLI, DeBERTa-v3) — contradiction classification
- STALE (arXiv 2605.06527) — memory-validity benchmark
- TOKI (arXiv 2606.06240) — bitemporal operator algebra

**Autonomous science:**
- Sakana AI Scientist (2024, 2025) — autonomous research agent
- Robot Scientist (King et al., 2009) — hypothesis generation + testing
- ML experiment tracking (W&B, MLflow) — experiment management

**Software evolution:**
- Code2Vec (Alon et al., 2019) — code embeddings
- CodeBERT (Feng et al., 2020) — pre-trained code models
- Representation drift (Bolukbasi et al., 2016) — embedding changes over time

---

## Summary of Novelty Gaps

| Paper | Closest prior | Gap | Scoop risk |
|-------|--------------|-----|------------|
| 1 — Agent priors | SWE-bench | Cross-agent transfer + coverage gate | MODERATE |
| 2 — Trajectories | Code2Vec | Directionality from embeddings alone | LOW |
| 3 — Ontology instability | Ethayarajh | Cross-embedder ARI on induced clusterings | LOW |
| 4 — Propose/certify | IR retrieval/ranking | Empirical gap on same task with nulls | LOW |
| 5 — Failure modes | Sakana AI Scientist | Systematic catalog + executable falsification | LOW |
| 6 — Contradiction routing | NLI models | Symbolic+NLI complementary failures | MODERATE |
| 7 — Cheap/expensive routing | Cascade retrieval | Corpus statistics predict when neural is needed | MODERATE |
| 8 — Self-falsifying | AGM/TMS | Executable workflow with retained failures | MODERATE |
| 9 — Dataset | SWE-bench | Multi-harness + coverage protocol | MODERATE |
