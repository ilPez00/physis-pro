#!/usr/bin/env python3
"""P2 analysis: identity, the trivial-overlap killer, and dimensionality.

Representations
---------------
* `nomic` -- nomic-embed-text via the local ollama HTTP API. This replaces the
  pilot's BGE, which **cannot be reproduced on this box**: there is no torch, no
  sentence-transformers and no BGE weight cache, and installing torch (2 GB+)
  onto a box whose swap is already exhausted was judged a worse trade than
  using the embedder the fleet already serves. This is a deviation from the
  brief's representation list and is recorded as one.
* `tfidf`, `tokjac`, `edit` -- lexical baselines. These are not filler: the
  central question of 2E is whether dense geometry sees anything the *literal
  text* does not already contain, so they are the control, not the backup.

Inference unit
--------------
The **file**, not the revision. A file with 20 revisions contributes one
trajectory observation, not 20 independent ones; treating revisions as
independent is the pseudo-replication error this program has already corrected
once (P1's bootstrap-unit fix). Where a per-revision statistic is needed
(e.g. adjacent-step cosine), per-file means are taken first and the file is the
unit of the cross-file summary.

REPRODUCTION: python3 research/papers/p2_analysis.py --task all
"""

from __future__ import annotations

import argparse
import glob
import json
import math
import os
import random
import shutil
import sys
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor

import numpy as np

def _np_load_chunk(path):
    """Load a cached embedding chunk. Returns a 2-D (n, d) array, or a (0, 0)
    array when the chunk is missing/empty/unreadable. It must never return a
    1-D array: callers np.concatenate these along axis 0, and a 1-D element
    raises "all the input arrays must have same number of dimensions".
    """
    try:
        if os.path.getsize(path) == 0:
            return np.empty((0, 0), dtype=np.float32)
        a = np.load(path)
        return a if a.ndim == 2 else np.empty((0, 0), dtype=np.float32)
    except (EOFError, ValueError, OSError):
        return np.empty((0, 0), dtype=np.float32)



REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(REPO, "research", "trajectories_external")
SHARDS = os.path.join(OUT, "shards")
EMB_CACHE = os.path.join(OUT, "embeddings_nomic.npz")
OLLAMA = "http://localhost:11434/api/embed"
EMB_MODEL = "nomic-embed-text:latest"
SEED = 20260924


# ---------------------------------------------------------------- corpus

def load_corpus(max_files_per_repo=None):
    """-> list of trajectories; each is a dict with ordered revision texts."""
    import gzip
    trajs = []
    shard_paths = (sorted(glob.glob(os.path.join(SHARDS, "*.jsonl.gz")))
                   or sorted(glob.glob(os.path.join(SHARDS, "*.jsonl"))))
    for shard in shard_paths:
        opener = gzip.open if shard.endswith(".gz") else open
        by_file = {}
        with opener(shard, "rt") as fh:
            for line in fh:
                r = json.loads(line)
                by_file.setdefault((r["repo"], r["path"]), []).append(r)
        for (repo, path), rows in by_file.items():
            rows.sort(key=lambda r: r["rev"])
            if len(rows) < 8:
                continue
            trajs.append({
                "repo": repo, "path": path, "lang": rows[0]["lang"],
                "texts": [r["text"] for r in rows],
                "ts": [r["ts"] for r in rows],
                "n_rev": len(rows),
                "key": f"{repo}::{path}",
            })
    if max_files_per_repo:
        keep, count = [], {}
        for t in trajs:
            c = count.get(t["repo"], 0)
            if c < max_files_per_repo:
                keep.append(t)
                count[t["repo"]] = c + 1
        trajs = keep
    return trajs


# ------------------------------------------------------- representations

def _chunk_dir():
    d = os.path.join(OUT, "emb_chunks_nomic")
    os.makedirs(d, exist_ok=True)
    return d


