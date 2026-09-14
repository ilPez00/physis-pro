#!/usr/bin/env python3
"""E83 — does a WordNet antonym rule detect contradiction where cosine cannot?

`WHAT_WE_KNOW.md` records the settled negative: cosine does not separate
agreement from contradiction (precision 0.667 at perfect recall, C2). E82 showed
why — bare geometry is at chance on synonym versus antonym (AUC 0.488) — and
that the dictionary holds 7983 antonym pointers the embedder does not.

This tests the cheap symbolic arm against that same failure.

Fixture, built so the labels come from the dictionary and the templates rather
than from anyone's judgement:

  contradiction   the same frame with a word and its WordNet antonym
  agreement       the same frame with a word and its WordNet synonym
  unrelated       two frames about different subjects
  plus the repository's own pair (`extra-window-a/b.md`: mandatory/forbidden,
  forbidden/required), which is where this shape was first noticed

Arms:

  cosine          distance between the two sentences — the control that failed
  antonym         an antonym pair spans the two sentences
  negation        one sentence negates a content word the other asserts
  antonym+negation  either rule fires

Reported as AUROC for contradiction against everything else, plus precision and
recall at each rule's own decision.

    python3 benchmarks/conatus/e83_contradiction.py --pairs 150
"""
import argparse, json, math, os, random, re, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from wordnet import WordNet  # noqa: E402

EXAMPLE = os.path.expanduser("~/dev/physis-pro/target/release/examples/reference_spectrum")
MODEL = os.path.expanduser("~/dev/physis-pro/models/bge-base-en-v1.5")

TEMPLATES = [
    "The operator access window is {} from 06:00.",
    "The inlet pressure reading was {} during the night shift.",
    "Maintenance reported that the pump behaviour is {}.",
    "The specification says the valve state must be {}.",
    "After the run the log recorded the result as {}.",
]
NEGATORS = ("not", "never", "no longer", "cannot be", "is not")
STOP = {"the", "a", "an", "is", "was", "were", "be", "been", "of", "to", "in",
        "on", "for", "with", "and", "or", "that", "this", "it", "its", "at",
        "from", "after", "during", "must", "says", "said", "reported", "recorded"}


def content_words(text):
    return {w for w in re.findall(r"[a-zA-Z][a-zA-Z-]+", text.lower()) if w not in STOP}


