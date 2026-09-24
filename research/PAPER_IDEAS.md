# Paper Ideas — Complete Inventory

All viable and rejected paper concepts discovered during the Physis audit.

---

## Viable Papers (worth pursuing)

### 1. Transferable Operational Failure Priors for Coding Agents
- **Core question:** Can failure priors learned from one coding agent predict another agent's failures?
- **Why physis enables it:** 75,933-action redacted corpus from 4 harnesses across 3 machines already exists
- **Minimal experiment:** External replication on public coding-agent datasets
- **Strong null:** Leave-one-scaffold-out; global failure rate baseline
- **Kill condition:** If prior collapses when target scaffold differs from all sources
- **Cost:** Medium (needs external dataset acquisition)
- **Classification:** READY TO WRITE

### 2. Semantic Trajectories of Evolving Software Objects
- **Core question:** Are software objects better represented as trajectories than static points?
- **Why physis enables it:** E78/E79 instrument already measures trajectory directionality
- **Minimal experiment:** Expand to thousands of files across public repos
- **Strong null:** Random-walk control in embedding space
- **Kill condition:** If random walks produce same directionality, result is geometry not temporal signal
- **Cost:** High (needs many repos, multiple encoders)
- **Classification:** ONE EXPERIMENT AWAY

### 3. Representation Instability in Induced Ontologies
- **Core question:** How much of an induced ontology is data vs embedding model?
- **Why physis enables it:** E1-E18 settled negative with 18 experiments
- **Minimal experiment:** Externalize to WordNet/SUMO/DOLCE with 3+ embedders
- **Strong null:** Simple ensemble achieving ARI >0.5 would make it "fixable"
- **Kill condition:** If ensemble fixes instability, the negative becomes a fix recommendation
- **Cost:** Medium
- **Classification:** RESEARCH PROGRAM

### 4. Propose, Do Not Certify
- **Core question:** Can representations be unreliable judges but useful proposal mechanisms?
- **Why physis enables it:** E21 shows top-3 0.712 vs null 0.136 while certification is at chance
- **Minimal experiment:** Second rater or human-in-the-loop study
- **Strong null:** If proposer doesn't transfer to a different rater, it's rater-specific
- **Kill condition:** Second rater shows no transfer
- **Cost:** Medium
- **Classification:** ONE EXPERIMENT AWAY

### 5. Failure Modes of AI-Assisted Empirical Research
- **Core question:** What systematic errors arise when AI agents design and run their own experiments?
- **Why physis enables it:** 12+ documented cases across 5 research tracks
- **Minimal experiment:** Writing only — all evidence exists
- **Strong null:** If failure modes aren't reproducible by other AI assistants, they're author-specific
- **Kill condition:** Non-reproducibility
- **Cost:** Low
- **Classification:** READY TO WRITE

### 6. Routed Symbolic/Neural Contradiction Detection
- **Core question:** Does relation polarity resist proximity, with symbolic and neural methods failing complementarily?
- **Why physis enables it:** E82-E89 trace the full arc from cosine failure to NLI routing
- **Minimal experiment:** Large public NLI/contradiction dataset
- **Strong null:** If rules+NLI doesn't beat either alone on large data
- **Kill condition:** No improvement over single-method baseline
- **Cost:** High (needs large datasets)
- **Classification:** RESEARCH PROGRAM

### 7. Cheap Representation / Expensive Model Routing
- **Core question:** Can corpus statistics predict when lexical methods suffice vs when neural is worth paying for?
- **Why physis enables it:** Cascade rerank + floor experiments exist (E63-E65)
- **Minimal experiment:** Pareto curve across tasks and corpora
- **Strong null:** If simple methods don't dominate on any task cluster
- **Kill condition:** No clear frontier
- **Cost:** Medium
- **Classification:** RESEARCH PROGRAM

### 8. Self-Falsifying Adaptive Systems / Research Methodology
- **Core question:** Does the claim/verdict/history architecture constitute a scientific-agent methodology?
- **Why physis enables it:** The architecture exists and has been stress-tested
- **Minimal experiment:** Formal comparison with Sakana AI Scientist and other autonomous science agents
- **Strong null:** If workflow doesn't catch errors humans wouldn't
- **Kill condition:** Ceremonial value only
- **Cost:** Low
- **Classification:** READY TO WRITE

### 9. Dataset Release — Multi-Agent Coding Action Corpus
- **Core question:** Does an anonymized multi-harness coding-action corpus enable cross-agent research?
- **Why physis enables it:** 75,933 actions already redacted and committed
- **Minimal experiment:** Ethics review + license + documentation
- **Strong null:** If simple baseline matches full model, value is limited
- **Kill condition:** No improvement over simple baselines
- **Cost:** Low
- **Classification:** ONE EXPERIMENT AWAY

### 10. Empedocles Criticality
- **Core question:** Does a deterministic node-field exhibit power-law cluster distribution at criticality?
- **Why physis enables it:** 300-node sweep shows critical band
- **Minimal experiment:** Multiple seeds + distribution fitting
- **Strong null:** If power-law fit isn't better than lognormal
- **Kill condition:** No significant power-law fit
- **Cost:** Low
- **Classification:** WORKSHOP

