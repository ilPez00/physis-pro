#!/usr/bin/env python3
"""Generate retrieval holdout cases from a parameterized spec.

Why this exists
---------------
Four hand-authored retrieval holdouts were spent on 2026-09-16, and two of them
returned **opposite verdicts on the same comparison** (set #2: BM25 4/12 vs
cosine 3/12; set #4: BM25 3/12 vs cosine 5/12). Nothing about the packer changed
between them — the difficulty of the cases did, because difficulty was an
accident of authoring rather than a controlled variable.

CausalARC (arXiv:2509.03636, see `research/RELATED_WORK_2026-09-16.md` §3) makes
the fix explicit: sample task instances from a parameterized generator, report
performance over the distribution, and keep the difficulty knob visible. This is
that generator for retrieval.

Ground truth is by construction, not by judgement: a case is a definition site
found by pattern, and its query is built from that site. Nothing is asserted
about what "should" be retrieved.

Knobs recorded per case (and emitted as classes so the harness's existing
grouped-by-class report shows them):

    template      id | sig | body      how the query was built
    df            low | mid | high     mean corpus document-frequency of the
                                       query's tokens — the axis that decides
                                       whether a lexical leg can work at all
    span          short | mid | long    span length in lines

`df` is the knob that matters. A query built from tokens that appear in a
handful of chunks is what BM25 is for; a query built from tokens that appear
everywhere matches everything and needs the semantic leg. The `body` template
deliberately draws from the *most* common words available in its span, so the
generator produces both ends of the axis on purpose rather than by luck.

Usage
-----
    python3 benchmarks/retrieval/gen_cases.py --per-root 30 --seed 1 \
        --out benchmarks/retrieval/cases_gen1.json

Then score it like any other frozen set:

    python3 benchmarks/retrieval/equal_budget.py \
        --cases benchmarks/retrieval/cases_gen1.json --candidate mode:auto
"""
from __future__ import annotations

import argparse
import json
import pathlib
import random
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))


def _eb():
    """The harness module, imported lazily.

    It imports this one back when `--generate` is used, so the cycle has to be
    resolved at call time rather than at import time. Importing it also gives
    the generator the same chunking the scorer uses, which is what makes the
    `df` axis mean the same thing on both sides.
    """
    import equal_budget

    return equal_budget

GROUPS = {"core": "physis-core/src", "pro": "src"}

SKIP_DIRS = {".git", "target", "node_modules", "vendor", "dist", "build",
             ".venv", "__pycache__"}

# `pub fn name(` / `pub struct Name` / `pub enum Name` / `pub trait Name`, with
# the optional visibility marker. Deliberately shallow: this finds *definition
# sites*, and anything it misses simply is not sampled.
DEF = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(fn|struct|enum|trait)\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)"
)

WORD = re.compile(r"[A-Za-z][A-Za-z0-9_]{2,}")

# Words that carry no retrieval signal on their own; dropping them keeps a
# `body` query from being a bag of grammar.
STOPWORDS = {
    "the", "and", "for", "with", "that", "this", "from", "into", "not", "are",
    "was", "were", "has", "have", "had", "its", "his", "her", "their", "they",
    "you", "your", "our", "but", "all", "any", "can", "will", "would", "should",
    "must", "may", "one", "two", "use", "used", "using", "when", "where",
    "which", "while", "then", "than", "them", "these", "those", "there", "here",
    "only", "also", "more", "most", "some", "such", "same", "each", "other",
    "own", "out", "over", "under", "after", "before", "because", "about",
}


def is_text(path: pathlib.Path) -> bool:
    try:
        with open(path, "rb") as fh:
            return b"\x00" not in fh.read(512)
    except OSError:
        return False


def read_lines(root: pathlib.Path, rel: str) -> list[str]:
    """Lines as the scorer counts them.

    `str.split("\n")` on a newline-terminated file yields a phantom empty last
    element, which made `end` land one line past the end of the file and the
    harness's span check reject two generated cases. Mirror the scorer's rule
    exactly.
    """
    text = (root / rel).read_text(encoding="utf-8")
    lines = [ln[:-1] if ln.endswith("\r") else ln for ln in text.split("\n")]
    if text.endswith("\n"):
        lines.pop()
    return lines


def find_definitions(root: pathlib.Path) -> dict[str, list[tuple[int, str, str]]]:
    """rel path -> [(1-based line, kind, symbol name)]"""
    out: dict[str, list[tuple[int, str, str]]] = {}
    for dirpath, dirnames, filenames in __import__("os").walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS)
        for name in sorted(filenames):
            if not name.endswith(".rs"):
                continue
            p = pathlib.Path(dirpath) / name
            if not is_text(p):
                continue
            rel = str(p.relative_to(root))
            try:
                lines = read_lines(root, rel)
            except (OSError, UnicodeDecodeError):
                continue
            sites = []
            for i, line in enumerate(lines, start=1):
                m = DEF.match(line)
                if m:
                    sites.append((i, m.group(1), m.group(2)))
            if sites:
                out[rel] = sites
    return out


def tokens(text: str) -> list[str]:
    return [w for w in WORD.findall(text.lower()) if w not in STOPWORDS]


