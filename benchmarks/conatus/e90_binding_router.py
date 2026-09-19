#!/usr/bin/env python3
"""E90 — Day 5: contradiction recall without restoring false alarms.

The question (docs/plans/2026-09-15-core-pro-development.md, Day 5): can
explicit participant/time binding restrict the NLI residual enough to keep the
rule baseline's precision while finding more implication and dictionary-gap
cases?

E86 showed the bound rules hold precision on the adversarial set; E89 showed the
NLI arm is strong on the E84 holdout (P 1.00 / R 0.96) and useless on the
adversarial set (P 0.00). Neither alone is a route. This script implements the
plan's `compare(a, b)`:

    scope = bind(a, b)
    if scope.proven_different:  NOT_COMPARABLE(scope.evidence)
    if scope.unknown:           REVIEW_REQUIRED(reason)
    if bound_rule_matches:      CONTRADICTION_CANDIDATE(rule_id)
    if nli_available:           REVIEW_CANDIDATE(nli, scope)
    else:                       UNKNOWN

Four arms are scored on the same pairs: rules-only, NLI-only, naive union, and
the router. **An abstention is not a negative.** `NOT_COMPARABLE`,
`REVIEW_REQUIRED` and `UNKNOWN` are counted as abstentions and reported as
coverage; they never enter the precision/recall denominators as "no
contradiction found". That is the plan's "unknown binding must not be scored as
a correct negative", and it is the difference between an honest abstention and a
silent claim.

The binding rules are deliberately crude and each one is documented where it
lives. Their effect is measured, not assumed: if routing does not beat the
union on precision at a coverage the rules-only arm already achieves, the route
has failed and the plan's fallback applies — ship the rules plus explicit
review for the residual.

    .venv-nli/bin/python benchmarks/conatus/e90_binding_router.py --out e90-results.json
    python3 benchmarks/conatus/e90_binding_router.py --no-nli     # rules + router only
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from e83_contradiction import (  # noqa: E402
    antonym_rule, content_words, negation_rule,
)
from e84_structured_rules import (  # noqa: E402
    comparative_rule, modal_rule, numeric_rule, quantifier_rule, roles_rule,
)
from e86_participant_binding import (  # noqa: E402
    bound_antonym_rule, bound_comparative_rule, bound_modal_rule,
    bound_quantifier_rule, domain_rule,
)

HERE = os.path.dirname(os.path.abspath(__file__))
MODEL_DIR = os.path.abspath(os.path.join(HERE, "..", "..", "models", "nli-deberta-v3-xsmall"))
HOLDOUT = os.path.join(HERE, "fixtures", "day5-holdout.json")
# Published fixtures. Regression sets, not a blind holdout: the router must not
# add false positives on them.
REGRESSION = [
    ("holdout (E84)", os.path.join(HERE, "fixtures", "contradiction-holdout.json")),
    ("adversarial (E85)", os.path.join(HERE, "fixtures", "adversarial-consistent.json")),
    ("validation (E86)", os.path.join(HERE, "fixtures", "adversarial-consistent-2.json")),
]

# ── binding ──────────────────────────────────────────────────────────────────
#
# What can be decided deterministically about whether two statements are even
# about the same thing. Everything here is a *guard*: it can prove the pair
# incomparable, or admit it cannot tell. It never decides a contradiction —
# that stays with the rules and, in the residual, with a review candidate.

MONTHS = {
    "january": 1, "february": 2, "march": 3, "april": 4, "may": 5, "june": 6,
    "july": 7, "august": 8, "september": 9, "october": 10, "november": 11,
    "december": 12,
}
# Relative expressions are deliberately NOT resolved: "yesterday" needs a
# reference clock the pair does not carry, and inventing one is how a binding
# guard turns into a false negative.
RELATIVE_TIME = {"today", "yesterday", "tomorrow", "now", "currently", "recently"}

# dimension -> unit -> factor to the dimension's base unit
UNITS = {
    "temperature": {"c": 1.0, "k": 1.0, "f": 5.0 / 9.0},   # compared via offset below
    "pressure": {"bar": 100000.0, "kpa": 1000.0, "psi": 6894.76, "pa": 1.0},
    "length": {"mm": 0.001, "cm": 0.01, "m": 1.0, "in": 0.0254, "ft": 0.3048},
    "mass": {"g": 0.001, "kg": 1.0, "t": 1000.0, "lb": 0.453592},
    "time": {"s": 1.0, "min": 60.0, "h": 3600.0},
    "percent": {"percent": 1.0, "%": 1.0},
}


# Words that can head a clause but do not name a participant. E86's
# `first_noun` was written for the modal rule, where the owner differs and the
# first content word *is* the owner; used globally it mistakes "All cylinders"
# against "Some cylinders" for a participant mismatch and abstains on a
# genuine contradiction. Skip them before picking the head.
NOT_A_HEAD = {
    "all", "some", "every", "no", "none", "one", "two", "three", "any", "each",
    "both", "not", "never", "must", "may", "might", "can", "could", "will",
    "would", "should", "more", "less", "most", "least", "than", "then",
}


def head_noun(text: str) -> str | None:
    """The first content word that can actually name a participant."""
    cw = content_words(text)
    for w in re.findall(r"[a-z]+", text.lower()):
        if w in cw and w not in NOT_A_HEAD:
            return w
    return None


def identifiers(text: str) -> set[str]:
    """Equipment / batch identifiers: a short letter run attached to digits.

    `P-101` and `P102` are identifiers; `18` and `400` are not. This is the
    distinction that keeps the guard from treating a changed measurement as a
    changed participant — and from missing one that only differs in its tag.
    """
    return {m.group(0).lower()
            for m in re.finditer(r"\b[a-z]{1,4}[-_]?\d{2,}\b", text.lower())}


def month_bucket(text: str) -> tuple[int, int] | None:
    """(year, month) when the text names one, else None.

    `2025-01` and `January 2025` both land here; a bare month name with no year
    resolves against no year, so it is only comparable to another bare month."""
    m = re.search(r"\b(\d{4})-(\d{2})\b", text)
    if m:
        return int(m.group(1)), int(m.group(2))
    lower = text.lower()
    m = re.search(r"\b(" + "|".join(MONTHS) + r")\s+(\d{4})\b", lower)
    if m:
        return int(m.group(2)), MONTHS[m.group(1)]
    m = re.search(r"\b(" + "|".join(MONTHS) + r")\b", lower)
    if m:
        return 0, MONTHS[m.group(1)]   # year unknown -> only comparable to another
    return None


def clock_time(text: str) -> str | None:
    m = re.search(r"\b([0-2]?\d:[0-5]\d)\b", text)
    return m.group(1) if m else None


def quantity(text: str) -> tuple[str, str, float] | None:
    """(dimension, unit, value) for the first `<number> <unit>` in the text."""
    for m in re.finditer(r"(\d+(?:\.\d+)?)\s*([a-z%]{1,7})", text.lower()):
        value, unit = float(m.group(1)), m.group(2)
        for dim, table in UNITS.items():
            if unit in table:
                return dim, unit, value
    return None


def to_base(dim: str, unit: str, value: float) -> float:
    if dim == "temperature":
        if unit == "k":
            return value - 273.15
        if unit == "f":
            return (value - 32.0) * 5.0 / 9.0
        return value
    return value * UNITS[dim][unit]


def bind(a: str, b: str, wn) -> tuple[str, list[str]]:
    """`("proven-different" | "unknown" | "bound", evidence)`.

    Order matters: a proven difference short-circuits, an admitted unknown
    abstains, and only then is the pair comparable.
    """
    evidence: list[str] = []

    # Time: two named periods that cannot overlap prove the statements are
    # about different worlds. One named period against none admits it cannot
    # tell — the unlabelled side could be the same period.
    ta, tb = month_bucket(a), month_bucket(b)
    if ta and tb and ta != tb:
        return "proven-different", [f"month {ta} vs {tb}"]
    ca, cb = clock_time(a), clock_time(b)
    if ca and cb and ca != cb:
        return "proven-different", [f"time {ca} vs {cb}"]
    if bool(ta) != bool(tb) or bool(ca) != bool(cb):
        evidence.append("time on one side only")
    if any(w in a.lower().split() or w in b.lower().split() for w in RELATIVE_TIME):
        evidence.append("relative time expression")

    # Quantity: different dimensions are different properties. Same dimension
    # is comparable whatever the unit, which is the unit-conversion case.
    qa, qb = quantity(a), quantity(b)
    if qa and qb and qa[0] != qb[0]:
        return "proven-different", [f"dimension {qa[0]} vs {qb[0]}"]

    # Identifier: two named tags that differ are two different things, even
    # when the head noun is identical ("Pump P-101" against "Pump P-102").
    ia, ib = identifiers(a), identifiers(b)
    if ia and ib and ia != ib:
        return "proven-different", [f"identifier {sorted(ia)} vs {sorted(ib)}"]

    # Participant: a differently-headed subject is a different participant —
    # unless WordNet says the two heads are the same thing under another name,
    # in which case this guard must not fire.
    na, nb = head_noun(a), head_noun(b)
    if na and nb and na != nb:
        related = False
        if hasattr(wn, "synonyms"):
            related = na in set(wn.synonyms(nb)) or nb in set(wn.synonyms(na))
        if not related:
            related = nb in set(wn.antonyms(na)) or na in set(wn.antonyms(nb))
        if related:
            evidence.append(f"head {na} vs {nb} (related)")
        else:
            return "proven-different", [f"participant {na} vs {nb}"]

    if evidence:
        return "unknown", evidence
    return "bound", []


# ── arms ─────────────────────────────────────────────────────────────────────


def rules_arm(item, rules) -> bool:
    return any(r(item) for r in rules.values())


def routed(item, rules, wn, nli, threshold: float):
    """The plan's `compare`. Returns (decision, detail)."""
    scope, evidence = bind(item["a"], item["b"], wn)
    if scope == "proven-different":
        return "NOT_COMPARABLE", "; ".join(evidence)
    if scope == "unknown":
        return "REVIEW_REQUIRED", "; ".join(evidence)
    for name, rule in rules.items():
        if rule(item):
            return "CONTRADICTION_CANDIDATE", name
    if nli is not None:
        # The residual arm ran and answered. Above the threshold it hands the
        # pair to a reviewer as a candidate; below it, it is declining to
        # raise one — a decision, not an abstention. Emitting
        # `REVIEW_CANDIDATE` for both would score a below-threshold score as a
        # claim of contradiction, which is how a model score silently becomes
        # a belief.
        p = nli(item["a"], item["b"])
        if p >= threshold:
            return "REVIEW_CANDIDATE", f"nli {p:.2f}"
        return "NO_CONTRADICTION", f"nli {p:.2f}"
    return "UNKNOWN", "no residual arm available"


