#!/usr/bin/env python3
"""The machine-log corpus — the third text regime, and the one the shell runs on.

E59 measured hand-written prose. E60 measured documentation. `physis-world`
actually runs over neither: its log is filenames, commit messages and shell
commands. This builds a scored corpus out of exactly that.

**Where gold comes from, and why it is not circular.** Only `git` and `terminal`
observations are used, and the gold entity of each is the annotation its *source*
already carries:

    git       the conventional-commit scope — `fix(ci):` → `ci`, `feat(fs/graph):`
              → `fs`, `graph` — plus any dotted filename in the message
    terminal  the program invoked (argv[0], basename) plus path-like arguments

`fs` observations are excluded deliberately: their subject *is* a path, so a
path-derived gold would be the same string the code rule extracts, and the score
would measure nothing. Commit scopes and program names are written by the author
and by the shell, and no rule in `worldstate.rs` targets either.

Usage:  python3 benchmarks/worldstate/make_corpus_log.py [--log PATH]
Writes: corpus-log.jsonl, corpus-log.sha256
"""
import argparse
import hashlib
import json
import pathlib
import re

HERE = pathlib.Path(__file__).resolve().parent
DEFAULT_LOG = pathlib.Path.home() / ".physis-core" / "observations.jsonl"

SCOPE = re.compile(r"^(?:[a-z]+)\(([^)]{1,40})\)!?:")
DOTTED = re.compile(r"\b([\w./-]+\.(?:rs|py|md|toml|json|sh|yml|yaml|lock))\b")
PATHARG = re.compile(r"(?:^|\s)((?:\./|/|~/)[\w./-]{3,60})")


def key(s: str) -> str:
    s = s.strip().lower().strip("./~")
    s = re.sub(r"[^a-z0-9\-_./]", "", s)
    return s.strip("-/")


def add(ents, raw):
    k = key(raw)
    if len(k) > 2 and k not in ents:
        ents.append(k)
        seg = k.split("/")[-1]
        stem = seg.rsplit(".", 1)[0] if "." in seg else seg
        for h in (seg, stem):
            if len(h) > 2 and h != k and h not in ents:
                ents.append(h)


def build(log_path):
    recs = []
    for line in open(log_path):
        line = line.strip()
        if not line:
            continue
        o = json.loads(line)
        src = o.get("source")
        if src not in ("git", "terminal"):
            continue
        body = (o.get("body") or "").strip()
        subject = (o.get("subject") or "").strip()
        ents = []
        if src == "git":
            text = body or subject
            m = SCOPE.match(text)
            if m:
                for part in re.split(r"[/,]", m.group(1)):
                    add(ents, part)
            for d in DOTTED.findall(text):
                add(ents, d)
        else:
            text = subject
            head = text.split()
            if head:
                prog = head[0].split("/")[-1]
                if prog not in ("sudo", "time", "env"):
                    add(ents, prog)
                elif len(head) > 1:
                    add(ents, head[1].split("/")[-1])
            for p in PATHARG.findall(text):
                add(ents, p)
        if not ents or len(text) < 12:
            continue
        recs.append(
            {
                "pos": len(recs),
                "id": f"l{len(recs):04d}",
                "text": text,
                "entities": ents,
                "relation": None,
                "transition": None,
                "source": src,
                "seq": o.get("seq"),
                "gold_from": "commit-scope" if src == "git" else "argv0",
            }
        )
    return recs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--log", default=str(DEFAULT_LOG))
    args = ap.parse_args()
    recs = build(args.log)
    body = "".join(json.dumps(r, sort_keys=True) + "\n" for r in recs)
    (HERE / "corpus-log.jsonl").write_text(body)
    digest = hashlib.sha256(body.encode()).hexdigest()
    (HERE / "corpus-log.sha256").write_text(digest + "  corpus-log.jsonl\n")
    git = sum(1 for r in recs if r["source"] == "git")
    ments = sum(len(r["entities"]) for r in recs)
    print(f"{len(recs)} observations kept ({git} git, {len(recs) - git} terminal)")
    print(f"{ments} gold entity mentions")
    print("sha256:", digest)


if __name__ == "__main__":
    main()
