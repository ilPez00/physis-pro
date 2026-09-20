#!/usr/bin/env python3
"""Retrieval benchmark over this repository — the one that can fail.

physis-core's benchmarks/ground-truth scores IDENTICALLY under a lexical hash
and a real semantic model (repeat 100%, anomaly 100%, contradiction 50%,
compression ~24% either way). Its repeat families are lexically near-identical,
so string overlap recovers them perfectly. A benchmark whose score does not move
when retrieval quality moves is not measuring retrieval quality.

This harness fixes that by asking questions in a developer's words about code
written in a compiler's words, then checking whether the retriever surfaces the
file that actually answers them. Ground truth is in cases.json, established by
reading the code.

Usage:
    python3 benchmarks/retrieval/run.py [--budget N] [--top-k K] [--arms rp,semantic]

Writes benchmarks/retrieval/results.json and prints a per-arm table.
"""
import argparse, json, os, pathlib, subprocess, sys, time

ROOT = pathlib.Path(__file__).resolve().parents[2]
BIN = ROOT / "target" / "release" / "physis-pro"
CASES = pathlib.Path(__file__).with_name("cases.json")
OUT = pathlib.Path(__file__).with_name("results.json")

# Both arms must exist for the benchmark to mean anything: the whole point is
# the DIFFERENCE between them. An arm that cannot run is reported, not skipped
# silently.
ARMS = {
    "rp": {"PHYSIS_EMBEDDER": "random-projection"},
    "semantic": {"PHYSIS_EMBEDDER": ""},
    # Q6 BM25 legs (2026-09-18 rerun): same 7 cases through the `find` CLI
    # (section-level ContentIndex) instead of `pack` (chunk-level). The hit
    # lines are `<score>  <basename>`; snippet prose is never scored, so a
    # code token quoted inside a snippet cannot count as a retrieval hit.
    "lexical": {"PHYSIS_EMBEDDER": "random-projection", "FIND_MODE": "lexical"},
    "hybrid": {"PHYSIS_EMBEDDER": "", "FIND_MODE": "hybrid"},
    # E1 (docs/plans/2026-09-19-small-encoders.md): the distilled encoder, so
    # its hit@5 can be read beside bge-base's. A faster encoder that retrieves
    # worse is not an improvement, which is why this arm exists before the swap
    # is believed. Run: python3 benchmarks/retrieval/run.py --arms semantic,bge-small
    "bge-small": {"PHYSIS_EMBEDDER": "bge-small"},
}


def pack(query, root, budget, top_k, arm_env, data_dir):
    env = dict(os.environ, PHYSIS_ALLOW_DEV_LICENCE="1", PHYSIS_DATA_DIR=str(data_dir),
               PHYSIS_SCAN_ROOT=str(ROOT), **arm_env)
    env = {k: v for k, v in env.items() if v != ""}
    t0 = time.time()
    p = subprocess.run([str(BIN), "pack", str(root), query,
                        "--budget", str(budget), "--top-k", str(top_k)],
                       cwd=ROOT, env=env, capture_output=True, text=True, timeout=1800)
    elapsed = time.time() - t0
    if p.returncode != 0:
        return None, elapsed, p.stderr.strip().splitlines()[-1:] or ["nonzero exit"]
    # `pack` prints a human table; the paths are what we score.
    paths = []
    for line in p.stdout.splitlines():
        for tok in line.replace(":", " ").split():
            if tok.endswith((".rs", ".md", ".toml", ".py", ".sh", ".json")):
                paths.append(tok)
    return paths, elapsed, []


def find(query, root, top_k, arm_env, data_dir):
    """Run the `find` CLI (section-level) and return ranked basenames.

    Only header lines `<score>  <name>` are scored. Snippet prose below
    each header is ignored — the 2026-09-17 sweep scored find output with
    the pack table parser and saw no paths (harness bug, arms VOID).
    """
    import re
    env = dict(os.environ, PHYSIS_ALLOW_DEV_LICENCE="1", PHYSIS_DATA_DIR=str(data_dir),
               PHYSIS_SCAN_ROOT=str(ROOT), **arm_env)
    mode = env.pop("FIND_MODE", "hybrid")
    env = {k: v for k, v in env.items() if v != ""}
    t0 = time.time()
    p = subprocess.run([str(BIN), "find", query, "--dir", str(root),
                        "--mode", mode, "--top-k", str(top_k)],
                       cwd=ROOT, env=env, capture_output=True, text=True, timeout=1800)
    elapsed = time.time() - t0
    if p.returncode != 0:
        return None, elapsed, p.stderr.strip().splitlines()[-1:] or ["nonzero exit"]
    header = re.compile(r"^\d+\.\d+\s+(\S+)\s*$")
    paths = [m.group(1) for line in p.stdout.splitlines()
             if (m := header.match(line.strip()))]
    return paths, elapsed, []


