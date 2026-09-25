# Superseded Papers

Nothing here is deleted. Every entry is retained as research lineage, with the
reason it was superseded and where the canonical result now lives.

| Archived manuscript | Path | Superseded by | Reason |
|---|---|---|---|
| `paper1_agent_priors` (global-negative framing) | `papers/archive/superseded/paper1_global_negative/` | `paper1_failure_priors` | One-sided framing: led with "No pair shows positive transfer" without the pair-specific and multiplicity qualifications that the artifacts support. |
| `paper1b_priors_pair_specific` | `papers/archive/superseded/paper1_pair_specific/` | `paper1_failure_priors` | Identical dataset and experiments as the above; two manuscripts asserting opposite emphases on the same evidence is not a publication strategy. Merged. |
| `paper3_ontology_instability` | still in `papers/` with front-page banner | `paper4_propose_certify` | Historical proposer/ontology narrative; stale figures. Retained as lineage, not maintained. |
| `paper5_failure_modes`, `paper8_self_falsifying`, `paper11_null_instrument` | `papers/archive/superseded/` | `paperM_methodology` | Already merged into M in an earlier pass. |

## Git lineage
Archived P1 variants were moved with `git mv`, so file history is preserved and
`git log --follow` still resolves the full edit trail.

## Guard
`research/results_registry/validate.py` (37 checks) fails the build if an
archived framing returns to a live manuscript — e.g. the pre-correction Holm
denominator `0.05/10`, an unqualified "established pair", or the leaked
cold-start curve presented as a result.
