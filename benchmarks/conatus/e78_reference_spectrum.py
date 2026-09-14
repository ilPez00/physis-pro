#!/usr/bin/env python3
"""E78 — do coherent references to the same object form a spectrum that moves?

The proposal under test: log the states of reference to one object, embed each,
and watch how the set moves in the vector field as a function of some variable x
(time, or any metadata axis). If that motion is real, three things must hold and
each can fail:

  1. IDENTITY   states of the same object stay closer to each other than to
                states of a different object — otherwise "same object" is not a
                region of the field at all
  2. DIRECTION  distance from the first state grows with x beyond what a
                shuffled x produces — otherwise the motion is not motion, it is
                scatter with a timestamp attached
  3. STRUCTURE  the trajectory uses few dimensions (a spectrum with a knee),
                rather than filling the space like noise

Object: one source file, its states being its successive git revisions; x is
commit time. A second file provides the between-object scale.

Embeddings come from the engine's own `embed::select` via the
`reference_spectrum` example, so the label (`onnx-*` or `random-projection`) is
carried into the result — on the fallback embedder none of this is about
meaning, and the harness says so rather than reporting the numbers anyway.

    python3 benchmarks/conatus/e78_reference_spectrum.py [--file src/hypothesis.rs]
"""
import argparse, json, math, os, random, subprocess, statistics, sys

REPO = os.path.expanduser("~/dev/physis-pro/physis-core")
EXAMPLE = os.path.expanduser("~/dev/physis-pro/target/release/examples/reference_spectrum")
MODEL = os.path.expanduser("~/dev/physis-pro/models/bge-base-en-v1.5")
# A revision is embedded whole, in chunks, because the first 1200 bytes of a
# source file are its module header and barely change: the first run of this
# harness embedded 12 revisions to the same vector, reported a within-object
# distance of 0.0000, and the "spectrum" was the constant function.
CHUNK_CHARS = 1500
MAX_CHUNKS = 8


def git(args, cwd=REPO):
    return subprocess.run(["git"] + args, cwd=cwd, capture_output=True, text=True).stdout


def revisions(path, limit):
    """(commit time, content) for each revision of one file, oldest first."""
    log = git(["log", "--format=%H %ct", "--", path]).strip().splitlines()
    out = []
    for line in reversed(log[:limit]):
        sha, when = line.split()
        content = git(["show", f"{sha}:{path}"])
        if content.strip():
            out.append((int(when), content))
    return out


def chunks_of(text):
    """The whole revision, in embeddable pieces. Mean-pooling their vectors
    gives a document vector that moves when any part of the document moves."""
    pieces = [text[i:i + CHUNK_CHARS] for i in range(0, len(text), CHUNK_CHARS)]
    return [p for p in pieces if p.strip()][:MAX_CHUNKS]


def embed(records):
    """records: [(x, text)] -> (embedder label, [document vector])

    Each revision is chunked, every chunk embedded, and the chunks mean-pooled
    back into one vector per revision."""
    lines, owner = [], []
    for index, (x, text) in enumerate(records):
        for piece in chunks_of(text):
            lines.append(json.dumps({"x": x, "text": piece}))
            owner.append(index)
    if not lines:
        sys.exit("0 chunks over 0 revisions: nothing was embedded")
    env = dict(os.environ, PHYSIS_MODEL_DIR=MODEL, PHYSIS_MODEL_DIM="768",
               PHYSIS_MODEL_MAX_LEN="512")
    p = subprocess.run([EXAMPLE], input="\n".join(lines), capture_output=True,
                       text=True, env=env, timeout=1800)
    if p.returncode != 0 or not p.stdout.strip():
        sys.exit(f"embedding failed:\n{p.stderr[:400]}")
    data = json.loads(p.stdout)
    pooled = []
    for index in range(len(records)):
        mine = [v for v, o in zip(data["vectors"], owner) if o == index]
        dim = len(mine[0])
        pooled.append([sum(v[i] for v in mine) / len(mine) for i in range(dim)])
    return data["embedder"], pooled


def cosine(a, b):
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(x * x for x in b))
    return dot / (na * nb) if na and nb else 0.0


def distance(a, b):
    return 1.0 - cosine(a, b)


def spearman(xs, ys):
    def rank(v):
        order = sorted(range(len(v)), key=lambda i: v[i])
        r = [0.0] * len(v)
        for pos, i in enumerate(order):
            r[i] = float(pos)
        return r
    rx, ry = rank(xs), rank(ys)
    mx, my = statistics.mean(rx), statistics.mean(ry)
    num = sum((a - mx) * (b - my) for a, b in zip(rx, ry))
    den = math.sqrt(sum((a - mx) ** 2 for a in rx) * sum((b - my) ** 2 for b in ry))
    return num / den if den else 0.0


