#!/usr/bin/env python3
"""E80 — does context matching per directory recover what a project *is*?

The proposal: a directory holding two android apps, two webapps, some PDFs and
videos should sort itself out by content — an ontology induced per directory,
recursively — so that search and fast diffs work across siblings without anyone
declaring a schema.

That is falsifiable at the level that matters: if content embeddings alone
recover the project kind, the induced grouping is doing work; if they do not,
the directory listing is doing the work and the embedding is decoration.

Ground truth comes from build signatures, never from the text the embedder
sees: `build.gradle*` = android, `Cargo.toml` = rust, `package.json` without
gradle = webapp, `*.py` majority with none of the above = python. Labels are
assigned before any embedding happens.

Two questions:

  PROJECT-LEVEL  leave one project out: is its nearest neighbour (cosine, over
                 the pooled vector of its sampled files) a project of the same
                 kind? Against a label-permuted null.
  FILE-LEVEL     the same test per file, which is the granularity a retriever
                 would actually index.

    python3 benchmarks/conatus/e80_directory_ontology.py
"""
import argparse, json, math, os, random, statistics, subprocess, sys

EXAMPLE = os.path.expanduser("~/dev/physis-pro/target/release/examples/reference_spectrum")
MODEL = os.path.expanduser("~/dev/physis-pro/models/bge-base-en-v1.5")
DEV = os.path.expanduser("~/dev")

PROJECTS = {
    "praxis/praxis-android": "android",
    "agora": "android",
    "physis-pro/physis-core": "rust",
    "physis-global": "rust",
    "aion": "python",
    "llm-wiki-rag": "python",
    "trustgraph": "python",
    "multiagent": "python",
    "praxis/praxis_webapp": "webapp",
    "repowise": "webapp",
}
TEXT_EXT = (".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".kt", ".java", ".md",
            ".toml", ".json", ".gradle", ".yml", ".yaml", ".sh")
SKIP = ("node_modules", "target", ".git", "build", "dist", "vendor", "__pycache__",
        ".venv", "venv")
FILES_PER_PROJECT = 14
CHARS = 3000


def sample_files(root, limit, seed=0):
    found = []
    for base, dirs, names in os.walk(root):
        dirs[:] = [d for d in dirs if d not in SKIP and not d.startswith(".")]
        for name in names:
            if name.endswith(TEXT_EXT):
                found.append(os.path.join(base, name))
        if len(found) > 4000:
            break
    rng = random.Random(seed)
    rng.shuffle(found)
    picked = []
    for path in found:
        try:
            with open(path, encoding="utf-8", errors="replace") as fh:
                text = fh.read(CHARS)
        except OSError:
            continue
        if len(text.strip()) > 200:
            picked.append((path, text))
        if len(picked) >= limit:
            break
    return picked


def embed(records):
    lines = [json.dumps({"x": i, "text": text, "meta": {"path": path}})
             for i, (path, text) in enumerate(records)]
    env = dict(os.environ, PHYSIS_MODEL_DIR=MODEL, PHYSIS_MODEL_DIM="768",
               PHYSIS_MODEL_MAX_LEN="512")
    p = subprocess.run([EXAMPLE], input="\n".join(lines), capture_output=True,
                       text=True, env=env, timeout=3600)
    if p.returncode != 0 or not p.stdout.strip():
        sys.exit(f"embedding failed:\n{p.stderr[:400]}")
    data = json.loads(p.stdout)
    return data["embedder"], data["vectors"]


def cosine(a, b):
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(x * x for x in b))
    return dot / (na * nb) if na and nb else 0.0


def pooled(vectors):
    n = len(vectors)
    return [sum(v[i] for v in vectors) / n for i in range(len(vectors[0]))]


def nn_accuracy(vectors, labels):
    """Leave-one-out: is the nearest other item's label the same?"""
    hits = 0
    for i in range(len(vectors)):
        best, best_score = None, -2.0
        for j in range(len(vectors)):
            if i == j:
                continue
            score = cosine(vectors[i], vectors[j])
            if score > best_score:
                best, best_score = j, score
        hits += labels[best] == labels[i]
    return hits / len(vectors)


