# Paper M related-work audit

## Searches run
- "AI scientist autonomous research agent verification reproducibility critique"
  — useful results obtained (below).

## Comparators found

| Source | Relevance to M |
|---|---|
| **Autonomous Research Agents: A Survey of AI Scientists and the Verification Gap** (Ding, Nannapaneni, Liu, Zhang; arXiv:2608.05179) | Codes 26 entries across 7 audit dimensions. In 24 runnable systems: **83% release code but only 38% release seeds/execution traces and 38% report any novelty-verification method.** Among nine closed-loop L4 systems, seven are mechanical reruns, one is author-claimed without external check, and **no LLM-era system demonstrates an externally validated in-loop oracle**. This is the closest existing statement of M's thesis and must be cited. |
| **The Age of AI Agents Demands A New Scientific Paradigm** (arXiv:2607.26064) | Argues verification/attribution/reproducibility infrastructure breaks under agent scale; cites Luo et al. (2025) identifying four failure modes in AI-Scientist systems: benchmark cherry-picking, data leakage, metric misuse, post-hoc selection bias. M's I1 (threshold frozen) and its leakage case are concrete instances. |
| Cornelio et al. (2025); Bisht et al. (2026) | Verification problem for AI-driven discovery; agents "not built for autonomous discovery" (biased problem selection, missing tacit knowledge, diversity collapse, single-turn benchmarks). |

## What this changes for M

M's contribution is now **positioned rather than asserted**: the field's
reported gap is that claim-verification artifacts are far rarer than code
release (38% vs 83%), and that no LLM-era system in the survey demonstrates an
externally validated in-loop oracle. M's executable ledger — nine invariants,
with I8's stripped-record probe proving the suite can fail — is that oracle for
one programme, at the scale of twelve worked cases. The honest framing is *one
worked instrumentation*, not a field-wide solution, and M already says it is "an
instrumentation proposal with twelve worked examples".

The verifier-falsification section added this pass is the part the survey's
coding dimensions do **not** cover: the survey audits whether systems release
verification artifacts, not whether the released verifiers were themselves
falsified.
