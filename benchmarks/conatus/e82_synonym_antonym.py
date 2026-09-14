#!/usr/bin/env python3
"""E82 — can the vector field tell a synonym from an antonym?

The symbolic layer is only worth its cost if it carries something the geometry
does not. Synonymy and antonymy are the sharpest case: *increase/growth* and
*increase/decrease* are both about the same subject matter, and a distributional
embedding is trained on exactly that co-occurrence.

Pairs come from WordNet on disk (117791 synsets, 7983 antonym pointers), never
from the embedder:

  synonym    two words of one synset
  antonym    two words joined by the `!` pointer
  hypernym   a word and its `@` parent — a different kind of closeness
  random     two words drawn independently, the floor

Each pair is scored twice: **bare words**, and the same words in a neutral
sentence frame, because bge is a sentence encoder and a bare token is out of
distribution for it. The question at both: given only the cosine distance, can a
reader decide which pairs are synonyms and which are antonyms? Reported as AUC —
0.5 means the geometry cannot tell.

    python3 benchmarks/conatus/e82_synonym_antonym.py --pairs 250
"""
import argparse, json, math, os, random, statistics, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from wordnet import WordNet  # noqa: E402

EXAMPLE = os.path.expanduser("~/dev/physis-pro/target/release/examples/reference_spectrum")
MODEL = os.path.expanduser("~/dev/physis-pro/models/bge-base-en-v1.5")
# Two frames, because a single one confounds "context helps" with "this
# sentence helps". If the separation is a property of context it should survive
# a change of frame; if it is a property of one sentence it will not.
FRAMES = [
    "The report says the {} of the system was recorded during the test.",
    "After the run we measured {} and wrote it into the log.",
]
FRAME = FRAMES[0]


def build_pairs(wn, n, seed=0):
    rng = random.Random(seed)
    words = [w for w in wn.index if w.isalpha() and 3 <= len(w) <= 14]
    rng.shuffle(words)
    synonyms, antonyms, hypernyms = [], [], []
    for word in words:
        if len(antonyms) < n:
            for other in wn.antonyms(word):
                if other.isalpha():
                    antonyms.append((word, other))
                    break
        if len(synonyms) < n:
            for other in wn.synonyms(word):
                if other.isalpha():
                    synonyms.append((word, other))
                    break
        if len(hypernyms) < n:
            for other in wn.hypernyms(word):
                if other.isalpha():
                    hypernyms.append((word, other))
                    break
        if min(len(synonyms), len(antonyms), len(hypernyms)) >= n:
            break
    pool = [w for w in words[:20000] if w.isalpha()]
    randoms = [(rng.choice(pool), rng.choice(pool)) for _ in range(n)]
    return {"synonym": synonyms[:n], "antonym": antonyms[:n],
            "hypernym": hypernyms[:n], "random": randoms}


def embed(texts):
    lines = [json.dumps({"x": i, "text": t}) for i, t in enumerate(texts)]
    env = dict(os.environ, PHYSIS_MODEL_DIR=MODEL, PHYSIS_MODEL_DIM="768",
               PHYSIS_MODEL_MAX_LEN="128")
    p = subprocess.run([EXAMPLE], input="\n".join(lines), capture_output=True,
                       text=True, env=env, timeout=3600)
    if p.returncode != 0 or not p.stdout.strip():
        sys.exit(f"embedding failed:\n{p.stderr[:300]}")
    data = json.loads(p.stdout)
    return data["embedder"], data["vectors"]


def cosine_distance(a, b):
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(x * x for x in b))
    return 1.0 - (dot / (na * nb) if na and nb else 0.0)


def auc(positive, negative):
    """P(a positive scores lower-distance than a negative), ties at 0.5."""
    if not positive or not negative:
        return None
    wins = sum((p < n) + 0.5 * (p == n) for p in positive for n in negative)
    return wins / (len(positive) * len(negative))