def build_query(template: str, symbol: str, def_line: str, span_text: str,
                rng: random.Random, df: dict[str, int]) -> str | None:
    if template == "id":
        return symbol
    if template == "sig":
        return def_line.strip().rstrip("{").strip()
    # `body`: words from the span that are *not* on the definition line and do
    # not spell the symbol, so lexical overlap with the span is genuinely low.
    body_lines = [
        ln for ln in span_text.split("\n")
        if ln.strip() and ln.strip() != def_line.strip()
        and symbol not in ln and symbol.lower() not in ln.lower()
    ]
    words = [w for ln in body_lines for w in tokens(ln)]
    # De-duplicate while preserving order, then take a sample.
    unique: list[str] = []
    for w in words:
        if w not in unique:
            unique.append(w)
    if len(unique) < 4:
        return None
    # Draw from the most common words available, so this template lands at the
    # "lexical cannot help here" end of the df axis on purpose.
    ranked = sorted(unique, key=lambda w: (-df.get(w, 0), w))
    pool = ranked[: max(4, len(ranked) // 2)]
    picked = rng.sample(pool, min(8, len(pool)))
    return " ".join(picked)


def document_frequency(chunks) -> dict[str, int]:
    """token -> number of chunks containing it, over the frozen grid."""
    df: dict[str, int] = {}
    for chunk in chunks:
        for word in set(tokens(chunk.text)):
            df[word] = df.get(word, 0) + 1
    return df


def df_bin(query: str, df: dict[str, int], n_chunks: int) -> tuple[str, float]:
    """Mean fraction of the corpus that contains the query's tokens.

    Low means the query is distinctive (lexical-friendly); high means its words
    are everywhere (needs meaning)."""
    q = set(tokens(query))
    if not q or n_chunks == 0:
        return "low", 0.0
    frac = sum(df.get(w, 0) for w in q) / (len(q) * n_chunks)
    if frac >= 0.10:
        return "high", frac
    if frac >= 0.01:
        return "mid", frac
    return "low", frac


def span_bin(n: int) -> str:
    if n < 12:
        return "short"
    if n < 24:
        return "mid"
    return "long"


def generate(root: pathlib.Path, group: str, per_root: int, rng: random.Random,
             min_span: int, max_span: int) -> list[dict]:
    chunks = _eb().corpus(root)
    df = document_frequency(chunks)
    defs = find_definitions(root)
    files = sorted(defs)
    rng.shuffle(files)
    templates = ["id", "sig", "body"]
    cases: list[dict] = []
    used_files: set[str] = set()
    template_i = 0
    for rel in files:
        if len(cases) >= per_root:
            break
        if rel in used_files:
            continue
        used_files.add(rel)
        lines = read_lines(root, rel)
        n_lines = len(lines)
        sites = defs[rel]
        line, kind, symbol = rng.choice(sites)
        span_len = rng.randint(min_span, max_span)
        start = line
        end = min(start + span_len - 1, n_lines)
        if end - start + 1 < 4:
            continue
        span_text = "\n".join(lines[start - 1:end])
        template = templates[template_i % len(templates)]
        template_i += 1
        query = build_query(template, symbol, lines[line - 1], span_text, rng, df)
        if query is None:
            continue
        db, df_frac = df_bin(query, df, len(chunks))
        sb = span_bin(end - start + 1)
        cases.append({
            "id": f"gen-{group}-{len(cases):03d}-{template}",
            "group": group,
            "split": "heldout",
            "classes": ["generated", f"{template}-query", f"df-{db}", f"span-{sb}"],
            "query": query,
            "required_spans": [{
                "path": rel,
                "start": start,
                "end": end,
                "why": f"{kind} `{symbol}` at {rel}:{line}",
            }],
            # Knobs kept on the record even though the harness only reads
            # `classes`: they are what makes the difficulty auditable.
            "knobs": {
                "template": template,
                "symbol": symbol,
                "kind": kind,
                "def_line": line,
                "span_lines": end - start + 1,
                "file_lines": n_lines,
                "definitions_in_file": len(sites),
                "query_tokens": len(tokens(query)),
                "df_fraction": round(df_frac, 5),
            },
        })
    return cases


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--per-root", type=int, default=30,
                    help="cases to sample from each corpus root")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--min-span", type=int, default=6)
    ap.add_argument("--max-span", type=int, default=40)
    ap.add_argument("--out", type=pathlib.Path, required=True)
    args = ap.parse_args()

    out = args.out if args.out.is_absolute() else (pathlib.Path.cwd() / args.out)
    out = out.resolve()
    if out.exists():
        sys.exit(f"{out} exists; refusing to overwrite a frozen set")

    rng = random.Random(args.seed)
    cases: list[dict] = []
    for group, rel in GROUPS.items():
        cases.extend(generate(ROOT / rel, group, args.per_root, rng,
                              args.min_span, args.max_span))

    spec = {
        "comment": (
            "GENERATED holdout — do not hand-edit. Produced by "
            "benchmarks/retrieval/gen_cases.py "
            f"(seed={args.seed}, per_root={args.per_root}, "
            f"span={args.min_span}..{args.max_span}). Ground truth is by "
            "construction: a case is a definition site found by pattern and its "
            "query is built from that site. Regenerate rather than edit, and "
            "regenerate immediately before scoring so the spans match the "
            "frozen corpus."
        ),
        "generator": {
            "script": "benchmarks/retrieval/gen_cases.py",
            "seed": args.seed,
            "per_root": args.per_root,
            "min_span": args.min_span,
            "max_span": args.max_span,
        },
        "groups": GROUPS,
        "cases": cases,
    }
    out.write_text(json.dumps(spec, indent=2) + "\n")
    from collections import Counter
    tpl = Counter(c["knobs"]["template"] for c in cases)
    dfb = Counter(k for c in cases for k in c["classes"] if k.startswith("df-"))
    sp = Counter(k for c in cases for k in c["classes"] if k.startswith("span-"))
    print(f"wrote {out.relative_to(ROOT)}: {len(cases)} cases")
    print(f"  template {dict(tpl)}")
    print(f"  df       {dict(dfb)}")
    print(f"  span     {dict(sp)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
