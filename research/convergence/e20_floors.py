#!/usr/bin/env python3
"""E20 pre-flight — the floors the n-gram sieve has to beat, measured BEFORE it exists.

E20's design (HANDOFF section 6) declares three controls non-negotiable, because
"attestedness in an n-gram table is a frequency measure wearing a hat" and
E17/E18 burned four experiments on exactly that confound:

  (a) a dense sentence embedding
  (b) plain corpus frequency          <- the one that killed ANCHORVOC
  (c) length                          <- the one that killed it again in E18

None of the three needs the 51GB table. Measuring them first means the table
arrives into a scored board rather than an empty one, and if a control already
separates the matched pairs then the sieve's "expected honest positive" is not
evidence for the sieve.

METRIC. Leave-one-PAIR-out forced choice, as in preflight.py: plain LOO is
invalid here because a held-out probe's nearest neighbour is its own partner in
the opposite class. Scalar scorers (length, frequency) are scored directly as a
forced choice within the pair, which needs no training and so no holdout.

SUBSTITUTION, STATED. The design says BGE. `nomic-embed-text` is what the fleet
serves, and it is a strong general text embedder, so it makes the floor HIGHER
and the test of the sieve harder, not easier.
"""
import json, math, os, re, sys, urllib.request
from collections import Counter, defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
PR = json.load(open(os.path.join(HERE, "probes.json")))["probes"]
BY_PAIR = defaultdict(dict)
for p in PR:
    BY_PAIR[p["pair"]][p["label"]] = p
PAIRS = sorted(BY_PAIR)
EMB_HOST = os.environ.get("E20_EMB_HOST", "http://100.116.39.57:11434")
EMB_MODEL = os.environ.get("E20_EMB_MODEL", "nomic-embed-text:latest")


def toks(s):
    return re.findall(r"[a-z][a-z\-']{2,}", s.lower())


def auc(sc):
    v = sorted(sc, key=lambda x: x[0])
    pos = sum(1 for x in v if x[1]); neg = len(v) - pos
    rs, i = 0.0, 0
    while i < len(v):
        j = i
        while j + 1 < len(v) and v[j + 1][0] == v[i][0]:
            j += 1
        a = (i + 1 + j + 1) / 2.0
        for x in v[i:j + 1]:
            if x[1]:
                rs += a
        i = j + 1
    return (rs - pos * (pos + 1) / 2.0) / (pos * neg)


def forced_choice(score_of):
    """Within each matched pair, does the scorer rank the positive above the negative?"""
    hits, per_clause = 0, defaultdict(list)
    flat = []
    for pair in PAIRS:
        d = BY_PAIR[pair]
        if 0 not in d or 1 not in d:
            continue
        sp, sn = score_of(d[1]), score_of(d[0])
        ok = sp > sn
        hits += ok
        per_clause[d[0]["violates"]].append(ok)
        flat += [(sp, 1), (sn, 0)]
    return hits / len(PAIRS), auc(flat), per_clause


def embed(texts):
    out = []
    for t in texts:
        body = json.dumps({"model": EMB_MODEL, "input": t}).encode()
        req = urllib.request.Request(EMB_HOST + "/api/embed", data=body,
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=120) as f:
            out.append(json.load(f)["embeddings"][0])
    return out


def lopo_centroid(V):
    """Leave-one-pair-out nearest-centroid forced choice over unit vectors."""
    idx = {p["id"]: i for i, p in enumerate(PR)}
    hits, per_clause, flat = 0, defaultdict(list), []
    for held in PAIRS:
        cent = {}
        for lab in (0, 1):
            acc, k = None, 0
            for p in PR:
                if p["pair"] != held and p["label"] == lab:
                    v = V[idx[p["id"]]]
                    acc = list(v) if acc is None else [a + b for a, b in zip(acc, v)]
                    k += 1
            n = math.sqrt(sum(x * x for x in acc)) or 1.0
            cent[lab] = [x / n for x in acc]
        d = BY_PAIR[held]
        marg = {}
        for lab in (0, 1):
            v = V[idx[d[lab]["id"]]]
            marg[lab] = (sum(a * b for a, b in zip(v, cent[1]))
                         - sum(a * b for a, b in zip(v, cent[0])))
            flat.append((marg[lab], lab))
        ok = marg[1] > marg[0]
        hits += ok
        per_clause[d[0]["violates"]].append(ok)
    return hits / len(PAIRS), auc(flat), per_clause


def main():
    try:
        from wordfreq import zipf_frequency
    except ImportError:
        sys.exit("needs wordfreq: run with the scratch venv's python")

    rows = []

    rows.append(("LEN-WORDS", forced_choice(lambda p: len(p["text"].split()))))
    rows.append(("LEN-CHARS", forced_choice(lambda p: len(p["text"]))))

    def zipfs(p):
        z = [zipf_frequency(w, "en") for w in toks(p["text"])]
        return [x for x in z if x > 0]
    rows.append(("FREQ-MEAN", forced_choice(lambda p: sum(zipfs(p)) / max(1, len(zipfs(p))))))
    rows.append(("FREQ-MIN", forced_choice(lambda p: min(zipfs(p) or [0]))))
    rows.append(("FREQ-RARE-N", forced_choice(
        lambda p: sum(1 for x in zipfs(p) if x < 3.0) / max(1, len(zipfs(p))))))

    print("E20 FLOORS — what the n-gram sieve must beat")
    print("=" * 72)
    print(f"  {'scorer':14s} {'forced choice':>13s} {'AUC':>7s}   (chance 0.500)")
    for name, (fc, a, _) in rows:
        print(f"  {name:14s} {fc:13.3f} {a:7.3f}")

    try:
        V = embed([p["text"] for p in PR])
        V = [[x / (math.sqrt(sum(y * y for y in v)) or 1.0) for x in v] for v in V]
        fc, a, pc = lopo_centroid(V)
        rows.append((f"EMB-LOPO", (fc, a, pc)))
        print(f"  {'EMB-LOPO':14s} {fc:13.3f} {a:7.3f}   ({EMB_MODEL})")
    except Exception as exc:
        print(f"  EMB-LOPO       unavailable ({type(exc).__name__}: {exc})")

    print("\nPER-CLAUSE forced choice — which decoy type each control can already split")
    clauses = sorted({p["violates"] for p in PR if p["violates"]})
    n_of = Counter(p["violates"] for p in PR if p["violates"])
    print(f"  {'scorer':14s} " + "  ".join(f"{c}(n={n_of[c]})" for c in clauses))
    for name, (_, _, pc) in rows:
        cells = []
        for c in clauses:
            v = pc.get(c, [])
            cells.append(f"{sum(v)/len(v):.2f}" if v else "  - ")
        print(f"  {name:14s} " + "      ".join(cells))

    print("""
READ THIS BEFORE RUNNING E20
  The pre-registered prediction is that the sieve separates the C3
  (identity-destruction) pairs and NOT the C1 (common-cause) pairs, because C1
  is causal rather than lexical. If a control above ALREADY splits C3, then that
  predicted result is not evidence for the sieve -- it is the floor, and E20
  needs a target that beats the floor on the SAME clause.""")


if __name__ == "__main__":
    main()
