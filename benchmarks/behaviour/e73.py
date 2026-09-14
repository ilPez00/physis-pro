#!/usr/bin/env python3
"""E73 — does the prior port to another machine, or another harness?

E71 concluded that the cross-session prior over action kinds is the product.
That claim is only worth anything if the prior transfers: a prior that works
only on the sessions it was built from is a memory, not a model.

Groups are disjoint by construction:

  machine sets   sessions found ONLY on pansa / omo / feather (synced copies,
                 which are byte-identical, are excluded from both sides)
  harness sets   claude-code vs codex, two different agents on the same machines

For each ordered pair, the per-kind failure prior is fitted on the source group
and scored on the target group, against two references: the target's own
leave-one-session-out prior (the in-corpus ceiling) and the source's global
failure rate (the transferable base rate, which needs no kinds at all).

    python3 benchmarks/behaviour/e73.py [--actions actions-merged.json]

A pair with too few sessions or no failures is reported as NOT MEASURED, never
as a tie.
"""
import argparse, collections, itertools, json, os, random, sys

BOOTSTRAP = 2000

MIN_SESSIONS = 2


def priors(sessions, label):
    seen, failed = collections.Counter(), collections.Counter()
    for s in sessions:
        for a in s["actions"]:
            seen[a["kind"]] += 1
            failed[a["kind"]] += bool(a[label])
    total = sum(seen.values()) or 1
    glob = sum(failed.values()) / total
    return {k: (failed[k] + 2 * glob) / (seen[k] + 2) for k in seen}, glob


def score(sessions, prior, glob, label):
    ps, ys = [], []
    for s in sessions:
        for a in s["actions"]:
            ps.append(prior.get(a["kind"], glob))
            ys.append(float(a[label]))
    return ps, ys


def auc(ps, ys):
    pos = [p for p, y in zip(ps, ys) if y == 1.0]
    neg = [p for p, y in zip(ps, ys) if y == 0.0]
    if not pos or not neg:
        return None
    return sum((a > b) + 0.5 * (a == b) for a in pos for b in neg) / (len(pos) * len(neg))


def brier(ps, ys):
    return sum((p - y) ** 2 for p, y in zip(ps, ys)) / len(ps) if ps else 0.0


def session_auc_ci(sessions, prior, glob, label, seed=0):
    """Bootstrap over target SESSIONS, not over actions: actions inside one
    session are not independent draws, and resampling them would report an
    interval several times too narrow."""
    per = []
    for s in sessions:
        ps, ys = score([s], prior, glob, label)
        a = auc(ps, ys)
        if a is not None:
            per.append(a)
    if len(per) < 2:
        return None, (None, None)
    rng = random.Random(seed)
    means = sorted(sum(per[rng.randrange(len(per))] for _ in per) / len(per)
                   for _ in range(BOOTSTRAP))
    mean = sum(per) / len(per)
    return mean, (means[int(0.025 * BOOTSTRAP)], means[int(0.975 * BOOTSTRAP)])


def coverage(sessions, prior):
    """Share of the target's actions whose kind the source has ever seen. A
    prior that transfers only because both corpora run `git status` should say
    so."""
    total = hit = 0
    for s in sessions:
        for a in s["actions"]:
            total += 1
            hit += a["kind"] in prior
    return hit / total if total else 0.0


def groups(data):
    by_machine = collections.defaultdict(list)
    for s in data["sessions"]:
        if s["harness"] != "claude-code":
            continue
        # Sessions synced to several machines belong to none of the disjoint
        # machine sets: counting a byte-identical copy as both source and target
        # would make transfer look perfect.
        if len(s["seen_on"]) == 1:
            by_machine[f"machine:{s['seen_on'][0]}"].append(s)
    out = {k: v for k, v in by_machine.items() if len(v) >= MIN_SESSIONS}
    for harness in ("claude-code", "codex"):
        sessions = [s for s in data["sessions"] if s["harness"] == harness]
        if len(sessions) >= MIN_SESSIONS:
            out[f"harness:{harness}"] = sessions
    return out


def main():
    ap = argparse.ArgumentParser()
    here = os.path.dirname(os.path.abspath(__file__))
    ap.add_argument("--actions", default=os.path.join(here, "actions-merged.json"))
    ap.add_argument("--label", choices=["harness", "markers"], default="markers")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()
    data = json.load(open(args.actions))
    label = "harness_error" if args.label == "harness" else "marker_error"

    sets = groups(data)
    if len(sets) < 2:
        sys.exit(f"{len(sets)} usable group(s): nothing to transfer between")

    print(f"label={args.label} · groups: " +
          ", ".join(f"{k} ({len(v)} sessions, "
                    f"{sum(len(s['actions']) for s in v)} actions)" for k, v in sets.items()))
    print(f"\n{'source → target':38}{'AUC':>7}{'in-corpus':>11}{'base':>7}{'coverage':>10}")

    rows = []
    for src, dst in itertools.permutations(sets, 2):
        if src.split(":")[0] != dst.split(":")[0]:
            continue  # machine→machine and harness→harness only
        prior, glob = priors(sets[src], label)
        ps, ys = score(sets[dst], prior, glob, label)
        transferred = auc(ps, ys)
        # The in-corpus ceiling: the target scored by its own leave-one-out
        # prior, which is what E71 reported.
        own = []
        for held in sets[dst]:
            rest = [s for s in sets[dst] if s is not held]
            if not rest:
                continue
            p2, g2 = priors(rest, label)
            q, r = score([held], p2, g2, label)
            own.append((q, r))
        own_ps = [p for q, _ in own for p in q]
        own_ys = [y for _, r in own for y in r]
        in_corpus = auc(own_ps, own_ys)
        base_ps = [glob] * len(ys)
        row = {
            "source": src, "target": dst,
            "auc": transferred, "in_corpus": in_corpus,
            "base_auc": auc(base_ps, ys),
            "brier": brier(ps, ys), "base_brier": brier(base_ps, ys),
            "coverage": coverage(sets[dst], prior),
            "target_actions": len(ys), "target_failures": int(sum(ys)),
        }
        rows.append(row)
        if transferred is None:
            print(f"{src} → {dst:20}  NOT MEASURED (no failure in the target)")
            continue
        per_session, (lo, hi) = session_auc_ci(sets[dst], prior, glob, label)
        row["per_session_auc"] = per_session
        row["ci"] = [lo, hi]
        ci = (f"[{lo:.3f}, {hi:.3f}]" if lo is not None else "n/a")
        print(f"{src + ' → ' + dst:38}{transferred:7.3f}"
              f"{(in_corpus if in_corpus is not None else float('nan')):11.3f}"
              f"{0.5:7.3f}{row['coverage']:10.3f}   per-session {per_session if per_session is None else format(per_session, '.3f')} {ci}")

    print("\nin-corpus = the target's own leave-one-session-out prior (E71's number)")
    print("base      = a single failure rate carries no ranking information, so 0.500")
    print("coverage  = share of the target's actions whose kind the source had ever seen")
    if args.out:
        json.dump(rows, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
