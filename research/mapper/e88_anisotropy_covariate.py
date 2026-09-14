#!/usr/bin/env python3
"""E88 — is cross-embedder disagreement a function of anisotropy?

The settled verdict (PLAN.md research track): embedding geometry cannot give
stable ontology structure, cross-embedder ARI ~ 0.10. The verdict measured the
*fact* of disagreement, not its *cause*. The anisotropy literature (PLAN.md
E4, R1/R2) predicts the cause: in an anisotropic space every point sits in a
narrow cone, so each embedder's cone has a different orientation and cosine
clusterings disagree for reasons that have nothing to do with ontology.

This run measures, over the mapper corpus (cached vectors, no re-embedding):

  anisotropy   mean pairwise cosine similarity of the embedder's vectors —
               the standard cone-narrowness statistic (0 = isotropic)
  fit ARI      agreement between the embedder's cosine clustering and the
               corpus's true DOMAIN/MODE cells
  cross ARI    agreement between each pair of embedders' clusterings

Prediction under R1/R2: higher anisotropy -> lower fit ARI and lower cross ARI.
With three embedders this is machinery plus first numbers, not a verdict; the
contribution is the covariate itself, measured the same way for every future
embedder the track adopts.

    python3 research/mapper/e88_anisotropy_covariate.py
"""
import itertools, json, os, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
SETS = [
    ("bge namehints", os.path.join(HERE, "corpus_vecs_namehints.json")),
    ("bge nameonly", os.path.join(HERE, "corpus_vecs_nameonly.json")),
    ("nomic-embed", os.path.join(HERE, "corpus_vecs_nomic_embed_text.json")),
]
SEED = 0x9E3779B97F4A7C15 & 0xFFFFFFFF


def l2(x):
    n = np.linalg.norm(x, axis=1, keepdims=True)
    n[n == 0] = 1.0
    return x / n


def load(path, names):
    """Vectors are keyed by entry name; align to the corpus order and keep
    only entries every set carries, so the sets stay comparable."""
    d = json.load(open(path))
    have = d["vecs"]
    keep = [n for n in names if n in have]
    return keep, np.asarray([have[n] for n in keep], dtype=np.float32)


def true_cells(corpus):
    return [f"{e['domain']}/{e['mode']}" for e in corpus["entries"]]


def anisotropy(v, samples=20000, seed=1):
    """Mean pairwise cosine over a random sample of pairs."""
    rng = np.random.default_rng(seed)
    n = len(v)
    if n * (n - 1) // 2 <= samples:
        i, j = np.triu_indices(n, 1)
    else:
        i = rng.integers(0, n, samples)
        j = rng.integers(0, n, samples)
        keep = i != j
        i, j = i[keep], j[keep]
    sims = np.einsum("ij,ij->i", v[i], v[j])
    return float(sims.mean()), float(np.median(sims))


def cosine_kmeans(v, k, iters=64, seed=SEED):
    """Spherical k-means, k-means++ style seeding, deterministic."""
    rng = np.random.default_rng(seed)
    centers = [v[rng.integers(len(v))]]
    for _ in range(k - 1):
        d = 1.0 - v @ np.stack(centers).T
        nearest = np.clip(d.min(axis=1), 0.0, None)
        total = nearest.sum()
        idx = rng.integers(len(v)) if total <= 0 else \
            rng.choice(len(v), p=nearest / total)
        centers.append(v[idx])
    C = np.stack(centers)
    for _ in range(iters):
        assign = np.argmax(v @ C.T, axis=1)
        new = np.zeros_like(C)
        for c in range(k):
            mask = assign == c
            if not mask.any():
                new[c] = v[rng.integers(len(v))]
            else:
                m = v[mask].mean(axis=0)
                n = np.linalg.norm(m)
                new[c] = m / n if n else v[rng.integers(len(v))]
        if np.allclose(new, C, atol=1e-6):
            C = new
            break
        C = new
    return np.argmax(v @ C.T, axis=1)


def ari(a, b):
    """Adjusted Rand index from the contingency table."""
    ka, kb = a.max() + 1, b.max() + 1
    table = np.zeros((ka, kb), dtype=np.int64)
    for x, y in zip(a, b):
        table[x, y] += 1
    comb = lambda r: r * (r - 1) / 2
    sum_ij = comb(table).sum()
    sum_a = comb(table.sum(axis=1)).sum()
    sum_b = comb(table.sum(axis=0)).sum()
    n = comb(len(a))
    expected = sum_a * sum_b / n
    max_index = (sum_a + sum_b) / 2
    if max_index == expected:
        return 1.0
    return float((sum_ij - expected) / (max_index - expected))


def main():
    corpus = json.load(open(os.path.join(HERE, "corpus.json")))
    cells = true_cells(corpus)
    uniq = sorted(set(cells))
    k = len(uniq)
    runs = {}
    common = None
    for name, path in SETS:
        keep, raw = load(path, [e["name"] for e in corpus["entries"]])
        common = keep if common is None else [n for n in common if n in set(keep)]
    print(f"corpus: {len(cells)} entries, {len(common)} aligned · {k} true cells · spherical k-means at k={k}")
    for name, path in SETS:
        keep, raw = load(path, [e["name"] for e in corpus["entries"]])
        idx = {n: i for i, n in enumerate(keep)}
        order = [idx[n] for n in common]
        v = l2(raw[order])
        sub_cells = [cells[[e["name"] for e in corpus["entries"]].index(n)] for n in common]
        cells_local = sub_cells
        a_mean, a_med = anisotropy(v)
        assign = cosine_kmeans(v, k)
        fit = ari(assign, np.array([uniq.index(c) for c in cells_local]))
        runs[name] = (v, assign, a_mean, a_med, fit)
        print(f"\n{name}: anisotropy {a_mean:.3f} (median {a_med:.3f}) · "
              f"fit ARI vs true cells {fit:.3f}")

    print("\nCROSS-EMBEDDER — ARI between clusterings")
    pairs = {}
    for (n1, _), (n2, _) in itertools.combinations(SETS, 2):
        score = ari(runs[n1][1], runs[n2][1])
        pairs[(n1, n2)] = score
        print(f"  {n1:14} vs {n2:14} ARI {score:.3f}")

    print("\nCOVARIATE — anisotropy vs fit ARI (prediction: negative slope)")
    xs = [runs[n][2] for n, _ in SETS]
    ys = [runs[n][4] for n, _ in SETS]
    if len(set(xs)) > 1:
        slope = np.polyfit(xs, ys, 1)[0]
        print(f"  slope {slope:+.3f} per unit anisotropy over {len(xs)} embedders")
    for n, _ in SETS:
        print(f"  {n:14} anisotropy {runs[n][2]:.3f}  fit {runs[n][4]:.3f}")

    out = {n: {"anisotropy": runs[n][2], "fit_ari": runs[n][4]} for n, _ in SETS}
    out["cross_ari"] = {f"{a} vs {b}": s for (a, b), s in pairs.items()}
    json.dump(out, open(os.path.join(HERE, "e88_results.json"), "w"), indent=1)
    print("\nwrote e88_results.json")


if __name__ == "__main__":
    main()