def build_fixture(wn, n, seed=0):
    rng = random.Random(seed)
    words = [w for w in wn.index if w.isalpha() and 4 <= len(w) <= 12]
    rng.shuffle(words)
    items = []
    anto, syn = [], []
    for word in words:
        if len(anto) < n:
            for other in wn.antonyms(word):
                if other.isalpha() and " " not in other:
                    anto.append((word, other))
                    break
        if len(syn) < n:
            for other in wn.synonyms(word):
                if other.isalpha() and " " not in other:
                    syn.append((word, other))
                    break
        if len(anto) >= n and len(syn) >= n:
            break
    for a, b in anto:
        frame = rng.choice(TEMPLATES)
        items.append({"label": "contradiction", "a": frame.format(a), "b": frame.format(b)})
    for a, b in syn:
        frame = rng.choice(TEMPLATES)
        items.append({"label": "agreement", "a": frame.format(a), "b": frame.format(b)})
    for _ in range(n):
        f1, f2 = rng.sample(TEMPLATES, 2)
        items.append({"label": "unrelated",
                      "a": f1.format(rng.choice(words)), "b": f2.format(rng.choice(words))})
    # Negation contradictions: the same claim, one of them denied.
    for _ in range(n // 2):
        frame = rng.choice(TEMPLATES)
        word = rng.choice(words)
        items.append({"label": "contradiction",
                      "a": frame.format(word),
                      "b": frame.format(f"{rng.choice(NEGATORS)} {word}")})
    return items


# Contradictions that carry no WordNet antonym: numbers, modals, roles and
# scope. The generated fixture above is built by inserting antonyms, so a rule
# that looks for antonyms scores near 1.0 on it by construction — the rule and
# the generator are the same instrument. These are written by hand to break it.
HARD = [
    ("The inlet pressure stayed above 2 bar.", "The inlet pressure stayed below 2 bar.", True),
    ("The pump ran for 12 minutes.", "The pump ran for 40 minutes.", True),
    ("Night access must be requested in advance.", "Night access must not be requested in advance.", True),
    ("The valve feeds the pump.", "The pump feeds the valve.", True),
    ("Every operator logged the reading.", "No operator logged the reading.", True),
    ("The test passed on the first attempt.", "The test needed three attempts.", True),
    ("Access is mandatory from 06:00 and forbidden at night.",
     "Access is forbidden from 06:00 and required at night.", True),
    ("The report was filed on Monday.", "The report was filed at the start of the week.", False),
    ("The pump stalled below 2 bar.", "Stalling was observed under 2 bar of inlet pressure.", False),
    ("Maintenance replaced the impeller.", "The impeller was replaced by maintenance.", False),
    ("The valve is checked weekly.", "Weekly checks cover the valve.", False),
    ("Two stalls were recorded in March.", "March saw a pair of stalls.", False),
]


def hard_items():
    return [{"label": "contradiction" if bad else "agreement", "a": a, "b": b,
             "source": "hand-written, no antonym inserted"} for a, b, bad in HARD]


def repo_pair():
    root = os.path.expanduser("~/dev/physis-pro/physis-core/benchmarks/ground-truth")
    a = os.path.join(root, "extra-window-a.md")
    b = os.path.join(root, "extra-window-b.md")
    if os.path.exists(a) and os.path.exists(b):
        return [{"label": "contradiction", "a": open(a).read().strip(),
                 "b": open(b).read().strip(), "source": "repository fixture"}]
    return []


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


def antonym_rule(wn, item, cache):
    """Fires when a word in one sentence has an antonym in the other."""
    wa, wb = content_words(item["a"]), content_words(item["b"])
    for word in wa - wb:
        if word not in cache:
            cache[word] = set(wn.antonyms(word))
        if cache[word] & wb:
            return True
    return False


def negation_rule(item):
    """Fires when one sentence negates a content word the other asserts."""
    a, b = item["a"].lower(), item["b"].lower()
    for negator in NEGATORS:
        if negator in a and negator not in b:
            tail = a.split(negator, 1)[1].strip().rstrip(".")
            if tail and tail.split()[0] in content_words(b):
                return True
        if negator in b and negator not in a:
            tail = b.split(negator, 1)[1].strip().rstrip(".")
            if tail and tail.split()[0] in content_words(a):
                return True
    return False


def auroc(scores, labels):
    pos = [s for s, y in zip(scores, labels) if y]
    neg = [s for s, y in zip(scores, labels) if not y]
    if not pos or not neg:
        return None
    return sum((p > n) + 0.5 * (p == n) for p in pos for n in neg) / (len(pos) * len(neg))


def prf(pred, labels):
    tp = sum(1 for p, y in zip(pred, labels) if p and y)
    fp = sum(1 for p, y in zip(pred, labels) if p and not y)
    fn = sum(1 for p, y in zip(pred, labels) if not p and y)
    precision = tp / (tp + fp) if tp + fp else 0.0
    recall = tp / (tp + fn) if tp + fn else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return precision, recall, f1, tp, fp, fn


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pairs", type=int, default=150)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    wn = WordNet()
    items = build_fixture(wn, args.pairs) + repo_pair()
    hard = hard_items()
    labels = [i["label"] == "contradiction" for i in items]
    counts = {}
    for i in items:
        counts[i["label"]] = counts.get(i["label"], 0) + 1
    print(f"fixture: {counts} · {len(items)} pair(s) · WordNet {wn.stats()['antonym_pointers']} "
          "antonym pointers")

    texts = [t for i in items for t in (i["a"], i["b"])]
    label, vectors = embed(texts)
    distances = [cosine_distance(vectors[2 * k], vectors[2 * k + 1]) for k in range(len(items))]
    print(f"embedder {label}")

    cache = {}
    anto = [antonym_rule(wn, i, cache) for i in items]
    neg = [negation_rule(i) for i in items]
    both = [a or n for a, n in zip(anto, neg)]

    print(f"\n{'arm':18}{'AUROC':>8}{'precision':>11}{'recall':>9}{'F1':>7}   (tp/fp/fn)")
    a_cos = auroc(distances, labels)
    print(f"{'cosine distance':18}{a_cos:8.3f}{'—':>11}{'—':>9}{'—':>7}   ranking only")
    for name, pred in (("antonym rule", anto), ("negation rule", neg),
                       ("antonym+negation", both)):
        p, r, f1, tp, fp, fn = prf(pred, labels)
        a = auroc([1.0 if x else 0.0 for x in pred], labels)
        print(f"{name:18}{a:8.3f}{p:11.3f}{r:9.3f}{f1:7.3f}   ({tp}/{fp}/{fn})")

    # The repository's own pair, called out because it is the only item here
    # that was not generated by this harness.
    if items and items[-1].get("source"):
        k = len(items) - 1
        print(f"\nrepository fixture (mandatory/forbidden): cosine distance {distances[k]:.4f}, "
              f"antonym rule {'fires' if anto[k] else 'silent'}, "
              f"negation rule {'fires' if neg[k] else 'silent'}")

    # The generated fixture and the rule share a generator, so the numbers above
    # are an upper bound and not evidence. This set is not built from antonyms.
    hard_labels = [i["label"] == "contradiction" for i in hard]
    hard_texts = [t for i in hard for t in (i["a"], i["b"])]
    _, hard_vectors = embed(hard_texts)
    hard_distances = [cosine_distance(hard_vectors[2 * k], hard_vectors[2 * k + 1])
                      for k in range(len(hard))]
    hard_cache = {}
    hard_anto = [antonym_rule(wn, i, hard_cache) for i in hard]
    hard_neg = [negation_rule(i) for i in hard]
    hard_both = [a or n for a, n in zip(hard_anto, hard_neg)]
    print(f"\n── held-out set: {len(hard)} hand-written pairs, no antonym inserted")
    p, r, f1, tp, fp, fn = prf(hard_both, hard_labels)
    print(f"   antonym+negation  precision {p:.3f} recall {r:.3f} F1 {f1:.3f} "
          f"({tp}/{fp}/{fn})")
    print(f"   cosine AUROC      {auroc(hard_distances, hard_labels):.3f}")
    for item, d, a, n, y in zip(hard, hard_distances, hard_anto, hard_neg, hard_labels):
        verdict = "contradiction" if (a or n) else "—"
        mark = "ok  " if (a or n) == y else "MISS"
        print(f"     {mark} {'(contra)' if y else '(agree) '} d={d:.3f} "
              f"{'ant' if a else '   '}{'neg' if n else '   '} · {item['a'][:46]}")

    print(f"\nFor reference, WHAT_WE_KNOW.md records cosine at precision 0.667 with perfect "
          f"recall (C2); this run puts its ranking AUROC at {a_cos:.3f}.")
    if args.out:
        json.dump({"counts": counts, "auroc_cosine": a_cos,
                   "rules": {"antonym": prf(anto, labels), "negation": prf(neg, labels),
                             "both": prf(both, labels)}}, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