def gram_spectrum(vectors, top=6):
    """Eigenvalue shares of the centred Gram matrix, by power iteration on the
    deflated matrix — enough to see whether the trajectory has a knee."""
    n = len(vectors)
    mean = [sum(v[i] for v in vectors) / n for i in range(len(vectors[0]))]
    centred = [[x - m for x, m in zip(v, mean)] for v in vectors]
    gram = [[sum(a * b for a, b in zip(u, w)) for w in centred] for u in centred]
    total = sum(gram[i][i] for i in range(n)) or 1.0
    shares, rng = [], random.Random(0)
    for _ in range(min(top, n)):
        vec = [rng.random() for _ in range(n)]
        value = 0.0
        for _ in range(200):
            nxt = [sum(gram[i][j] * vec[j] for j in range(n)) for i in range(n)]
            norm = math.sqrt(sum(x * x for x in nxt))
            if norm < 1e-12:
                break
            vec = [x / norm for x in nxt]
            value = norm
        shares.append(value / total)
        for i in range(n):          # deflate
            for j in range(n):
                gram[i][j] -= value * vec[i] * vec[j]
    return shares


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--file", default="src/hypothesis.rs")
    ap.add_argument("--other", default="src/ontology.rs")
    ap.add_argument("--limit", type=int, default=40)
    ap.add_argument("--permutations", type=int, default=2000)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    states = revisions(args.file, args.limit)
    others = revisions(args.other, args.limit)
    if len(states) < 4:
        sys.exit(f"{len(states)} revision(s) of {args.file}: too few to call anything a spectrum")

    label, vectors = embed(states)
    other_label, other_vectors = embed(others) if others else (label, [])
    xs = [x for x, _ in states]

    # 1. identity
    within = [distance(vectors[i], vectors[j])
              for i in range(len(vectors)) for j in range(i + 1, len(vectors))]
    between = [distance(a, b) for a in vectors for b in other_vectors]

    # 2. direction
    drift = [distance(vectors[0], v) for v in vectors]
    steps = [distance(vectors[i], vectors[i + 1]) for i in range(len(vectors) - 1)]
    rho = spearman(xs, drift)
    rng = random.Random(0)
    null = []
    for _ in range(args.permutations):
        shuffled = drift[:]
        rng.shuffle(shuffled)
        null.append(spearman(xs, shuffled))
    null.sort()
    lo, hi = null[int(0.025 * len(null))], null[int(0.975 * len(null))]
    # If every state sits on top of every other, the correlation and its null
    # are both degenerate and the ±1.000 band that comes out is an artifact of
    # tied ranks, not a result.
    degenerate = statistics.pstdev(drift) < 1e-9

    # 3. structure
    shares = gram_spectrum(vectors)
    displacement = distance(vectors[0], vectors[-1])
    path = sum(steps)

    changed = [abs(len(states[i + 1][1]) - len(states[i][1])) for i in range(len(states) - 1)]
    print(f"object: {args.file} · {len(states)} revision(s) · embedder {label}")
    print(f"input actually varies: median byte delta between revisions "
          f"{statistics.median(changed) if changed else 0:.0f}")
    if label.startswith("random"):
        print("STOP: this is the random-projection fallback — a lexical hash. "
              "Nothing below is about meaning. Set PHYSIS_MODEL_DIR and rebuild "
              "with --features embed-onnx.")
    print(f"between-object scale: {args.other}, {len(other_vectors)} revision(s)\n")

    print("1. IDENTITY — is 'the same object' a region of the field?")
    print(f"   within-object  mean distance {statistics.mean(within):.4f} "
          f"(sd {statistics.pstdev(within):.4f}, n {len(within)})")
    if between:
        print(f"   between-object mean distance {statistics.mean(between):.4f} "
              f"(sd {statistics.pstdev(between):.4f}, n {len(between)})")
        verdict = "YES" if statistics.mean(within) < statistics.mean(between) else "NO"
        print(f"   separated: {verdict}")

    print("\n2. DIRECTION — does the set move with x, or is x decoration?")
    print(f"   Spearman(x, distance from first state) = {rho:+.3f}")
    print(f"   shuffled-x null 95% band [{lo:+.3f}, {hi:+.3f}] over {args.permutations} permutations")
    moved = (rho > hi or rho < lo) and not degenerate
    print(f"   beats its null: "
          f"{'NOT MEASURED (every state has the same vector)' if degenerate else ('YES' if moved else 'NO')}")
    print(f"   path length {path:.4f} vs displacement {displacement:.4f} "
          f"(straightness {displacement / path if path else 0:.3f}; 1.0 = a straight line, "
          f"0 = returns where it started)")

    print("\n3. STRUCTURE — how many dimensions does the trajectory use?")
    print("   eigenvalue shares: " + ", ".join(f"{s:.3f}" for s in shares))
    print(f"   first two carry {sum(shares[:2]):.3f} of the variance")

    if args.out:
        json.dump({
            "file": args.file, "embedder": label, "revisions": len(states),
            "x": xs, "drift": drift, "steps": steps,
            "within_mean": statistics.mean(within),
            "between_mean": statistics.mean(between) if between else None,
            "spearman": rho, "null_band": [lo, hi],
            "path": path, "displacement": displacement, "spectrum": shares,
        }, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
