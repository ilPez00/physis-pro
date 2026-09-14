#!/usr/bin/env python3
"""E75 — is history storage, or does it constrain the future?

Phase 8/9/11/14 of the 360 investigation, in its smallest falsifiable form.

A `fitness` field existing proves nothing. What has to be shown is that the
SAME final evidence, arriving in a different ORDER — or arriving and then being
erased — produces a DIFFERENT state now. If the engine's state is a function of
the evidence multiset alone, then history is storage plus a scoring function,
and every stronger word (path dependence, learning, preference) is unsupported.

Arms, each in its own isolated store (PHYSIS_CORE_DIR):

  ordered      evidence in a fixed order
  shuffled-k   the same evidence, permuted (several seeds)
  erased       the same evidence added, then the hypothesis re-created fresh
  interleaved  supporting and contradicting alternating rather than blocked

Measured: composite fitness, status, and the audit trail length, read back with
`hypothesis explain` / `list`.

    python3 benchmarks/conatus/e75_history.py [--bin ...] [--repeats 5]
"""
import argparse, json, os, random, re, shutil, subprocess, sys, tempfile

FIT = re.compile(r"fitness[^0-9]*([0-9]*\.?[0-9]+)", re.I)
STATUS = re.compile(r"status[^A-Za-z]*([A-Za-z]+)", re.I)


def run(bin_path, store, args, timeout=120):
    env = dict(os.environ, PHYSIS_CORE_DIR=store, PHYSIS_ALLOW_DEV_LICENCE="1")
    p = subprocess.run([bin_path] + args, capture_output=True, text=True,
                       env=env, timeout=timeout)
    return p.stdout + p.stderr


def make_store(bin_path, claim, evidence, tmp):
    """Create one hypothesis and feed it a sequence of evidence."""
    store = tempfile.mkdtemp(dir=tmp)
    out = run(bin_path, store, ["hypothesis", "create", claim])
    ids = re.findall(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", out)
    if not ids:
        raise SystemExit(f"could not create a hypothesis; engine said:\n{out}")
    hid = ids[0]
    for text, polarity, weight in evidence:
        run(bin_path, store, ["hypothesis", "evidence", hid, text,
                              "--polarity", polarity, "--weight", str(weight)])
    return store, hid


def read_state(bin_path, store, hid):
    """Exact values from the store, not the display.

    `hypothesis explain` prints fitness to two decimals, so an order effect
    smaller than 0.005 would read as "identical" — the rounding would be doing
    the measuring. The store holds the f32."""
    path = os.path.join(store, "nodes.json")
    with open(path) as fh:
        nodes = json.load(fh)
    h = nodes.get("hypotheses", {}).get(hid)
    if h is None:
        return {"fitness": None, "status": None}
    return {
        "fitness": h.get("fitness"),
        "composite": h.get("composite_fitness"),
        "status": h.get("status"),
        "evidence_count": len(h.get("evidence", [])),
        "breakdown": h.get("fitness_breakdown"),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=os.path.expanduser("~/dev/physis-pro/target/release/physis-core"))
    ap.add_argument("--repeats", type=int, default=5)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    claim = "the pump fails when inlet pressure drops below 2 bar"
    evidence = [
        ("run 1 confirmed the pump stalled at 1.8 bar", "supporting", 0.8),
        ("run 2 confirmed the pump stalled at 1.6 bar", "supporting", 0.8),
        ("run 3 the pump kept running at 1.5 bar", "contradicting", 0.8),
        ("run 4 confirmed the pump stalled at 1.9 bar", "supporting", 0.6),
        ("run 5 the pump kept running at 1.2 bar", "contradicting", 0.6),
        ("run 6 confirmed the pump stalled at 1.7 bar", "supporting", 0.7),
    ]

    tmp = tempfile.mkdtemp(prefix="e75-")
    results = {}
    try:
        store, hid = make_store(args.bin, claim, evidence, tmp)
        results["ordered"] = read_state(args.bin, store, hid)

        shuffled = []
        for seed in range(args.repeats):
            permuted = evidence[:]
            random.Random(seed).shuffle(permuted)
            s, h = make_store(args.bin, claim, permuted, tmp)
            shuffled.append(read_state(args.bin, s, h))
        results["shuffled"] = shuffled

        # Erased: the same evidence happened, then a fresh hypothesis is asked
        # the same question in the same store. If history constrains the future,
        # the store's past should reach the new claim somehow.
        s, _ = make_store(args.bin, claim, evidence, tmp)
        out = run(args.bin, s, ["hypothesis", "create", claim])
        fresh = re.findall(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", out)
        results["erased"] = read_state(args.bin, s, fresh[0]) if fresh else None

        # Interleaved: same multiset, maximally different arrival pattern.
        blocked = [e for e in evidence if e[1] == "supporting"] + \
                  [e for e in evidence if e[1] == "contradicting"]
        s, h = make_store(args.bin, claim, blocked, tmp)
        results["blocked"] = read_state(args.bin, s, h)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    print(f"{'arm':14}{'fitness (exact)':>20}{'status':>14}{'evidence':>10}")
    def row(name, r):
        print(f"{name:14}{r['fitness']!s:>20}{r['status']!s:>14}{r.get('evidence_count','')!s:>10}")
    row("ordered", results["ordered"])
    fits = [r["fitness"] for r in results["shuffled"] if r["fitness"] is not None]
    for i, r in enumerate(results["shuffled"]):
        row(f"shuffled {i}", r)
    row("blocked", results["blocked"])
    if results["erased"]:
        row("fresh claim", results["erased"])

    base = results["ordered"]["fitness"]
    if base is not None and fits:
        spread = max(fits + [base]) - min(fits + [base])
        print(f"\norder spread over {len(fits)} permutation(s) + the ordered arm: {spread:.6f}")
        print("VERDICT: " + (
            "state depends on arrival ORDER — path dependence is present"
            if spread > 1e-9 else
            "state is a function of the evidence MULTISET alone — history is storage "
            "plus a scoring function, not path dependence"))
    else:
        print("\nNOT MEASURED: the engine did not report a fitness this harness could read")

    if args.out:
        json.dump(results, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
