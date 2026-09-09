#!/usr/bin/env python3
"""E23 — can activity text carry an activity label? The premise behind machine supervision.

WHY THIS EXISTS
  E21's positive rests on 436 human decisions, and its honest limit was that
  supervision has to come from somewhere. The proposal under test is that it need
  not come from a person: compilers, exit codes, test outcomes and commit
  metadata label things continuously and for free.

  Before building ingestion for any of that, one premise has to hold. The axis
  that needs help is MODE — what is being DONE — and E22 found the corpus's
  subject-matter hints actively degrade it (t = -5.39). But E22 also found that
  entries whose NAMES name an activity are no better predicted (-0.009, null),
  so "mode needs activity words" is not supported.

  This tests the premise directly on data where the label IS an activity and the
  text IS about that activity: 562 conventional-commit subjects from this repo.
  If activity text cannot predict an activity label here, machine supervision
  will not rescue the mode axis either, and the direction should be dropped
  cheaply rather than built.

CIRCULARITY, HANDLED
  The type prefix IS the label, so it is stripped before embedding. A lexical
  leak remains possible ("fix a crash" contains "fix"), so a TF-IDF centroid runs
  alongside the embedding: if TF-IDF matches the embedder, the task is lexical
  and says nothing about activity semantics.
"""
import json, math, os, random, re, statistics, subprocess, sys, urllib.request
from collections import Counter, defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
HOST = os.environ.get("E23_EMB_HOST", "http://100.116.39.57:11434")
MODEL = os.environ.get("E23_EMB_MODEL", "nomic-embed-text:latest")
CACHE = os.path.join(HERE, "e23_commit_vecs.json")
MIN_N = 14


def commits():
    out = subprocess.run(["git", "log", "--pretty=%s"], cwd=os.path.join(HERE, "..", ".."),
                         capture_output=True, text=True).stdout.splitlines()
    rows = []
    for s in out:
        m = re.match(r"^([a-z]{3,12})(\([^)]*\))?:\s*(.+)$", s)
        if not m:
            continue
        typ, _, text = m.groups()
        if len(text) < 8:
            continue
        rows.append(dict(typ=typ, text=text))
    keep = {t for t, n in Counter(r["typ"] for r in rows).items() if n >= MIN_N}
    return [r for r in rows if r["typ"] in keep]


def embed(t):
    body = json.dumps({"model": MODEL, "input": t}).encode()
    req = urllib.request.Request(HOST + "/api/embed", data=body,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as f:
        return json.load(f)["embeddings"][0]


def norm(v):
    n = sum(x * x for x in v) ** 0.5 or 1.0
    return [x / n for x in v]


def centroid_eval(rows, vecs, seeds=20, frac=0.75, permute=False):
    accs, top2 = [], []
    for s in range(seeds):
        rng = random.Random(3000 + s)
        idx = list(range(len(rows)))
        rng.shuffle(idx)
        n = int(frac * len(rows))
        tr = [rows[i] for i in idx[:n]]
        te = [rows[i] for i in idx[n:]]
        if permute:
            labs = [r["typ"] for r in tr]
            rng.shuffle(labs)
            tr = [dict(r, typ=l) for r, l in zip(tr, labs)]
        acc = defaultdict(lambda: None)
        for r in tr:
            v = vecs[r["text"]]
            acc[r["typ"]] = list(v) if acc[r["typ"]] is None else \
                [a + b for a, b in zip(acc[r["typ"]], v)]
        cent = {c: norm(v) for c, v in acc.items()}
        h1 = h2 = 0
        for r in te:
            v = vecs[r["text"]]
            order = sorted(cent, key=lambda c: -sum(a * b for a, b in zip(v, cent[c])))
            h1 += order[0] == r["typ"]
            h2 += r["typ"] in order[:2]
        accs.append(h1 / len(te)); top2.append(h2 / len(te))
    return statistics.mean(accs), statistics.stdev(accs), statistics.mean(top2)


def tfidf_vecs(rows):
    docs = {r["text"]: Counter(re.findall(r"[a-z][a-z\-']{2,}", r["text"].lower())) for r in rows}
    df = Counter()
    for d in docs.values():
        df.update(d.keys())
    N = len(docs)
    idf = {w: math.log(N / (1 + df[w])) for w in df}
    vocab = sorted(idf)
    pos = {w: i for i, w in enumerate(vocab)}
    out = {}
    for t, d in docs.items():
        v = [0.0] * len(vocab)
        for w, c in d.items():
            v[pos[w]] = (1 + math.log(c)) * idf[w]
        out[t] = norm(v)
    return out


def main():
    rows = commits()
    types = Counter(r["typ"] for r in rows)
    print(f"E23 — activity text -> activity label.  {len(rows)} commits, "
          f"{len(types)} types (n >= {MIN_N})")
    print("  " + "  ".join(f"{t}:{n}" for t, n in types.most_common()))
    maj = types.most_common(1)[0][1] / len(rows)
    print(f"  majority floor {maj:.3f}")

    cache = json.load(open(CACHE)) if os.path.exists(CACHE) else {}
    todo = [r["text"] for r in rows if r["text"] not in cache]
    for i, t in enumerate(todo):
        cache[t] = embed(t)
        if i % 100 == 0:
            print(f"  embedding {i}/{len(todo)}", file=sys.stderr, flush=True)
            json.dump(cache, open(CACHE, "w"))
    json.dump(cache, open(CACHE, "w"))
    V = {t: norm(v) for t, v in cache.items()}

    m, sd, t2 = centroid_eval(rows, V)
    pm, psd, pt2 = centroid_eval(rows, V, permute=True)
    T = tfidf_vecs(rows)
    tm, tsd, tt2 = centroid_eval(rows, T)

    print(f"\n  {'scorer':22s} {'top-1':>16s} {'top-2':>8s}")
    print(f"  {'EMBEDDING ('+MODEL.split(':')[0]+')':22s} {m:.3f} +/- {sd:.3f}   {t2:8.3f}")
    print(f"  {'TF-IDF (lexical)':22s} {tm:.3f} +/- {tsd:.3f}   {tt2:8.3f}")
    print(f"  {'PERMUTED (null)':22s} {pm:.3f} +/- {psd:.3f}   {pt2:8.3f}")
    print(f"  {'MAJORITY':22s} {maj:.3f}")
    print(f"""
READING
  If EMBEDDING clears PERMUTED and MAJORITY, activity text does carry an
  activity label, and a trace of what a system DID can supervise a mode-like
  axis. If TF-IDF matches EMBEDDING, the signal is lexical convention rather
  than activity semantics, and the result does not transfer to entries that
  carry no such convention.""")


if __name__ == "__main__":
    main()
