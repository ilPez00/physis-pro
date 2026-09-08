#!/usr/bin/env python3
"""Score a Disposer Bench run against the labels held back from the UI.

The bench elicits; this measures. Nothing here is visible to the rater, and the
gold file is never loaded by the page.

WHAT THIS CAN AND CANNOT ESTABLISH
----------------------------------
The worklist mixes two populations and they support different claims:

  INJECTED  deterministically misfiled by the Stage 12 rule. Certain positives,
            so the rater's RECALL is exact and their AUC is directly comparable
            to the machine's on the same items.
  NATIVE    the corpus's own filing. Ground truth UNKNOWN. A rater flag here is
            a real misfiling CANDIDATE -- the thing this track has never had --
            and it cannot be counted as a false positive, because roughly 16% of
            native filings are expected to be wrong after 0.1.21.

So: recall is measured, precision is bounded, and the native flags are output
rather than scored. Reporting a "false positive rate" over native items would be
the exact error Stage 7 caught -- treating an unlabelled corpus as ground truth.

USAGE
-----
  # judgements pulled with the Artifact read_db tool into a directory:
  python3 scripts/score_rerank.py --judgements <dir>

  # or the raw JSON the bench's own textarea produces:
  python3 scripts/score_rerank.py --judgements run.json
"""
import argparse
import json
import os
import sys

GOLD = "research/perspective-discovery/rerank_gold.json"
TASK = "research/perspective-discovery/rerank_task.json"


def load_judgements(path):
    """A directory of one-doc-per-file (read_db --out_dir), or one JSON object."""
    if os.path.isdir(path):
        out = {}
        for root, _, files in os.walk(path):
            for f in files:
                if not f.endswith(".json"):
                    continue
                with open(os.path.join(root, f)) as fh:
                    out[f[:-5]] = json.load(fh)
        return out
    with open(path) as fh:
        raw = json.load(fh)
    # tolerate {id: {...}} and {"documents": [{"id":..., "data": {...}}]}
    if isinstance(raw, dict) and "documents" in raw:
        return {d["id"]: d.get("data", d) for d in raw["documents"]}
    return raw


def auc(scored):
    """Rank-sum AUC with ties averaged -- the same function the Rust harness uses."""
    v = sorted(scored, key=lambda x: x[0])
    pos = sum(1 for x in v if x[1])
    neg = len(v) - pos
    if pos == 0 or neg == 0:
        return float("nan")
    rank_sum, i = 0.0, 0
    while i < len(v):
        j = i
        while j + 1 < len(v) and v[j + 1][0] == v[i][0]:
            j += 1
        avg = (i + 1 + j + 1) / 2.0
        for x in v[i:j + 1]:
            if x[1]:
                rank_sum += avg
        i = j + 1
    return (rank_sum - pos * (pos + 1) / 2.0) / (pos * neg)


def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * ((p * (1 - p) / n + z * z / (4 * n * n)) ** 0.5) / d
    return (max(0.0, c - h), min(1.0, c + h))


