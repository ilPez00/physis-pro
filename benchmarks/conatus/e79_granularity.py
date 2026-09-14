#!/usr/bin/env python3
"""E79 — scan one object at several granularities, with position as metadata.

E78 gave each revision of an object a single vector and watched the set move
with time. That throws away two axes the proposal asks for: **granularity**
(document / section / paragraph / window) and **position in the document**.
This scans all of them, tags every vector with its metadata, and asks what the
field looks like along each axis.

Three questions, each able to fail:

  A. NESTING      does a coarser vector sit near the mean of the finer vectors
                  it contains? If not, granularities are separate spaces and
                  cannot be compared, let alone mixed in one index.
  B. POSITION     does distance from the document's opening grow with position
                  in the text, beyond a shuffled-position null? This is the
                  spatial analogue of E78's temporal drift: a document that
                  develops has a direction along its own length.
  C. SCALE        how much of the variance at each granularity is the object's
                  identity, and how much is internal variation? A granularity
                  where between-object distance stops exceeding within-object
                  distance is a granularity at which the object has dissolved.

Theory anchors this is built to serve, in the order they were named: Allen's
interval relations need *intervals*, which is what a section or paragraph is;
Reichenbach's three times need event time separable from ingestion time, which
is what `meta` carries; Vendler's aspectual classes predict different drift
signatures per granularity. None of those are tested here — this builds the
instrument they need and measures that it holds together.

    python3 benchmarks/conatus/e79_granularity.py --file docs/WHAT_PHYSIS_IS.md
"""
import argparse, json, math, os, random, statistics, subprocess, sys

EXAMPLE = os.path.expanduser("~/dev/physis-pro/target/release/examples/reference_spectrum")
MODEL = os.path.expanduser("~/dev/physis-pro/models/bge-base-en-v1.5")

# Granularities, coarse to fine. "section" splits on markdown headings or Rust
# item boundaries; "window" is a fixed character window, the unit a retriever
# would actually index.
WINDOW_CHARS = 700


def read(path):
    with open(path, encoding="utf-8", errors="replace") as fh:
        return fh.read()


def sections(text):
    out, current = [], []
    for line in text.splitlines(keepends=True):
        if (line.startswith("#") or line.startswith("pub fn ") or line.startswith("impl ")) and current:
            out.append("".join(current))
            current = [line]
        else:
            current.append(line)
    if current:
        out.append("".join(current))
    return [s for s in out if s.strip()]


def paragraphs(text):
    return [p for p in text.split("\n\n") if p.strip()]


def windows(text, size=WINDOW_CHARS):
    return [text[i:i + size] for i in range(0, len(text), size) if text[i:i + size].strip()]


def units(text):
    """Every granularity, each unit tagged with where it starts in the document."""
    out = []
    out.append(("document", 0, 0.0, text))
    for level, pieces in (("section", sections(text)),
                          ("paragraph", paragraphs(text)),
                          ("window", windows(text))):
        cursor = 0
        for index, piece in enumerate(pieces):
            start = text.find(piece, cursor)
            if start < 0:
                start = cursor
            cursor = start + len(piece)
            out.append((level, index, start / max(1, len(text)), piece))
    return out


def embed(records):
    """records: [(level, index, position, text)] -> (label, vectors)"""
    lines = [json.dumps({"x": position, "text": text[:6000],
                         "meta": {"level": level, "index": index}})
             for level, index, position, text in records]
    env = dict(os.environ, PHYSIS_MODEL_DIR=MODEL, PHYSIS_MODEL_DIM="768",
               PHYSIS_MODEL_MAX_LEN="512")
    p = subprocess.run([EXAMPLE], input="\n".join(lines), capture_output=True,
                       text=True, env=env, timeout=1800)
    if p.returncode != 0 or not p.stdout.strip():
        sys.exit(f"embedding failed:\n{p.stderr[:400]}")
    data = json.loads(p.stdout)
    return data["embedder"], data["vectors"]


def cosine_distance(a, b):
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(x * x for x in b))
    return 1.0 - (dot / (na * nb) if na and nb else 0.0)


