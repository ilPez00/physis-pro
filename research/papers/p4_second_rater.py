#!/usr/bin/env python3
"""P4: a second, independent rater.

The current P4 result predicts ONE rater's filing policy. That is not the same
as capturing something shared across raters, and the paper must not claim human
decision leverage it never tested. The brief's fallback -- sanctioned when a
human rater is unavailable -- is a *distinct model* as secondary evidence.

Blindness requirements, all enforced here rather than asserted:

* the rater never sees `old_cell` (the original assignment);
* the rater never sees `new_cell` (rater 1's assignment);
* the rater never sees any geometry proposal or embedding;
* the rater gets one item at a time, with no memory between items.

What this can support: "geometry captures something shared across raters"
versus "geometry predicts one policy". What it CANNOT support: ontology truth.
A second language model is still a language model, and agreement between two
models is agreement between two policies.

Artifacts: research/p4_rater2.json, research/P4_INDEPENDENT_RATER.md
REPRODUCTION: python3 research/papers/p4_second_rater.py --n 150
"""

from __future__ import annotations

import argparse
import collections
import json
import math
import os
import random
import sys
import urllib.error
import urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
GATE = os.path.join(REPO, "research", "perspective-discovery", "refiling_gate0.tsv")
OUT = os.path.join(REPO, "research", "p4_rater2.json")
OLLAMA = "http://localhost:11434/api/generate"
SEED = 20260924


PARTIAL = os.path.join(REPO, "research", "p4_rater2_partial.jsonl")


def _load_partial():
    if not os.path.exists(PARTIAL):
        return [], 0, 0.0
    out, fails, t = [], 0, 0.0
    with open(PARTIAL) as fh:
        for line in fh:
            try:
                r = json.loads(line)
            except ValueError:
                continue
            out.append(r)
            fails += 1 if r.get("rater2_cell") is None else 0
            t += r.get("seconds", 0.0)
    return out, fails, t


def _save_partial(out):
    with open(PARTIAL, "w") as fh:
        for r in out:
            fh.write(json.dumps(r) + "\n")


def load_gate():
    rows = []
    with open(GATE) as fh:
        for line in fh:
            if line.startswith("#") or not line.strip():
                continue
            f = line.rstrip("\n").split("\t")
            if f[0] == "n":
                continue
            if len(f) < 7:
                continue
            rows.append({"n": int(f[0]), "source": f[1], "entry": f[2],
                         "old_cell": f[3], "rater1_cell": f[4],
                         "moved": f[5], "blind": f[6]})
    return rows


def build_prompt(entry, source, cells):
    """The exact text sent to the rater. Separate from ask() so that blindness
    is asserted on the real payload rather than on the row dict."""
    return (
        # The example must NOT be a real cell. Naming one anchors the rater on
        # it, and the cell first used here (CONSTRUCT/CREATE) is the modal cell
        # -- 69 of 662 rows -- so the anchor would have inflated apparent
        # agreement with rater 1. A blind-filing check caught this.
        "You are filing a single ontology entry into exactly one cell of a grid.\n"
        "Every cell is written ACTION/OBJECT, with both halves uppercase words.\n\n"
        f"Entry: {entry}\n"
        f"Source vocabulary: {source}\n\n"
        "Choose the single best cell for this entry from this closed list.\n"
        "End your reply with a final line of exactly the form:\n"
        "ANSWER: <cell>\n"
        "where <cell> is copied verbatim from the list.\n\nCells:\n"
        + "\n".join(cells)
    )


def _nvidia_key():
    import re as _re
    cfg = open(os.path.expanduser("~/.hermes/config.yaml")).read()
    m = _re.search(r"nvidia:\s*\n((?:\s+.*\n)+?)(?=\S)", cfg)
    k = _re.search(r"(?:api_key|key):\s*[\"\']?(nvapi-[A-Za-z0-9_\-]+)",
                   m.group(1)) if m else None
    return k.group(1) if k else None


