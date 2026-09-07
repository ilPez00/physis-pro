#!/usr/bin/env python3
"""Author a per-cell vocabulary for physis's 5x14 grid, two ways, and write both
to disk as reviewable artifacts.

WHY THIS EXISTS
---------------
Iteration 38c established the remaining blocker precisely. A dependency
constraint needs a controlled vocabulary that is *informative about the grid*,
and physis has one -- config/mode_anchors_ontology.json, 70 hand-authored cells
-- but it scored AUC 0.532 against a 0.650 cosine baseline. The diagnosis: the
anchors define each cell in THEIR OWN idiom ("deadlift", "squat heavy" for
HEAL/LIFT) while corpus entries are written in theirs, and purely lexical
overlap between two independently-authored vocabularies is thin.

So the prerequisite is a vocabulary in the ENTRIES' idiom.

THE CIRCULARITY THIS AVOIDS
---------------------------
Building a cell's vocabulary from its entries and then testing whether those
entries match it proves nothing. Every cell is therefore split
deterministically into train/test halves; vocabulary is built from TRAIN only
and scored on TEST (plus injected misfilings, in the Rust harness).

THE CONTROL THAT MAKES THE LLM FALSIFIABLE
------------------------------------------
Two vocabularies are produced from identical inputs, differing in one thing:

  A  frequency  -- top IDF-weighted terms from the cell's TRAIN entries. No LLM.
  B  generated  -- qwen3:4b, given the cell's anchor and its TRAIN examples,
                   asked for ADDITIONAL terms in the same register.

The question is not "does a vocabulary help" but "does B beat A". If plain term
frequency suffices, the model contributes nothing and should not be in the
pipeline.

DETERMINISM
-----------
temperature=0, and the output is written once to a JSON artifact that the
scorer reads. The model is an authoring assistant whose product is committed
and reviewable -- not a runtime dependency. Everything downstream stays
byte-reproducible, which two production bugs were fixed this session to
achieve.

Usage:
  ~/.venv/bin/python scripts/gen_cell_vocab.py --out cell_vocab.json [--no-llm]
"""
import argparse
import collections
import glob
import json
import math
import os
import re
import sys

CONFIG_GLOB = "physis-core/config/*.json"
ANCHORS = "physis-core/config/mode_anchors_ontology.json"


def words(s):
    return [w for w in re.findall(r"[a-z0-9]+", (s or "").lower()) if len(w) > 2]


def load():
    """Corpus entries (excluding the anchors) and the anchor vocabulary."""
    anchor_names, anchors = set(), {}
    with open(ANCHORS) as fh:
        for e in json.load(fh).get("domains", []):
            if e.get("domain") and e.get("mode"):
                anchor_names.add(e["name"])
                anchors[(e["domain"], e["mode"])] = e
    entries = []
    for path in sorted(glob.glob(CONFIG_GLOB)):
        with open(path) as fh:
            try:
                doc = json.load(fh)
            except Exception:
                continue
        for e in doc.get("domains", []):
            if e.get("domain") and e.get("mode") and e["name"] not in anchor_names:
                entries.append(e)
    return entries, anchors


def split(entries):
    """Deterministic train/test halves within each cell. Sorted by name first so
    the split never depends on file order."""
    by_cell = collections.defaultdict(list)
    for e in entries:
        by_cell[(e["domain"], e["mode"])].append(e)
    train, test = collections.defaultdict(list), collections.defaultdict(list)
    for cell, es in by_cell.items():
        for i, e in enumerate(sorted(es, key=lambda x: x["name"])):
            (train if i % 2 == 0 else test)[cell].append(e)
    return train, test


def text_of(e):
    return " ".join([e.get("name", "")] + list(e.get("hints", [])))


def vocab_frequency(train, k=25):
    """Arm A: top IDF-weighted terms per cell, from TRAIN entries only."""
    df = collections.Counter()
    ncell = len(train)
    for cell, es in train.items():
        for w in {w for e in es for w in words(text_of(e))}:
            df[w] += 1
    out = {}
    for cell, es in train.items():
        tf = collections.Counter(w for e in es for w in words(text_of(e)))
        scored = {
            w: c * math.log(ncell / (1 + df[w]))
            for w, c in tf.items()
        }
        top = sorted(scored.items(), key=lambda kv: (-kv[1], kv[0]))[:k]
        out["%s/%s" % cell] = [w for w, _ in top]
    return out