def score(pairs, framed, frame=None):
    frame = frame or FRAME
    texts = []
    for kind, items in pairs.items():
        for a, b in items:
            texts.append(frame.format(a) if framed else a)
            texts.append(frame.format(b) if framed else b)
    label, vectors = embed(texts)
    out, cursor = {}, 0
    for kind, items in pairs.items():
        distances = []
        for _ in items:
            distances.append(cosine_distance(vectors[cursor], vectors[cursor + 1]))
            cursor += 2
        out[kind] = distances
    return label, out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pairs", type=int, default=250)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    wn = WordNet()
    pairs = build_pairs(wn, args.pairs)
    counts = {k: len(v) for k, v in pairs.items()}
    if min(counts.values()) < 20:
        sys.exit(f"too few pairs to measure: {counts}")
    print(f"WordNet: {wn.stats()}")
    print(f"pairs: {counts}")

    report = {}
    for framed in (False, True):
        label, distances = score(pairs, framed)
        name = "in a sentence frame" if framed else "bare words"
        print(f"\n── {name} · embedder {label}")
        print(f"   {'relation':10}{'mean':>8}{'sd':>8}{'median':>8}")
        for kind in ("synonym", "hypernym", "antonym", "random"):
            d = distances[kind]
            print(f"   {kind:10}{statistics.mean(d):8.4f}{statistics.pstdev(d):8.4f}"
                  f"{statistics.median(d):8.4f}")
        syn_anto = auc(distances["synonym"], distances["antonym"])
        syn_random = auc(distances["synonym"], distances["random"])
        anto_random = auc(distances["antonym"], distances["random"])
        print(f"   AUC synonym vs antonym  {syn_anto:.3f}   "
              f"(0.5 = the geometry cannot tell them apart)")
        print(f"   AUC synonym vs random   {syn_random:.3f}")
        print(f"   AUC antonym vs random   {anto_random:.3f}")
        report[name] = {"means": {k: statistics.mean(v) for k, v in distances.items()},
                        "auc_syn_anto": syn_anto, "auc_syn_random": syn_random,
                        "auc_anto_random": anto_random, "embedder": label}

    # Frame sensitivity, and a mis-pairing control: if synonym pairs are close
    # because of what a synonym IS, re-pairing across pairs must destroy it.
    label, second = score(pairs, True, FRAMES[1])
    auc_second = auc(second["synonym"], second["antonym"])
    print(f"\n── second frame · AUC synonym vs antonym {auc_second:.3f} "
          f"(first frame gave {report['in a sentence frame']['auc_syn_anto']:.3f})")
    rng = random.Random(1)
    mis = {"synonym": [(a, rng.choice(pairs["synonym"])[1]) for a, _ in pairs["synonym"]],
           "antonym": [(a, rng.choice(pairs["antonym"])[1]) for a, _ in pairs["antonym"]],
           "hypernym": pairs["hypernym"], "random": pairs["random"]}
    _, mis_d = score(mis, True)
    auc_mis = auc(mis_d["synonym"], mis_d["antonym"])
    print(f"── mis-paired control · AUC {auc_mis:.3f} "
          "(should collapse toward 0.5 if the effect is the relation, not the words)")
    report["second_frame_auc"] = auc_second
    report["mispaired_auc"] = auc_mis

    bare = report["bare words"]["auc_syn_anto"]
    framed = report["in a sentence frame"]["auc_syn_anto"]
    print()
    if max(bare, framed) < 0.65:
        print("VERDICT: distance alone does not separate synonymy from antonymy "
              f"(best AUC {max(bare, framed):.3f}). Both relations are 'about the same "
              "thing', which is what the geometry encodes. A dictionary carries the "
              "distinction the field does not.")
    else:
        print(f"VERDICT: distance separates them at AUC {max(bare, framed):.3f} — the "
              "geometry carries more of the distinction than expected; check the pair "
              "sampling before trusting it.")
    if args.out:
        json.dump(report, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
