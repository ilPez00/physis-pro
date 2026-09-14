#!/usr/bin/env python3
"""Ingestion pipeline: heterogeneous files in, normalised records out.

The proposal needs a directory of mixed things — two android apps, two webapps,
sparse PDFs and videos — to become one addressable field. This is the front of
that: walk a root, dispatch each file to an extractor by type, and emit one
record per file with its text (when there is any), its metadata, and the reason
when there is not.

Extractors present on this machine are detected at start-up and printed, because
a pipeline that silently skips every PDF and one that has no PDFs produce the
same empty result:

  text/code   read directly
  pdf         `pdftotext -layout` when installed
  media       metadata only (`file`, size, mtime) — no transcript, no OCR, and
              the record says `content_unavailable` rather than being dropped
  binary      skipped, counted, with the reason

Also applies a **static rule dictionary** (`relations.json`) over extracted text
to pull typed relations — the symbolic layer, cues mapped onto the vocabulary
already in `physis-core/src/relation.rs`, where `Causes` and `Explains` are
distinct types. Yield is reported against a word-order-shuffled null: a cue set
that fires as often on scrambled text is matching noise, not structure.

    python3 benchmarks/conatus/ingest.py ~/dev/physis_wow --out records.json
"""
import argparse, json, os, random, re, shutil, stat, subprocess, sys, time

TEXT_EXT = (".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".kt", ".java", ".md",
            ".txt", ".toml", ".json", ".yml", ".yaml", ".sh", ".sql", ".html",
            ".css", ".tex", ".csv")
