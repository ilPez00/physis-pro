#!/usr/bin/env python3
"""E71 — can the world model anticipate what an action will do to the world?

E67/E70 scored "which file next" and found the directory tree explains it: a
static property of the workspace, not anything learned. This asks the question a
tree cannot answer — **will this action fail?** — where the label is the tool's
own outcome and reality, not the model, has the last word.

Leave-one-session-out over `extract_actions.py`'s output. Before each action's
result is seen, an arm emits P(failure) from the prefix alone. Scored by Brier
(lower is better) and by AUC, paired over sessions.

Arms, each a control that can fire:

  base-rate       the global failure prior                  (the null)
  kind-prior      failure rate of this action kind elsewhere (the cheap arm)
  frozen          logistic weights fitted on other sessions, never updated
  deform          the same, updated online on every outcome  (the thesis, I5)
  cold-deform     the same but starting from nothing         (learning, not prior)
  deny            deform, except a surprise failure is discarded instead of
                  learned from                               (I2 stated as a rival)
  shuffled        deform, on a permuted action order         (sequence shape)

    python3 benchmarks/behaviour/e71.py [--label harness|markers] [--out f.json]
"""
import argparse, collections, json, math, random, sys

BOOTSTRAP = 2000
LR = 0.30           # online step size
L2 = 1e-4
EPOCHS = 3          # batch passes for the fitted-on-others weights

FEATURES = [
    "bias",
    "kind_prior",        # how often this kind fails in other sessions
    "kind_here",         # how often it has failed in this session so far
    "last_failed",       # did the previous action fail
    "streak",            # consecutive failures just now, capped
    "target_failed",     # this exact target failed earlier in this session
    "first_time",        # first appearance of this kind in this session
    "position",          # how far into the session we are
]


def sigmoid(z):
    if z < -30:
        return 1e-13
    if z > 30:
        return 1.0 - 1e-13
    return 1.0 / (1.0 + math.exp(-z))


def featurise(action, state, prior, position):
    kind = action["kind"]
    seen, failed = state["kind_seen"][kind], state["kind_failed"][kind]
    return {
        "bias": 1.0,
        "kind_prior": prior.get(kind, state["global_prior"]),
        "kind_here": (failed / seen) if seen else 0.0,
        "last_failed": 1.0 if state["last_failed"] else 0.0,
        "streak": min(state["streak"], 3) / 3.0,
        "target_failed": 1.0 if action["target"] in state["failed_targets"] else 0.0,
        "first_time": 0.0 if seen else 1.0,
        "position": position,
    }


def predict(weights, feats):
    return sigmoid(sum(weights[k] * feats[k] for k in FEATURES))


def update(weights, feats, label, p, lr):
    error = p - label
    for k in FEATURES:
        weights[k] -= lr * (error * feats[k] + L2 * weights[k])


def priors(sessions, label):
    """Per-kind failure rate from a set of sessions, Laplace-smoothed."""
    seen, failed = collections.Counter(), collections.Counter()
    for s in sessions:
        for a in s["actions"]:
            seen[a["kind"]] += 1
            failed[a["kind"]] += bool(a[label])
    total = sum(seen.values()) or 1
    glob = sum(failed.values()) / total
    return {k: (failed[k] + 2 * glob) / (seen[k] + 2) for k in seen}, glob


def fit_on(sessions, prior, glob, label, lr):
    """Batch-fit the frozen weights on other sessions, replaying each in order
    so the session-local features mean the same thing they will at test time."""
    weights = {k: 0.0 for k in FEATURES}
    for _ in range(EPOCHS):
        for s in sessions:
            state = new_state(glob)
            n = len(s["actions"])
            for i, a in enumerate(s["actions"]):
                feats = featurise(a, state, prior, i / n)
                p = predict(weights, feats)
                update(weights, feats, float(a[label]), p, lr / 3)
                advance(state, a, label)
    return weights


def new_state(glob):
    return {
        "kind_seen": collections.Counter(),
        "kind_failed": collections.Counter(),
        "failed_targets": set(),
        "last_failed": False,
        "streak": 0,
        "global_prior": glob,
    }


def advance(state, action, label):
    failed = bool(action[label])
    state["kind_seen"][action["kind"]] += 1
    state["kind_failed"][action["kind"]] += failed
    if failed:
        state["failed_targets"].add(action["target"])
        state["streak"] += 1
    else:
        state["streak"] = 0
    state["last_failed"] = failed


def run(session, arm, prior, glob, frozen, label, lr, seed=0):
    """One pass over a held-out session. Returns (probabilities, labels)."""
    actions = session["actions"]
    if arm == "shuffled":
        actions = actions[:]
        random.Random(seed).shuffle(actions)
    weights = (
        {k: 0.0 for k in FEATURES} if arm == "cold-deform" else dict(frozen)
    )
    state = new_state(glob)
    ps, ys = [], []
    n = len(actions)
    for i, a in enumerate(actions):
        y = float(a[label])
        feats = featurise(a, state, prior, i / n)
        if arm == "base-rate":
            p = glob
        elif arm == "kind-prior":
            p = feats["kind_prior"]
        else:
            p = predict(weights, feats)
        ps.append(p)
        ys.append(y)
        if arm in ("deform", "cold-deform", "shuffled"):
            update(weights, feats, y, p, lr)
        elif arm == "deny":
            # The rival policy: a failure the model did not expect is treated as
            # noise and dropped, instead of being the thing it learns from.
            if not (y == 1.0 and p < 0.5):
                update(weights, feats, y, p, lr)
        advance(state, a, label)
    return ps, ys


