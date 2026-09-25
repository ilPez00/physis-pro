# P3 Supersession Note

`paper3_ontology_instability` is **historical research lineage**. It is retained,
not maintained. A visible front-page banner now says so in the PDF.

## What was superseded
- **Its proposer story.** P3 carried an early version of "geometry can certify
  ontology membership" and the first proposer figures. `paper4_propose_certify`
  ("Propose, Do Not Certify") is the canonical treatment: geometry is a useful
  **proposer** and an uninformative **certifier**, and the 57-cell structure is
  additionally **rater-specific** (cross-rater arms, second rater = another
  language model).
- **Its ontology narrative.** The "geometry certifies ontology" arc is dead.
  What survives is representation instability as a *measured* phenomenon
  (cross-embedder ARI near chance) plus the proposer/certifier split.

## Stale numbers (do not cite from P3)
| P3 claims | Status | Canonical location |
|---|---|---|
| proposer top-k figures from the original single-rater run | stale, superseded | `research/e21_proposer_results.json`, `research/e21_cross_rater_results.json` |
| the withdrawn rising cold-start curve | **leaked**, withdrawn | `research/p1_repeated_splits.json` (leak-free) |
| certification-from-geometry | refuted | `research/e15_e18_rerun.json` |
| "correct cell" phrasing | replaced | now "rater-assigned cell" in P4 |

## Canonical current results
- `research/e21_cross_rater_results.json` — 4 crossed arms, matched nulls, CIs
- `research/e21_sequential_split.json` — sequential protocol
- `research/e15_e18_rerun.json` — certification line (COSINE 0.519, CONSTRAINT 0.536)
- Prose: `research/papers/paper4_propose_certify.tex`

## Rule
Do not continue P3 as an independent submission. Cite it only as error lineage.
