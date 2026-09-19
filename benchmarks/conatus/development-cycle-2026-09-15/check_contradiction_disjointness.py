#!/usr/bin/env python3
"""Checks that the new held-out contradiction fixtures
(fixtures/contradiction_holdout_disjoint_2026-09-15.json) do not share exact
sentences or a suspiciously high vocabulary overlap with the existing E83-E86
fixtures this worker does not own and must not edit. Read-only against those
files. Reports a number; does not assert a pass/fail threshold beyond exact
duplicate sentences, since "disjoint enough" is a judgement call for whoever
runs the routed comparison, not this script.
"""
import argparse
import json
import re
import sys
from pathlib import Path

EXISTING = [
    "fixtures/adversarial-consistent.json",
    "fixtures/adversarial-consistent-2.json",
    "fixtures/contradiction-holdout.json",
]


def sentences_from(obj):
    out = []
    if isinstance(obj, dict):
        for k, v in obj.items():
            if k in ("a", "b") and isinstance(v, str):
                out.append(v)
            else:
                out.extend(sentences_from(v))
    elif isinstance(obj, list):
        for item in obj:
            out.extend(sentences_from(item))
    return out


def words(s):
    return set(re.findall(r"[a-z0-9]+", s.lower()))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--conatus-dir", default="..")
    ap.add_argument("--new-fixture", default="fixtures/contradiction_holdout_disjoint_2026-09-15.json")
    args = ap.parse_args()

    base = Path(args.conatus_dir)
    existing_sentences = set()
    existing_words = set()
    missing = []
    for rel in EXISTING:
        p = base / rel
        if not p.exists():
            missing.append(str(p))
            continue
        data = json.loads(p.read_text())
        for s in sentences_from(data):
            existing_sentences.add(s.strip())
            existing_words |= words(s)

    new_data = json.loads(Path(args.new_fixture).read_text())
    new_sentences = set(sentences_from(new_data))
    new_words = set()
    for s in new_sentences:
        new_words |= words(s)

    exact_dupes = existing_sentences & new_sentences
    overlap = new_words & existing_words
    overlap_rate = len(overlap) / len(new_words) if new_words else 0.0

    print(f"existing fixture files read: {len(EXISTING) - len(missing)}/{len(EXISTING)}")
    if missing:
        print(f"  NOT FOUND (skipped, not scored): {missing}")
    print(f"existing sentences: {len(existing_sentences)}, existing vocabulary: {len(existing_words)} words")
    print(f"new sentences: {len(new_sentences)}, new vocabulary: {len(new_words)} words")
    print(f"exact sentence duplicates: {len(exact_dupes)}")
    for d in sorted(exact_dupes):
        print(f"  DUPLICATE: {d!r}")
    print(f"word-level vocabulary overlap: {len(overlap)} words shared "
          f"({overlap_rate:.0%} of the new fixture's vocabulary)")

    if exact_dupes:
        print("\nFAIL: exact sentence duplicates found -- this is not a disjoint fixture.")
        sys.exit(1)
    print("\nPASS: no exact sentence duplicates. Vocabulary overlap is reported, not gated -- "
          "shared function words (the/was/at/on) are expected and not evidence of copying.")
    sys.exit(0)


if __name__ == "__main__":
    main()