def embed_nomic(texts, batch=8, workers=1):
    """Embed via ollama, RESUMABLY.

    The first version accumulated everything in memory and wrote one npz at the
    end. The process was OOM-killed partway through 11k revisions and every
    embedding was lost, so the job could not be restarted without redoing all
    of it -- on a box where swap is already exhausted and kills are expected,
    not exceptional. Each completed batch is flushed to its own .npy and
    progress is recovered on the next run.
    """
    d = _chunk_dir()

    def one(b):
        req = urllib.request.Request(
            OLLAMA, data=json.dumps({"model": EMB_MODEL, "input": b}).encode(),
            headers={"Content-Type": "application/json"})
        for attempt in range(3):
            try:
                return json.load(urllib.request.urlopen(req, timeout=900))["embeddings"]
            except (urllib.error.URLError, TimeoutError, KeyError, OSError):
                if attempt == 2:
                    raise
        return []

    def embed_slice(sl):
        a = np.asarray(one(list(sl)), dtype=np.float32)
        a /= np.maximum(np.linalg.norm(a, axis=1, keepdims=True), 1e-12)
        return a

    # Repair before resuming. Chunk i covers texts[i*batch:(i+1)*batch] and the
    # final concatenation is POSITIONAL, so a zero-length or wrong-shaped chunk
    # must be REBUILT, not skipped: skipping it shifts every later chunk's rows
    # onto the wrong revision and yields a silently misaligned embedding matrix
    # -- wrong results rather than an error. An interrupted run leaves exactly
    # such files behind, which is what killed the previous two attempts.
    files = sorted(f for f in os.listdir(d) if f.endswith(".npy"))
    for i, f in enumerate(files):
        good = _np_load_chunk(os.path.join(d, f)).shape[0] > 0 or \
            (i + 1) * batch > len(texts) and os.path.getsize(os.path.join(d, f)) > 0
        if good:
            continue
        lo, hi = i * batch, min((i + 1) * batch, len(texts))
        if lo >= len(texts):
            os.remove(os.path.join(d, f))
            continue
        print(f"  repairing corrupt chunk {f} (rows {lo}:{hi})", flush=True)
        np.save(os.path.join(d, f), embed_slice(texts[lo:hi]))

    files = sorted(f for f in os.listdir(d) if f.endswith(".npy"))
    n_done = sum(len(_np_load_chunk(os.path.join(d, f))) for f in files)
    # Validate against len(texts), NOT against files*batch: this cache has been
    # written by runs using different batch sizes, so the row total is the only
    # invariant that holds. Too few rows is normal (resume); too many means the
    # positional alignment is suspect and the cache must be rebuilt, because a
    # longer-than-expected cache can pair vectors with the wrong revisions.
    if n_done > len(texts):
        raise SystemExit(
            f"chunk cache holds {n_done} rows for {len(texts)} revisions -- "
            f"alignment cannot be trusted. Rebuild: "
            f"rm -rf {d} or re-run with --force-embed.")
    n_done = min(n_done, len(texts))
    pending = texts[n_done:]
    if n_done:
        print(f"  resuming: {n_done}/{len(texts)} already embedded", flush=True)

    batches = [pending[k:k + batch] for k in range(0, len(pending), batch)]

    n0 = len(files)
    for k, b in enumerate(batches):
        try:
            arr = embed_slice(b)
        except Exception as e:            # noqa: BLE001
            # Exit CLEANLY with what we have instead of raising. Every earlier
            # chunk is already on disk, so a rerun resumes instead of restarting;
            # the first version raised and a single slow request ended the run.
            print(f"  stopping at chunk {n0+k}: {type(e).__name__}: {str(e)[:120]}\n"
                  f"  {n0+k} chunks of {len(batches)+n0} are on disk; "
                  f"re-run to resume", flush=True)
            break
        np.save(os.path.join(d, f"chunk_{n0 + k:06d}.npy"), arr)
        if (k + 1) % 20 == 0:
            print(f"    {n_done + (k + 1) * batch}/{len(texts)}", flush=True)

    files = sorted(f for f in os.listdir(d) if f.endswith(".npy"))
    parts = [_np_load_chunk(os.path.join(d, f)) for f in files]
    parts = [a for a in parts if a.shape[0] > 0]
    A = np.concatenate(parts, axis=0)
    if A.shape[0] < len(texts):
        raise SystemExit(
            f"only {A.shape[0]}/{len(texts)} revisions embedded; re-run to resume")
    return A[:len(texts)]


def tok_jaccard(a, b):
    A, B = set(a.split()), set(b.split())
    if not A and not B:
        return 1.0
    return len(A & B) / max(1, len(A | B))


def norm_edit(a, b):
    """difflib-based normalised similarity in [0,1]; 1 = identical."""
    import difflib
    if a == b:
        return 1.0
    return difflib.SequenceMatcher(None, a, b, autojunk=False).ratio()


def tfidf_matrix(texts):
    from sklearn.feature_extraction.text import TfidfVectorizer
    v = TfidfVectorizer(max_features=20000, sublinear_tf=True)
    X = v.fit_transform(texts).toarray()
    n = np.linalg.norm(X, axis=1, keepdims=True)
    n[n == 0] = 1.0
    return X / n


# ------------------------------------------------------------- helpers

