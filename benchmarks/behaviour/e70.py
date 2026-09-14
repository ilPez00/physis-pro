#!/usr/bin/env python3
"""E70 — the smallest machine for becoming: predict, mismatch, deform, settle.

E67 holds the geometry still and asks whether it predicts behaviour. E70 lets
the geometry move and asks the thesis directly: does being wrong, and changing
because of it, beat not changing?

State is a weight vector over E67's ranking arms. At each step the arms are
fused by those weights, the rank of the file actually touched is the mismatch,
and the weights move toward whichever arms ranked it best (a Hedge update — one
line, deliberately, so the experiment is about whether deformation helps rather
than about a clever rule).

Arms, each a control that can fire:

  deform          the loop above                              (the thesis)
  frozen          identical weights, never updated            (I5's control)
  random-deform   same update magnitude, random direction     (does change help,
                                                               or this change?)
  deny            on a mismatch, drop the observation instead
                  of updating                                 (I2 stated as a rival)
  shuffled        deform, with the session's order shuffled   (sequence shape)

And one question no next-step arm answers (I4): from the first quarter of a
session, name the files its last quarter will touch.

    python3 benchmarks/behaviour/e70.py [--sessions f.json] [--out results.json]
"""
import argparse, collections, json, math, random, sys, os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from e67 import ARMS, build_lexical, tables, rank_of, mean, boot_ci  # noqa: E402

# Only the arms that say something about *where in the workspace* the agent is.
# `frequency` is a constant prior and would make the weight vector's job
# trivial; it stays in E67 as the baseline it was.
FEATURES = ["recency", "tree", "co-access", "transition", "lexical"]
ETA = 0.5  # Hedge learning rate


def fuse(ctx, weights):
    """Weighted reciprocal-rank fusion of the feature arms."""
    fused = collections.defaultdict(float)
    for name, weight in weights.items():
        if weight <= 0:
            continue
        scores = ARMS[name](ctx)
        if not scores:
            continue
        order = sorted(scores, key=lambda p: -scores[p])
        for rank, path in enumerate(order[:50]):
            fused[path] += weight / (60.0 + rank + 1)
    return dict(fused)


def per_feature_rank(ctx, candidates, truth):
    """Where each feature alone puts the file that was actually touched."""
    out = {}
    for name in FEATURES:
        scores = ARMS[name](ctx)
        out[name] = rank_of(scores, candidates, truth, candidates)
    return out


def run_session(seq, candidates, ctx_common, mode, seed=0):
    """One pass of predict → mismatch → deform. Returns the reciprocal rank of
    every step, in order, so error over time is visible rather than averaged."""
    rng = random.Random(seed)
    weights = {name: 1.0 for name in FEATURES}
    rr = []
    kept = []  # the observations this policy has accepted so far
    for k in range(len(seq) - 1):
        truth = seq[k + 1]
        ctx = dict(ctx_common)
        ctx["prefix"] = kept + [seq[k]]
        ctx["current"] = seq[k]
        ctx["candidates"] = candidates
        switch_candidates = [c for c in candidates if c != seq[k]]
        scored = switch_candidates if truth != seq[k] else candidates
        rank = rank_of(fuse(ctx, weights), scored, truth, scored)
        rr.append(1.0 / rank)

        if mode == "deny":
            # The rival policy: a mismatch means the observation was noise.
            # Nothing deforms; the offending step is dropped from the history.
            if rank > 5:
                continue
            kept.append(seq[k])
            continue
        kept.append(seq[k])
        if mode == "frozen":
            continue
        if mode == "random-deform":
            for name in weights:
                weights[name] *= math.exp(ETA * (rng.random() - 0.5))
        else:
            # Hedge: reward the features that ranked the truth best this step.
            ranks = per_feature_rank(ctx, scored, truth)
            best = min(ranks.values())
            for name, r in ranks.items():
                loss = 1.0 - (best / r if r else 0.0)
                weights[name] *= math.exp(-ETA * loss)
        total = sum(weights.values()) or 1.0
        for name in weights:
            weights[name] = weights[name] / total * len(weights)
    return rr, weights


