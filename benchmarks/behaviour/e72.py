#!/usr/bin/env python3
"""E72 — does the state of *this* workspace add anything to the prior?

E71 ended with a specific gap: the cross-session prior over action kinds carries
the signal, and what a prior cannot hold is the state of the workspace right now
— an edit that has not compiled yet, a service that is down. This measures
exactly that increment, on the same 9466 recorded actions, before any of it is
shipped.

Four arms, fitted leave-one-session-out and scored by AUC and Brier, paired over
sessions:

  kind-prior      the failure rate of this action kind elsewhere   (what ships today)
  fitted          + session state: last failure, streak, this target failed
  fitted+edits    + edits since the last success of this kind, and recent edits
  edits-only      the new features alone, without the prior        (are they anything?)

Pre-registered: `predict` gains the state features only if `fitted+edits` beats
`kind-prior` on AUC with a bootstrap CI over sessions excluding zero.

    python3 benchmarks/behaviour/e72.py [--label harness|markers]
"""
import argparse, collections, json, math, os, random, sys

BOOTSTRAP = 2000
EPOCHS = 4
LR = 0.10
L2 = 1e-4

EDIT_KINDS = {"Edit", "Write", "NotebookEdit"}

BASE_FEATURES = ["bias", "kind_prior"]
STATE_FEATURES = ["kind_here", "last_failed", "streak", "target_failed"]
EDIT_FEATURES = ["edits_since_ok", "edits_recent"]

ARMS = {
    "kind-prior": None,                                     # not fitted at all
    "fitted": BASE_FEATURES + STATE_FEATURES,
    "fitted+edits": BASE_FEATURES + STATE_FEATURES + EDIT_FEATURES,
    "edits-only": ["bias"] + EDIT_FEATURES,
}


def sigmoid(z):
    return 1e-13 if z < -30 else (1 - 1e-13 if z > 30 else 1 / (1 + math.exp(-z)))


def priors(sessions, label):
    seen, failed = collections.Counter(), collections.Counter()
    for s in sessions:
        for a in s["actions"]:
            seen[a["kind"]] += 1
            failed[a["kind"]] += bool(a[label])
    total = sum(seen.values()) or 1
    glob = sum(failed.values()) / total
    return {k: (failed[k] + 2 * glob) / (seen[k] + 2) for k in seen}, glob


def new_state(glob):
    return {"kind_seen": collections.Counter(), "kind_failed": collections.Counter(),
            "failed_targets": set(), "last_failed": False, "streak": 0,
            "edits_since_ok": collections.Counter(), "recent": collections.deque(maxlen=5),
            "global_prior": glob}


def featurise(action, state, prior):
    kind = action["kind"]
    seen, failed = state["kind_seen"][kind], state["kind_failed"][kind]
    return {
        "bias": 1.0,
        "kind_prior": prior.get(kind, state["global_prior"]),
        "kind_here": (failed / seen) if seen else 0.0,
        "last_failed": 1.0 if state["last_failed"] else 0.0,
        "streak": min(state["streak"], 3) / 3.0,
        "target_failed": 1.0 if action["target"] in state["failed_targets"] else 0.0,
        # The new pair: what has happened to the workspace since this kind of
        # action last worked, and whether anything was edited just now.
        "edits_since_ok": min(state["edits_since_ok"][kind], 5) / 5.0,
        "edits_recent": sum(1 for k in state["recent"] if k in EDIT_KINDS) / 5.0,
    }


def advance(state, action, label):
    failed = bool(action[label])
    kind = action["kind"]
    state["kind_seen"][kind] += 1
    state["kind_failed"][kind] += failed
    if failed:
        state["failed_targets"].add(action["target"])
        state["streak"] += 1
    else:
        state["streak"] = 0
        state["edits_since_ok"][kind] = 0
    state["last_failed"] = failed
    if kind in EDIT_KINDS:
        for other in list(state["edits_since_ok"]) + list(state["kind_seen"]):
            state["edits_since_ok"][other] += 1
    state["recent"].append(kind)


