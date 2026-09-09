#!/usr/bin/env python3
"""E21 — geometry as a proposer, not a judge. Human/machine guided.

THE REFRAME THIS TESTS
  Every refutation in this track (E12-E18, Stage 12) refuted UNSUPERVISED,
  ZERO-SHOT judgement: COSINE, NAME-ONLY, CONSTRAINT, ANCESTRY, ANCHORVOC are
  fixed formulas, nothing fitted to a human decision. On real misfilings they sit
  at chance (0.519-0.536). What has never been measured is the SUPERVISED,
  human-in-the-loop version: give geometry k decisions a person actually made,
  and ask what it does with the rest.

WHAT IS MEASURED, AND WHY IT IS NOT ACCURACY
  Not "is the machine right" — that question is settled NO and asking it again
  would be the same experiment a seventh time. The question is whether geometry
  NARROWS THE HUMAN'S CHOICE enough to be worth using:

      top-1 / top-3 / top-5 containment of the rater's own cell,
      as a function of k, the number of decisions the rater has already made.

  A human choosing among 57 cells who is handed 3 candidates containing the
  right one has had their work cut, whether or not the top-1 is "correct".
  That is standing rule 5 (sell the filter, not the judgement) applied to
  filing rather than to discovery.

GROUND TRUTH
  refiling_gate0.tsv — Stage 11's Gate 0 blind re-filing. The target is
  new_cell, the rater's REVISED answer, not the old broken corpus. The 76
  non-blind entries are excluded, as in E16.

  Caveat carried from E16: the rater was Claude, and the same rater produced the
  replacement. Under a "is the machine right" framing that is a confound. Under
  THIS framing it is the target — the artifact is meant to predict the arbiter's
  policy so it can pre-sort for them, and whose policy it is, is a separate
  question from whether it is learnable.

CONTROLS, house rule 1
  PERMUTED   same pipeline, labels shuffled — the construction-matched null
  LEN-ONLY   name length as the only feature — the E18 confound
  PRIOR      old_cell -> most common new_cell, no geometry at all. THE ONE THAT
             MATTERS: if the rater's moves are systematic, a lookup table gets
             them and the embedder adds nothing.
  MAJORITY   the single most common cell
"""
import json, os, random, sys
from collections import Counter, defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
GT = os.path.join(HERE, "..", "perspective-discovery", "refiling_gate0.tsv")


VECS = os.environ.get("E21_VECS")  # alternate embedder, e.g. corpus_vecs_nomic_embed_text.json


def load():
    corpus = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
    if VECS:
        alt = json.load(open(os.path.join(HERE, VECS)))
        vec = alt["vecs"]
        print(f"# embedder override: {alt['model']} ({len(vec)} vectors)", file=sys.stderr)
    else:
        vec = {e["name"]: e["vec"] for e in corpus}
    rows = []
    for line in open(GT):
        if line.startswith("#") or line.startswith("n\t"):
            continue
        f = line.rstrip("\n").split("\t")
        if len(f) < 7 or f[6] != "blind":
            continue
        name, old, new = f[2], f[3], f[4]
        if name in vec:
            rows.append(dict(name=name, old=old, new=new, moved=f[5] == "MOVED",
                             v=vec[name]))
    return rows


def norm(v):
    n = sum(x * x for x in v) ** 0.5 or 1.0
    return [x / n for x in v]


def centroids(train, key="new"):
    acc = defaultdict(lambda: None)
    cnt = Counter()
    for r in train:
        c = r[key]
        acc[c] = list(r["v"]) if acc[c] is None else [a + b for a, b in zip(acc[c], r["v"])]
        cnt[c] += 1
    return {c: norm(acc[c]) for c in acc}, cnt


def rank_cells(v, cent):
    return [c for c, _ in sorted(((c, sum(a * b for a, b in zip(v, m)))
                                  for c, m in cent.items()),
                                 key=lambda t: -t[1])]


