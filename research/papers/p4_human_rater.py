#!/usr/bin/env python3
"""P4: human rater worksheet (blind, resumable).

You are the independent rater. The point is to measure how much of a filing is
the entry versus the rater, and then to test whether geometry fitted on one
rater's labels predicts a DIFFERENT rater's filing.

BLINDNESS: this shows the entry and the source vocabulary only. It does not show
the original cell, rater 1's cell, either model's answer, or any geometry
proposal. That is checked, not assumed -- see --dry-run.

WHY A RANDOM SAMPLE, NOT THE STRATIFIED ONE THE MODELS GOT: the model runs drew
one item per rater-1 cell, which forces rater 1 to be uniform over cells, drives
chance agreement down to ~1/57 and inflates kappa. A plain random sample avoids
that, so your agreement number is comparable to the deepseek arm (which used the
full corpus) rather than to the small-model arm.

USAGE

    python3 research/papers/p4_human_rater.py --n 150

At each item, type the NUMBER of the cell (or the cell itself). Commands:

    l   re-print the cell list
    s   skip this item
    q   save and quit (progress is kept; rerun to resume)

Every answer is appended to disk immediately, so quitting or a crash costs at
most the item you were on.
"""

import argparse
import json
import os
import random
import sys

REPO = "/home/gio/dev/physis-pro"
GATE = os.path.join(REPO, "research", "perspective-discovery", "refiling_gate0.tsv")
OUT = os.path.join(REPO, "research", "p4_human.json")
PARTIAL = os.path.join(REPO, "research", "p4_human_partial.jsonl")
SEED = 20260924


def load_gate():
    rows = []
    with open(GATE) as fh:
        for line in fh:
            if line.startswith("#") or not line.strip():
                continue
            f = line.rstrip("\n").split("\t")
            if f[0] == "n" or len(f) < 7:
                continue
            rows.append({"n": int(f[0]), "source": f[1], "entry": f[2],
                         "old_cell": f[3], "rater1_cell": f[4]})
    return rows


def load_cells(rows):
    return sorted({r["rater1_cell"] for r in rows})


def already_done():
    if not os.path.exists(PARTIAL):
        return {}
    out = {}
    with open(PARTIAL) as fh:
        for line in fh:
            try:
                r = json.loads(line)
            except ValueError:
                continue
            out[r["n"]] = r
    return out


def show_cells(cells):
    print()
    width = max(len(c) for c in cells) + 2
    percol = 3
    for i in range(0, len(cells), percol):
        chunk = cells[i:i + percol]
        print("  " + "".join(
            f"{i + j + 1:>3}. {c:<{width}}" for j, c in enumerate(chunk)))
    print()


def prompt_text(row):
    """The precise text put in front of the human. Separate so blindness is
    asserted on what is actually displayed."""
    return f"Entry:  {row['entry']}\nSource: {row['source']}"


def resolve(ans, cells):
    a = ans.strip()
    if not a:
        return None
    if a.isdigit():
        i = int(a)
        return cells[i - 1] if 1 <= i <= len(cells) else None
    for c in cells:
        if c.upper().replace(" ", "") == a.upper().replace(" ", ""):
            return c
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=150)
    ap.add_argument("--dry-run", action="store_true",
                    help="print the sample and run the blindness check only")
    a = ap.parse_args()

    rows = load_gate()
    cells = load_cells(rows)
    rng = random.Random(SEED)
    sample = rows[:]
    rng.shuffle(sample)
    sample = sample[:a.n]

    # blindness check on the real displayed payload: no label may appear anywhere
    # the human can see. Cheap, and it is the one property that makes this rater
    # worth anything.
    leaks = 0
    for s in sample:
        shown = prompt_text(s)
        for label in (s["old_cell"], s["rater1_cell"]):
            if label and label in shown:
                leaks += 1
    print(f"corpus: {len(rows)} entries, {len(cells)} cells")
    print(f"sample: {len(sample)} entries (random, seed {SEED})")
    print(f"blindness check: {len(sample) * 2} checks, {leaks} leaks")
    if leaks:
        print("REFUSING: the displayed text leaks a label.")
        return 1

    if a.dry_run:
        print("\nfirst 3 items as the human will see them:")
        for s in sample[:3]:
            print("  " + prompt_text(s).replace("\n", "  |  "))
        return 0

    done = already_done()
    if done:
        print(f"resuming: {len(done)} items already rated")

    print("\nCells (type the NUMBER, or the cell itself):")
    show_cells(cells)
    print("  l = list cells again   s = skip   q = save & quit\n")

    todo = [s for s in sample if s["n"] not in done]
    with open(PARTIAL, "a") as fh:
        for k, s in enumerate(todo, 1):
            print(f"=== item {len(done) + k}/{len(sample)}  (n={s['n']}) ===")
            print(prompt_text(s))
            while True:
                try:
                    ans = input("Cell #> ").strip()
                except (EOFError, KeyboardInterrupt):
                    print("\n(saved)")
                    return _finish(sample, cells)
                if ans.lower() == "q":
                    return _finish(sample, cells)
                if ans.lower() == "l":
                    show_cells(cells)
                    continue
                if ans.lower() == "s":
                    break
                cell = resolve(ans, cells)
                if cell is None:
                    print("  (not a valid cell -- number 1-%d, a cell name, l, s or q)"
                          % len(cells))
                    continue
                row = {"n": s["n"], "entry": s["entry"], "source": s["source"],
                       "human_cell": cell}
                fh.write(json.dumps(row) + "\n")
                fh.flush()
                print(f"  -> {cell}\n")
                break
    return _finish(sample, cells)


def _finish(sample, cells):
    done = already_done()
    rows = [done[s["n"]] for s in sample if s["n"] in done]
    art = {"rater": "human", "seed": SEED, "n_sampled": len(sample),
           "n_rated": len(rows), "n_cells": len(cells), "rows": rows,
           "reproduce": "python3 research/papers/p4_human_rater.py --n %d"
                        % len(sample),
           "claim_boundary": ("A single human rater is one rater. This measures "
                              "whether geometry fitted on rater 1's labels predicts "
                              "a different rater's filing; it does not establish "
                              "ontology truth. Keep saying 'rater-assigned cell'.")}
    with open(OUT, "w") as fh:
        json.dump(art, fh, indent=1)
    print(f"\nrated {len(rows)}/{len(sample)}. wrote {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
