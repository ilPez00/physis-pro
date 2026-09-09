#!/usr/bin/env python3
"""E26 — are domain and mode perpendicular? And if not, does making them so help?

THE CLAIM (user's). The vector collapses dog-tooth, but geometry need not be
parallel or additive BETWEEN domain and mode if the two are perpendicular.

If the two axes occupy orthogonal subspaces, each can be read without disturbing
the other, and E24's alpha hack — halve the hints and hope — has a principled
replacement: PROJECT THE OTHER AXIS OUT and classify in what remains.

  1. MEASURE   principal angles between the domain subspace and the mode
               subspace, both spanned by centred class centroids. 90 degrees is
               perpendicular; near 0 means the axes are reading the same
               directions and interfere by construction.
  2. TEST      remove the domain subspace from every vector, then classify mode.
               Symmetrically for domain. If the axes were already perpendicular
               this changes nothing; if they are entangled it should HELP.

LEAKAGE. Subspaces are fitted on the TRAIN split only and applied to both, so the
projection never sees a held-out label. The alternative — fitting on everything —
would manufacture exactly the improvement being looked for.
"""
import json, os, random, statistics, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import run_e21 as E


def basis(vs, labels):
    """Orthonormal basis for the span of centred class centroids."""
    cents = []
    for c in sorted(set(labels)):
        m = vs[[i for i, l in enumerate(labels) if l == c]].mean(0)
        cents.append(m)
    C = np.array(cents)
    C = C - C.mean(0)                      # centre: drop the shared mean direction
    U, S, _ = np.linalg.svd(C, full_matrices=False)
    k = int((S > 1e-8).sum())
    return np.linalg.qr(C[:k].T)[0] if k else np.zeros((vs.shape[1], 0))


def project_out(V, B):
    return V - (V @ B) @ B.T if B.shape[1] else V


def nearest_centroid(Vtr, ytr, Vte, yte):
    cs = sorted(set(ytr))
    M = np.array([Vtr[[i for i, l in enumerate(ytr) if l == c]].mean(0) for c in cs])
    M = M / (np.linalg.norm(M, axis=1, keepdims=True) + 1e-12)
    X = Vte / (np.linalg.norm(Vte, axis=1, keepdims=True) + 1e-12)
    pred = [cs[i] for i in (X @ M.T).argmax(1)]
    return sum(p == t for p, t in zip(pred, yte)) / len(yte)


def main():
    rows = E.load()
    V = np.array([r["v"] for r in rows], dtype=float)
    V = V / (np.linalg.norm(V, axis=1, keepdims=True) + 1e-12)
    dom = [r["new"].split("/")[0] for r in rows]
    mod = [r["new"].split("/")[1] for r in rows]
    print(f"E26 — domain/mode geometry.  {len(rows)} entries, {V.shape[1]}-d\n")

    Bd, Bm = basis(V, dom), basis(V, mod)
    s = np.linalg.svd(Bd.T @ Bm, compute_uv=False)
    ang = np.degrees(np.arccos(np.clip(s, -1, 1)))
    print(f"PRINCIPAL ANGLES between the domain span ({Bd.shape[1]}-d) "
          f"and the mode span ({Bm.shape[1]}-d)")
    print("  " + "  ".join(f"{a:.1f}" for a in ang))
    print(f"  smallest {ang.min():.1f} deg,  mean {ang.mean():.1f} deg"
          f"   (90 = perpendicular, 0 = the same direction)")

    res = {}
    for tag in ("raw", "project-other-out"):
        accs = {"domain": [], "mode": []}
        for s_ in range(24):
            rng = random.Random(7000 + s_)
            idx = list(range(len(rows)))
            rng.shuffle(idx)
            n = int(0.75 * len(rows))
            tr, te = idx[:n], idx[n:]
            for axis, y, other in (("domain", dom, mod), ("mode", mod, dom)):
                if tag == "raw":
                    A, B_ = V[tr], V[te]
                else:
                    Bo = basis(V[tr], [other[i] for i in tr])   # fitted on TRAIN only
                    A, B_ = project_out(V[tr], Bo), project_out(V[te], Bo)
                accs[axis].append(nearest_centroid(A, [y[i] for i in tr],
                                                   B_, [y[i] for i in te]))
        res[tag] = {k: (statistics.mean(v), statistics.stdev(v)) for k, v in accs.items()}

    print(f"\n{'':20s} {'domain top-1':>16s} {'mode top-1':>16s}")
    for tag in ("raw", "project-other-out"):
        r = res[tag]
        print(f"  {tag:18s} {r['domain'][0]:.3f} +/- {r['domain'][1]:.3f}   "
              f"{r['mode'][0]:.3f} +/- {r['mode'][1]:.3f}")
    d = res["project-other-out"]["domain"][0] - res["raw"]["domain"][0]
    m = res["project-other-out"]["mode"][0] - res["raw"]["mode"][0]
    print(f"  {'delta':18s} {d:+.3f}            {m:+.3f}")
    print("""
READING
  Perpendicular axes: the deltas are ~0, because removing one subspace takes
  nothing the other was using. Entangled axes: removing the other subspace HELPS,
  and the size of the help is how much the two were competing for the same
  directions.""")


if __name__ == "__main__":
    main()
