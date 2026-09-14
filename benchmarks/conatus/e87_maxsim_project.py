#!/usr/bin/env python3
"""E87 — MaxSim: does max-over-file scoring recover what pooling destroyed?

E80 measured the pooling damage at project level: file-level nearest neighbour
sorts a directory at 0.643 (well above the null), but pooling each project's
sampled files into one mean vector drops project-level accuracy to 0.400 —
inside the null. Averaged over its files, every project converges on "a
software project".

The retrieval literature's answer to pooling is late interaction (ColBERT's
MaxSim): score two things by the BEST-matching pair of their parts, not the
cosine of their averages. This run re-uses E80's sampling, embedding and
ground truth unchanged and adds one arm:

  MaxSim    score(P, Q) = max cosine(u, v) over files u of P, v of Q.
            Leave one project out, nearest neighbour by MaxSim.

Null: the same scoring under label permutation (the score matrix is computed
once; only the labels move, exactly as in E80's pooled null).

    python3 benchmarks/conatus/e87_maxsim_project.py --out e87-results.json
"""
import argparse, json, math, os, random, statistics, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from e80_directory_ontology import (  # noqa: E402
    PROJECTS, sample_files, embed, cosine, pooled, nn_accuracy,
    permutation_null, DEV, FILES_PER_PROJECT,
)


def maxsim_block(mine, others):
    best = -2.0
    for u in mine:
        for v in others:
            c = cosine(u, v)
            if c > best:
                best = c
    return best


def maxsim_nn_accuracy(mine_map, names, labels):
    hits = 0
    for i, p in enumerate(names):
        best, best_score = None, -2.0
        for j, q in enumerate(names):
            if i == j:
                continue
            score = maxsim_block(mine_map[p], mine_map[q])
            if score > best_score:
                best, best_score = j, score
        hits += labels[best] == labels[i]
    return hits / len(names)


def maxsim_null(mine_map, names, labels, rounds, seed=0):
    """Score matrix once, then permute the project labels."""
    n = len(names)
    scores = [[0.0] * n for _ in range(n)]
    for i, p in enumerate(names):
        for j, q in enumerate(names):
            if i != j:
                scores[i][j] = maxsim_block(mine_map[p], mine_map[q])
    rng = random.Random(seed)
    out = []
    for _ in range(rounds):
        shuffled = labels[:]
        rng.shuffle(shuffled)
        hits = 0
        for i in range(n):
            best, best_score = None, -2.0
            for j in range(n):
                if i != j and scores[i][j] > best_score:
                    best, best_score = j, scores[i][j]
            hits += shuffled[best] == shuffled[i]
        out.append(hits / n)
    out.sort()
    return out[int(0.025 * len(out))], out[int(0.975 * len(out))], statistics.mean(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--permutations", type=int, default=500)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    records, file_labels, project_of = [], [], []
    present = {}
    for project, kind in PROJECTS.items():
        root = os.path.join(DEV, project)
        if not os.path.isdir(root):
            print(f"skipped (absent): {project}", file=sys.stderr)
            continue
        picked = sample_files(root, FILES_PER_PROJECT)
        if len(picked) < 4:
            print(f"skipped ({len(picked)} usable): {project}", file=sys.stderr)
            continue
        present[project] = kind
        for path, text in picked:
            records.append((path, text))
            file_labels.append(kind)
            project_of.append(project)
    if len(present) < 4:
        sys.exit(f"{len(present)} project(s) usable")

    label, vectors = embed(records)
    print(f"{len(present)} projects · {len(records)} files · embedder {label}")
    if label.startswith("random"):
        print("STOP: random-projection fallback — lexical hash, not content.")
        return

    mine_map = {p: [v for v, po in zip(vectors, project_of) if po == p]
                for p in present}
    names = list(present)
    proj_labels = [present[p] for p in names]

    print("\nARMS — leave one project out, nearest neighbour")
    pooled_vecs = [pooled(mine_map[p]) for p in names]
    acc_pooled = nn_accuracy(pooled_vecs, proj_labels)
    lo, hi, mean = permutation_null(pooled_vecs, proj_labels, args.permutations)
    print(f"  pooled (E80):  {acc_pooled:.3f}  null mean {mean:.3f}, band [{lo:.3f}, {hi:.3f}] "
          f"{'BEATS' if acc_pooled > hi else 'inside'}")

    acc_max = maxsim_nn_accuracy(mine_map, names, proj_labels)
    mlo, mhi, mmean = maxsim_null(mine_map, names, proj_labels, args.permutations)
    print(f"  MaxSim (E87):  {acc_max:.3f}  null mean {mmean:.3f}, band [{mlo:.3f}, {mhi:.3f}] "
          f"{'BEATS' if acc_max > mhi else 'inside'}")

    for i, p in enumerate(names):
        best, best_score = None, -2.0
        for j, q in enumerate(names):
            if i != j:
                s = maxsim_block(mine_map[p], mine_map[q])
                if s > best_score:
                    best, best_score = j, s
        mark = "ok " if proj_labels[best] == proj_labels[i] else "MISS"
        print(f"     {mark} {p:26} → {names[best]:26} "
              f"({proj_labels[i]} → {proj_labels[best]}, {best_score:.3f})")

    if args.out:
        json.dump({"embedder": label, "files": len(records),
                   "pooled_accuracy": acc_pooled, "pooled_null": [lo, hi],
                   "maxsim_accuracy": acc_max, "maxsim_null": [mlo, mhi],
                   "projects": present}, open(args.out, "w"), indent=1)
        print(f"\nwrote {args.out}")


if __name__ == "__main__":
    main()