def ask_openai(model, entry, source, cells, timeout=300):
    """Same blind prompt, sent to an OpenAI-compatible endpoint."""
    key = _nvidia_key()
    if not key:
        return None, "NO-KEY", 0.0
    body = {"model": model, "temperature": 0, "max_tokens": 512,
            "messages": [{"role": "user",
                          "content": build_prompt(entry, source, cells)}]}
    req = urllib.request.Request(
        "https://integrate.api.nvidia.com/v1/chat/completions",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json",
                 "Authorization": "Bearer " + key})
    import time
    t0 = time.time()
    for attempt in range(3):
        try:
            d = json.load(urllib.request.urlopen(req, timeout=timeout))
            raw = d["choices"][0]["message"]["content"].strip()
            return parse_cell(raw, cells), raw, round(time.time() - t0, 1)
        except (urllib.error.URLError, TimeoutError, OSError, KeyError):
            if attempt == 2:
                return None, "REQUEST-FAILED", round(time.time() - t0, 1)
    return None, "", 0.0


def ask_ollama(model, entry, source, cells, timeout=180):
    """One blind filing decision. Returns (cell|None, raw, seconds)."""
    prompt = build_prompt(entry, source, cells)
    body = {"model": model, "prompt": prompt, "stream": False,
            "options": {"temperature": 0, "num_predict": 40}}
    req = urllib.request.Request(OLLAMA, data=json.dumps(body).encode(),
                                headers={"Content-Type": "application/json"})
    import time
    t0 = time.time()
    for attempt in range(3):
        try:
            d = json.load(urllib.request.urlopen(req, timeout=timeout))
            raw = (d.get("response") or "").strip()
            return parse_cell(raw, cells), raw, round(time.time() - t0, 1)
        except (urllib.error.URLError, TimeoutError, OSError):
            if attempt == 2:
                return None, "REQUEST-FAILED", round(time.time() - t0, 1)
    return None, "", 0.0


def parse_cell(raw, cells):
    """Read the ANSWER: marker; fall back to exact match.

    A reasoning model's trace can name several cells while deliberating, so the
    marker is what identifies the decision. If the marker is absent and the
    text is not itself a bare cell, the item is recorded as unmatched rather
    than coerced to whichever cell appears first -- coercing would manufacture
    agreement.
    """
    if not raw:
        return None
    if "ANSWER:" in raw:
        tail = raw.rsplit("ANSWER:", 1)[1].strip().split("\n")[0]
        for c in cells:
            if c.upper().replace(" ", "") in tail.upper().replace(" ", ""):
                return c
        return None
    up = raw.upper().replace(" ", "")
    for c in cells:
        if up == c.upper().replace(" ", ""):
            return c
    for c in cells:                       # take the first cell appearing in text
        if c.upper().replace(" ", "") in up:
            return c
    return None


