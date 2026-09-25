# P4 related-work audit

## Searches run
- "inter-rater reliability LLM annotation agreement ontology classification kappa"
  — useful results obtained (below).

## Comparators found

| Source | Setting | Reported agreement |
|---|---|---|
| LLM judges of unstructured text (arXiv:2501.08167) | Claude vs human on thematic summaries | Cohen's kappa **0.34–0.44**; percentage agreement 76–79%; Krippendorff alpha (ordinal) 0.49–0.60 |
| Same study, model vs model | pairs of LLMs | kappa **0.32–0.70** |
| USDC stance/dogmatism (ACL Findings 2025) | GPT-4 / Mistral on Reddit threads | pairwise kappa **0.31–0.72**; Fleiss kappa 0.435–0.485; **human–human 0.52–0.57** |
| Reliability framework for LLM classification (2026) | 14 LLMs, financial sentiment | intra-model agreement 90–98% |
| Speech-act annotation (2026) | ChatGPT-4o vs human gold | kappa 0.44–0.95 depending on layer, and **unstable on re-run** |

## How this bears on P4's kappa = 0.202

The literature's model–model kappa on annotation tasks sits around **0.32–0.70**
and human–human around **0.49–0.57**. P4's crossed-rater exact-cell agreement of
**0.202** is therefore *below* the typical range in this literature.

Two consequences, both of which strengthen the paper's existing claim rather
than the opposite:

1. The exact-cell structure is unusually rater-specific here relative to
   published annotation agreement, which is consistent with P4's finding that
   fine-grained cell identity does not transfer while coarse axes do.
2. Two facts common in this literature and relevant to P4's framing: agreement
   is **higher within a model family** than across families, and instability on
   re-run has been documented for a rater that was otherwise accurate. P4's
   single measurement per rater cannot distinguish a stable rater policy from
   run-to-run instability. That is a limitation P4 should name.

## Not claimed
Model–model agreement is **not** an estimate of human agreement. Every source
above reports LLM–human and human–human separately for exactly this reason, and
P4 already says so in its Limits section.
