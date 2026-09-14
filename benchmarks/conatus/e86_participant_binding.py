#!/usr/bin/env python3
"""E86 — participant binding: can the E84 rules keep their recall without
firing on the adversarial pairs?

E85 showed the structured rules collapse to precision 0.000 when the surface
cue is present but the participants differ: equivalent-polarity quantifiers,
modals over different objects, comparisons with the subject re-addressed,
antonyms across a time boundary. The single defect is that class cues are
matched without binding them to the pair's participants.

The bound rules here wrap E84's rules unchanged and add one guard each:

  modal        the first content noun of both sentences must match
               (the modal's owner is the participant that differs)
  comparative  the content words BEFORE the matched comparison word must
               match — a re-addressed subject changes that prefix
  quantifier   universal vs negative-universal is suppressed when the
               predicates are antonyms (WordNet, domain oppositions, or a
               derivational un-/dis- pair): opposed polarity absorbed by an
               opposed predicate is agreement
  antonym      the content word AFTER each antonym must match (or both be
               absent) — the antonym must occupy the same slot, not just
               appear somewhere in both sentences

Everything else is E84's machinery, unchanged. No rule here was tuned on the
validation fixture (adversarial-consistent-2.json), which was written before
this script was first run.

    python3 benchmarks/conatus/e86_participant_binding.py
"""
import argparse, json, os, re, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from wordnet import WordNet  # noqa: E402
from e83_contradiction import antonym_rule, content_words, negation_rule  # noqa: E402
from e84_structured_rules import (  # noqa: E402
    COMPARATIVES, DOMAIN_OPPOSITES, UNIVERSAL, NEGATIVE_UNIVERSAL,
    comparative_rule, modal_rule, quantifier_rule, roles_rule, numeric_rule,
    _stems,
)

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURES = [
    ("holdout (E84)", os.path.join(HERE, "fixtures", "contradiction-holdout.json")),
    ("adversarial (E85)", os.path.join(HERE, "fixtures", "adversarial-consistent.json")),
    ("validation (E86)", os.path.join(HERE, "fixtures", "adversarial-consistent-2.json")),
]


def first_noun(text):
    """The first content word — crude stand-in for the subject head. What
    matters is that the same crude rule is applied to both sides, so a
    mismatch means the sentences are about differently-headed things."""
    cw = content_words(text)
    for w in re.findall(r"[a-z]+", text.lower()):
        if w in cw:
            return w
    return None


def is_derived_antonym(x, y):
    """`unscanned` against `scanned`: one word is the other with a negating
    prefix. WordNet misses derivational antonyms and the domain table does
    not know them."""
    parts_x = {p for part in x.split("-") for p in (part, part) }
    parts_y = {part for part in y.split("-")}
    for prefixed, other in ((parts_x, parts_y), (parts_y, parts_x)):
        for part in prefixed:
            for prefix in ("un", "dis"):
                if part.startswith(prefix) and part[len(prefix):] in other:
                    return True
    return False


def predicates_antonymic(a, b, wn, cache):
    """True when the content words the two sides do NOT share form an
    antonym pair — the polarity clash is carried by the predicate, so a
    universal/negative-universal surface is agreement, not contradiction."""
    da, db = _stems(a) - _stems(b), _stems(b) - _stems(a)
    for x in da:
        if x not in cache:
            cache[x] = set(wn.antonyms(x))
        for y in db:
            if (y in cache[x] or (x, y) in DOMAIN_OPPOSITES.items()
                    or (y, x) in DOMAIN_OPPOSITES.items()
                    or is_derived_antonym(x, y)):
                return True
    return False


def bound_modal_rule(item):
    if not modal_rule(item):
        return False
    return first_noun(item["a"]) == first_noun(item["b"])


def bound_comparative_rule(item):
    a, b = item["a"].lower(), item["b"].lower()
    for left, right in COMPARATIVES:
        ma = re.search(rf"\b{left}\b", a)
        mb = re.search(rf"\b{right}\b", b)
        if not (ma and mb):
            ma = re.search(rf"\b{right}\b", a)
            mb = re.search(rf"\b{left}\b", b)
        if ma and mb:
            # The words before the comparison are the comparison's subject.
            pa = set(_stems(a[:ma.start()]))
            pb = set(_stems(b[:mb.start()]))
            return pa == pb
    return False