def topk(rows, ks=(1, 3, 5), seeds=5, fracs=(0.1, 0.25, 0.5, 0.75), mode="geom"):
    out = {}
    for frac in fracs:
        hits = {k: [] for k in ks}
        for s in range(seeds):
            rng = random.Random(s)
            idx = list(range(len(rows)))
            rng.shuffle(idx)
            n = max(2, int(frac * len(rows)))
            tr = [rows[i] for i in idx[:n]]
            te = [rows[i] for i in idx[n:]]
            if not te:
                continue
            if mode == "permuted":
                labs = [r["new"] for r in tr]
                rng.shuffle(labs)
                tr = [dict(r, new=l) for r, l in zip(tr, labs)]
            if mode in ("geom", "permuted"):
                cent, _ = centroids(tr)
                order = [rank_cells(r["v"], cent) for r in te]
            elif mode == "len":
                # one scalar feature: bucket cells by mean name length in train
                by = defaultdict(list)
                for r in tr:
                    by[r["new"]].append(len(r["name"]))
                mu = {c: sum(v) / len(v) for c, v in by.items()}
                order = [[c for c, _ in sorted(mu.items(),
                          key=lambda t: abs(t[1] - len(r["name"])))] for r in te]
            elif mode == "prior":
                tab = defaultdict(Counter)
                for r in tr:
                    tab[r["old"]][r["new"]] += 1
                glob = Counter(r["new"] for r in tr)
                order = [[c for c, _ in (tab[r["old"]] or glob).most_common()]
                         + [c for c, _ in glob.most_common()] for r in te]
            elif mode == "majority":
                glob = Counter(r["new"] for r in tr)
                order = [[c for c, _ in glob.most_common()] for _ in te]
            for k in ks:
                hits[k].append(sum(r["new"] in o[:k] for r, o in zip(te, order)) / len(te))
        out[frac] = {k: sum(v) / len(v) for k, v in hits.items() if v}
    return out


def axis_report(rows, vecs=None):
    """Same proposer, but on each axis alone.

    The 57 cells are domain x mode. A human handed "it is a BOND thing, now pick
    the mode" has had more work removed than the top-3 number alone shows — and
    E6/E7 found the mode axis is the weak one, explained by population rather
    than by missing definition. If that holds here, it says exactly which half of
    the filing the machine can carry and which half it cannot.
    """
    out = {}
    for axis in ("domain", "mode"):
        sub = [dict(r, new=r["new"].split("/")[0 if axis == "domain" else 1]) for r in rows]
        n_cls = len({r["new"] for r in sub})
        g = topk(sub, ks=(1, 2), seeds=5, fracs=(0.75,), mode="geom")[0.75]
        p_ = topk(sub, ks=(1, 2), seeds=5, fracs=(0.75,), mode="permuted")[0.75]
        m_ = topk(sub, ks=(1, 2), seeds=5, fracs=(0.75,), mode="majority")[0.75]
        out[axis] = (n_cls, g, p_, m_)
    return out


def main():
    rows = load()
    cells = Counter(r["new"] for r in rows)
    print(f"E21 — geometry as proposer.  {len(rows)} blind entries, "
          f"{len(cells)} distinct target cells")
    print(f"  embedder: bge-base-en-v1.5 768d (corpus.json)")
    print(f"  moved: {sum(r['moved'] for r in rows)}  kept: {sum(not r['moved'] for r in rows)}")
    print(f"  largest cell {cells.most_common(1)[0][1]}/{len(rows)} "
          f"= {cells.most_common(1)[0][1]/len(rows):.3f} (the majority floor)")
    fracs = (0.1, 0.25, 0.5, 0.75)
    modes = [("GEOMETRY", "geom"), ("PRIOR old->new", "prior"),
             ("LEN-ONLY", "len"), ("PERMUTED", "permuted"), ("MAJORITY", "majority")]
    print("\nTOP-k CONTAINMENT of the rater's own cell, by fraction of decisions seen")
    for label, mode in modes:
        r = topk(rows, mode=mode, fracs=fracs)
        print(f"\n  {label}")
        print(f"    {'k seen':>8s} " + "".join(f"{'top-'+str(k):>9s}" for k in (1, 3, 5)))
        for f in fracs:
            if f in r:
                print(f"    {int(f*len(rows)):8d} " +
                      "".join(f"{r[f][k]:9.3f}" for k in (1, 3, 5)))

    print("\nPER-AXIS, at 75% of decisions seen — which half the machine can carry")
    ax = axis_report(rows)
    print(f"  {'axis':8s} {'classes':>7s} {'top-1':>7s} {'top-2':>7s}  "
          f"{'permuted':>9s} {'majority':>9s}")
    for axis, (n_cls, g, p_, m_) in ax.items():
        print(f"  {axis:8s} {n_cls:7d} {g[1]:7.3f} {g[2]:7.3f}  "
              f"{p_[1]:9.3f} {m_[1]:9.3f}")


if __name__ == "__main__":
    main()