def score(paths, expect, k):
    """hit@k: did any expected file appear in the first k retrieved paths?

    Deliberately generous — a single hit anywhere in the top k counts. If even
    this cannot separate a semantic model from a string hash, nothing will.
    """
    seen, ordered = set(), []
    for p in paths:
        if p not in seen:
            seen.add(p)
            ordered.append(p)
    top = ordered[:k]
    want = [e.split("/")[-1] for e in expect]
    hit = any(any(t.endswith(w) for w in want) for t in top)
    rank = next((i + 1 for i, t in enumerate(ordered) if any(t.endswith(w) for w in want)), None)
    return hit, rank, top


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--budget", type=int, default=3000)
    ap.add_argument("--top-k", type=int, default=30)
    ap.add_argument("--at", type=int, default=5, help="k for hit@k")
    ap.add_argument("--arms", default="rp,semantic")
    ap.add_argument("--root", default="physis-core/src")
    ap.add_argument("--data-dir", default=".physis-bench")
    args = ap.parse_args()

    if not BIN.exists():
        sys.exit(f"missing {BIN}; build it:\n  cargo build --release --features mcp,cli --bin physis-pro")
    cases = json.loads(CASES.read_text())["cases"]
    data_dir = ROOT / args.data_dir
    data_dir.mkdir(exist_ok=True)
    lic = data_dir / "license.key"
    if not lic.exists():
        sys.exit(f"no dev licence at {lic}\n  cd physis-dev-license && cargo run -q > {lic}")

    # A case whose expected file does not live under --root is unanswerable
    # there. Scoring it as a miss would blame the retriever for a corpus that
    # never contained the answer, so it is skipped and counted as skipped.
    rootrel = args.root.rstrip("/") + "/"
    scoped, skipped = [], []
    for c in cases:
        (scoped if all(e.startswith(rootrel) for e in c["expect"]) else skipped).append(c)
    if skipped:
        print(f"skipped (answer not under {args.root}): "
              + ", ".join(c["id"] for c in skipped))
    cases = scoped

    results = {"budget": args.budget, "top_k": args.top_k, "at": args.at,
               "root": args.root, "skipped": [c["id"] for c in skipped], "arms": {}}
    for arm in args.arms.split(","):
        arm = arm.strip()
        if arm not in ARMS:
            sys.exit(f"unknown arm {arm!r}; known: {', '.join(ARMS)}")
        hits, rows, total_s = 0, [], 0.0
        print(f"\n=== arm: {arm} ===")
        for c in cases:
            if "FIND_MODE" in ARMS[arm]:
                paths, el, err = find(c["query"], args.root, args.top_k,
                                      ARMS[arm], data_dir)
            else:
                paths, el, err = pack(c["query"], args.root, args.budget, args.top_k,
                                      ARMS[arm], data_dir)
            total_s += el
            if paths is None:
                rows.append({"id": c["id"], "hit": None, "rank": None, "error": err})
                print(f"  {c['id']:<26} ERROR {err}")
                continue
            hit, rank, top = score(paths, c["expect"], args.at)
            hits += bool(hit)
            rows.append({"id": c["id"], "hit": hit, "rank": rank,
                         "expect": c["expect"], "top": top, "seconds": round(el, 2)})
            print(f"  {c['id']:<26} {'HIT ' if hit else 'miss'} rank={rank} "
                  f"top1={top[0] if top else '-'} ({el:.1f}s)")
        n = len(cases)
        results["arms"][arm] = {"hit_at_k": hits / n if n else 0.0, "hits": hits,
                                "cases": n, "seconds": round(total_s, 1), "rows": rows}
        print(f"  -> hit@{args.at} = {hits}/{n} = {hits / n:.2f}   ({total_s:.0f}s)")

    OUT.write_text(json.dumps(results, indent=2))
    print(f"\nwrote {OUT.relative_to(ROOT)}")
    got = {a: r["hit_at_k"] for a, r in results["arms"].items()}
    if "rp" in got and "semantic" in got:
        d = got["semantic"] - got["rp"]
        print(f"DISCRIMINATION: semantic - rp = {d:+.2f}")
        if d <= 0:
            print("  This benchmark does NOT discriminate. Do not quote its numbers "
                  "as evidence of retrieval quality until it does.")


if __name__ == "__main__":
    main()
