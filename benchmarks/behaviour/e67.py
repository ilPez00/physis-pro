#!/usr/bin/env python3
"""E67 — does an agent's file-touch sequence carry any geometry at all?

Leave-one-session-out over the sessions `extract.py` wrote. At each step the
arm ranks the project's candidate files given the prefix; the score is where the
file actually touched next lands. The arm to beat is recency+frequency: if no
geometry beats it, behaviour at this scale carries nothing a world model can
recover, and the surrogate test (L5) is dead until the data is richer.

    python3 benchmarks/behaviour/e67.py [--sessions f.json] [--out results.json]

Every arm is scored on two populations, because they answer different
questions and the first one is easy:

  all      every step, including the very common "touch the same file again"
  switch   only steps where the next file differs from the current one

Reports top-1, top-5 and MRR with a bootstrap CI over sessions, and prints the
denominators (steps scored, files reachable, file contents found on disk).
"""
import argparse, collections, json, math, os, random, re, sys

BOOTSTRAP = 2000


# ── arms ────────────────────────────────────────────────────────────────────
# Each arm is `score(ctx) -> {path: score}`; higher is better. Unscored
# candidates are ranked after every scored one, in a fixed order, so an arm that
# returns nothing degrades to a stable arbitrary ranking rather than to chance
# that varies between runs.

def arm_recency(ctx):
    return {p: float(i) for i, p in enumerate(ctx["prefix"])}


def arm_frequency(ctx):
    return {p: float(n) for p, n in ctx["other_counts"].items()}


def rrf(*rankings, k=60.0):
    """Reciprocal-rank fusion: combines arms without a shared score scale."""
    fused = collections.defaultdict(float)
    for scores in rankings:
        order = sorted(scores, key=lambda p: -scores[p])
        for rank, path in enumerate(order):
            fused[path] += 1.0 / (k + rank + 1)
    return dict(fused)


def arm_recency_frequency(ctx):
    return rrf(arm_recency(ctx), arm_frequency(ctx))


def arm_tree(ctx):
    """Directory distance from the current file: shared path prefix length."""
    cur = ctx["current"].split("/")
    out = {}
    for path in ctx["candidates"]:
        parts = path.split("/")
        shared = 0
        for a, b in zip(cur[:-1], parts[:-1]):
            if a != b:
                break
            shared += 1
        out[path] = float(shared)
    return out


def arm_coaccess(ctx):
    """Files touched near this one in OTHER sessions of the same project."""
    return {p: float(n) for p, n in ctx["coaccess"].get(ctx["current"], {}).items()}


def arm_transition(ctx):
    """First-order transition table from other sessions: what followed this
    file there. The cheap stand-in for a world-state transition table."""
    return {p: float(n) for p, n in ctx["transitions"].get(ctx["current"], {}).items()}


def arm_lexical(ctx):
    """BM25 of the current file's text against candidate file texts."""
    idx = ctx["lexical"]
    query = idx["terms"].get(ctx["current"])
    if not query:
        return {}
    out = {}
    for path in ctx["candidates"]:
        terms = idx["terms"].get(path)
        if not terms:
            continue
        out[path] = bm25(query, terms, idx)
    return out


def arm_structure(ctx):
    """co-access + transitions + tree, fused. The 'geometry' arm proper."""
    return rrf(arm_coaccess(ctx), arm_transition(ctx), arm_tree(ctx))


ARMS = {
    "recency": arm_recency,
    "frequency": arm_frequency,
    "recency+frequency": arm_recency_frequency,
    "tree": arm_tree,
    "co-access": arm_coaccess,
    "transition": arm_transition,
    "lexical": arm_lexical,
    "structure(co+trans+tree)": arm_structure,
}


# ── a small BM25 over file contents, dependency-free ────────────────────────
WORD = re.compile(r"[A-Za-z_][A-Za-z0-9_]{2,}")
PREFIX_BYTES = 16 * 1024


def build_lexical(paths):
    terms, found = {}, 0
    for path in paths:
        try:
            with open(path, "rb") as fh:
                raw = fh.read(PREFIX_BYTES)
            text = raw.decode("utf-8", "replace")
        except OSError:
            continue
        found += 1
        counts = collections.Counter(w.lower() for w in WORD.findall(text))
        if counts:
            terms[path] = counts
    df = collections.Counter()
    for counts in terms.values():
        df.update(counts.keys())
    avg = (sum(sum(c.values()) for c in terms.values()) / len(terms)) if terms else 0.0
    return {"terms": terms, "df": df, "n": len(terms), "avg": avg, "found": found}


