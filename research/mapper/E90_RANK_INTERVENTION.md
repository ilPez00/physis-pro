# E90 — the rank intervention fails: anisotropy is real, and not the whole story

> Follows `E88_ANISOTROPY_COVARIATE.md`. The sweep's prescription (PLAN.md E4,
> R1 — Parupudi: rank-based metrics beat cosine under anisotropy) predicted an
> intervention: replace raw cosine with Spearman-rank similarity and watch fit
> ARI rise most where anisotropy is highest. That prediction was registered in
> the script header before the run. It failed.

**Run:** `python3 research/mapper/e90_rank_intervention.py` — same corpus
(658 entries, 57 true cells), same spherical k-means at k=57, same seed and
ARI machinery as E88. The only difference per arm: vectors replaced by
within-row ranks, standardized (cosine in this space is Spearman).

## Result

| set | cosine (E88) | rank (E90) | delta |
|---|---:|---:|---:|
| bge namehints | 0.062 | 0.060 | −0.002 |
| bge nameonly | 0.063 | 0.063 | −0.002 |
| nomic-embed | 0.080 | 0.074 | −0.006 |

Cross-embedder ARI barely moves: 0.108→0.133 (same model, different text),
0.165→0.175, 0.178→0.161.

## What this changes

* **The preregistered prediction is falsified, which is the point of
  preregistration.** Rank space does not recover the geometry — nowhere, and
  least of all where the cone is narrowest. The anisotropy slope E88 measured
  is real as a correlation and unusable as a lever, at least through this
  intervention.
* Two readings, honestly separated. One: **anisotropy is not the whole
  story** — the cone orientation differs by embedder, but removing the cone
  (ranks are cone-blind by construction) does not bring the clusterings into
  agreement, so the disagreement has a second source the refutation also did
  not name. Two: **the intervention is too crude** — within-row ranks discard
  the magnitude structure cosine k-means was using, and Spearman-on-768-dims
  is not what Parupudi's metric study tested (parameter-free similarities on
  *sentence pairs*, not ranks of raw coordinates fed to k-means).
* The interesting cell is the one that moved: same model, different text
  construction (namehints vs nameonly) rises 0.108→0.133 under ranks. Rank
  space aligns *input constructions* within one embedder slightly better than
  cosine does. That is a hint for theMapper's input-formatting work, not for
  cross-embedder agreement — recorded, not claimed.
* The R1 line in PLAN.md E4 stays as literature, but the E88 slope no longer
  carries a "fix this and geometry recovers" promise. The R2 arm (outlier
  clipping, Luo et al.) is the remaining untested lever — it changes the
  cone's *shape* instead of discarding it, which is what this failure
  suggests the right lever has to do.

## Limits

One rank transform, one clustering, three sets. A finer reading of R1 (its 19
metrics evaluated on retrieval pairs, not on k-means input) could still name
an intervention this run does not test. E88's slope itself remains n=3 and
indicative.

*Script and raw results: `e90_rank_intervention.py`, `e90_results.json`.*