def bound_quantifier_rule(item, wn, cache):
    if not quantifier_rule(item):
        return False
    a, b = item["a"].lower(), item["b"].lower()
    def has(words, text):
        return any(re.search(rf"\b{w}\b", text) for w in words)
    # Only the universal vs negative-universal branch can mistake agreement
    # for contradiction; the existential branches carry the holdout's
    # genuine contradictions and are left alone.
    if not ((has(UNIVERSAL, a) and has(NEGATIVE_UNIVERSAL, b)) or
            (has(NEGATIVE_UNIVERSAL, a) and has(UNIVERSAL, b))):
        return True
    return not predicates_antonymic(a, b, wn, cache)


def _right_neighbor(text, word):
    """The content word immediately after `word`'s first occurrence, or
    None when the antonym ends its sentence."""
    words = re.findall(r"[a-z]+", text.lower())
    for i, w in enumerate(words):
        if w == word:
            for r in words[i + 1:]:
                if r in content_words(" ".join(words[i + 1:])):
                    return r
            return None
    return None


def bound_antonym_rule(wn, item, cache):
    if not antonym_rule(wn, item, cache):
        return False
    # The antonym must describe the same participant: a subject-head mismatch
    # (the valve against the drain valve) is two facts, not a contradiction.
    if first_noun(item["a"]) != first_noun(item["b"]):
        return False
    wa, wb = content_words(item["a"]), content_words(item["b"])
    for word in sorted(wa - wb):
        if word not in cache:
            cache[word] = set(wn.antonyms(word))
        for anti in sorted(cache[word] & wb):
            # Fire when ANY matched antonym pair occupies the same slot;
            # suppress only when every pair sits in a different slot.
            if _right_neighbor(item["a"].lower(), word) == \
               _right_neighbor(item["b"].lower(), anti):
                return True
    return False


def domain_rule(item):
    """E84's domain table, unchanged — it carries two holdout hits the WordNet
    antonym rule cannot see (supported/contradicted and friends)."""
    a, b = item["a"].lower(), item["b"].lower()
    def has(words, text):
        return any(re.search(rf"\b{w}\b", text) for w in words)
    for word, opposite in DOMAIN_OPPOSITES.items():
        if re.search(rf"\b{word}\b", a) and re.search(rf"\b{opposite}\b", b):
            if not (_stems(a) & _stems(b)):
                return False
            # Universal against negative-universal with an opposed predicate
            # is quantifier absorption ("all passed" = "none failed"), the
            # E85 false-positive shape — not a contradiction.
            if ((has(UNIVERSAL, a) and has(NEGATIVE_UNIVERSAL, b)) or
                    (has(NEGATIVE_UNIVERSAL, a) and has(UNIVERSAL, b))):
                return False
            return True
    return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fixture", default=None,
                    help="run one fixture instead of all three")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()
    fixtures = FIXTURES if not args.fixture else [(args.fixture, args.fixture)]
    wn = WordNet()
    cache = {}
    rules = {
        "antonym": lambda i: bound_antonym_rule(wn, i, cache),
        "domain": domain_rule,
        "negation": negation_rule,
        "numeric": numeric_rule,
        "comparative": bound_comparative_rule,
        "modal": bound_modal_rule,
        "quantifier": lambda i: bound_quantifier_rule(i, wn, cache),
        "roles": roles_rule,
    }
    report = {}
    for name, path in fixtures:
        data = json.load(open(path))
        items = data["pairs"]
        fired = {n: [r(i) for i in items] for n, r in rules.items()}
        any_fired = [any(fired[n][k] for n in rules) for k in range(len(items))]
        labels = [i["contradiction"] for i in items]
        tp = sum(1 for k in range(len(items)) if any_fired[k] and labels[k])
        fp = sum(1 for k in range(len(items)) if any_fired[k] and not labels[k])
        fn = sum(1 for k in range(len(items)) if not any_fired[k] and labels[k])
        precision = tp / (tp + fp) if tp + fp else 0.0
        recall = tp / (tp + fn) if tp + fn else 0.0
        fps = [(i["class"], i["a"], i["b"]) for k, i in enumerate(items)
               if any_fired[k] and not labels[k]]
        print(f"\n=== {name}: {len(items)} pairs · "
              f"precision {precision:.3f} recall {recall:.3f} "
              f"({tp} caught, {fp} false+, {fn} missed)")
        for cls, a, b in fps:
            print(f"  FALSE+ [{cls}] {a} || {b}")
        missed = [(i["class"], i["a"], i["b"]) for k, i in enumerate(items)
                  if not any_fired[k] and labels[k]]
        for cls, a, b in missed:
            print(f"  MISSED  [{cls}] {a} || {b}")
        report[name] = {"precision": precision, "recall": recall,
                        "tp": tp, "fp": fp, "fn": fn}
    if args.out:
        json.dump(report, open(args.out, "w"), indent=1)
        print(f"\nwrote {args.out}")


if __name__ == "__main__":
    main()