def bm25(query, doc, idx, k1=1.5, b=0.75):
    if not idx["n"] or not idx["avg"]:
        return 0.0
    length = sum(doc.values())
    score = 0.0
    for term in query:
        f = doc.get(term)
        if not f:
            continue
        df = idx["df"].get(term, 0) or 1
        w = math.log(1 + (idx["n"] - df + 0.5) / (df + 0.5))
        score += w * (f * (k1 + 1)) / (f + k1 * (1 - b + b * length / idx["avg"]))
    return score


# ── evaluation ──────────────────────────────────────────────────────────────
def tables(sessions, shuffle=False, seed=0):
    """Co-access and transition tables from a set of sessions."""
    rng = random.Random(seed)
    co = collections.defaultdict(collections.Counter)
    trans = collections.defaultdict(collections.Counter)
    counts = collections.Counter()
    for s in sessions:
        seq = [t["path"] for t in s["touches"]]
        if shuffle:
            seq = seq[:]
            rng.shuffle(seq)
        counts.update(seq)
        for i, path in enumerate(seq):
            if i + 1 < len(seq) and seq[i + 1] != path:
                trans[path][seq[i + 1]] += 1
            for j in range(max(0, i - 5), min(len(seq), i + 6)):
                if seq[j] != path:
                    co[path][seq[j]] += 1
    return co, trans, counts


def rank_of(scores, candidates, truth, tiebreak):
    """Rank of `truth` (1-based). Unscored candidates follow scored ones in a
    stable order, so an empty arm is arbitrary but not random."""
    scored = sorted((p for p in candidates if p in scores),
                    key=lambda p: (-scores[p], tiebreak.index(p)))
    rest = [p for p in candidates if p not in scores]
    order = scored + rest
    return order.index(truth) + 1 if truth in order else len(order) + 1


def evaluate(data, shuffle_tables=False):
    sessions = data["sessions"]
    by_project = collections.defaultdict(list)
    for s in sessions:
        by_project[s["project"]].append(s)

    per_arm = {a: {"all": collections.defaultdict(list), "switch": collections.defaultdict(list)}
               for a in ARMS}
    steps_all = steps_switch = 0
    lexical_cache = {}

    for project, group in by_project.items():
        candidates = sorted({t["path"] for s in group for t in s["touches"]})
        if project not in lexical_cache:
            lexical_cache[project] = build_lexical(candidates)
        lexical = lexical_cache[project]
        for held in group:
            others = [s for s in group if s is not held]
            if not others:
                continue  # a project with one session cannot be held out
            co, trans, counts = tables(others, shuffle=shuffle_tables)
            seq = [t["path"] for t in held["touches"]]
            per_session = {a: {"all": [], "switch": []} for a in ARMS}
            for k in range(len(seq) - 1):
                truth = seq[k + 1]
                ctx = {
                    "prefix": seq[: k + 1],
                    "current": seq[k],
                    "candidates": candidates,
                    "other_counts": counts,
                    "coaccess": co,
                    "transitions": trans,
                    "lexical": lexical,
                }
                switch = truth != seq[k]
                steps_all += 1
                steps_switch += switch
                # On switch steps the current file is removed from the candidate
                # set. Leaving it in flatters every arm that ranks a file near
                # itself — `lexical` scores a file against its own text, so it
                # holds rank 1 with a prediction that is known to be wrong here.
                switch_candidates = [c for c in candidates if c != seq[k]]
                for name, fn in ARMS.items():
                    scores = fn(ctx)
                    per_session[name]["all"].append(
                        rank_of(scores, candidates, truth, candidates))
                    if switch:
                        per_session[name]["switch"].append(
                            rank_of(scores, switch_candidates, truth, switch_candidates))
            for name in ARMS:
                for pop in ("all", "switch"):
                    ranks = per_session[name][pop]
                    if ranks:
                        per_arm[name][pop]["top1"].append(
                            sum(1 for r in ranks if r == 1) / len(ranks))
                        per_arm[name][pop]["top5"].append(
                            sum(1 for r in ranks if r <= 5) / len(ranks))
                        per_arm[name][pop]["mrr"].append(
                            sum(1.0 / r for r in ranks) / len(ranks))
    return per_arm, steps_all, steps_switch


def mean(v):
    return sum(v) / len(v) if v else 0.0


