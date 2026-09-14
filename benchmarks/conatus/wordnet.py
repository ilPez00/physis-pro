#!/usr/bin/env python3
"""A dependency-free WordNet reader: synonyms, antonyms, hypernyms.

The database is already on disk (`models/wordnet`, 53 MB, the standard
`data.*` / `index.*` files) and the crate lists `wordnet-db` as a dev
dependency. This parses the files directly — no nltk, no network — because the
symbolic layer the pipeline needs is exactly what these files are: relations
between *words*, not between contents.

Format, for the parts used here: each line of `data.pos` is
`offset lex_filenum ss_type w_cnt word lex_id [word lex_id …] p_cnt [ptr …] | gloss`
and each pointer is `symbol offset pos source/target`. `!` is antonym, `@` is
hypernym, and the words sharing a synset are its synonyms.

    python3 benchmarks/conatus/wordnet.py --word increase
"""
import argparse, os, sys
from collections import defaultdict

ROOT = os.path.expanduser("~/dev/physis-pro/models/wordnet")
POS_FILES = {"n": "noun", "v": "verb", "a": "adj", "r": "adv"}


class WordNet:
    def __init__(self, root=ROOT):
        self.root = root
        self.synsets = {}                     # (pos, offset) -> [words]
        self.pointers = defaultdict(list)     # (pos, offset) -> [(symbol, pos, offset)]
        self.index = defaultdict(list)        # word -> [(pos, offset)]
        missing = [p for p in POS_FILES.values()
                   if not os.path.exists(os.path.join(root, f"data.{p}"))]
        if missing:
            raise SystemExit(f"missing WordNet data files in {root}: {missing}")
        for pos, name in POS_FILES.items():
            self._load(pos, os.path.join(root, f"data.{name}"))

    def _load(self, pos, path):
        with open(path, encoding="utf-8", errors="replace") as fh:
            for line in fh:
                if line.startswith("  "):     # licence header
                    continue
                head = line.split("|")[0].split()
                if len(head) < 4:
                    continue
                offset, _lex, ss_type, w_cnt = head[0], head[1], head[2], head[3]
                try:
                    count = int(w_cnt, 16)
                except ValueError:
                    continue
                words, i = [], 4
                for _ in range(count):
                    if i + 1 >= len(head):
                        break
                    words.append(head[i].replace("_", " ").lower())
                    i += 2
                key = (pos, offset)
                self.synsets[key] = words
                for word in words:
                    self.index[word].append(key)
                if i >= len(head):
                    continue
                try:
                    p_cnt = int(head[i])
                except ValueError:
                    continue
                i += 1
                for _ in range(p_cnt):
                    if i + 3 >= len(head) + 1:
                        break
                    symbol, target, target_pos = head[i], head[i + 1], head[i + 2]
                    self.pointers[key].append((symbol, target_pos, target))
                    i += 4

    def synonyms(self, word):
        word = word.lower()
        out = set()
        for key in self.index.get(word, []):
            out.update(w for w in self.synsets[key] if w != word)
        return sorted(out)

    def _related(self, word, symbol):
        word = word.lower()
        out = set()
        for key in self.index.get(word, []):
            for sym, pos, offset in self.pointers.get(key, []):
                if sym != symbol:
                    continue
                out.update(self.synsets.get((pos, offset), []))
        return sorted(w for w in out if w != word)

    def antonyms(self, word):
        return self._related(word, "!")

    def hypernyms(self, word):
        return self._related(word, "@")

    def stats(self):
        return {
            "synsets": len(self.synsets),
            "words": len(self.index),
            "antonym_pointers": sum(1 for ptrs in self.pointers.values()
                                    for s, _, _ in ptrs if s == "!"),
        }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--word", default="increase")
    ap.add_argument("--root", default=ROOT)
    args = ap.parse_args()
    wn = WordNet(args.root)
    print(wn.stats())
    print(f"{args.word}:")
    print(f"  synonyms  {wn.synonyms(args.word)[:10]}")
    print(f"  antonyms  {wn.antonyms(args.word)[:10]}")
    print(f"  hypernyms {wn.hypernyms(args.word)[:10]}")


if __name__ == "__main__":
    main()