def fit(sessions, features, prior, glob, label):
    weights = {f: 0.0 for f in features}
    for _ in range(EPOCHS):
        for s in sessions:
            state = new_state(glob)
            for a in s["actions"]:
                feats = featurise(a, state, prior)
                p = sigmoid(sum(weights[f] * feats[f] for f in features))
                err = p - float(a[label])
                for f in features:
                    weights[f] -= LR * (err * feats[f] + L2 * weights[f])
                advance(state, a, label)
    return weights


def score_session(session, arm, features, weights, prior, glob, label):
    state = new_state(glob)
    ps, ys = [], []
    for a in session["actions"]:
        feats = featurise(a, state, prior)
        p = (feats["kind_prior"] if arm == "kind-prior"
             else sigmoid(sum(weights[f] * feats[f] for f in features)))
        ps.append(p)
        ys.append(float(a[label]))
        advance(state, a, label)
    return ps, ys


def auc(ps, ys):
    pos = [p for p, y in zip(ps, ys) if y == 1.0]
    neg = [p for p, y in zip(ps, ys) if y == 0.0]
    if not pos or not neg:
        return None
    return sum((a > b) + 0.5 * (a == b) for a in pos for b in neg) / (len(pos) * len(neg))


def mean(v):
    return sum(v) / len(v) if v else 0.0


def boot_ci(d, seed=0):
    if not d:
        return (0.0, 0.0)
    rng = random.Random(seed)
    m = sorted(mean([d[rng.randrange(len(d))] for _ in d]) for _ in range(BOOTSTRAP))
    return m[int(0.025 * BOOTSTRAP)], m[int(0.975 * BOOTSTRAP)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--actions", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "actions.json"))
    ap.add_argument("--label", choices=["harness", "markers"], default="harness")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()
    data = json.load(open(args.actions))
    label = "harness_error" if args.label == "harness" else "marker_error"

    aucs = {a: [] for a in ARMS}
    briers = {a: [] for a in ARMS}
    scored = 0
    for held in data["sessions"]:
        others = [s for s in data["sessions"] if s is not held]
        prior, glob = priors(others, label)
        for arm, features in ARMS.items():
            weights = fit(others, features, prior, glob, label) if features else {}
            ps, ys = score_session(held, arm, features or [], weights, prior, glob, label)
            if not ps:
                continue
            briers[arm].append(sum((p - y) ** 2 for p, y in zip(ps, ys)) / len(ps))
            a = auc(ps, ys)
            if a is not None:
                aucs[arm].append(a)
        scored += len(held["actions"])

    if scored == 0:
        sys.exit("0 actions over 0 sessions: nothing was measured")

    print(f"{len(briers['kind-prior'])} held-out sessions · {scored} actions · label={args.label}")
    print(f"{'arm':14}{'AUC':>8}{'Brier':>9}")
    for arm in ARMS:
        print(f"{arm:14}{mean(aucs[arm]):8.3f}{mean(briers[arm]):9.4f}")

    print("\n── paired over sessions, 95% CI (the pre-registered gate is AUC vs kind-prior)")
    for arm in ("fitted", "fitted+edits", "edits-only"):
        d = [a - b for a, b in zip(aucs[arm], aucs["kind-prior"])]
        lo, hi = boot_ci(d)
        print(f"  {arm:13} vs kind-prior   AUC {mean(d):+.4f} [{lo:+.4f}, {hi:+.4f}] "
              f"{'beats' if lo > 0 else ('loses' if hi < 0 else 'TIES')}")
    d = [a - b for a, b in zip(aucs["fitted+edits"], aucs["fitted"])]
    lo, hi = boot_ci(d)
    print(f"  fitted+edits  vs fitted       AUC {mean(d):+.4f} [{lo:+.4f}, {hi:+.4f}] "
          f"{'beats' if lo > 0 else ('loses' if hi < 0 else 'TIES')}")

    if args.out:
        json.dump({"auc": aucs, "brier": briers, "label": args.label}, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