def permutation_null(vectors, labels, rounds, seed=0):
    rng = random.Random(seed)
    out = []
    for _ in range(rounds):
        shuffled = labels[:]
        rng.shuffle(shuffled)
        out.append(nn_accuracy(vectors, shuffled))
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
            print(f"skipped ({len(picked)} usable file(s)): {project}", file=sys.stderr)
            continue
        present[project] = kind
        for path, text in picked:
            records.append((path, text))
            file_labels.append(kind)
            project_of.append(project)

    if len(present) < 4:
        sys.exit(f"{len(present)} project(s) usable: nothing to group")

    label, vectors = embed(records)
    print(f"{len(present)} projects · {len(records)} files · "
          f"{len(set(present.values()))} kinds · embedder {label}")
    for kind in sorted(set(present.values())):
        names = [p for p, k in present.items() if k == kind]
        print(f"   {kind:8} {len(names)} project(s): {', '.join(names)}")
    if label.startswith("random"):
        print("STOP: random-projection fallback — this would be a lexical hash, not content.")
        return

    project_vectors, project_labels, names = [], [], []
    for project, kind in present.items():
        mine = [v for v, p in zip(vectors, project_of) if p == project]
        project_vectors.append(pooled(mine))
        project_labels.append(kind)
        names.append(project)

    print("\nPROJECT LEVEL — leave one out, nearest neighbour by cosine")
    acc = nn_accuracy(project_vectors, project_labels)
    lo, hi, mean = permutation_null(project_vectors, project_labels, args.permutations)
    print(f"   same-kind nearest neighbour: {acc:.3f} "
          f"({round(acc * len(project_vectors))}/{len(project_vectors)})")
    print(f"   label-permuted null: mean {mean:.3f}, 95% band [{lo:.3f}, {hi:.3f}]")
    print(f"   {'BEATS the null' if acc > hi else 'inside the null — content adds nothing here'}")
    for i, name in enumerate(names):
        best = max((j for j in range(len(names)) if j != i),
                   key=lambda j: cosine(project_vectors[i], project_vectors[j]))
        mark = "ok " if project_labels[best] == project_labels[i] else "MISS"
        print(f"     {mark} {name:24} → {names[best]:24} "
              f"({project_labels[i]} → {project_labels[best]})")

    print("\nPROJECT LEVEL BY VOTE — each file votes with its nearest neighbour")
    # Pooling a project into one vector averages away what distinguishes it
    # (E79 found the same: a coarse vector is only weakly its own summary).
    # The recursion the proposal wants runs the other way: fine units are
    # classified, and the directory takes the majority. Cross-project only —
    # a file's nearest neighbour inside its own project would vote for itself.
    votes_right = 0
    for project, kind in present.items():
        tally = {}
        for i, (v, p_of) in enumerate(zip(vectors, project_of)):
            if p_of != project:
                continue
            best = max((j for j in range(len(vectors)) if project_of[j] != project),
                       key=lambda j: cosine(v, vectors[j]))
            tally[file_labels[best]] = tally.get(file_labels[best], 0) + 1
        winner = max(tally, key=tally.get) if tally else None
        votes_right += winner == kind
        share = (tally.get(kind, 0) / sum(tally.values())) if tally else 0.0
        print(f"     {'ok ' if winner == kind else 'MISS'} {project:24} "
              f"voted {winner!s:8} (true {kind}, {share:.0%} of its files voted true)")
    vote_acc = votes_right / len(present)
    print(f"   majority-vote accuracy {vote_acc:.3f} ({votes_right}/{len(present)}) "
          f"against a 4-kind chance of ~0.25 and the pooled arm's {acc:.3f}")

    print("\nFILE LEVEL — the granularity a retriever would index")
    facc = nn_accuracy(vectors, file_labels)
    flo, fhi, fmean = permutation_null(vectors, file_labels, min(args.permutations, 200))
    print(f"   same-kind nearest neighbour: {facc:.3f} over {len(vectors)} files")
    print(f"   label-permuted null: mean {fmean:.3f}, 95% band [{flo:.3f}, {fhi:.3f}]")
    print(f"   {'BEATS the null' if facc > fhi else 'inside the null'}")

    if args.out:
        json.dump({"embedder": label, "projects": present, "vote_accuracy": vote_acc,
                   "project_accuracy": acc, "project_null": [lo, hi],
                   "file_accuracy": facc, "file_null": [flo, fhi],
                   "files": len(records)}, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