def traj_sim_matrix(vecs):
    """Pairwise cosine for a trajectory's revisions (rows already normalised)."""
    return vecs @ vecs.T


def adjacent_metrics(vecs):
    """Per-trajectory directional statistics from consecutive revisions."""
    if len(vecs) < 3:
        return None
    steps = vecs[1:] - vecs[:-1]
    step_len = np.linalg.norm(steps, axis=1)
    cos_adj = (vecs[1:] * vecs[:-1]).sum(1)
    # path length vs net displacement: 1 = straight drift, >>1 = wandering
    path = float(step_len.sum())
    disp = float(np.linalg.norm(vecs[-1] - vecs[0]))
    ratio = path / disp if disp > 1e-9 else float("inf")
    # curvature: mean turning angle between consecutive steps
    cos_turn = []
    for i in range(len(steps) - 1):
        a, b = steps[i], steps[i + 1]
        na, nb = np.linalg.norm(a), np.linalg.norm(b)
        if na > 1e-9 and nb > 1e-9:
            cos_turn.append(float((a @ b) / (na * nb)))
    return {
        "d_first_last": float(np.linalg.norm(vecs[-1] - vecs[0])),
        "adj_cos_mean": float(cos_adj.mean()),
        "path_over_disp": ratio,
        "turn_cos_mean": float(np.mean(cos_turn)) if cos_turn else None,
        "return_dist": float(np.linalg.norm(vecs[-1] - vecs[0])),
    }


def effective_metrics(vecs):
    """PCA spectrum -> participation ratio and effective rank."""
    if len(vecs) < 3:
        return None
    X = vecs - vecs.mean(0, keepdims=True)
    s = np.linalg.svd(X, compute_uv=False)
    ev = s ** 2
    tot = ev.sum()
    if tot <= 0:
        return None
    p = ev / tot
    return {
        "pc1": float(p[0]),
        "pc12": float(p[:2].sum()),
        "participation_ratio": float((ev.sum() ** 2) / (ev ** 2).sum()),
        "effective_rank": float(math.exp(-(p[p > 0] * np.log(p[p > 0])).sum())),
        "n_rev": int(len(vecs)),
    }


def wilson(k, n):
    if n == 0:
        return (0.0, 0.0)
    z = 1.96
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (c - h, c + h)


def summarize(vals):
    a = np.asarray([v for v in vals if v is not None], dtype=float)
    if not len(a):
        return None
    return {"n": int(len(a)), "median": float(np.median(a)),
            "mean": float(a.mean()), "p25": float(np.percentile(a, 25)),
            "p75": float(np.percentile(a, 75))}


# ------------------------------------------------------------- tasks

def ensure_embeddings(trajs, force=False):
    """Embed all revisions, resumably (see embed_nomic)."""
    if force:
        shutil.rmtree(_chunk_dir(), ignore_errors=True)
    flat, owner = [], []
    for i, t in enumerate(trajs):
        for txt in t["texts"]:
            flat.append(txt)
            owner.append(i)
    print(f"  embedding {len(flat)} revisions via {EMB_MODEL} ...", flush=True)
    E = embed_nomic(flat)
    embs = []
    for i, t in enumerate(trajs):
        embs.append(E[[j for j, o in enumerate(owner) if o == i]])
    return embs


def task_identity(trajs, embs, rng):
    """2C: are revisions of the SAME file closer than matched cross-file pairs?

    Matched negatives control for language and revision count, and we compare
    against lexical baselines: if literal text overlap already separates
    same-file from different-file pairs, dense geometry has to beat that to
    count for anything.
    """
    by_lang = {}
    for i, t in enumerate(trajs):
        by_lang.setdefault(t["lang"], []).append(i)

    rows = []
    for i, t in enumerate(trajs):
        pool = [j for j in by_lang[t["lang"]] if j != i and abs(trajs[j]["n_rev"] - t["n_rev"]) <= 3]
        if len(pool) < 3:
            continue
        j = rng.choice(pool)
        u = trajs[j]
        k = min(len(t["texts"]), len(u["texts"]))
        # same-file pair (non-adjacent revisions) vs cross-file pair
        a_i, b_i = 0, k - 1
        A, B = trajs[i]["texts"], u["texts"]
        same = {
            "nomic": float(embs[i][a_i] @ embs[i][b_i]),
            "tfidf": None, "tokjac": tok_jaccard(A[a_i], A[b_i]),
        }
        cross = {
            "nomic": float(embs[i][a_i] @ embs[j][b_i]),
            "tokjac": tok_jaccard(A[a_i], B[b_i]),
        }
        same["edit"] = norm_edit(A[a_i], A[b_i])
        cross["edit"] = norm_edit(A[a_i], B[b_i])
        rows.append({"same_nomic": same["nomic"], "cross_nomic": cross["nomic"],
                     "d_nomic": same["nomic"] - cross["nomic"],
                     "same_tokjac": same["tokjac"], "cross_tokjac": cross["tokjac"],
                     "d_tokjac": same["tokjac"] - cross["tokjac"],
                     "same_edit": same["edit"], "cross_edit": cross["edit"],
                     "d_edit": same["edit"] - cross["edit"]})

    out = {"n_files": len(rows)}
    for k in ("nomic", "tokjac", "edit"):
        d = [r[f"d_{k}"] for r in rows]
        k_pos = sum(1 for x in d if x > 0)
        out[k] = {
            "same_minus_cross": summarize(d),
            "frac_same_closer": k_pos / len(d) if d else None,
            "ci95": wilson(k_pos, len(d)),
        }
    out["verdict"] = ("identity holds if frac_same_closer is near 1 for the "
                      "representation; the LEXICAL baselines are the bar dense "
                      "geometry must clear")
    return out, rows


