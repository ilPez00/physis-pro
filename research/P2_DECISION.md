# P2 decision record — external replication

Artifact: `research/trajectories_external/p2_results.json`
Retained broken-null evidence: `p2_results_rowperm_null_broken.json`
Generator: `research/papers/p2_analysis.py --task all`
Corpus: 434 trajectories, 11,312 revisions, 10 public repos
(c, cpp, js, python, rust). Inference unit: **file**.
Embedder: nomic-embed-text via ollama (768-d).

## Results

### Identity — survives, but is NOT specifically semantic
| repr | frac_same_closer | CI95 | same−cross median |
|---|---:|---|---:|
| nomic | 0.9906 | [0.976, 0.996] | 0.214 |
| token-jaccard (lexical) | 0.9883 | [0.973, 0.995] | 0.369 |
| edit distance (lexical) | 0.9742 | [0.954, 0.986] | 0.516 |

All three separate same-object from cross-object revisions at 97–99%, and on
the gap medians the lexical baselines are **better** than the embedding
(0.369 / 0.516 vs 0.214). Identity across revisions is real; it is not
evidence for a semantic representation. Decision rule branch **C** applies
*to this claim only*.

### Direction — the claim that IS specifically semantic
- corr(geo_adj, lex_adj) = 0.826
- corr(geo_trend, lex_trend) = **−0.593**
- partial corr(geo_adj, n_rev | lex_adj) = 0.232
- median geo_adj 0.993 vs median lex_adj 0.941

The geometric temporal statistic is not explained by text overlap — it
anti-correlates with the lexical one and retains a 0.232 partial correlation
with revision depth after controlling for lexical adjacency.

### Dimensionality — low, and unusual against a proper null
| statistic | real | null (isotropic, same n×d) | ratio | unusual |
|---|---:|---:|---:|---|
| PC1+PC2 share | 0.843 | 0.113 | 7.5x | True |
| participation ratio | 2.127 | 22.362 | 10.5x | True |
| effective rank | 3.063 | 22.674 | 7.4x | True |

Participation ratio ~2.13 and effective rank ~3.06 support "2–3 dimensional".
Branch **D** does NOT apply.

### Prediction — never implemented
`p2_results.json` carries identity/overlap/dimensionality only. Branch **E**
cannot be claimed.

## Verdict

Neither branch A nor C wholesale. The defending claim is narrower than the
original manuscript:

- **Holds:** low intrinsic dimensionality (PR ~2.1, ER ~3.1, all three
  statistics 7–10x more concentrated than an isotropic cloud), and a temporal
  direction statistic that lexical overlap cannot supply.
- **Does not hold:** identity-across-revisions as evidence *for an embedding*
  — the trivial lexical baselines match or beat it.
- **Cannot be claimed:** operational next-step prediction (not implemented).

## Defects found and fixed in the same test

1. The null permuted revision *rows*. PCA / participation ratio / effective
   rank are invariant under permutation, so the null reproduced the real value
   to 8 decimals (`real_pc12` 0.8431565762 vs `null_pc12` 0.8431565662) and
   `is_*_unusual` could never be True. A verifier that cannot fail.
2. The unusual-flag tested `real > null*1.3` for every statistic, but a **low**
   participation ratio is what concentration means. Inverted for pr/er, it
   reported `is_pr_unusual=false` for real 2.13 against a null of 22.36 —
   concealing a 10x effect in the direction the paper wanted.

Both were invisible until the two numbers were read side by side. Cited in
`paperM_methodology.tex` as a checker-must-have-kill-tests example.

## Remaining

`paper2_trajectories.tex` is still unmodified. It must be rewritten to the
narrowed claim above, must not assert "2–3 dimensional" without the isotropic
null and the per-metric direction, and must drop any operational-prediction
claim.