PDF_EXT = (".pdf",)
MEDIA_EXT = (".mp4", ".mkv", ".mov", ".webm", ".mp3", ".wav", ".m4a",
             ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp")
SKIP_DIRS = ("node_modules", "target", ".git", "build", "dist", "vendor",
             "__pycache__", ".venv", "venv")
MAX_TEXT = 200_000
SENTENCE = re.compile(r"(?<=[.!?])\s+")


def tools():
    return {name: shutil.which(name) for name in
            ("pdftotext", "ffprobe", "tesseract", "whisper", "exiftool")}


def extract_pdf(path, have):
    if not have["pdftotext"]:
        return None, "pdftotext not installed"
    try:
        p = subprocess.run(["pdftotext", "-layout", "-q", path, "-"],
                           capture_output=True, text=True, timeout=120)
    except subprocess.TimeoutExpired:
        return None, "pdftotext timed out"
    text = p.stdout.strip()
    return (text[:MAX_TEXT], None) if text else (None, "pdf has no extractable text layer")


def extract_media(path, have):
    """No ffprobe, tesseract or whisper on this machine, so media contributes
    metadata and an explicit gap rather than a guess."""
    if have["ffprobe"]:
        p = subprocess.run(["ffprobe", "-v", "quiet", "-print_format", "json",
                            "-show_format", path], capture_output=True, text=True, timeout=60)
        if p.stdout.strip():
            return None, f"metadata only: {p.stdout[:200]}"
    return None, "content_unavailable: no transcriber or OCR installed"


def extract_text(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return fh.read(MAX_TEXT), None
    except OSError as e:
        return None, f"unreadable: {e}"


# Function words carry no argument. A "subject" that is only these is the
# matcher finding a cue and inventing the rest of the triple around it.
STOPWORDS = {
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "with", "is",
    "are", "was", "were", "be", "been", "it", "its", "this", "that", "these",
    "those", "as", "at", "by", "from", "we", "i", "you", "they", "he", "she",
    "not", "no", "so", "if", "then", "than", "which", "what", "when", "who",
}
ARG_WINDOW = 4          # tokens either side of the cue that may form an argument
MIN_CONTENT = 1         # at least this many non-stopwords in each argument


def _argument(tokens, stopwords=STOPWORDS):
    """An argument is a short contiguous span carrying at least one content
    word. Requiring contiguity next to the cue is what makes the rule
    order-sensitive: scrambling a sentence moves the content words away from
    the cue, and the match stops."""
    if not tokens:
        return None
    content = [t for t in tokens if t.lower().strip(".,;:()[]\"'") not in stopwords]
    if len(content) < MIN_CONTENT:
        return None
    return " ".join(tokens)


def relations(text, dictionary, limit=200):
    """Subject–cue–object triples from the static dictionary.

    Version 1 took everything before the cue as the subject and everything
    after as the object, and measured **1.00** against the word-shuffled null:
    with the words still present, the cue still had words on both sides, so the
    rule was detecting vocabulary. Arguments are now local spans that must
    carry content words next to the cue."""
    found = []
    for sentence in SENTENCE.split(text):
        s = " ".join(sentence.split())
        if not (20 <= len(s) <= 400):
            continue
        tokens = s.split()
        lowered = s.lower()
        for kind, patterns in dictionary.items():
            if kind.startswith("_"):
                continue
            hit = None
            for pattern in patterns:
                m = re.search(pattern, lowered)
                if m:
                    hit = m
                    break
            if not hit:
                continue
            before = len(s[:hit.start()].split())
            after = len(s[:hit.end()].split())
            subject = _argument(tokens[max(0, before - ARG_WINDOW):before])
            obj = _argument(tokens[after:after + ARG_WINDOW])
            if subject and obj:
                found.append({"type": kind, "cue": hit.group(0).strip(),
                              "subject": subject, "object": obj})
            if len(found) >= limit:
                break
    return found


def shuffled_text(text, seed=0):
    """Same words, same sentence lengths, order destroyed inside each sentence.
    A cue dictionary that fires as often here is matching vocabulary, not
    structure."""
    rng = random.Random(seed)
    out = []
    for sentence in SENTENCE.split(text):
        words = sentence.split()
        rng.shuffle(words)
        out.append(" ".join(words))
    return ". ".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("root")
    ap.add_argument("--out", default=None)
    ap.add_argument("--limit", type=int, default=400)
    ap.add_argument("--dictionary", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "relations.json"))
    args = ap.parse_args()

    have = tools()
    print("extractors: " + ", ".join(
        f"{name}={'yes' if path else 'NO'}" for name, path in have.items()))
    dictionary = json.load(open(args.dictionary))
    cue_count = sum(len(v) for k, v in dictionary.items() if not k.startswith("_"))
    print(f"relation dictionary: {len([k for k in dictionary if not k.startswith('_')])} "
          f"types, {cue_count} cues")

    records, reasons = [], {}
    seen = 0
    for base, dirs, names in os.walk(args.root):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS and not d.startswith(".")]
        for name in sorted(names):
            path = os.path.join(base, name)
            seen += 1
            if len(records) >= args.limit:
                continue
            ext = os.path.splitext(name)[1].lower()
            try:
                info = os.stat(path)
            except OSError:
                reasons["stat failed"] = reasons.get("stat failed", 0) + 1
                continue
            if ext in PDF_EXT:
                kind, (text, why) = "pdf", extract_pdf(path, have)
            elif ext in MEDIA_EXT:
                kind, (text, why) = "media", extract_media(path, have)
            elif ext in TEXT_EXT:
                kind, (text, why) = "text", extract_text(path)
            else:
                kind, text, why = "other", None, "no extractor for this extension"
            if why:
                reasons[why.split(":")[0]] = reasons.get(why.split(":")[0], 0) + 1
            shuffled_rels = relations(shuffled_text(text), dictionary) if text else []
            records.append({
                "path": os.path.relpath(path, args.root),
                "media": kind,
                "bytes": info.st_size,
                "modified": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(info.st_mtime)),
                "has_text": bool(text),
                "reason": why,
                "chars": len(text) if text else 0,
                "relations": relations(text, dictionary) if text else [],
                "relations_shuffled": len(shuffled_rels),
                "shuffled_by_cue": [r["cue"] for r in shuffled_rels],
            })

    with_text = [r for r in records if r["has_text"]]
    triples = sum(len(r["relations"]) for r in records)
    null_triples = sum(r["relations_shuffled"] for r in records)
    by_media = {}
    for r in records:
        slot = by_media.setdefault(r["media"], [0, 0])
        slot[0] += 1
        slot[1] += r["has_text"]

    print(f"\nfiles seen {seen} · ingested {len(records)} · with text {len(with_text)}")
    for media, (total, texted) in sorted(by_media.items()):
        print(f"   {media:6} {total:4} file(s), {texted:4} with extractable text")
    if reasons:
        print("   gaps: " + " · ".join(f"{k} ×{v}" for k, v in sorted(reasons.items())))

    print(f"\nrelations extracted {triples} over {len(with_text)} text file(s)")
    print(f"   word-shuffled null  {null_triples}")
    if null_triples:
        print(f"   ratio real/null {triples / null_triples:.2f} "
              f"{'— the dictionary is matching structure' if triples > 1.3 * null_triples else '— the dictionary is matching vocabulary, not structure'}")
    counts = {}
    for r in records:
        for rel in r["relations"]:
            counts[rel["type"]] = counts.get(rel["type"], 0) + 1
    if counts:
        print("   by type: " + ", ".join(f"{k}={v}" for k, v in
                                         sorted(counts.items(), key=lambda kv: -kv[1])))

    # Which cues are order-sensitive at all? A multi-word cue ("depends on")
    # cannot survive a shuffle; a single word ("requires") always can. Reporting
    # the aggregate ratio hides that the dictionary is two different instruments
    # mixed together.
    real_cue, null_cue = {}, {}
    for r in records:
        for rel in r["relations"]:
            real_cue[rel["cue"].lower()] = real_cue.get(rel["cue"].lower(), 0) + 1
        for cue in r.get("shuffled_by_cue", []):
            null_cue[cue.lower()] = null_cue.get(cue.lower(), 0) + 1
    multi_real = sum(v for k, v in real_cue.items() if " " in k)
    multi_null = sum(v for k, v in null_cue.items() if " " in k)
    single_real = sum(v for k, v in real_cue.items() if " " not in k)
    single_null = sum(v for k, v in null_cue.items() if " " not in k)
    print(f"\n   multi-word cues : real {multi_real:4} · null {multi_null:4} · "
          f"ratio {multi_real / multi_null if multi_null else float('inf'):.2f}")
    print(f"   single-word cues: real {single_real:4} · null {single_null:4} · "
          f"ratio {single_real / single_null if single_null else float('inf'):.2f}")
    worth = sorted(((k, real_cue.get(k, 0), null_cue.get(k, 0)) for k in real_cue
                    if real_cue[k] >= 5),
                   key=lambda t: -(t[1] / max(1, t[2])))[:8]
    print("   most order-sensitive cues (real, null):")
    for cue, real, null in worth:
        print(f"     {cue:18} {real:4} {null:4}  ratio "
              f"{real / null if null else float('inf'):.2f}")
    if args.out:
        json.dump({"root": args.root, "seen": seen, "records": records,
                   "triples": triples, "null_triples": null_triples},
                  open(args.out, "w"), indent=1)
        print(f"\nwritten to {args.out}")


if __name__ == "__main__":
    main()