def task_overlap(trajs, embs):
    """2E: does geometry see temporal structure beyond literal text overlap?

    For each trajectory we regress a geometry-based temporal statistic on a
    lexical-similarity statistic. If the geometry statistic is explained by
    lexical overlap (high correlation), the semantic claim is not supported:
    the trajectory is the text, and the text is 90% identical between adjacent
    revisions.
    """
    rows = []
    for i, t in enumerate(trajs):
        T = t["texts"]
        if len(T) < 8:
            continue
        V = embs[i]
        adj_geo = float((V[1:] * V[:-1]).sum(1).mean())
        adj_lex = float(np.mean([tok_jaccard(T[k], T[k + 1])
                                 for k in range(len(T) - 1)]))
        # distance-from-first vs time, in both spaces
        def trend(vecs):
            if len(vecs) < 3:
                return None
            d = np.linalg.norm(vecs - vecs[0], axis=1)
            t_ = np.arange(len(d), dtype=float)
            if d.std() < 1e-12:
                return 0.0
            return float(np.corrcoef(t_, d)[0, 1])
        # lexical "vector" for the same computation: token jaccard to first
        lex_first = np.array([tok_jaccard(T[k], T[0]) for k in range(len(T))])
        t_ = np.arange(len(T), dtype=float)
        lex_trend = (float(np.corrcoef(t_, lex_first)[0, 1])
                     if lex_first.std() > 1e-12 else 0.0)
        rows.append({"geo_adj": adj_geo, "lex_adj": adj_lex,
                     "geo_trend": trend(V), "lex_trend": lex_trend,
                     "n_rev": len(T)})

    def corr(a, b):
        A = np.asarray([r[a] for r in rows if r[a] is not None and r[b] is not None])
        B = np.asarray([r[b] for r in rows if r[a] is not None and r[b] is not None])
        if len(A) < 5 or A.std() < 1e-12 or B.std() < 1e-12:
            return None
        return float(np.corrcoef(A, B)[0, 1])

    # partial correlation of geo_trend and n_rev controlling for lex_trend
    def partial(x, y, z):
        X = np.asarray([r[x] for r in rows if all(r[k] is not None for k in (x, y, z))])
        Y = np.asarray([r[y] for r in rows if all(r[k] is not None for k in (x, y, z))])
        Z = np.asarray([r[z] for r in rows if all(r[k] is not None for k in (x, y, z))])
        if len(X) < 5:
            return None
        rx = X - np.polyval(np.polyfit(Z, X, 1), Z)
        ry = Y - np.polyval(np.polyfit(Z, Y, 1), Z)
        if rx.std() < 1e-12 or ry.std() < 1e-12:
            return None
        return float(np.corrcoef(rx, ry)[0, 1])

    out = {
        "n_files": len(rows),
        "corr_geo_adj_vs_lex_adj": corr("geo_adj", "lex_adj"),
        "corr_geo_trend_vs_lex_trend": corr("geo_trend", "lex_trend"),
        "corr_geo_adj_vs_nrev": corr("geo_adj", "n_rev"),
        "partial_geo_adj_nrev_given_lexadj":
            partial("geo_adj", "n_rev", "lex_adj"),
        "median_geo_adj": summarize([r["geo_adj"] for r in rows]),
        "median_lex_adj": summarize([r["lex_adj"] for r in rows]),
        "verdict": ("if geometry's temporal statistics correlate near-perfectly "
                    "with lexical overlap, the semantic claim adds nothing the "
                    "text does not already contain"),
    }
    return out, rows