def vocab_generated(train, anchors, k=25, model="qwen3:4b"):
    """Arm B: qwen3:4b proposes ADDITIONAL terms in the register the train
    entries already use. Pydantic constrains the output shape -- the paper's
    'ontology as validation contract' applied to the generator itself."""
    import dspy
    from pydantic import BaseModel, Field

    class CellVocab(BaseModel):
        terms: list[str] = Field(
            description="Lowercase single words a practitioner would actually "
            "write in this category. No punctuation, no phrases."
        )

    class ProposeVocab(dspy.Signature):
        """Propose vocabulary for one cell of a work-activity ontology.

        Match the register of the examples given. Produce words that would
        plausibly occur in OTHER entries of the same cell -- not synonyms of the
        examples, and not restatements of the cell name."""

        domain: str = dspy.InputField(desc="what area of life this cell covers")
        mode: str = dspy.InputField(desc="what kind of activity")
        anchor_hints: str = dspy.InputField(desc="how the cell was defined")
        examples: str = dspy.InputField(desc="real entries filed in this cell")
        vocab: CellVocab = dspy.OutputField()

    lm = dspy.LM(
        f"ollama_chat/{model}",
        api_base="http://localhost:11434",
        api_key="",
        temperature=0.0,
        max_tokens=600,
    )
    dspy.configure(lm=lm)
    propose = dspy.Predict(ProposeVocab)

    out = {}
    cells = sorted(train.keys())
    for i, cell in enumerate(cells, 1):
        es = train[cell]
        anchor = anchors.get(cell, {})
        try:
            r = propose(
                domain=cell[0],
                mode=cell[1],
                anchor_hints=", ".join(anchor.get("hints", [])[:6]),
                examples="; ".join(text_of(e)[:90] for e in es[:8]),
            )
            terms = [w.lower().strip() for w in r.vocab.terms if w and w.isalpha()]
        except Exception as exc:  # a failed cell must not poison the artifact
            print(f"  [{i}/{len(cells)}] {cell} FAILED: {type(exc).__name__}", file=sys.stderr)
            terms = []
        out["%s/%s" % cell] = sorted(set(terms))[:k]
        print(f"  [{i}/{len(cells)}] {cell[0]}/{cell[1]}: {len(terms)} terms", file=sys.stderr)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="cell_vocab.json")
    ap.add_argument("--no-llm", action="store_true", help="arm A only")
    ap.add_argument("--model", default="qwen3:4b")
    args = ap.parse_args()

    if not os.path.exists(ANCHORS):
        sys.exit(f"run from the physis-pro root; {ANCHORS} not found")

    entries, anchors = load()
    train, test = split(entries)
    print(f"{len(entries)} entries (anchors excluded), {len(train)} cells")
    print(f"train {sum(len(v) for v in train.values())} / test {sum(len(v) for v in test.values())}")

    artifact = {
        "note": "Generated once, committed, read by the scorer. The LLM is an "
                "authoring assistant, not a runtime dependency.",
        "model": None if args.no_llm else args.model,
        "temperature": 0.0,
        "split": "deterministic even/odd by name within each cell; TRAIN only",
        "test_entries": {
            "%s/%s" % c: [e["name"] for e in es] for c, es in sorted(test.items())
        },
        "frequency": vocab_frequency(train),
    }
    if not args.no_llm:
        print("generating with", args.model, "(temperature 0)...")
        artifact["generated"] = vocab_generated(train, anchors, model=args.model)

    with open(args.out, "w") as fh:
        json.dump(artifact, fh, indent=1, sort_keys=True)
    fa = sum(len(v) for v in artifact["frequency"].values())
    print(f"wrote {args.out}: frequency {fa} terms", end="")
    if "generated" in artifact:
        print(f", generated {sum(len(v) for v in artifact['generated'].values())} terms")
    else:
        print()


if __name__ == "__main__":
    main()
