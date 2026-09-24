# P1 — two versions, and what differs

Both compile clean (11 and 12 pages, 0 LaTeX errors, 0 undefined refs) and both
are built by `build_all.sh`. **They share the same results, tables and numbers.**
They differ only in which test is treated as primary, and each names the other
in its abstract and discussion.

| | A — `paper1_agent_priors` | B — `paper1b_priors_pair_specific` |
|---|---|---|
| title | Transferable Operational Failure Priors for Coding Agents | Cross-Harness Failure Priors Transfer Only Between Specific Agent Pairs |
| primary claim | No pair shows positive transfer after Holm; the survivors are anti-predictive | Two of nine pairs transfer, reciprocally: cline↔opencode |
| role of the other test | 1C's pair-level benefit disclosed as a refutation of the global reading | Holm failure of that pair disclosed as an unresolved tension |
| pages | 11 | 12 |

## Why both, and why neither is the "honest" one

The two tests answer **different questions**, which is the whole reason this is a
framing choice rather than a factual one:

- **Section "Multiplicity correction"** asks: does a single AUC exceed chance,
  after correcting for twelve comparisons? Answer: no pair does.
- **Section "All source–target pairs"** asks: does a *classifier built from that
  prior* beat chance across repeated partitions? Answer: for cline↔opencode, yes,
  in a majority of partitions.

A pair can fail the first and pass the second. Neither test is wrong, and neither
subsumes the other. So the defensible move is to publish both emphases and state
the tension explicitly — which is what each version does, in the abstract, the
discussion and the conclusion.

**Version A is the more conservative choice**: it leads with the result that
survives the stricter correction, and treats the pair-level finding as a
refutation it reports rather than suppresses. **Version B is the more
interesting choice**: it leads with the effect that is actually there and labels
the Holm failure as a qualification. A reviewer who cares more about
multiplicity will prefer A; a reviewer who cares more about effect detection
will prefer B.

## Evidence both versions now carry

Neither version existed before this session's experiments, which are new:

| | experiment | artifact |
|---|---|---|
| 1A | Holm recomputed and tested; nominal vs corrected separated | `research/p1_multiplicity_v2.json` |
| 1B | 500 repeated partitions, Claude Code target | `research/p1_repeated_splits.json` |
| 1C | 9 source→target pairs, 16,200 cells | `research/p1_all_targets_coldstart.json` |
| 1D | shrinkage prior vs equally-tuned target-global control | `research/p1_shrinkage_prior.json` |

Every quoted number traces to those artifacts; the build fails if the registry
disagrees.

## If you want one version and not two

Say which, and delete the other from `PAPERS` in `build_all.sh`. Nothing else
depends on both existing — the second file is a standalone manuscript, not an
include/supplement relationship. The one thing not to do is merge them into a
single paper that picks one emphasis silently, which is what the original
draft did before this evidence existed.
