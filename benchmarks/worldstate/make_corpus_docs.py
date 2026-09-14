#!/usr/bin/env python3
"""The unseen-prose corpus: this repository's own documentation.

E59's chunker was written beside `corpus.jsonl`, so every number it scored there
is a number about text it was authored against. This corpus fixes that, and it
solves the harder half — where does gold come from on text nobody annotated? —
with a trick that costs nothing and cannot be gamed:

    **Backticked spans in the repo's Markdown are an entity annotation written
    by an author who had never heard of this extractor.**

`act::bearing_on`, `TokenFixedRetriever`, `RECUR_GAP`, `worldstate.rs` — every
one is a thing the prose is about, marked as a thing by someone documenting
code. The chunker never sees the backticks: they are stripped from `text`, and
kept only in `entities`.

Known bias, stated before any number is read: backticked terms are
identifier-shaped, so the `id` rule is favoured and the noun-phrase rule is not.
That makes this corpus a hard case for E59's chunker and an easy one for E58's
id rule — the opposite of `corpus.jsonl`, which is the point of running both.

Sentences are kept only when they carry at least one backticked span, so every
record has a gold set. Order is document order, which is the position coordinate.

Usage:  python3 benchmarks/worldstate/make_corpus_docs.py
Writes: corpus-docs.jsonl, corpus-docs.sha256
"""
import hashlib
import json
import pathlib
import re

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]

SOURCES = [
    "docs/WHAT_PHYSIS_IS.md",
    "RESCOPE.md",
    "physis-core/research/E53_TRANSFORM_ANALOGY.md",
    "docs/plans/2026-09-13-research-agenda.md",
]

TICK = re.compile(r"`([^`\n]{2,60})`")
SENT = re.compile(r"(?<=[.!?])\s+")


def key(s: str) -> str:
    """Same normalisation the Rust side uses: lowercase, spaces to hyphens."""
    s = s.strip().strip("`").lower()
    s = re.sub(r"[^a-z0-9\-_. /]", "", s)
    return s.replace(" ", "-").strip("-")


def clean(line: str) -> str:
    line = re.sub(r"^#{1,6}\s*", "", line)
    line = re.sub(r"^[-*>]\s*", "", line)
    line = re.sub(r"\*\*([^*]+)\*\*", r"\1", line)
    line = re.sub(r"\*([^*]+)\*", r"\1", line)
    line = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", line)
    return line.strip()


def build():
    recs = []
    for src in SOURCES:
        path = ROOT / src
        if not path.exists():
            continue
        in_code = False
        for raw in path.read_text().splitlines():
            if raw.strip().startswith("```"):
                in_code = not in_code
                continue
            if in_code or "|" in raw:          # skip code blocks and tables
                continue
            line = clean(raw)
            if len(line) < 40:
                continue
            for sent in SENT.split(line):
                ticks = TICK.findall(sent)
                if not ticks:
                    continue
                text = TICK.sub(r"\1", sent).strip()
                if len(text) < 40 or len(text) > 400:
                    continue
                ents = []
                for t in ticks:
                    k = key(t)
                    if k and k not in ents:
                        ents.append(k)
                        # a path contributes its last segment and that
                        # segment's stem, matching the chunker's phrase+head
                        # convention. Splitting on "." as well would emit "rs"
                        # from "models.rs", which is a file extension, not a
                        # thing the prose is about.
                        seg = k.split("/")[-1]
                        stem = seg.rsplit(".", 1)[0] if "." in seg else seg
                        for head in (seg, stem):
                            if len(head) > 2 and head != k and head not in ents:
                                ents.append(head)
                if ents:
                    recs.append(
                        {
                            "pos": len(recs),
                            "id": f"d{len(recs):04d}",
                            "text": text,
                            "entities": ents,
                            "relation": None,
                            "transition": None,
                            "source": src,
                            "gold_from": "backticks",
                        }
                    )
    return recs


def main():
    recs = build()
    body = "".join(json.dumps(r, sort_keys=True) + "\n" for r in recs)
    (HERE / "corpus-docs.jsonl").write_text(body)
    digest = hashlib.sha256(body.encode()).hexdigest()
    (HERE / "corpus-docs.sha256").write_text(digest + "  corpus-docs.jsonl\n")
    ents = sum(len(r["entities"]) for r in recs)
    print(f"{len(recs)} sentences, {ents} gold entity mentions")
    print("sources:", {s: sum(1 for r in recs if r["source"] == s) for s in SOURCES})
    print("sha256:", digest)


if __name__ == "__main__":
    main()