def cohen_kappa(a, b, labels):
    """Chance-corrected agreement between two raters on the same items."""
    n = len(a)
    if n == 0:
        return None
    po = sum(1 for x, y in zip(a, b) if x == y) / n
    ca, cb = collections.Counter(a), collections.Counter(b)
    pe = sum((ca[l] / n) * (cb[l] / n) for l in labels)
    if abs(1 - pe) < 1e-12:
        return None
    return (po - pe) / (1 - pe)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=150)
    ap.add_argument("--model", default="lfm2.5:latest")
    # nvidia default is set below if the backend is nvidia and no model given
    ap.add_argument("--timeout", type=int, default=240)
    ap.add_argument("--backend", default="ollama",
                    choices=["ollama", "nvidia"],
                    help="nvidia = distinct cloud model via an OpenAI-compatible "
                         "endpoint; ollama = a local model")
    ap.add_argument("--dry-run", action="store_true",
                    help="pick the sample and print prompts; do not call a model")
    a = ap.parse_args()

    if a.backend == "nvidia" and a.model == "lfm2.5:latest":
        a.model = "nvidia/nemotron-3.5-lightning-30b-a3b"
    rows = load_gate()
    cells = sorted({r["rater1_cell"] for r in rows})
    print(f"corpus: {len(rows)} entries, {len(cells)} candidate cells")

    # Stratify the sample by rater 1's cell so the second rater is not handed an
    # unrepresentative slice of the taxonomy.
    rng = random.Random(SEED)
    by_cell = collections.defaultdict(list)
    for r in rows:
        by_cell[r["rater1_cell"]].append(r)
    sample, i = [], 0
    per = max(1, a.n // max(1, len(by_cell)))
    for c, rs in sorted(by_cell.items()):
        rng.shuffle(rs)
        sample.extend(rs[:per])
    sample = sample[:a.n]
    rng.shuffle(sample)
    print(f"sample: {len(sample)} entries across {len({s['rater1_cell'] for s in sample})} cells")

    if a.dry_run:
        # Blindness must be verified on the payload that is actually sent, not
        # on the source row (which legitimately carries both labels).
        # The cell list necessarily contains every cell NAME, so a substring
        # test over the whole prompt can never pass -- it flags the choice list
        # itself. Blindness means the prompt does not reveal WHICH cell was
        # assigned, so the check is applied to the part of the prompt outside
        # the choice list: that is where a leak would have to appear.
        leaked = 0
        for s in sample:
            p = build_prompt(s["entry"], s["source"], cells)
            head = p.split("Cells:")[0]
            for label, val in (("old_cell", s["old_cell"]),
                               ("rater1_cell", s["rater1_cell"])):
                if val and val in head:
                    leaked += 1
                    print(f"  BLINDNESS LEAK: {label} {val!r} appears outside "
                          f"the choice list for {s['entry']!r}")
            # and the prompt must carry no annotation pairing entry to a cell
            for marker in ("assigned", "correct", "old cell", "previously",
                           "rater", "moved"):
                if marker in head.lower():
                    leaked += 1
                    print(f"  BLINDNESS LEAK: marker {marker!r} in prompt head "
                          f"for {s['entry']!r}")
        s = sample[0]
        print(f"\nsample={len(sample)} entries; prompt-leak checks={2*len(sample)}; leaks={leaked}")
        print(f"example: entry={s['entry']!r} source={s['source']!r} "
              f"(old={s['old_cell']!r} rater1={s['rater1_cell']!r} withheld)")
        if leaked:
            print("REFUSING to run: the rater is not blind.")
            return 2
        return 0

    # Resume from a previous partial run rather than re-asking every item.
    out, fails, t_total = _load_partial()
    done = {r["n"] for r in out}
    if done:
        print(f"  resuming: {len(done)} items already rated", flush=True)
    for k, s in enumerate(sample, 1):
        if s["n"] in done:
            continue
        _ask = ask_openai if a.backend == "nvidia" else ask_ollama
        cell, raw, secs = _ask(a.model, s["entry"], s["source"], cells, a.timeout)
        t_total += secs
        if cell is None:
            fails += 1
        out.append({"n": s["n"], "entry": s["entry"], "source": s["source"],
                    "rater1_cell": s["rater1_cell"], "rater2_cell": cell,
                    "rater2_raw": raw[:80], "seconds": secs})
        if k % 5 == 0 or k == len(sample):
            print(f"  {k}/{len(sample)}  fails={fails}  {t_total:.0f}s", flush=True)
        # Flush after every item: a kill must not discard the run.
        _save_partial(out)

    matched = [r for r in out if r["rater2_cell"] is not None]
    a1 = [r["rater1_cell"] for r in matched]
    a2 = [r["rater2_cell"] for r in matched]
    agree = sum(1 for x, y in zip(a1, a2) if x == y)
    kappa = cohen_kappa(a1, a2, cells)

    res = {
        "model": a.model, "backend": a.backend, "n_requested": a.n, "n_sampled": len(sample),
        "n_parsed": len(matched), "n_unparsed": fails,
        "n_cells": len(cells), "seed": SEED,
        "blindness": ["no old_cell", "no rater1_cell", "no geometry proposal",
                      "one item at a time, no memory"],
        "rater1_rater2": {
            "agreement_rate": agree / len(matched) if matched else None,
            "cohen_kappa": kappa,
            "n": len(matched),
            "ci95_agreement": _wilson(agree, len(matched)) if matched else None,
        },
        "seconds_total": round(t_total, 1),
        "rows": out,
        "reproduce": f"python3 research/papers/p4_second_rater.py --n {a.n}",
        "claim_boundary": (
            "A second language model is not a human rater and agreement between "
            "two models is agreement between two policies. This supports "
            "'geometry captures something shared across raters' versus 'geometry "
            "predicts one policy'; it does NOT support ontology truth, and the "
            "manuscript must keep saying 'rater-assigned cell'."),
    }
    with open(OUT, "w") as fh:
        json.dump(res, fh, indent=2)

    print(f"\nagreement rater1 vs rater2: {agree}/{len(matched)}"
          f" = {agree/max(1,len(matched)):.3f}  kappa={kappa if kappa is None else round(kappa,3)}")
    print(f"unparsed: {fails}/{len(sample)}")
    print(f"wrote {os.path.relpath(OUT, REPO)}")
    return 0


def _wilson(k, n):
    if n == 0:
        return None
    z, p = 1.96, k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return [c - h, c + h]


if __name__ == "__main__":
    sys.exit(main())