def task_dim(trajs, embs, rng, n_perm=200):
    """2F: intrinsic dimensionality vs a shuffled-revision null.

    'PC1+PC2 explain 57-82%' is not a dimensionality measurement -- a random
    walk in 768 dimensions also concentrates variance. The null is the point:
    permute the revision order (destroying temporal structure, preserving the
    point cloud) and recompute.
    """
    rows = []
    for i, t in enumerate(trajs):
        V = embs[i]
        if len(V) < 8:
            continue
        real = effective_metrics(V)
        if not real:
            continue
        nulls = []
        for _ in range(n_perm):
            idx = rng.permutation(len(V))
            m = effective_metrics(V[idx])
            if m:
                nulls.append(m)
        rows.append({
            "repo": t["repo"], "real_pc12": real["pc12"],
            "real_pr": real["participation_ratio"],
            "real_er": real["effective_rank"],
            "null_pc12": float(np.mean([m["pc12"] for m in nulls])) if nulls else None,
            "null_pr": float(np.mean([m["participation_ratio"] for m in nulls])) if nulls else None,
            "null_er": float(np.mean([m["effective_rank"] for m in nulls])) if nulls else None,
            "n_rev": real["n_rev"],
        })
    out = {"n_files": len(rows), "n_perm": n_perm}
    for k in ("pc12", "pr", "er"):
        real = summarize([r[f"real_{k}"] for r in rows])
        null = summarize([r[f"null_{k}"] for r in rows])
        out[f"real_{k}"] = real
        out[f"null_{k}"] = null
        out[f"is_{k}_unusual"] = bool(
            real and null and real["median"] > null["median"] * 1.3)
    out["verdict"] = ("low intrinsic dimension is supported only if the real "
                      "spectrum is far more concentrated than the "
                      "order-shuffled null on the SAME point cloud; PC1+PC2 "
                      "explained variance alone cannot support it")
    return out, rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--task", default="all",
                    choices=["all", "identity", "overlap", "dim"])
    ap.add_argument("--max-files-per-repo", type=int, default=60)
    ap.add_argument("--n-perm", type=int, default=200)
    ap.add_argument("--no-embed", action="store_true",
                    help="skip the nomic arm (lexical baselines only)")
    ap.add_argument("--force-embed", action="store_true")
    a = ap.parse_args()

    rng = random.Random(SEED)
    trajs = load_corpus(a.max_files_per_repo)
    if not trajs:
        print("no corpus: run p2_build_corpus.py first"); return 1
    print(f"trajectories: {len(trajs)}  "
          f"revisions: {sum(t['n_rev'] for t in trajs)}  "
          f"repos: {len(set(t['repo'] for t in trajs))}  "
          f"langs: {sorted(set(t['lang'] for t in trajs))}")

    embs = None
    if not a.no_embed:
        embs = ensure_embeddings(trajs, force=a.force_embed)

    results = {"n_trajectories": len(trajs),
               "n_revisions": sum(t["n_rev"] for t in trajs),
               "representation_note": (
                   "nomic via ollama (BGE unavailable: no torch / "
                   "sentence-transformers / weight cache on this box)"),
               "inference_unit": "file"}
    if a.task in ("all", "identity") and embs is not None:
        r, rows = task_identity(trajs, embs, rng)
        results["identity"] = r
        with open(os.path.join(OUT, "identity_rows.jsonl"), "w") as fh:
            for x in rows:
                fh.write(json.dumps(x) + "\n")
    if a.task in ("all", "overlap") and embs is not None:
        r, rows = task_overlap(trajs, embs)
        results["overlap"] = r
        with open(os.path.join(OUT, "overlap_rows.jsonl"), "w") as fh:
            for x in rows:
                fh.write(json.dumps(x) + "\n")
    if a.task in ("all", "dim") and embs is not None:
        r, rows = task_dim(trajs, embs, rng, a.n_perm)
        results["dimensionality"] = r
        with open(os.path.join(OUT, "dim_rows.jsonl"), "w") as fh:
            for x in rows:
                fh.write(json.dumps(x) + "\n")

    path = os.path.join(OUT, "p2_results.json")
    with open(path, "w") as fh:
        json.dump(results, fh, indent=2)
    print(json.dumps({k: v for k, v in results.items()
                      if isinstance(v, dict)}, indent=2)[:3000])
    print(f"\nwrote {os.path.relpath(path, REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
