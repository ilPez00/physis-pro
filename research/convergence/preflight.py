#!/usr/bin/env python3
"""E19 pre-flight — is the probe set solvable without reading it?

House rule 4: run the pre-flight and honour it. House rule 1: treat any positive
as artifact until a construction-matched control says otherwise. Both apply to
the *instrument* before they apply to the result, so this runs first.

Three cheap scorers that know nothing about the target structure T. If any of
them separates the probe set, the probe set is broken and no LLM result measured
on it means anything.

  LEN-WORDS / LEN-CHARS   the E18 confound: entry length alone scored AUC 0.619
                          on real misfilings and buried a whole hypothesis.
  NOT CHECKED HERE, AND IT SHOULD BE: a dense sentence embedding reaches 0.917
  leave-one-pair-out forced choice on this same probe set (E20_FLOORS.md), against
  the 0.750 tf-idf figure declared below. tf-idf under-reports how separable these
  probes are. Add an embedding row before authoring another probe set.
  TFIDF-CENTROID          leave-one-out nearest class centroid over tf-idf.
                          Detects a vocabulary tell -- if the positives all say
                          "species" and the negatives all say "single", the task
                          is lexical and not structural.
"""
import json, math, os, re, sys
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
PR = json.load(open(os.path.join(HERE, "probes.json")))["probes"]


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


def toks(s):
    return re.findall(r"[a-z][a-z\-']{2,}", s.lower())


def tfidf_lopo():
    """Leave-one-PAIR-out nearest-centroid on tf-idf. Returns (forced-choice acc, auc).

    Plain leave-one-out is invalid under the matched-pair design: the held-out
    probe's nearest neighbour is its own pair partner, which sits in the
    opposite class, so LOO reports near-perfect INVERTED separation (0.083) and
    measures the pairing rather than any vocabulary tell. Holding out both
    members of a pair removes that leak.

    Reported as a forced choice within the held-out pair -- which of the two is
    the positive -- because that is the metric the arms are scored on.
    """
    docs = [Counter(toks(p["text"])) for p in PR]
    y = [p["label"] for p in PR]
    df = Counter()
    for d in docs:
        df.update(d.keys())
    N = len(docs)
    idf = {w: math.log(N / (1 + df[w])) for w in df}

    def vec(d):
        v = {w: (1 + math.log(c)) * idf[w] for w, c in d.items() if w in idf}
        n = math.sqrt(sum(x * x for x in v.values())) or 1.0
        return {w: x / n for w, x in v.items()}

    V = [vec(d) for d in docs]
    pairs = sorted({p["pair"] for p in PR})
    scores, correct = [], 0
    for held in pairs:
        cent = {}
        for lab in (0, 1):
            acc, k = Counter(), 0
            for j in range(N):
                if PR[j]["pair"] != held and y[j] == lab:
                    k += 1
                    for w, x in V[j].items():
                        acc[w] += x
            n = math.sqrt(sum((x / k) ** 2 for x in acc.values())) or 1.0
            cent[lab] = {w: (x / k) / n for w, x in acc.items()}
        idx = [j for j in range(N) if PR[j]["pair"] == held]
        marg = {}
        for j in idx:
            s = {lab: sum(V[j].get(w, 0.0) * x for w, x in cent[lab].items()) for lab in (0, 1)}
            marg[j] = s[1] - s[0]
            scores.append((marg[j], y[j]))
        pick = max(idx, key=lambda j: marg[j])       # forced choice within the pair
        correct += int(y[pick] == 1)
    return correct / len(pairs), auc(scores)


if __name__ == "__main__":
    print("E19 PRE-FLIGHT — probe-set integrity\n" + "=" * 46)
    rows = [
        ("LEN-WORDS", auc([(len(p["text"].split()), p["label"]) for p in PR])),
        ("LEN-CHARS", auc([(len(p["text"]), p["label"]) for p in PR])),
    ]
    acc, a = tfidf_lopo()
    rows.append(("TFIDF-LOPO", a))
    for name, a_ in rows:
        flag = "OK" if abs(a_ - 0.5) <= 0.20 else "*** LEAK ***"
        print(f"  {name:16s} AUC {a_:.3f}   {flag}")
    print(f"  {'TFIDF-LOPO':16s} forced-choice acc {acc:.3f}  (chance 0.500)")
    print()
    n_pos = sum(p["label"] for p in PR)
    print(f"  probes {len(PR)}  positives {n_pos}  negatives {len(PR)-n_pos}")
    viol = Counter(p["violates"] for p in PR if p["violates"])
    print(f"  negative construction: {dict(viol)}")
    worst = max(abs(a_ - 0.5) for _, a_ in rows)
    print("\n  VERDICT:", "GO" if worst <= 0.20 else "STOP — rewrite the probes")
    sys.exit(0 if worst <= 0.20 else 1)