def bar(label, value, width=34, lo=0.0, hi=1.0):
    frac = 0 if value != value else max(0.0, min(1.0, (value - lo) / (hi - lo)))
    n = int(round(frac * width))
    return f"  {label:<34} {'#' * n}{'.' * (width - n)}  {value:.3f}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--judgements", required=True)
    ap.add_argument("--gold", default=GOLD)
    ap.add_argument("--task", default=TASK)
    a = ap.parse_args()

    gold = json.load(open(a.gold))["labels"]
    task = {i["id"]: i for i in json.load(open(a.task))["items"]}
    J = load_judgements(a.judgements)
    J = {k: v for k, v in J.items() if isinstance(v, dict) and v.get("verdict")}

    if not J:
        sys.exit("no judgements found -- nothing to score")

    print("Disposer Bench -- scored against the held-back labels\n")
    print(f"  judged {len(J)} of {len(gold)} items"
          f"   ({100.0 * len(J) / len(gold):.0f}% of the worklist)\n")

    # verdict -> a suspicion score, so the rater ranks on the same scale as the machine
    score = {"wrong": 1.0, "unsure": 0.5, "belongs": 0.0}

    inj = [i for i in J if gold[i]["injected"]]
    nat = [i for i in J if not gold[i]["injected"]]

    # ── 1. the measured half: recall on certain positives ──────────────
    caught = [i for i in inj if J[i]["verdict"] == "wrong"]
    held = [i for i in inj if J[i]["verdict"] == "unsure"]
    missed = [i for i in inj if J[i]["verdict"] == "belongs"]
    rec = len(caught) / len(inj) if inj else float("nan")
    lo, hi = wilson(len(caught), len(inj))
    print("── 1. Recall on the known-wrong items (this is exact) ──────────────\n")
    print(f"  injected in the judged set : {len(inj)}")
    print(f"  called wrong               : {len(caught)}   <- recall {rec:.3f}  95% CI [{lo:.3f}, {hi:.3f}]")
    print(f"  called unsure              : {len(held)}")
    print(f"  called belongs (missed)    : {len(missed)}\n")

    # ── 2. human vs machine, same items, same metric ────────────────────
    hs = [(score[J[i]["verdict"]], gold[i]["injected"]) for i in J]
    ms = [(-task[i]["fit"], gold[i]["injected"]) for i in J]  # lower fit = more suspect
    ha, ma = auc(hs), auc(ms)
    print("── 2. Human against machine, on the same judged items ──────────────\n")
    print(bar("human verdict", ha))
    print(bar("machine (BGE cosine)", ma))
    print(f"\n  difference: {ha - ma:+.3f}"
          f"   ({'the human ranks better' if ha > ma else 'the machine ranks better' if ma > ha else 'tied'})")
    print("  (the machine's AUC here is over a worklist it already selected, so it is not"
          "\n   comparable to its 0.866 over the whole corpus -- only to the human's, above.)\n")

    # ── 3. the operational question: does human filtering improve the head? ──
    head = [i for i in J if gold[i]["in_head"]]
    tail = [i for i in J if not gold[i]["in_head"]]
    hp = sum(1 for i in head if gold[i]["injected"]) / len(head) if head else float("nan")
    kept = [i for i in head if J[i]["verdict"] == "wrong"]
    kp = sum(1 for i in kept if gold[i]["injected"]) / len(kept) if kept else float("nan")
    print("── 3. Machine proposes, human disposes ─────────────────────────────\n")
    print(f"  machine's worklist head    : {len(head)} items, precision {hp:.3f}")
    print(f"  after the human filters it : {len(kept)} items, precision {kp:.3f}")
    if kept and head:
        print(f"  lift from the human        : {kp - hp:+.3f}")
        print(f"  positives retained         : {sum(1 for i in kept if gold[i]['injected'])}"
              f" of {sum(1 for i in head if gold[i]['injected'])}")
    print()

    # ── 4. is the rater just flagging everything? ───────────────────────
    tn = [i for i in tail if not gold[i]["injected"]]
    tf = [i for i in tn if J[i]["verdict"] == "wrong"]
    print("── 4. The control tail (uniformly sampled, mostly native) ──────────\n")
    if tn:
        r = len(tf) / len(tn)
        lo, hi = wilson(len(tf), len(tn))
        print(f"  native items in the tail   : {len(tn)}")
        print(f"  flagged wrong              : {len(tf)}   -> {r:.3f}  95% CI [{lo:.3f}, {hi:.3f}]")
        print(f"  corpus prior after 0.1.21  : ~0.162")
        if lo > 0.45:
            print("  -> the rater flags most of what they see; treat the recall above as inflated.")
        elif hi < 0.162:
            print("  -> flags below the corpus prior: the rater is conservative, or the")
            print("     regeneration left the corpus cleaner than the proxy suggested.")
        else:
            print("  -> consistent with the corpus prior; the recall above is not just")
            print("     a rater who says wrong to everything.")
    else:
        print("  no native tail items judged yet")
    print()

    # ── 5. anchoring check ──────────────────────────────────────────────
    rev = [i for i in J if J[i].get("revealed")]
    print("── 5. Anchoring: did seeing the machine's score change the rater? ──\n")
    print(f"  items where the machine was revealed: {len(rev)} of {len(J)}")
    if 3 <= len(rev) <= len(J) - 3:
        bl = [i for i in J if not J[i].get("revealed")]
        for name, grp in (("blind", bl), ("revealed", rev)):
            gi = [i for i in grp if gold[i]["injected"]]
            r = sum(1 for i in gi if J[i]["verdict"] == "wrong") / len(gi) if gi else float("nan")
            print(f"    {name:<9} n={len(grp):<3} recall on injected {r:.3f}  (n={len(gi)})")
        print("  (small n -- read this as a flag for a rerun, not as a result.)")
    elif not rev:
        print("  none revealed: the whole run is blind, which is the clean condition.")
    else:
        print("  too few in one arm to compare.")
    print()

    # ── 6. can the rater repair, not only detect? ───────────────────────
    tg = [i for i in inj if J[i]["verdict"] == "wrong" and J[i].get("target")]
    ok = [i for i in tg if J[i]["target"] == gold[i]["true_cell"]]
    print("── 6. Repair: when the rater named a destination, was it right? ────\n")
    if tg:
        print(f"  injected items with a target named : {len(tg)}")
        print(f"  target matched the true cell       : {len(ok)}  ({len(ok) / len(tg):.3f})")
        print("  (detection is the easier half; this is the one that would let a")
        print("   human-in-the-loop actually re-file rather than only reject.)")
    else:
        print("  no destinations named on injected items -- nothing to score.")
    print()

    # ── 7. the deliverable ──────────────────────────────────────────────
    flagged = sorted((i for i in nat if J[i]["verdict"] == "wrong"),
                     key=lambda i: task[i]["fit"])
    unsure = [i for i in nat if J[i]["verdict"] == "unsure"]
    print("── 7. Real misfiling candidates (native filings, no ground truth) ──\n")
    print(f"  flagged: {len(flagged)}   unsure: {len(unsure)}   of {len(nat)} native items judged\n")
    for i in flagged:
        t = task[i]
        tgt = J[i].get("target")
        print(f"    {t['name'][:44]:<44} {t['cell']:<20} fit {t['fit']:.3f}"
              + (f"  -> {tgt}" if tgt else ""))
    if not flagged:
        print("    (none)")
    print("\n  These are the run's actual output. Nothing in this track has produced")
    print("  labelled real misfilings before; Stage 9 measured the machine at AUC 0.410")
    print("  on exactly this population, which is why a person had to look.\n")

    out = {
        "n_judged": len(J),
        "recall_injected": rec,
        "auc_human": ha,
        "auc_machine_on_worklist": ma,
        "head_precision_machine": hp,
        "head_precision_after_human": kp,
        "tail_native_flag_rate": (len(tf) / len(tn)) if tn else None,
        "repair_correct": (len(ok) / len(tg)) if tg else None,
        "native_flagged": [{"id": i, "name": task[i]["name"], "cell": task[i]["cell"],
                            "target": J[i].get("target")} for i in flagged],
    }
    dest = "research/perspective-discovery/rerank_result.json"
    with open(dest, "w") as fh:
        json.dump(out, fh, indent=1)
    print(f"wrote {dest}")


if __name__ == "__main__":
    main()
