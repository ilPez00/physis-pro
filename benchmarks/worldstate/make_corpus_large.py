#!/usr/bin/env python3
"""The scale corpus — GENERATED, and labelled as such everywhere it is used.

`corpus.jsonl` is hand-authored and small (72 sentences). Its E56 L3 interval
cannot close at n = 28 usable queries. This file produces a larger corpus for
one purpose: **closing intervals on retrieval arms**. It must never be used for
a claim about structure, because its structure is a property of this script.

Two things it fixes that the hand corpus cannot:

1. **A gold that is not recency-biased.** In the hand corpus the gold antecedent
   is "the nearest earlier sentence sharing an entity", so an arm answering
   `p-1` is handed a large share for free (E56 L3, position-only 0.475 that
   collapses to 0.000 once a gap is required). Here the gold is the sentence
   whose fact the query **supersedes**: same subject and object, different
   predicate. Between them the script places distractors that mention the same
   entity *more recently than the gold*, so recency is an actively wrong answer.

2. **Controlled distance.** `gap` is drawn per cycle, so the gold sits a known
   number of positions back and the effect can be read against distance.

Determinism: one seed, recorded in the artifact and in the sha256.

Usage:  python3 benchmarks/worldstate/make_corpus_large.py [--cycles 60]
Writes: corpus-large.jsonl, corpus-large.sha256
"""
import argparse
import hashlib
import json
import pathlib
import random

HERE = pathlib.Path(__file__).resolve().parent
SEED = 20260914

MACHINES = [
    ("lathe", "spindle", "coolant"),
    ("press", "ram", "hydraulic oil"),
    ("mill", "cutter", "air"),
    ("grinder", "wheel", "coolant"),
    ("saw", "blade", "lubricant"),
    ("robot", "gripper", "compressed air"),
    ("oven", "element", "gas"),
    ("pump", "impeller", "seal water"),
]
# Distractor sentences mention the machine but assert nothing about the fact
# under test. They sit BETWEEN the gold and the query, so any arm that answers
# "most recent mention of this entity" is wrong by construction.
DISTRACTORS = [
    "{m} was cleaned at the start of the shift.",
    "The guard on {m} was checked and signed off.",
    "{m} appears on the weekly inspection list.",
    "Two operators are trained on {m}.",
    "The manual for {m} is kept at the bench.",
    "{m} was photographed for the maintenance file.",
    "A spare {part} for {m} is held in the store.",
    "{m} is scheduled for a paint touch-up next quarter.",
]
# HARD distractors, added 2026-09-14 after the first run scored 1.000 at every
# distance. That run's distractors were template sentences with no lexical
# overlap with the gold, so "same entity + most similar" was the gold by a mile
# and the benchmark could not fail. These share the entity AND nearly all the
# wording, differing in the attribute, the resource or the polarity's scope —
# which is what makes finding the superseded fact an actual discrimination.
HARD = [
    "{M} holds {res} temperature within tolerance.",
    "{M} holds {res} flow within tolerance.",
    "{M} held {res} pressure within tolerance during the previous run.",
    "{M2} holds {res} pressure within tolerance.",
    "{M} reports {res} pressure within tolerance on the panel.",
    # Paraphrases of the GOLD's wording about a different attribute. These exist
    # because the second run still scored 1.000: with the query reusing the
    # gold's exact words, "most similar earlier sentence about this entity" is
    # near-duplicate matching and cannot fail.
    "Temperature on the {m} line sits inside the specified band.",
    "Flow through {m} remains inside the specified band.",
]

# The query no longer reuses the gold's wording. Supersession has to be found
# across a paraphrase, which is the case the whole design is about: an embedder
# that only matches strings has nothing to contribute here.
QUERIES = [
    "Pressure on the {m} line has fallen out of specification.",
    "The {m} line can no longer sustain its rated {res} pressure.",
    "{M} has dropped below the pressure it was commissioned to hold.",
]
FILLER = [
    "The shop floor was swept before the second shift.",
    "A delivery of bar stock arrived at the goods door.",
    "The compressor ran at its usual duty cycle.",
    "Inspection cleared the morning batch.",
    "The scrap bin was emptied.",
    "A visitor was shown around the north wall.",
]


def build(cycles: int):
    rng = random.Random(SEED)
    recs = []

    def emit(text, ents, rel, trans, role=None, gold=None, cycle=None, gap=None):
        recs.append(
            {
                "pos": len(recs),
                "id": f"g{len(recs):04d}",
                "text": text,
                "entities": ents,
                "relation": rel,
                "transition": trans,
                "role": role,
                "supersedes": gold,
                "cycle": cycle,
                "gap": gap,
                "generated": True,
            }
        )

    for c in range(cycles):
        base, part, resource = MACHINES[c % len(MACHINES)]
        m = f"{base}-{c}"
        gap = rng.choice([4, 6, 8, 10, 12])

        # 1. the fact that will later be superseded — this is the gold
        emit(
            f"{m.capitalize()} holds {resource} pressure within tolerance.",
            [m],
            [m, "Supports", resource],
            "emerge",
            role="gold",
            cycle=c,
        )
        gold_pos = len(recs) - 1

        # 2. distractors that mention the same entity MORE RECENTLY than the
        # gold, at least two of them lexically near-identical to it.
        neighbour = f"{base}-{c + 1}"
        hard_slots = rng.sample(range(max(1, gap - 2)), k=min(2, max(1, gap - 2)))
        for slot in range(max(1, gap - 2)):
            if slot in hard_slots:
                t = rng.choice(HARD).format(
                    m=m, M=m.capitalize(), M2=neighbour.capitalize(), res=resource
                )
                ents = [m] if "{M2}" not in t else [m, neighbour]
                emit(t, ents, None, "persist", role="hard", cycle=c)
            elif rng.random() < 0.65:
                t = rng.choice(DISTRACTORS).format(m=m, part=part)
                emit(t, [m], None, "persist", role="distractor", cycle=c)
            else:
                emit(rng.choice(FILLER), [], None, "none", role="filler", cycle=c)

        # 3. the query: same subject and object, opposite predicate
        emit(
            rng.choice(QUERIES).format(m=m, M=m.capitalize(), res=resource),
            [m],
            [m, "Contradicts", resource],
            "change",
            role="query",
            gold=recs[gold_pos]["id"],
            cycle=c,
            gap=len(recs) - gold_pos,
        )

        # 4. tail so a query is never the last sentence of the corpus
        emit(
            f"The {part} of {m} was inspected after the fault.",
            [m],
            None,
            "persist",
            role="tail",
            cycle=c,
        )
    return recs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cycles", type=int, default=60)
    args = ap.parse_args()
    recs = build(args.cycles)
    body = "".join(json.dumps(r, sort_keys=True) + "\n" for r in recs)
    (HERE / "corpus-large.jsonl").write_text(body)
    digest = hashlib.sha256(body.encode()).hexdigest()
    (HERE / "corpus-large.sha256").write_text(digest + "  corpus-large.jsonl\n")
    queries = [r for r in recs if r["role"] == "query"]
    gaps = sorted({r["gap"] for r in queries})
    print(f"{len(recs)} sentences, {len(queries)} queries, seed {SEED}")
    print("gaps:", gaps)
    print("sha256:", digest)


if __name__ == "__main__":
    main()