def brier(ps, ys):
    return sum((p - y) ** 2 for p, y in zip(ps, ys)) / len(ps) if ps else 0.0


def auc(ps, ys):
    pos = [p for p, y in zip(ps, ys) if y == 1.0]
    neg = [p for p, y in zip(ps, ys) if y == 0.0]
    if not pos or not neg:
        return None  # a session with no failure cannot rank them
    wins = sum((a > b) + 0.5 * (a == b) for a in pos for b in neg)
    return wins / (len(pos) * len(neg))


def mean(v):
    return sum(v) / len(v) if v else 0.0


def boot_ci(deltas, seed=0):
    if not deltas:
        return (0.0, 0.0)
    rng = random.Random(seed)
    means = sorted(mean([deltas[rng.randrange(len(deltas))] for _ in deltas])
                   for _ in range(BOOTSTRAP))
    return means[int(0.025 * BOOTSTRAP)], means[int(0.975 * BOOTSTRAP)]


ARMS = ["base-rate", "kind-prior", "frozen", "deform", "cold-deform", "deny", "shuffled"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--actions", default="benchmarks/behaviour/actions.json")
    ap.add_argument("--label", choices=["harness", "markers"], default="harness")
    ap.add_argument("--out", default=None)
    # Reported as a sweep, never tuned to taste: an online step size chosen by
    # looking at the answer is the measurement choosing its own result.
    ap.add_argument("--lr", type=float, default=0.30)
    args = ap.parse_args()
    data = json.load(open(args.actions))
    label = "harness_error" if args.label == "harness" else "marker_error"

    briers = {a: [] for a in ARMS}
    aucs = {a: [] for a in ARMS}
    halves = {a: [] for a in ARMS}     # second-half Brier minus first-half
    scored = skipped = 0

    by_project = collections.defaultdict(list)
    for s in data["sessions"]:
        by_project[s["project"]].append(s)

    for project, group in by_project.items():
        for held in group:
            others = [s for s in data["sessions"] if s is not held]
            if not others:
                skipped += 1
                continue
            prior, glob = priors(others, label)
            frozen = fit_on(others, prior, glob, label, args.lr)
            for arm in ARMS:
                ps, ys = run(held, arm, prior, glob, frozen, label, args.lr)
                if not ps:
                    continue
                briers[arm].append(brier(ps, ys))
                a = auc(ps, ys)
                if a is not None:
                    aucs[arm].append(a)
                half = len(ps) // 2
                if half >= 5:
                    halves[arm].append(brier(ps[half:], ys[half:]) - brier(ps[:half], ys[:half]))
            scored += len(held["actions"])

    if scored == 0:
        sys.exit("0 actions over 0 sessions: nothing was measured")

    print(f"{len(briers['deform'])} held-out sessions · {scored} actions scored · "
          f"lr={args.lr} · label={args.label} (positive rate "
          f"{data['harness_error_rate' if args.label == 'harness' else 'marker_error_rate']:.3f})")
    print(f"sessions with no failure at all are excluded from AUC only "
          f"({len(briers['deform']) - len(aucs['deform'])} of them)")
    print(f"\n{'arm':14}{'Brier':>9}{'AUC':>8}{'2nd-1st half Brier':>22}")
    for arm in ARMS:
        print(f"{arm:14}{mean(briers[arm]):9.4f}{mean(aucs[arm]):8.3f}{mean(halves[arm]):+22.4f}")

    print("\n── paired over sessions, 95% CI (negative Brier delta = better)")
    for rival in ("base-rate", "kind-prior", "frozen", "deny", "shuffled", "cold-deform"):
        deltas = [d - f for d, f in zip(briers["deform"], briers[rival])]
        lo, hi = boot_ci(deltas)
        verdict = "beats" if hi < 0 else ("loses" if lo > 0 else "TIES")
        print(f"  deform vs {rival:12} Brier {mean(deltas):+.5f} [{lo:+.5f}, {hi:+.5f}] {verdict}")

    pairs = [(d, f) for d, f in zip(aucs["deform"], aucs["frozen"])]
    if pairs:
        deltas = [d - f for d, f in pairs]
        lo, hi = boot_ci(deltas)
        verdict = "beats" if lo > 0 else ("loses" if hi < 0 else "TIES")
        print(f"  deform vs frozen        AUC   {mean(deltas):+.4f} [{lo:+.4f}, {hi:+.4f}] {verdict}")

    if args.out:
        json.dump({"label": args.label, "actions_scored": scored,
                   "brier": briers, "auc": aucs, "half_delta": halves},
                  open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