def mean_vector(vectors):
    n = len(vectors)
    return [sum(v[i] for v in vectors) / n for i in range(len(vectors[0]))]


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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--file", default=os.path.expanduser("~/dev/physis-pro/docs/WHAT_PHYSIS_IS.md"))
    ap.add_argument("--other", default=os.path.expanduser("~/dev/physis-pro/docs/SYSTEM_INTERFACE.md"),
                    help="near control: a different document on the same subject")
    # Both defaults are physis documents, so "between-object" distance is
    # between two texts about the same system. Without a topically distant
    # third text there is no scale for what 'far' means, and a ratio of 1.1 gets
    # read as dissolution when it may be two documents that genuinely overlap.
    ap.add_argument("--far", default=None,
                    help="far control: a text about something else entirely")
    ap.add_argument("--permutations", type=int, default=2000)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    records = units(read(args.file))
    other_records = units(read(args.other))
    label, vectors = embed(records)
    _, other_vectors = embed(other_records)
    far_by_level = {}
    if args.far:
        far_records = units(read(args.far))
        _, far_vectors = embed(far_records)
        for (level, index, position, _), vector in zip(far_records, far_vectors):
            far_by_level.setdefault(level, []).append((index, position, vector))
    by_level = {}
    for (level, index, position, _), vector in zip(records, vectors):
        by_level.setdefault(level, []).append((index, position, vector))
    other_by_level = {}
    for (level, index, position, _), vector in zip(other_records, other_vectors):
        other_by_level.setdefault(level, []).append((index, position, vector))

    print(f"object: {os.path.basename(args.file)} · embedder {label}")
    print("units per granularity: " +
          ", ".join(f"{lvl}={len(v)}" for lvl, v in by_level.items()))
    if label.startswith("random"):
        print("STOP: random-projection fallback — nothing below is about meaning.")
        return

    report = {"file": args.file, "embedder": label, "levels": {}}

    print("\nA. NESTING — is a coarse vector near the mean of the fine ones it contains?")
    document = by_level["document"][0][2]
    for level in ("section", "paragraph", "window"):
        fine = [v for _, _, v in by_level[level]]
        pooled = mean_vector(fine)
        to_pooled = cosine_distance(document, pooled)
        # The scale to beat: the same distance measured against another
        # document's pooled vector at the same granularity.
        other_pooled = mean_vector([v for _, _, v in other_by_level[level]])
        to_other = cosine_distance(document, other_pooled)
        print(f"   {level:10} document↔pooled {to_pooled:.4f} · "
              f"document↔other-doc pooled {to_other:.4f} · "
              f"{'nested' if to_pooled < to_other else 'NOT NESTED'}")
        report["levels"].setdefault(level, {})["nesting"] = [to_pooled, to_other]

    print("\nB. POSITION — does the text have a direction along its own length?")
    rng = random.Random(0)
    for level in ("section", "paragraph", "window"):
        items = sorted(by_level[level])
        if len(items) < 5:
            print(f"   {level:10} NOT MEASURED ({len(items)} unit(s))")
            continue
        first = items[0][2]
        positions = [p for _, p, _ in items]
        drift = [cosine_distance(first, v) for _, _, v in items]
        rho = spearman(positions, drift)
        null = []
        for _ in range(args.permutations):
            shuffled = drift[:]
            rng.shuffle(shuffled)
            null.append(spearman(positions, shuffled))
        null.sort()
        lo, hi = null[int(0.025 * len(null))], null[int(0.975 * len(null))]
        beats = rho > hi or rho < lo
        print(f"   {level:10} ρ(position, drift) {rho:+.3f} · null [{lo:+.3f}, {hi:+.3f}] · "
              f"{'directed' if beats else 'no direction'}")
        report["levels"].setdefault(level, {})["position"] = [rho, lo, hi]

    print("\nC. SCALE — at which granularity does the object stop being distinguishable?")
    for level in ("section", "paragraph", "window"):
        mine = [v for _, _, v in by_level[level]]
        theirs = [v for _, _, v in other_by_level[level]]
        within = [cosine_distance(mine[i], mine[j])
                  for i in range(len(mine)) for j in range(i + 1, len(mine))]
        between = [cosine_distance(a, b) for a in mine for b in theirs]
        ratio = statistics.mean(between) / statistics.mean(within) if within else float("nan")
        line = (f"   {level:10} within {statistics.mean(within):.4f} · "
                f"near {statistics.mean(between):.4f} (ratio {ratio:.2f})")
        if far_by_level:
            far = [v for _, _, v in far_by_level.get(level, [])]
            far_d = [cosine_distance(a, b) for a in mine for b in far]
            far_ratio = statistics.mean(far_d) / statistics.mean(within) if within else float("nan")
            line += f" · far {statistics.mean(far_d):.4f} (ratio {far_ratio:.2f})"
            report["levels"].setdefault(level, {})["far"] = [statistics.mean(far_d), far_ratio]
        print(line)
        report["levels"].setdefault(level, {})["scale"] = [
            statistics.mean(within), statistics.mean(between), ratio]

    if args.out:
        json.dump(report, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
