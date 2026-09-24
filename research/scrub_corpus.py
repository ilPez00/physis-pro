#!/usr/bin/env python3
"""Scrub credential-shaped material from the behavioural corpus.

Two distinct leaks, both found by research/release_audit.py:

  1. The RAW captures (actions*.json) carry full command text in `target`,
     including real tokens and absolute paths that contain the username.
     These files are tracked and pushed. Per programme policy, raw session
     transcripts must not be published at all.

  2. The REDACTED release (corpus-redacted.json.gz) hashes `target` but
     derives `kind` labels from the command, so a secret can survive inside
     a kind label. One NVIDIA key did.

This scrubs both in place, replacing secret-shaped tokens with a fixed
placeholder so the label vocabulary stays stable (a kind label that differed
only by an embedded secret still collapses to one label, as it should).

Idempotent: running twice changes nothing the second time.

  python3 research/scrub_corpus.py --dry-run
  python3 research/scrub_corpus.py
"""
import argparse, gzip, json, os, re, sys

REPO = "/home/gio/dev/physis-pro"
BEH = f"{REPO}/benchmarks/behaviour"

SECRET_RX = re.compile(
    r"(?:"
    r"nvapi-[A-Za-z0-9_\-]{20,}"
    r"|sk-ant-[A-Za-z0-9_\-]{16,}"
    r"|sk-proj-[A-Za-z0-9_\-]{16,}"
    r"|sk-[A-Za-z0-9_\-]{20,}"
    r"|gsk_[A-Za-z0-9]{20,}"
    r"|gh[pousr]_[A-Za-z0-9]{20,}"
    r"|(?:AKIA|ASIA)[A-Z0-9]{16}"
    r"|xox[baprs]-[A-Za-z0-9\-]{10,}"
    r"|AIza[A-Za-z0-9_\-]{35}"
    r"|hf_[A-Za-z0-9]{20,}"
    r"|Bearer\s+[A-Za-z0-9._\-]{20,}"
    r")"
)
PLACEHOLDER = "<SECRET-REDACTED>"


def scrub_obj(o):
    """Recursively replace secret-shaped tokens in every string."""
    n = 0
    if isinstance(o, dict):
        for k, v in o.items():
            o[k], c = scrub_obj(v)
            n += c
    elif isinstance(o, list):
        for i, v in enumerate(o):
            o[i], c = scrub_obj(v)
            n += c
    elif isinstance(o, str):
        new, c = SECRET_RX.subn(PLACEHOLDER, o)
        return new, c
    return o, n


def scrub_path(path, dry):
    if path.endswith(".gz"):
        raw = gzip.open(path, "rb").read()
        doc = json.loads(raw)
    else:
        doc = json.load(open(path))
    doc, n = scrub_obj(doc)
    if n and not dry:
        if path.endswith(".gz"):
            # mtime=0 so the artifact is byte-reproducible
            with gzip.GzipFile(path, "wb", mtime=0) as fh:
                fh.write(json.dumps(doc).encode())
        else:
            json.dump(doc, open(path, "w"))
    return n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    targets = sorted(
        os.path.join(BEH, f) for f in os.listdir(BEH)
        if f.endswith(".json") or f.endswith(".json.gz")
    )
    total = 0
    print(f"{'DRY RUN — ' if args.dry_run else ''}scrubbing {len(targets)} files in {BEH}")
    for p in targets:
        try:
            n = scrub_path(p, args.dry_run)
        except Exception as e:
            print(f"  SKIP {os.path.basename(p)}: {e}")
            continue
        total += n
        if n:
            print(f"  {os.path.basename(p):34s} {n:4d} secrets replaced")
    print(f"\ntotal replaced: {total}")

    if not args.dry_run:
        # self-check: nothing secret-shaped should survive
        left = 0
        for p in targets:
            if p.endswith(".gz"):
                t = gzip.open(p, "rb").read().decode("utf-8", "replace")
            else:
                t = open(p, errors="replace").read()
            left += len(SECRET_RX.findall(t))
        print(f"post-scrub verification: {left} secret-shaped tokens remain")
        if left:
            sys.exit(f"FAIL: {left} tokens survived scrubbing")
        print("clean")


if __name__ == "__main__":
    main()
