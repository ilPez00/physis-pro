#!/usr/bin/env python3
"""E25 — is a compound entity additive, or a transform applied to its head?

THE CLAIM UNDER TEST (user's, 2026-09-09). "dog tooth" is not dog + tooth. It is
the dog-transform applied to tooth: a tooth attached to a dog is a specialised
instance of tooth, still its own entity, with dog acting as an operator —
`dog-modulus|tooth|`. Fractal, because the same move recurs at every scale.

That is a claim about REPRESENTATION and it has a sharp consequence.

  ADDITIVE   v(M H) = v(M) + v(H).  Then the modifier contributes a CONSTANT
             offset: v(M H) - v(H) is the same vector whichever head it attaches
             to. Offsets for one modifier should cluster tightly.

  TRANSFORM  v(M H) = T_M . v(H).  Then the modifier's contribution DEPENDS on
             what it acts on, so v(M H) - v(H) varies with H, and the offsets for
             one modifier do NOT cluster beyond what any random group does.

  ASYMMETRY  the head is the operand and keeps its identity, so v(M H) should sit
             closer to v(H) than to v(M). Additivity predicts no such asymmetry.

CONTROL, house rule 1. Offset coherence is compared against a construction-
matched null: groups of the same size assembled from RANDOM (modifier, head)
pairs. Any set of offsets has some coherence; only coherence above that null is
evidence for additivity.
"""
import json, math, os, random, statistics, sys, urllib.request
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
HOST = os.environ.get("E25_EMB_HOST", "http://100.116.39.57:11434")
MODEL = os.environ.get("E25_EMB_MODEL", "nomic-embed-text:latest")
CACHE = os.path.join(HERE, "e25_word_vecs.json")
MIN_GROUP = 4


def norm(v):
    n = sum(x * x for x in v) ** 0.5 or 1.0
    return [x / n for x in v]


def cos(a, b):
    return sum(x * y for x, y in zip(a, b))


def embed_all(strings, cache):
    todo = [s for s in strings if s not in cache]
    for i, s in enumerate(todo):
        body = json.dumps({"model": MODEL, "input": s}).encode()
        req = urllib.request.Request(HOST + "/api/embed", data=body,
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=120) as f:
            cache[s] = json.load(f)["embeddings"][0]
        if i % 100 == 0:
            print(f"  embed {i}/{len(todo)}", file=sys.stderr, flush=True)
            json.dump(cache, open(CACHE, "w"))
    json.dump(cache, open(CACHE, "w"))
    return cache


def main():
    entries = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
    comps = []
    for e in entries:
        w = e["name"].split()
        if len(w) >= 2:
            comps.append((e["name"], " ".join(w[:-1]), w[-1]))
    want = set()
    for full, m, h in comps:
        want |= {full, m, h}
    cache = json.load(open(CACHE)) if os.path.exists(CACHE) else {}
    cache = embed_all(sorted(want), cache)
    V = {k: norm(v) for k, v in cache.items()}

    print(f"E25 — composition.  {len(comps)} compound names, {len(want)} strings embedded")

    # --- asymmetry: is the compound nearer its head or its modifier?
    dh = [cos(V[f], V[h]) for f, m, h in comps]
    dm = [cos(V[f], V[m]) for f, m, h in comps]
    nearer_head = sum(1 for a, b in zip(dh, dm) if a > b)
    print(f"\nASYMMETRY  cos(compound, head)     {statistics.mean(dh):.3f}")
    print(f"           cos(compound, modifier) {statistics.mean(dm):.3f}")
    print(f"           compound nearer its HEAD in {nearer_head}/{len(comps)} "
          f"= {nearer_head/len(comps):.3f} of cases")

    # --- additivity: are a modifier's offsets constant across heads?
    groups = defaultdict(list)
    for f, m, h in comps:
        groups[m].append((f, h))
    big = {m: v for m, v in groups.items() if len(v) >= MIN_GROUP}

    def coherence(pairs):
        offs = [norm([a - b for a, b in zip(V[f], V[h])]) for f, h in pairs]
        cs = [cos(offs[i], offs[j]) for i in range(len(offs)) for j in range(i + 1, len(offs))]
        return statistics.mean(cs) if cs else None

    real = [coherence(v) for v in big.values()]
    rng = random.Random(0)
    allpairs = [(f, h) for f, m, h in comps]
    null = []
    for m, v in big.items():
        for _ in range(20):
            null.append(coherence(rng.sample(allpairs, len(v))))
    print(f"\nADDITIVITY  offset coherence, mean pairwise cosine of v(MH) - v(H)")
    print(f"  within a modifier ({len(big)} modifiers, n >= {MIN_GROUP})  "
          f"{statistics.mean(real):+.3f}")
    print(f"  random groups of the same sizes (the null)          "
          f"{statistics.mean(null):+.3f}")
    print(f"  excess over null                                    "
          f"{statistics.mean(real)-statistics.mean(null):+.3f}")

    # --- predictive: which model reconstructs the compound best?
    def rank(pred, f):
        return cos(norm(pred), V[f])
    add = [rank([a + b for a, b in zip(V[m], V[h])], f) for f, m, h in comps]
    hd = [cos(V[h], V[f]) for f, m, h in comps]
    md = [cos(V[m], V[f]) for f, m, h in comps]
    print(f"\nRECONSTRUCTION  cosine to the true compound vector")
    print(f"  additive  norm(v(M)+v(H))   {statistics.mean(add):.3f}")
    print(f"  head only v(H)              {statistics.mean(hd):.3f}")
    print(f"  modifier  v(M)              {statistics.mean(md):.3f}")
    print("""
READING
  ADDITIVE predicts offsets that cluster well above the null, and no head/
  modifier asymmetry. TRANSFORM predicts offsets no more coherent than the null,
  and the compound sitting closer to its head. Neither is proved by one corpus of
  short technical names, and this measures ENGLISH NOUN COMPOUNDS as an embedder
  sees them, not ontological part-whole structure — see the report's limits.""")


if __name__ == "__main__":
    main()