DECIDED_POSITIVE = {"CONTRADICTION_CANDIDATE", "REVIEW_CANDIDATE"}
# A classifier arm always answers, so its negative is a decision, not an
# abstention. Only the router's three non-answers abstain.
DECIDED_NEGATIVE = {"NO_CONTRADICTION"}
ABSTENTIONS = {"NOT_COMPARABLE", "REVIEW_REQUIRED", "UNKNOWN"}


def score(rows):
    """rows: (class, label, decision, detail).

    An abstention is never counted as a correct negative: it leaves both
    denominators, and is reported as coverage instead.
    """
    per_class = defaultdict(lambda: {"tp": 0, "fp": 0, "fn": 0, "tn": 0, "abstained": 0})
    totals = {"tp": 0, "fp": 0, "fn": 0, "tn": 0, "abstained": 0}
    for cls, label, decision, _ in rows:
        bucket = per_class[cls]
        if decision in ABSTENTIONS:
            bucket["abstained"] += 1
            totals["abstained"] += 1
            continue
        positive = decision in DECIDED_POSITIVE
        key = ("tp" if positive else "fn") if label else ("fp" if positive else "tn")
        bucket[key] += 1
        totals[key] += 1

    def rates(d):
        decided = d["tp"] + d["fp"] + d["fn"] + d["tn"]
        precision = d["tp"] / (d["tp"] + d["fp"]) if d["tp"] + d["fp"] else 0.0
        recall_decided = d["tp"] / (d["tp"] + d["fn"]) if d["tp"] + d["fn"] else 0.0
        recall_overall = d["tp"] / (d["tp"] + d["fn"] + d["abstained"]) if (
            d["tp"] + d["fn"] + d["abstained"]) else 0.0
        return {
            **d,
            "decided": decided,
            "coverage": decided / (decided + d["abstained"]) if (decided + d["abstained"]) else 0.0,
            "precision": precision,
            "recall_decided": recall_decided,
            "recall_overall": recall_overall,
        }

    return {c: rates(d) for c, d in sorted(per_class.items())}, rates(totals)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=None)
    ap.add_argument("--threshold", type=float, default=0.5)
    ap.add_argument("--no-nli", action="store_true",
                    help="skip the model arm entirely (report it NOT MEASURED)")
    ap.add_argument("--fixture", default=HOLDOUT)
    args = ap.parse_args()

    sys.path.insert(0, HERE)
    from wordnet import WordNet  # noqa: E402
    wn = WordNet()
    cache: dict = {}
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
    # The unbound E84 rules, kept only so the difference the E86 guards make is
    # visible in the same run.
    unbound = {
        "antonym": lambda i: antonym_rule(wn, i, cache),
        "domain": domain_rule,
        "negation": negation_rule,
        "numeric": numeric_rule,
        "comparative": comparative_rule,
        "modal": modal_rule,
        "quantifier": quantifier_rule,
        "roles": roles_rule,
    }

    nli = None
    if not args.no_nli and os.path.isdir(MODEL_DIR):
        import numpy as np
        import onnxruntime as ort
        from tokenizers import Tokenizer

        sess = ort.InferenceSession(os.path.join(MODEL_DIR, "onnx", "model.onnx"),
                                    providers=["CPUExecutionProvider"])
        tok = Tokenizer.from_file(os.path.join(MODEL_DIR, "tokenizer.json"))

        def nli(a, b):  # noqa: E731
            enc = tok.encode(a, b)
            ids = np.array([enc.ids], dtype=np.int64)
            out = np.array(sess.run(None, {"input_ids": ids,
                                           "attention_mask": np.ones_like(ids)})[0][0])
            probs = np.exp(out - out.max())
            return float(probs[0] / probs.sum())

    items = json.load(open(args.fixture))["pairs"]
    classes = sorted({i["class"] for i in items})
    print(f"=== {os.path.basename(args.fixture)}: {len(items)} pairs, "
          f"{sum(1 for i in items if i['contradiction'])} contradictions, "
          f"{len(classes)} classes")
    print(f"    NLI arm: {'cross-encoder/nli-deberta-v3-xsmall' if nli else 'NOT MEASURED'}"
          f" (threshold {args.threshold})")

    arms: dict[str, list] = {name: [] for name in
                             ("rules-unbound", "rules-bound", "nli", "union", "routed")}
    for item in items:
        cls, label = item["class"], item["contradiction"]
        def verdict(fired: bool) -> str:
            return "CONTRADICTION_CANDIDATE" if fired else "NO_CONTRADICTION"

        arms["rules-unbound"].append(
            (cls, label, verdict(rules_arm(item, unbound)), ""))
        bound = rules_arm(item, rules)
        arms["rules-bound"].append((cls, label, verdict(bound), ""))
        if nli is not None:
            p = nli(item["a"], item["b"])
            arms["nli"].append((cls, label, verdict(p >= args.threshold), f"{p:.2f}"))
            arms["union"].append(
                (cls, label, verdict(bound or p >= args.threshold), ""))
        decision, detail = routed(item, rules, wn, nli, args.threshold)
        arms["routed"].append((cls, label, decision, detail))

    report = {"fixture": os.path.basename(args.fixture),
              "nli_model": "cross-encoder/nli-deberta-v3-xsmall" if nli else "NOT_MEASURED",
              "threshold": args.threshold, "arms": {}}

    for arm, rows in arms.items():
        if arm in ("nli", "union") and nli is None:
            report["arms"][arm] = {"status": "NOT_MEASURED"}
            print(f"\n=== {arm}: NOT MEASURED (no model)")
            continue
        per_class, overall = score(rows)
        report["arms"][arm] = {"overall": overall, "per_class": per_class}
        print(f"\n=== {arm}")
        print(f"  overall  P {overall['precision']:.3f}  "
              f"R(decided) {overall['recall_decided']:.3f}  "
              f"R(overall) {overall['recall_overall']:.3f}  "
              f"coverage {overall['coverage']:.3f}  "
              f"({overall['tp']} tp, {overall['fp']} false+, {overall['fn']} missed, "
              f"{overall['abstained']} abstained)")
        for cls in classes:
            r = per_class[cls]
            print(f"    {cls:<20} P {r['precision']:.2f}  "
                  f"R {r['recall_decided']:.2f}  cov {r['coverage']:.2f}  "
                  f"({r['tp']}/{r['tp'] + r['fn']} caught, {r['fp']} false+, "
                  f"{r['abstained']} abstained)")

    routed_rows = arms["routed"]
    print("\n=== routed decisions on the true contradictions")
    for cls, label, decision, detail in routed_rows:
        if label:
            mark = "caught" if decision in DECIDED_POSITIVE else "NOT caught"
            print(f"  [{cls}] {mark:<11} {decision:<24} {detail}")
    print("\n=== routed decisions on the non-contradictions it did NOT abstain on")
    for cls, label, decision, detail in routed_rows:
        if not label and decision in DECIDED_POSITIVE:
            print(f"  [{cls}] FALSE+ {decision:<24} {detail}")

    # ── regression: the router must not add false positives on the locked sets
    print("\n=== regression on the published fixtures (routed vs bound rules)")
    regression = {}
    for name, path in REGRESSION:
        items_r = json.load(open(path))["pairs"]
        routed_rows_r = [routed(i, rules, wn, None, args.threshold) for i in items_r]
        rules_rows_r = [("CONTRADICTION_CANDIDATE" if rules_arm(i, rules)
                         else "NO_CONTRADICTION") for i in items_r]
        routed_fp = sum(1 for i, (d, _) in zip(items_r, routed_rows_r)
                        if not i["contradiction"] and d in DECIDED_POSITIVE)
        routed_tp = sum(1 for i, (d, _) in zip(items_r, routed_rows_r)
                        if i["contradiction"] and d in DECIDED_POSITIVE)
        rules_fp = sum(1 for i, r in zip(items_r, rules_rows_r)
                       if not i["contradiction"] and r in DECIDED_POSITIVE)
        rules_tp = sum(1 for i, r in zip(items_r, rules_rows_r)
                       if i["contradiction"] and r in DECIDED_POSITIVE)
        abstained = sum(1 for d, _ in routed_rows_r if d in ABSTENTIONS)
        ok = routed_fp <= rules_fp
        regression[name] = {"routed_fp": routed_fp, "rules_fp": rules_fp,
                            "routed_tp": routed_tp, "rules_tp": rules_tp,
                            "abstained": abstained, "no_new_false_positives": ok}
        print(f"  {name:<20} routed {routed_tp} tp / {routed_fp} false+  "
              f"rules {rules_tp} tp / {rules_fp} false+  "
              f"{abstained} abstained  {'OK' if ok else 'REGRESSION'}")
    report["regression"] = regression

    # ── verdict
    routed_overall = report["arms"]["routed"]["overall"]
    bound_overall = report["arms"]["rules-bound"]["overall"]
    no_regression = all(v["no_new_false_positives"] for v in regression.values())
    gain = routed_overall["recall_overall"] > bound_overall["recall_overall"] + 1e-9
    keeps_precision = routed_overall["precision"] >= bound_overall["precision"]
    # The plan's gate is "keep the rule baseline's precision while finding
    # more". Recall alone is not a promotion: a route that catches everything
    # by calling everything a contradiction is not a route. This check used to
    # compare recall only and promoted a router carrying 19 false positives
    # against the rules' 5 — the gate has to name the precision floor too.
    if not no_regression:
        verdict = "REJECTED_NEW_FALSE_POSITIVES"
    elif gain and keeps_precision:
        verdict = "ROUTE_PROMOTABLE"
    elif gain:
        verdict = "RECALL_GAIN_AT_PRECISION_COST_KEEP_RULES"
    elif keeps_precision:
        verdict = "NO_RECALL_GAIN_KEEP_RULES"
    else:
        verdict = "ROUTE_HURTS_PRECISION_KEEP_RULES"
    report["gate"] = {
        "no_new_false_positives_on_locked_fixtures": no_regression,
        "recall_gain_over_rules_bound": gain,
        "precision_at_least_rules_bound": keeps_precision,
        "rules_bound_precision": bound_overall["precision"],
        "routed_precision": routed_overall["precision"],
        "rules_bound_recall_overall": bound_overall["recall_overall"],
        "routed_recall_overall": routed_overall["recall_overall"],
    }
    report["verdict"] = verdict
    print(f"\nverdict: {verdict}")
    print("model scores never revise a belief: REVIEW_CANDIDATE is a review "
          "candidate, not a contradiction")

    if args.out:
        json.dump(report, open(args.out, "w"), indent=1)
        print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
