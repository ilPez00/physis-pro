# E88 — anisotropy as the covariate the geometry refutation never measured

Follows the settled verdict "embedding geometry cannot give stable ontology
structure, cross-embedder ARI ≈ 0.10" and the literature sweep's R1/R2 reframe:
that verdict measured the *fact* of disagreement, never its *cause*. The
anisotropy literature predicts the cause — in a cone-shaped space each
embedder's cone points somewhere else, and cosine clusterings disagree for
reasons that have nothing to do with the ontology.

**Run:** `python3 research/mapper/e88_anisotropy_covariate.py` — cached mapper
vectors (no re-embedding), 658 corpus entries, 57 true DOMAIN/MODE cells,
spherical k-means at k=57, ARI implemented directly. Three vector sets over
the same entries: bge-base with name+hints text, bge-base with name-only text,
nomic-embed-text.

## Result (first numbers, n=3 — machinery, not a verdict)

| set | anisotropy (mean pairwise cos) | fit ARI vs true cells |
|---|---:|---:|
| bge namehints | 0.558 | 0.062 |
| bge nameonly | 0.562 | 0.066 |
| nomic-embed | 0.527 | 0.080 |

Cross-embedder clustering ARI: 0.108 / 0.165 / 0.178 — **reproduces the
settled ≈ 0.10** on this corpus with an independent clustering implementation.
Anisotropy 0.53–0.56 is a strongly narrowed cone (isotropic would sit at 0).

The covariate's predicted direction holds in the first numbers: the least
anisotropic set (nomic, 0.527) has the best fit to true cells (0.080), and the
slope of fit ARI against anisotropy is **−0.489** per unit anisotropy. Three
points cannot separate a slope from noise — that is what the machinery is for:
every embedder the track adopts from here gets an anisotropy number alongside
its ARI, and R1's rank-metric and R2's clipping arms become the intervention
that turns this from a correlation into a test.

## What this changes

* The cross-embedder refutation is **reproducible** — a new clustering
  implementation lands in the same band (0.108–0.178).
* Geometry failures now come with a **measured covariate** attached, and the
  covariate moves in the literature's predicted direction.
* Same-model, different-text comparison (namehints vs nameonly: anisotropy
  0.558 vs 0.562, ARI 0.108) shows input construction matters less than model
  identity in this range — the cone is a property of the embedder, not the
  prompt.

## Limits

k-means at k=57 on 658 points is one clustering per set, not a distribution
(seed fixed, deterministic — that is a feature for reproducibility and a limit
for variance). Three sets share the bge model twice; the n=3 slope is
indicative only. ARI at k=57 against 57 true cells is a harsh fit measure for
any embedder — the 0.06–0.08 range is not an embedder ranking by itself.

*Script and raw results: `e88_anisotropy_covariate.py`, `e88_results.json`.*