def settle(seq, candidates, ctx_common, weights, cut=0.25):
    """I4 — from the first quarter, name the last quarter's files.

    Scored as precision@|target|: of the files the arm puts first, how many the
    session actually ends on. The nulls are a frequency prior over the prefix
    and the fused one-step ranking rolled forward, which is what a next-state
    machine would answer with."""
    k = max(2, int(len(seq) * cut))
    prefix, tail = seq[:k], seq[int(len(seq) * (1 - cut)):]
    target = set(tail)
    if not target:
        return None
    ctx = dict(ctx_common)
    ctx["prefix"] = prefix
    ctx["current"] = prefix[-1]
    ctx["candidates"] = candidates

    def precision_at(order):
        top = order[: len(target)]
        return len(set(top) & target) / len(target)

    fused = fuse(ctx, weights)
    one_step = sorted(fused, key=lambda p: -fused[p])
    counts = collections.Counter(prefix)
    freq = [p for p, _ in counts.most_common()]
    freq += [c for c in candidates if c not in counts]
    # The attractor arm: files that the *whole* prefix pulls toward, rather than
    # the ones adjacent to its last step. Accumulated fused score across every
    # position in the prefix, which is the cheapest observable stand-in for "the
    # configuration this is settling into".
    basin = collections.defaultdict(float)
    for i, path in enumerate(prefix):
        step_ctx = dict(ctx)
        step_ctx["prefix"] = prefix[: i + 1]
        step_ctx["current"] = path
        for p, v in fuse(step_ctx, weights).items():
            basin[p] += v
    attractor = sorted(basin, key=lambda p: -basin[p])
    return {
        "attractor": precision_at(attractor),
        "one_step": precision_at(one_step),
        "frequency": precision_at(freq),
        "target_size": len(target),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sessions", default="benchmarks/behaviour/sessions.json")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()
    data = json.load(open(args.sessions))

    by_project = collections.defaultdict(list)
    for s in data["sessions"]:
        by_project[s["project"]].append(s)

    modes = ["deform", "frozen", "random-deform", "deny", "shuffled"]
    mrr = {m: [] for m in modes}
    first_half = {m: [] for m in modes}
    second_half = {m: [] for m in modes}
    settles = {"attractor": [], "one_step": [], "frequency": []}
    weights_seen = []
    steps = 0

    for project, group in by_project.items():
        candidates = sorted({t["path"] for s in group for t in s["touches"]})
        lexical = build_lexical(candidates)
        for held in group:
            others = [s for s in group if s is not held]
            if not others:
                continue
            co, trans, counts = tables(others)
            ctx_common = {
                "other_counts": counts,
                "coaccess": co,
                "transitions": trans,
                "lexical": lexical,
            }
            seq = [t["path"] for t in held["touches"]]
            for mode in modes:
                run_seq = seq
                if mode == "shuffled":
                    run_seq = seq[:]
                    random.Random(7).shuffle(run_seq)
                rr, weights = run_session(run_seq, candidates, ctx_common, mode)
                if not rr:
                    continue
                if mode == "deform":
                    steps += len(rr)
                    weights_seen.append(weights)
                    out = settle(seq, candidates, ctx_common, weights)
                    if out:
                        for key in settles:
                            settles[key].append(out[key])
                mrr[mode].append(mean(rr))
                half = len(rr) // 2
                if half >= 2:
                    first_half[mode].append(mean(rr[:half]))
                    second_half[mode].append(mean(rr[half:]))

    if steps == 0:
        sys.exit("0 steps over 0 sessions: nothing was measured")

    print(f"{len(mrr['deform'])} held-out sessions · {steps} steps scored · "
          f"{len(FEATURES)} features, Hedge eta={ETA}")
    print(f"\n{'arm':16}{'MRR':>8}{'1st half':>10}{'2nd half':>10}{'within-session gain':>21}")
    for mode in modes:
        gain = mean([b - a for a, b in zip(first_half[mode], second_half[mode])])
        print(f"{mode:16}{mean(mrr[mode]):8.3f}{mean(first_half[mode]):10.3f}"
              f"{mean(second_half[mode]):10.3f}{gain:+21.3f}")

    print("\n── I5: does deforming beat a frozen copy of itself? "
          "(paired over sessions, 95% CI)")
    for rival in ("frozen", "random-deform", "deny", "shuffled"):
        if len(mrr[rival]) != len(mrr["deform"]):
            print(f"  {rival}: NOT COMPARABLE (different session counts)")
            continue
        deltas = [a - b for a, b in zip(mrr["deform"], mrr[rival])]
        lo, hi = boot_ci(deltas)
        verdict = "beats" if lo > 0 else ("loses" if hi < 0 else "TIES")
        print(f"  deform vs {rival:14} MRR {mean(deltas):+.4f} [{lo:+.4f}, {hi:+.4f}] {verdict}")

    print("\n── I4: from the first quarter, which files does the last quarter touch?")
    for name in ("attractor", "one_step", "frequency"):
        print(f"  {name:12} precision@|target| {mean(settles[name]):.3f}")
    for rival in ("one_step", "frequency"):
        deltas = [a - b for a, b in zip(settles["attractor"], settles[rival])]
        lo, hi = boot_ci(deltas)
        verdict = "beats" if lo > 0 else ("loses" if hi < 0 else "TIES")
        print(f"  attractor vs {rival:10} {mean(deltas):+.4f} [{lo:+.4f}, {hi:+.4f}] {verdict}")

    if weights_seen:
        print("\n── what the deformation learned (mean final weight, uniform start = 1.0)")
        for name in FEATURES:
            print(f"  {name:12} {mean([w[name] for w in weights_seen]):.3f}")

    if args.out:
        json.dump({"mrr": mrr, "first_half": first_half, "second_half": second_half,
                   "settle": settles, "steps": steps,
                   "final_weights": weights_seen}, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
