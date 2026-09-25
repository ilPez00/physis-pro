# Canonical Portfolio

Live papers, one file each, all compiling clean (verified 2026-09-25).

| # | Canonical title | File | Pages | Status |
|---|---|---|---|---|
| A | Executable Falsification for AI-Assisted Research | `papers/paperM_methodology.tex` | 10 | live — needs verifier-falsification section |
| B | Multi-Harness Coding-Agent Action Corpus | `papers/paper9_dataset.tex` | 5 | live — needs related-work positioning |
| C | Operational Failure Priors Are Pair-Specific Across Coding-Agent Harnesses | `papers/paper1_failure_priors.tex` | 12 | live — merged this pass, gated |
| D | Propose, Do Not Certify | `papers/paper4_propose_certify.tex` | 7 | live — strongest manuscript |
| E | Semantic Trajectories | `papers/paper2_trajectories.tex` | 3 | **blocked** — external replication analysis not run |
| F | Empedocles | `papers/paper10_empedocles.tex` | 4 | live — needs controls + FSS integrated |
| — | Representation Instability in Induced Ontologies | `papers/paper3_ontology_instability.tex` | 4 | **superseded** — banner added |

## Authorship convention (applied to all 7)
```
Giovanni Pezzin \ Physis Research Program
```

## Commands
```bash
cd research/papers
pdflatex -interaction=nonstopmode -halt-on-error paper1_failure_priors.tex
bash build_all.sh                       # registry validator + release audit gates
python3 ../results_registry/validate.py # 37 checks, 0 failures
```