def boot_ci(deltas, seed=0):
    if not deltas:
        return (0.0, 0.0)
    rng = random.Random(seed)
    means = []
    for _ in range(BOOTSTRAP):
        sample = [deltas[rng.randrange(len(deltas))] for _ in deltas]
        means.append(mean(sample))
    means.sort()
    return (means[int(0.025 * len(means))], means[int(0.975 * len(means))])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sessions", default="benchmarks/behaviour/sessions.json")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()
    data = json.load(open(args.sessions))

    per_arm, steps_all, steps_switch = evaluate(data)
    shuffled, _, _ = evaluate(data, shuffle_tables=True)

    if steps_all == 0:
        sys.exit("0 steps scored over 0 sessions: nothing was measured")

    print(f"{data['sessions_kept']} sessions · {data['total_touches']} touches · "
          f"{steps_all} steps scored ({steps_switch} of them switches)")
    print(f"candidate sets: {data['candidate_set_sizes']}")
    base = "recency+frequency"
    for pop in ("all", "switch"):
        print(f"\n── {pop} steps " + "─" * 46)
        print(f"{'arm':26}{'top1':>7}{'top5':>7}{'MRR':>7}   Δ vs recency+frequency, 95% CI over sessions"
          f"  (+ beats · - loses · ~ ties)")
        base_by_session = per_arm[base][pop]["top1"]
        for name in ARMS:
            t1 = per_arm[name][pop]["top1"]
            row = (f"{name:26}{mean(t1):7.3f}{mean(per_arm[name][pop]['top5']):7.3f}"
                   f"{mean(per_arm[name][pop]['mrr']):7.3f}")
            if name != base and len(t1) == len(base_by_session):
                cells = []
                for metric in ("top1", "top5", "mrr"):
                    arm_v = per_arm[name][pop][metric]
                    base_v = per_arm[base][pop][metric]
                    deltas = [a - b for a, b in zip(arm_v, base_v)]
                    lo, hi = boot_ci(deltas)
                    mark = "+" if lo > 0 else ("-" if hi < 0 else "~")
                    cells.append(f"{metric} {mean(deltas):+.3f}[{lo:+.3f},{hi:+.3f}]{mark}")
                row += "   " + "  ".join(cells)
            print(row)
        # The pre-registered gate: an arm counts only if it also beats its own
        # order-shuffled tables. Otherwise the win is a static property of the
        # workspace (the directory tree is the same under any order), not
        # something recovered from behaviour.
        for name in ("structure(co+trans+tree)", "co-access", "transition"):
            real = per_arm[name][pop]["top1"]
            null = shuffled[name][pop]["top1"]
            if len(real) == len(null) and real:
                deltas = [a - b for a, b in zip(real, null)]
                lo, hi = boot_ci(deltas)
                mark = "beats" if lo > 0 else ("loses" if hi < 0 else "TIES — order carries nothing here")
                print(f"  {name} vs its own shuffled tables: top1 {mean(deltas):+.3f} "
                      f"[{lo:+.3f}, {hi:+.3f}] {mark}")
        tree = per_arm["tree"][pop]["top1"]
        structure = per_arm["structure(co+trans+tree)"][pop]["top1"]
        if len(tree) == len(structure) and tree:
            deltas = [a - b for a, b in zip(structure, tree)]
            lo, hi = boot_ci(deltas)
            mark = "beats" if lo > 0 else ("loses" if hi < 0 else "TIES — the tree is doing the work")
            print(f"  structure vs tree alone: top1 {mean(deltas):+.3f} [{lo:+.3f}, {hi:+.3f}] {mark}")
        print(f"{'  (order-shuffled tables)':26}"
              f"{mean(shuffled['structure(co+trans+tree)'][pop]['top1']):7.3f}"
              f"{mean(shuffled['structure(co+trans+tree)'][pop]['top5']):7.3f}"
              f"{mean(shuffled['structure(co+trans+tree)'][pop]['mrr']):7.3f}"
              "   control: the structure arm with its tables built on shuffled order")

    if args.out:
        json.dump(
            {"steps_all": steps_all, "steps_switch": steps_switch,
             "arms": {a: {p: {m: per_arm[a][p][m] for m in per_arm[a][p]} for p in ("all", "switch")}
                      for a in ARMS},
             "shuffled_structure": {p: {m: shuffled["structure(co+trans+tree)"][p][m]
                                        for m in shuffled["structure(co+trans+tree)"][p]}
                                    for p in ("all", "switch")}},
            open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