### 11. The Null Is Part of the Instrument
- **Core question:** Is null design the primary instrument in AI-assisted experimental research?
- **Why physis enables it:** 12+ cases where null design changed the verdict
- **Minimal experiment:** Writing only
- **Strong null:** If examples from one project don't generalize
- **Kill condition:** Non-generalizability
- **Cost:** Low
- **Classification:** READY TO WRITE

---

## Rejected Ideas (do not paper)

### R1. Cross-Domain Structural Convergence (E19)
- **Reason:** NOT SUPPORTED. Ceiling doesn't climb with scale. C5 never separates from one domain.
- **Verdict:** Dead. Field is crowded (abstraction learning).

### R2. Mode Axis Discovery (E22-E29)
- **Reason:** Every computational route closed. Mode is absent from the representation, not underdetermined.
- **Verdict:** Dead until new data sources emerge (activity traces, not descriptions).

### R3. Mapper Hypothesis / Qwen N-grams (E2-E4)
- **Reason:** REFUTED. Map topology doesn't carry labels. Propagation worse than ignoring.
- **Verdict:** Dead.

### R4. Taxonomy Constraint (E5-E8)
- **Reason:** NOT SUPPORTED. Granularity not binding. Pre-flight STOPs on sparsity.
- **Verdict:** Dead.

### R5. Conatus as Implemented (E75-E77)
- **Reason:** Three of four ingredients are inert fields. No path dependence. No temporal semantics.
- **Verdict:** Dead until fields are wired and tested.

### R6. Structural Analogy (E53)
- **Reason:** Likely reduces to classical link prediction. Not genuinely analogical.
- **Verdict:** Dead unless externalization shows otherwise.

### R7. Signifier Address (E9/E10)
- **Reason:** Best address 0.244 vs geometry's 0.447. Not competitive.
- **Verdict:** Dead.

### R8. NeuralSubstrate / PH-037
- **Reason:** Feature-gated biological backends with no demonstrated advantage.
- **Verdict:** Dead until advantage is demonstrated.

### R9. Hardy-Littlewood Prime Rediscovery
- **Reason:** Rediscovery of known mathematical laws is not a mathematics discovery.
- **Verdict:** Not a paper. The experimental method may have value as autonomous hypothesis generation.

### R10. JEPA/Qwen Thesis
- **Reason:** REFUTED. JEPA wins on toy corpus only via supervision confound. Loses on 730 real entries.
- **Verdict:** Dead.

---

## New Ideas Discovered During Audit (not in original prompt)

### N1. Cold-Start Transfer Law
- Estimate mathematically where borrowed prior > local prior flips to local > borrowed
- As function of: target sample count, action-kind coverage, domain divergence, source entropy, label noise
- **Why now:** E74 shows the crossover exists but doesn't model it
- **Cost:** Medium

### N2. Agent Fingerprint vs Task Fingerprint
- Does action-failure structure belong to the agent, repo, task, tool, or environment?
- Factorial decomposition across E74's four harnesses
- **Why now:** E74 shows transfer but doesn't decompose the source
- **Cost:** Medium

### N3. Universal Operational Prior
- Does a pooled prior across many agents become a surprisingly strong pretrained failure model?
- **Why now:** E74 shows cross-agent transfer; pooling is the natural extension
- **Cost:** Low (analysis only)

### N4. Information Lost by Pooling
- At what abstraction level does semantic pooling destroy identity?
- E80 shows file-level survives, project-level destroys; test systematically
- **Why now:** E79/E80 suggest a scale-dependent law
- **Cost:** Medium

### N5. Temporal Arrow Without Temporal Features
- Can semantic trajectories recover chronological ordering better than lexical edit distance?
- **Why now:** E78 shows 5/6 files have temporal direction from embeddings alone
- **Cost:** Medium

### N6. Reversions as Geometric Loops
- Do Git reverts create closed/near-closed semantic trajectories?
- **Why now:** E78 shows direction; reverts are the natural test of loop closure
- **Cost:** Low

### N7. Software Trajectory Taxonomy
- Do bug fixes, refactors, features, and reverts occupy distinct trajectory geometries?
- **Why now:** E78/E79 instruments can measure this
- **Cost:** Medium

### N8. Control-Arm Validity Checker
- Can one automatically detect that a control is invalid because it fails to perturb the statistic?
- **Why now:** E19b, E14, E17 all show control validity is model/task-dependent
- **Cost:** Medium

### N9. Automated Confound Discovery
- Can Physis automatically discover that entry length, frequency, corpus overlap explain an apparent result?
- **Why now:** E18 shows length explains ANCHORVOC; automate the search
- **Cost:** High

### N10. Experimental Archaeology
- Can commit history be used as a reproducible research log from which claims are reconstructed?
- **Why now:** The 360 audit (E75-E77) demonstrates the approach
- **Cost:** Low
