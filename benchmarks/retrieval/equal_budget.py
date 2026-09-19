#!/usr/bin/env python3
"""Day 3 — equal-token-budget retrieval comparison.

Plan: docs/plans/2026-09-15-core-pro-development.md, Day 3 ("repair retrieval
using an equal-budget experiment"). The gate is not "does the packer work"; it
is "does anything beat the strongest simple baseline at the SAME token budget,
measured on evidence spans rather than on file names".

Why this is a separate harness from run.py: run.py scores hit@k over file names
with no token accounting and no notion of which lines answer the query. Day 3
requires both — the claim being tested is about evidence per token, and a
file-name hit at a different budget is not comparable.

Arms (all constrained to the same token budget, all scored on the same spans):
  file_heads  — first 40-line window of each file in path order, blind to the
                query. The context a naive retriever pulls in.
  bm25        — Okapi BM25 over the same 40-line chunk grid, greedy fill. The
                strongest simple lexical baseline.
  current_pack— the shipped `physis-pro pack` at its shipped default.
  candidate   — the same binary at the diversity chosen on the dev split.

Token counts come from a Python mirror of physis-core/src/rag.rs::count_tokens.
The mirror is checked against the shipped binary's own per-chunk counts before
any arm is scored (`--check-tokenizer`, and automatically on every run when a
pack arm succeeds). A mirror that disagrees makes the budget claim meaningless,
so a mismatch is a hard failure, not a warning.

Usage:
    python3 benchmarks/retrieval/equal_budget.py                 # dev sweep + held-out
    python3 benchmarks/retrieval/equal_budget.py --check-tokenizer
    python3 benchmarks/retrieval/equal_budget.py --budget 3000 --json out.json
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shutil
import tempfile
import pathlib
import random
import re
import subprocess
import sys
import time
from dataclasses import dataclass

ROOT = pathlib.Path(__file__).resolve().parents[2]
BIN = ROOT / "target" / "release" / "physis-pro"
CASES = pathlib.Path(__file__).with_name("cases_day3.json")
OUT = pathlib.Path(__file__).with_name("day3-results.json")

# Mirrors src/pack.rs. If the packer changes these, the baselines stop sharing a
# chunk grid with the arm they are compared against.
CHUNK_LINES = 40
MAX_FILE_BYTES = 512 * 1024
SKIP_DIRS = {".git", "target", "node_modules", "vendor", "dist", "build",
             ".venv", "__pycache__"}

SWEEP = (0.0, 0.3, 1.0)
DEFAULT_DIVERSITY = 0.3
TOP_K = 50


# --------------------------------------------------------------------------
# token counter — mirror of physis-core/src/rag.rs
# --------------------------------------------------------------------------
def tokenize_word(w: str) -> int:
    return 1 + max(len(w) - 6, 0) // 4


def count_tokens(text: str) -> int:
    n = 0
    word: list[str] = []

    def flush() -> None:
        nonlocal n
        if word:
            n += tokenize_word("".join(word))
            word.clear()

    for c in text:
        if c.isspace():
            flush()
        elif c.isalnum():
            word.append(c)
        else:
            flush()
            n += 1
    flush()
    return max(n, 1)


# --------------------------------------------------------------------------
# corpus — mirror of src/pack.rs::collect_candidates / chunk_file
# --------------------------------------------------------------------------
@dataclass(frozen=True)
class Chunk:
    path: str
    start: int
    end: int
    text: str
    tokens: int


def _is_text(path: pathlib.Path) -> bool:
    """Mirror scanner::is_text_file: no NUL in the first 512 bytes."""
    try:
        with open(path, "rb") as fh:
            head = fh.read(512)
    except OSError:
        return False
    return b"\x00" not in head


def _chunk_file(rel: str, contents: str) -> list[Chunk]:
    # Rust's str::lines() splits on \n and drops a trailing \r; Python's
    # splitlines() splits on more separators, which would change the grid.
    lines = [ln[:-1] if ln.endswith("\r") else ln for ln in contents.split("\n")]
    if contents.endswith("\n"):
        lines.pop()
    out: list[Chunk] = []
    for idx in range(0, len(lines), CHUNK_LINES):
        window = lines[idx:idx + CHUNK_LINES]
        text = "\n".join(window)
        if not text.strip():
            continue
        # idx is already the 0-based line offset, so the 1-based start is idx+1
        # (Rust's `lines.chunks(n).enumerate()` yields a chunk index instead).
        start = idx + 1
        out.append(Chunk(rel, start, start + len(window) - 1, text, count_tokens(text)))
    return out


def corpus(root: pathlib.Path) -> list[Chunk]:
    chunks: list[Chunk] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS)
        for name in sorted(filenames):
            p = pathlib.Path(dirpath) / name
            try:
                if p.stat().st_size > MAX_FILE_BYTES:
                    continue
            except OSError:
                continue
            if not _is_text(p):
                continue
            try:
                contents = p.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError):
                continue  # Rust's read_to_string skips the same files
            chunks.extend(_chunk_file(str(p.relative_to(root)), contents))
    return chunks


def freeze(root: pathlib.Path, dest: pathlib.Path) -> None:
    """Copy the corpus so a concurrent writer cannot move it mid-run.

    A run that reads the live tree is not a frozen-corpus measurement: on
    2026-09-16 a second session edited `src/bin/doc_tui.rs` *during* a run, and
    the citation check caught it — the packer had cited `1881-1881` from a
    revision that no longer existed by the time the run scored. The check was
    right and the measurement was still void. Copy first, then measure.
    """
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS)
        rel = pathlib.Path(dirpath).relative_to(root)
        (dest / rel).mkdir(parents=True, exist_ok=True)
        for name in sorted(filenames):
            src = pathlib.Path(dirpath) / name
            if src.is_symlink() or not src.is_file():
                continue
            shutil.copy2(src, dest / rel / name)


def corpus_hash(root: pathlib.Path, chunks: list[Chunk]) -> str:
    h = hashlib.sha256()
    for path in sorted({c.path for c in chunks}):
        h.update(path.encode())
        h.update(b"\x00")
        h.update(hashlib.sha256((root / path).read_bytes()).digest())
    return h.hexdigest()[:16]


# --------------------------------------------------------------------------
# arms — each returns (selected chunks, seconds). Every arm obeys the budget.
# --------------------------------------------------------------------------
def arm_file_heads(chunks: list[Chunk], _query: str, budget: int):
    t0 = time.time()
    seen: set[str] = set()
    sel: list[Chunk] = []
    used = 0
    for c in sorted(chunks, key=lambda c: (c.path, c.start)):
        if c.path in seen or used + c.tokens > budget:
            continue
        seen.add(c.path)
        sel.append(c)
        used += c.tokens
    return sel, time.time() - t0


_WORD = re.compile(r"[a-z0-9]+")


def arm_bm25(chunks: list[Chunk], query: str, budget: int, k1: float = 1.2, b: float = 0.75):
    t0 = time.time()
    docs = [_WORD.findall(c.text.lower()) for c in chunks]
    n = len(docs)
    avgdl = (sum(len(d) for d in docs) / n) if n else 0.0
    df: dict[str, int] = {}
    for d in docs:
        for term in set(d):
            df[term] = df.get(term, 0) + 1
    qterms = _WORD.findall(query.lower())
    scores: list[tuple[float, int]] = []
    for i, d in enumerate(docs):
        tf: dict[str, int] = {}
        for term in d:
            tf[term] = tf.get(term, 0) + 1
        dl = len(d)
        s = 0.0
        for term in qterms:
            if term not in tf:
                continue
            idf = math.log(1 + (n - df[term] + 0.5) / (df[term] + 0.5))
            s += idf * (tf[term] * (k1 + 1)) / (tf[term] + k1 * (1 - b + b * dl / avgdl))
        if s > 0:
            scores.append((s, i))
    # Deterministic ties: higher score first, then document order.
    scores.sort(key=lambda t: (-t[0], t[1]))
    sel: list[Chunk] = []
    used = 0
    for _, i in scores:
        if used + chunks[i].tokens > budget:
            continue
        sel.append(chunks[i])
        used += chunks[i].tokens
    return sel, time.time() - t0


_CHUNK_LINE = re.compile(r"^\s+[\d.]+\s+(\S+):(\d+)-(\d+)\s+\((\d+) tokens\)\s*$")
_TOTAL_LINE = re.compile(r"^packed (\d+)/(\d+) tokens from (\d+) chunks")


def arm_pack(root: pathlib.Path, query: str, budget: int, diversity: float,
             data_dir: pathlib.Path, mode: str = "cosine", expand: bool = False):
    env = dict(os.environ, PHYSIS_ALLOW_DEV_LICENCE="1",
               PHYSIS_DATA_DIR=str(data_dir))
    argv = [str(BIN), "pack", str(root), query,
            "--budget", str(budget), "--top-k", str(TOP_K),
            "--diversity", str(diversity), "--mode", mode]
    if expand:
        argv.append("--expand")
    t0 = time.time()
    p = subprocess.run(argv,
                       cwd=ROOT, env=env, capture_output=True, text=True, timeout=1800)
    elapsed = time.time() - t0
    if p.returncode != 0:
        raise RuntimeError(f"pack failed: {p.stderr.strip().splitlines()[-1:]}")
    sel: list[Chunk] = []
    reported = None
    for line in p.stdout.splitlines():
        m = _CHUNK_LINE.match(line)
        if m:
            sel.append(Chunk(m.group(1), int(m.group(2)), int(m.group(3)), "", int(m.group(4))))
            continue
        t = _TOTAL_LINE.match(line)
        if t:
            reported = int(t.group(1))
    return sel, elapsed, reported


# --------------------------------------------------------------------------
# scoring
# --------------------------------------------------------------------------
def merge(intervals: list[tuple[int, int]]) -> list[list[int]]:
    merged: list[list[int]] = []
    for s, e in sorted(intervals):
        if merged and s <= merged[-1][1] + 1:
            merged[-1][1] = max(merged[-1][1], e)
        else:
            merged.append([s, e])
    return merged


def union_covers(intervals: list[tuple[int, int]], start: int, end: int) -> bool:
    """True when the merged selected windows cover [start, end] end to end."""
    return any(s <= start and end <= e for s, e in merge(intervals))


def line_coverage(intervals: list[tuple[int, int]], start: int, end: int) -> float:
    """Fraction of the required span's lines present in the selected windows.

    Partial credit matters here: a 40-line grid can only ever approximate a span
    that straddles a boundary, and "missed by one line" and "missed entirely"
    are different findings.
    """
    total = end - start + 1
    got = 0
    for s, e in merge(intervals):
        lo, hi = max(s, start), min(e, end)
        if lo <= hi:
            got += hi - lo + 1
    return got / total


def score(sel: list[Chunk], case: dict, root: pathlib.Path) -> dict:
    by_path: dict[str, list[tuple[int, int]]] = {}
    for c in sel:
        by_path.setdefault(c.path, []).append((c.start, c.end))

    spans = case["required_spans"]
    covered = [union_covers(by_path.get(s["path"], []), s["start"], s["end"]) for s in spans]
    line_cov = [line_coverage(by_path.get(s["path"], []), s["start"], s["end"]) for s in spans]

    # Citation resolution: every cited window must exist in the corpus revision.
    unresolved = []
    for path, ivs in by_path.items():
        f = root / path
        if not f.is_file():
            unresolved.append(f"{path} (missing)")
            continue
        nlines = sum(1 for _ in f.open("rb"))
        for s, e in ivs:
            if not (1 <= s <= e <= max(nlines, 1)):
                unresolved.append(f"{path}:{s}-{e} (outside file)")
    return {
        "case_hit": all(covered),
        "spans_covered": sum(covered),
        "spans_total": len(spans),
        "line_recall": sum(line_cov) / len(line_cov) if line_cov else 0.0,
        "unresolved_citations": unresolved,
        "paths": sorted(by_path),
        "tokens": sum(c.tokens for c in sel),
        "chunks": len(sel),
    }


# --------------------------------------------------------------------------
# driver
# --------------------------------------------------------------------------
def check_tokenizer(root: pathlib.Path, budget: int, data_dir: pathlib.Path) -> int:
    """Compare the Python mirror against the shipped counter, chunk by chunk."""
    if not BIN.exists():
        print(f"missing {BIN}; build it: cargo build --release --features mcp,cli --bin physis-pro")
        return 2
    sel, _, _ = arm_pack(root, "how does the engine pick which embedding model to use",
                         budget, DEFAULT_DIVERSITY, data_dir)
    local = {(c.path, c.start): c.tokens for c in corpus(root)}
    bad = 0
    for c in sel:
        want = local.get((c.path, c.start))
        if want != c.tokens:
            bad += 1
            print(f"BAD {c.path}:{c.start} rust={c.tokens} py={want}")
    print(f"tokenizer mirror: {len(sel) - bad}/{len(sel)} chunks agree "
          f"against {BIN.name} over {root.relative_to(ROOT)}")
    return 1 if bad else 0


def run_arm(arm: str, case: dict, root: pathlib.Path, budget: int,
            diversity: float, chunks: list[Chunk], data_dir: pathlib.Path) -> dict:
    """`arm` is one of the fixed names, `pack@<diversity>`, or `mode@<leg>`."""
    reported = None
    if arm == "file_heads":
        sel, secs = arm_file_heads(chunks, case["query"], budget)
    elif arm == "bm25":
        sel, secs = arm_bm25(chunks, case["query"], budget)
    elif arm == "current_pack":
        sel, secs, reported = arm_pack(root, case["query"], budget, DEFAULT_DIVERSITY, data_dir)
    elif arm.startswith("pack@"):
        sel, secs, reported = arm_pack(root, case["query"], budget,
                                       float(arm.split("@")[1]), data_dir)
    elif arm.startswith("mode@"):
        sel, secs, reported = arm_pack(root, case["query"], budget,
                                       DEFAULT_DIVERSITY, data_dir,
                                       mode=arm.split("@")[1])
    elif arm.startswith("expand@"):
        sel, secs, reported = arm_pack(root, case["query"], budget,
                                       DEFAULT_DIVERSITY, data_dir,
                                       mode=arm.split("@")[1], expand=True)
    else:
        raise ValueError(arm)

    res = score(sel, case, root)
    res["seconds"] = round(secs, 2)
    res["over_budget"] = res["tokens"] > budget
    if reported is not None:
        res["reported_total"] = reported
        if reported > budget:
            res["over_budget"] = True
    return res


def summarise(rows: dict[str, dict[str, dict]], arm: str) -> dict:
    cases = [r for r in rows.values() if arm in r]
    hits = sum(r[arm]["case_hit"] for r in cases)
    spans = sum(r[arm]["spans_covered"] for r in cases)
    total_spans = sum(r[arm]["spans_total"] for r in cases)
    return {
        "cases": len(cases),
        "case_hits": hits,
        "span_recall": spans / total_spans if total_spans else 0.0,
        "line_recall": sum(r[arm]["line_recall"] for r in cases) / len(cases) if cases else 0.0,
        "tokens": sum(r[arm]["tokens"] for r in cases),
        "seconds": round(sum(r[arm]["seconds"] for r in cases), 1),
        "over_budget": sum(r[arm]["over_budget"] for r in cases),
        "unresolved_citations": sum(len(r[arm]["unresolved_citations"]) for r in cases),
    }


def repo_relative(path: pathlib.Path) -> str:
    """Repo-relative when it is inside the repo, absolute otherwise.

    `--json /tmp/...` is a legitimate thing to ask for, and it used to crash
    the run *after* every arm had been scored — losing the artifact for a
    display-path reason.
    """
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def spec_to_arm(spec: str) -> str:
    """`mode:hybrid` -> `mode@hybrid`, `diversity:0.0` -> `pack@0.0`."""
    kind, _, val = spec.partition(":")
    if kind == "mode":
        return f"mode@{val}"
    if kind == "diversity":
        return f"pack@{val}"
    if kind == "expand":
        # `expand:<leg>` — adjacency expansion on top of the named ranking leg.
        return f"expand@{val}"
    raise SystemExit(
        f"unknown --candidate {spec!r}; use `mode:cosine|hybrid`, "
        "`expand:cosine|hybrid`, or `diversity:<float>`"
    )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--budget", type=int, default=3000)
    ap.add_argument("--json", type=pathlib.Path, default=OUT)
    ap.add_argument("--data-dir", default=".physis-bench")
    ap.add_argument("--cases", type=pathlib.Path, default=CASES)
    ap.add_argument("--generate", type=int, default=None,
                    help="sample N cases per corpus root from the FROZEN copy "
                         "instead of reading --cases. Generating here rather "
                         "than in a separate pass is the point: spans and the "
                         "difficulty axis are computed over the same immutable "
                         "copy that is then scored.")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--span", type=int, nargs=2, default=[6, 40],
                    metavar=("MIN", "MAX"))
    ap.add_argument("--candidate", default=None,
                    help="freeze the candidate arm instead of sweeping: "
                         "`mode:hybrid`, `expand:cosine`, `diversity:0.0`")
    ap.add_argument("--candidates", default=None,
                    help="comma-separated specs, scored as extra arms in one "
                         "pass: `mode:lexical,mode:auto`. A diagnostic — it "
                         "reports every leg on the same frozen set and gives no "
                         "single verdict, because choosing the winner after "
                         "seeing the numbers is cherry-picking.")
    ap.add_argument("--check-tokenizer", action="store_true")
    args = ap.parse_args()

    data_dir = ROOT / args.data_dir
    if args.generate:
        # No cases file to read: the spec comes from the generator, and the
        # corpus roots are the generator's own.
        import gen_cases  # local: gen_cases imports this module back

        spec = {"groups": gen_cases.GROUPS, "cases": []}
        cases_path = pathlib.Path(f"<generated seed={args.seed}>")
    else:
        # A relative --cases is relative to the shell's cwd, but the result
        # record wants a repo-relative path, so resolve before reading.
        cases_path = args.cases if args.cases.is_absolute() else (pathlib.Path.cwd() / args.cases)
        cases_path = cases_path.resolve()
        spec = json.loads(cases_path.read_text())
    groups = {name: ROOT / rel for name, rel in spec["groups"].items()}

    if args.check_tokenizer:
        rc = 0
        for name, root in groups.items():
            rc |= check_tokenizer(root, args.budget, data_dir)
        return rc

    if not BIN.exists():
        sys.exit(f"missing {BIN}; build it:\n  cargo build --release --features mcp,cli --bin physis-pro")

    revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT,
                              capture_output=True, text=True).stdout.strip()
    dirty = subprocess.run(["git", "status", "--porcelain"], cwd=ROOT,
                           capture_output=True, text=True).stdout.strip().splitlines()

    # Freeze: every arm and the citation check read one immutable copy.
    freeze_dir = pathlib.Path(tempfile.mkdtemp(prefix="physis-freeze-"))
    frozen = {name: freeze_dir / name for name in groups}
    for name, root in groups.items():
        freeze(root, frozen[name])

    if args.generate:
        # Sample from the frozen copies, so the ground truth and the difficulty
        # axis describe exactly what is scored.
        rng = random.Random(args.seed)
        spec["comment"] = (
            f"GENERATED in-process (seed={args.seed}, per_root={args.generate}, "
            f"span={args.span[0]}..{args.span[1]}) from the frozen corpus copy."
        )
        spec["generator"] = {
            "script": "benchmarks/retrieval/gen_cases.py (via --generate)",
            "seed": args.seed,
            "per_root": args.generate,
            "min_span": args.span[0],
            "max_span": args.span[1],
        }
        for name in groups:
            spec["cases"].extend(
                gen_cases.generate(frozen[name], name, args.generate, rng,
                                   args.span[0], args.span[1])
            )

    corpora = {name: corpus(frozen[name]) for name in groups}
    hashes = {name: corpus_hash(frozen[name], corpora[name]) for name in groups}
    for name, root in groups.items():
        print(f"corpus {name:5s} {root.relative_to(ROOT)} (frozen): "
              f"{len(corpora[name])} chunks, {len({c.path for c in corpora[name]})} files, "
              f"hash {hashes[name]}")

    # Every required span must resolve in the *frozen* copy before anything is
    # scored. A generated set whose spans no longer match the corpus would
    # otherwise be scored as a set of honest misses, which is the one failure
    # mode a benchmark must not have.
    bad_spans = []
    for case in spec["cases"]:
        root = frozen[case["group"]]
        for span in case["required_spans"]:
            path = root / span["path"]
            n = sum(1 for _ in path.open("rb")) if path.is_file() else 0
            if not (1 <= span["start"] <= span["end"] <= n):
                bad_spans.append(
                    f"{case['id']}: {span['path']}:{span['start']}-{span['end']} "
                    f"({n} lines)"
                )
    if bad_spans:
        sys.exit(
            "required spans do not resolve in the frozen corpus "
            f"({len(bad_spans)}):\n  " + "\n  ".join(bad_spans)
        )

    dev = [c for c in spec["cases"] if c["split"] == "dev"]
    heldout = [c for c in spec["cases"] if c["split"] == "heldout"]

    # ---- dev sweep: pick the candidate, unless one is frozen -------------
    dev_rows: dict[str, dict] = {}
    cand_dev: dict[str, dict] = {}
    candidate = args.candidate
    if dev:
        print(f"\n=== dev split ({len(dev)} cases) — tuning the candidate ===")
        for case in dev:
            root, chunks = frozen[case["group"]], corpora[case["group"]]
            row = dev_rows.setdefault(case["id"], {})
            for arm in ["file_heads", "bm25", "current_pack"] + [f"pack@{d}" for d in SWEEP]:
                row[arm] = run_arm(arm, case, root, args.budget,
                                   0.0 if arm == "current_pack" else DEFAULT_DIVERSITY,
                                   chunks, data_dir)
            print(f"  {case['id']:<22} " + "  ".join(
                f"{a}={'H' if row[a]['case_hit'] else '.'}"
                f"{row[a]['spans_covered']}/{row[a]['spans_total']}" for a in row))
        cand_arms = [f"pack@{d}" for d in SWEEP]
        cand_dev = {a: summarise(dev_rows, a) for a in cand_arms}
        best = max(cand_arms,
                   key=lambda a: (cand_dev[a]["case_hits"], cand_dev[a]["span_recall"]))
        print(f"  best on dev: {best} "
              f"({cand_dev[best]['case_hits']}/{cand_dev[best]['cases']} cases, "
              f"span recall {cand_dev[best]['span_recall']:.3f})")
        if candidate is None:
            candidate = f"diversity:{best.split('@')[1]}"
    if candidate is None:
        # No dev split and nothing frozen: the candidate is the shipped default.
        candidate = "mode:cosine"
    candidate_arm = spec_to_arm(candidate)

    # ---- held-out: score once -------------------------------------------
    multi = [spec.strip() for spec in args.candidates.split(",")] if args.candidates else None
    if multi:
        arms = ["file_heads", "bm25", "current_pack"] + [spec_to_arm(s) for s in multi]
        frozen_at = ", ".join(multi)
    else:
        arms = ["file_heads", "bm25", "current_pack", "candidate_pack"]
        frozen_at = candidate
    print(f"\n=== held-out split ({len(heldout)} cases) — candidate frozen at "
          f"{frozen_at} ===")
    rows: dict[str, dict] = {}
    for case in heldout:
        root, chunks = frozen[case["group"]], corpora[case["group"]]
        row = rows.setdefault(case["id"], {"group": case["group"],
                                           "classes": case["classes"]})
        for arm in arms:
            if arm == "candidate_pack":
                row[arm] = run_arm(candidate_arm, case, root, args.budget,
                                   DEFAULT_DIVERSITY, chunks, data_dir)
            else:
                row[arm] = run_arm(arm, case, root, args.budget, DEFAULT_DIVERSITY,
                                   chunks, data_dir)
        print(f"  {case['id']:<22} " + "  ".join(
            f"{a}={'H' if row[a]['case_hit'] else '.'}"
            f"{row[a]['spans_covered']}/{row[a]['spans_total']}"
            f"({row[a]['tokens']}t)" for a in arms))

    print("\n=== held-out summary ===")
    summary = {a: summarise(rows, a) for a in arms}
    for a in arms:
        s = summary[a]
        print(f"  {a:<15} cases {s['case_hits']}/{s['cases']}  "
              f"span recall {s['span_recall']:.3f}  "
              f"line recall {s['line_recall']:.3f}  tokens {s['tokens']}  "
              f"{s['seconds']}s  over-budget {s['over_budget']}  "
              f"unresolved citations {s['unresolved_citations']}")

    print("\n=== paired deltas: vs the strongest simple baseline (bm25), and "
          "vs the shipped arm (current_pack) ===")
    for a in arms:
        if a == "bm25":
            continue
        d_base = summary[a]["case_hits"] - summary["bm25"]["case_hits"]
        d_ship = summary[a]["case_hits"] - summary["current_pack"]["case_hits"]
        d_line = summary[a]["line_recall"] - summary["bm25"]["line_recall"]
        print(f"  {a:<15} vs bm25 {d_base:+d}   vs shipped {d_ship:+d}   "
              f"line recall {d_line:+.3f}")

    print("\n=== grouped by corpus root ===")
    for g in sorted(groups):
        grp = {k: v for k, v in rows.items() if v["group"] == g}
        if not grp:
            continue
        line = f"  {g:<5}"
        for a in arms:
            s = summarise(grp, a)
            line += f"  {a}={s['case_hits']}/{s['cases']}"
        print(line)

    print("\n=== grouped by class ===")
    classes = sorted({c for v in rows.values() for c in v["classes"]})
    for cls in classes:
        grp = {k: v for k, v in rows.items() if cls in v["classes"]}
        line = f"  {cls:<13}"
        for a in arms:
            s = summarise(grp, a)
            line += f"  {a}={s['case_hits']}/{s['cases']}"
        print(line)

    # Pre-registered promotion rule, written before the run.
    #
    #   CANDIDATE_BEATS_BOTH              strictly ahead of the shipped arm and
    #                                     of the strongest simple baseline
    #   CANDIDATE_CATCHES_UP_TO_BASELINE  ahead of the shipped arm, level with
    #                                     the baseline: this is what a "make the
    #                                     shipped default use the leg the
    #                                     baseline already represents" change
    #                                     looks like, and beating BM25 with BM25
    #                                     was never the goal
    #   KEEP_BASELINE                     anything else
    if multi:
        # No single verdict on purpose: with several legs scored on one set,
        # naming the best one afterwards would be selection on the test set.
        verdict = "DIAGNOSTIC_NO_VERDICT"
        print("\nverdict: DIAGNOSTIC_NO_VERDICT (several legs scored on one "
              "frozen set; read the per-class table, do not pick a winner)")
    else:
        cand, cur, base = (summary["candidate_pack"], summary["current_pack"],
                           summary["bm25"])
        if cand["case_hits"] > cur["case_hits"] and cand["case_hits"] > base["case_hits"]:
            verdict = "CANDIDATE_BEATS_BOTH"
        elif cand["case_hits"] > cur["case_hits"] and cand["case_hits"] >= base["case_hits"]:
            verdict = "CANDIDATE_CATCHES_UP_TO_BASELINE"
        else:
            verdict = "KEEP_BASELINE"
        print(f"\nverdict: {verdict}")
    print("answer arm: NOT MEASURED (no answer model configured in this run)")

    payload = {
        "revision": revision,
        "dirty_files": len(dirty),
        "cases_file": repo_relative(cases_path),
        "candidate": candidate,
        "candidates": multi,
        "budget": args.budget,
        "top_k": TOP_K,
        "frozen_corpus": True,
        "corpus": {g: {"root": str(groups[g].relative_to(ROOT)),
                       "hash": hashes[g],
                       "chunks": len(corpora[g]),
                       "files": len({c.path for c in corpora[g]})} for g in groups},
        "dev": dev_rows,
        "dev_summary": cand_dev,
        "heldout": rows,
        "heldout_summary": summary,
        "verdict": verdict,
        "answer_arm": "NOT_MEASURED",
    }
    json_path = args.json if args.json.is_absolute() else (pathlib.Path.cwd() / args.json)
    json_path = json_path.resolve()
    json_path.write_text(json.dumps(payload, indent=2))
    print(f"wrote {repo_relative(json_path)}")

    hard_fail = any(s["over_budget"] or s["unresolved_citations"]
                    for s in summary.values())
    return 1 if hard_fail else 0


if __name__ == "__main__":
    sys.exit(main())
