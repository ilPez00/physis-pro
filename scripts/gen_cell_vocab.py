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
ONTOLOGY_RS = "physis-core/src/ontology.rs"
ANCHORS = "physis-core/config/mode_anchors_ontology.json"


def words(s):
    return [w for w in re.findall(r"[a-z0-9]+", (s or "").lower()) if len(w) > 2]


def load_list():
    """The config files Core actually classifies with.

    Globbing config/*.json is WRONG and was: `space_ontology.json` and
    `time_ontology.json` exist on disk but appear in neither
    `BUILTIN_ONTOLOGIES` nor `EXTRA_ONTOLOGIES`, so `load_all` never sees them
    while a glob does. They carry entries in cells that ARE loaded, which
    shifted the even/odd split on 12 of 33 cells -- silently giving arm B a
    different train half from arm A and making the comparison meaningless.

    `ONTOLOGY_SOURCES` is deliberately excluded: it is a file registry, not a
    load list, and says so in its own doc comment.

    So the load list is parsed from the two statics that define it, and stays
    correct when they change.
    """
    src = open(ONTOLOGY_RS).read()
    keep = set()
    for name in ("BUILTIN_ONTOLOGIES", "EXTRA_ONTOLOGIES"):
        i = src.index("pub static %s" % name)
        j = src.index("\n];", i)
        keep.update(re.findall(r'include_str!\("\.\./config/([^"]+)"\)', src[i:j]))
    return keep


def load():
    """Corpus entries (excluding the anchors) and the anchor vocabulary."""
    anchor_names, anchors = set(), {}
    with open(ANCHORS) as fh:
        for e in json.load(fh).get("domains", []):
            if e.get("domain") and e.get("mode"):
                anchor_names.add(e["name"])
                anchors[(e["domain"], e["mode"])] = e
    loaded = load_list()
    entries = []
    for path in sorted(glob.glob(CONFIG_GLOB)):
        if os.path.basename(path) not in loaded:
            continue
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
    """Arm B: the model proposes ADDITIONAL terms in the register the train
    entries already use.

    NOT via dspy, and the reason is measured rather than assumed. qwen3:4b is a
    reasoning model: with thinking enabled and a JSON schema imposed it never
    terminates (probe: TimeoutError at 600s, num_predict=600). dspy's adapter
    therefore never receives parseable output and retries at ~145s a go, which
    presents as a silent stall -- two runs produced nothing in 20+ minutes.
    Going straight at ollama's native structured output with think=False
    returns a valid object in ~75s.

    Pydantic is still the contract -- it is what constrains the shape and what
    validates the response. Only dspy's adapter negotiation is dropped.

    Also records how much of each response was COPIED from its own prompt. The
    probe returned 9/9 input terms for HEAL/LIFT, so echo is the failure mode to
    quantify, not a curiosity: a vocabulary that only repeats its train examples
    is a strictly worse arm A.
    """
    import urllib.request
    from pydantic import BaseModel, Field

    class CellVocab(BaseModel):
        terms: list[str] = Field(
            description="Lowercase single words a practitioner would actually "
            "write in this category. No punctuation, no phrases."
        )

    schema = CellVocab.model_json_schema()

    def ask(prompt, timeout=900):
        body = {
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "stream": False,
            "format": schema,
            "think": False,
            "options": {"temperature": 0.0, "num_predict": 600},
        }
        req = urllib.request.Request(
            "http://localhost:11434/api/chat",
            data=json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=timeout) as r:
            d = json.load(r)
        return CellVocab.model_validate_json(d["message"].get("content", ""))

    out, echo_stats = {}, {}
    cells = sorted(train.keys())
    for i, cell in enumerate(cells, 1):
        es = train[cell]
        anchor = anchors.get(cell, {})
        hints = ", ".join(anchor.get("hints", [])[:6])
        examples = "; ".join(text_of(e)[:70] for e in es[:5])
        prompt = (
            f"Ontology cell {cell[0]}/{cell[1]} of a work-activity ontology.\n"
            f"Defined by: {hints}\n"
            f"Real entries already filed here: {examples}\n\n"
            "List single lowercase words that would plausibly occur in OTHER "
            "entries of this same cell. Do NOT repeat any word above. Do not "
            "restate the cell name."
        )
        # what the model was shown, for the echo measurement
        shown = set(words(hints)) | set(words(examples)) | set(words(cell[0])) | set(words(cell[1]))
        try:
            terms = [w.lower().strip() for w in ask(prompt).terms if w and w.isalpha()]
        except Exception as exc:  # a failed cell must not poison the artifact
            print(f"  [{i}/{len(cells)}] {cell} FAILED: {type(exc).__name__}", file=sys.stderr)
            terms = []
        terms = sorted(set(terms))[:k]
        copied = [w for w in terms if w in shown]
        echo_stats["%s/%s" % cell] = {
            "n_terms": len(terms),
            "n_copied_from_prompt": len(copied),
            "echo_rate": round(len(copied) / len(terms), 3) if terms else None,
        }
        out["%s/%s" % cell] = terms
        print(
            f"  [{i}/{len(cells)}] {cell[0]}/{cell[1]}: {len(terms)} terms, "
            f"{len(copied)} echoed", file=sys.stderr, flush=True
        )
    return out, echo_stats


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
        print("generating with", args.model, "(temperature 0, think=False)...")
        gen, echo = vocab_generated(train, anchors, model=args.model)
        artifact["generated"] = gen
        artifact["generated_echo"] = echo
        rates = [v["echo_rate"] for v in echo.values() if v["echo_rate"] is not None]
        if rates:
            artifact["generated_echo_mean"] = round(sum(rates) / len(rates), 3)
            print(f"mean echo rate {artifact['generated_echo_mean']:.3f} over {len(rates)} cells")

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
