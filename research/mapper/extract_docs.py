#!/usr/bin/env python3
"""E4 — extract text from the personal document library for corpus density.

PRIVACY POLICY, APPLIED RATHER THAN ASSUMED
-------------------------------------------
This reads a person's own document library. Two rules follow, and the exporter
downstream enforces them:

  1. DIRECTORIES that are obviously personal, financial, medical or private
     correspondence are EXCLUDED by default (see SKIP_DIRS). Widening the scope
     is the owner's call, not this script's.
  2. NO TEXT AND NO FILENAMES ARE EVER COMMITTED. This script writes chunks to
     a gitignored working file. The committed artifact downstream carries only
     vectors, a machine-assigned cell, and a coarse top-level directory tag.

The corpus is used for CORPUS STATISTICS ONLY -- n-gram density and graph
substrate. Nothing here is evaluated against; the labelled evaluation set stays
the 658 human-authored ontology entries.
"""
import json, os, re, subprocess, sys, unicodedata

ROOT = os.path.expanduser("~/Documenti")
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), ".docs_chunks.jsonl")

# Excluded by default: personal, financial, medical, private correspondence.
SKIP_DIRS = {
    "Personal_and_Admin", "carte", "Psico", "cake_wallet", "negozio",
    ".Trash-1000", "NA", "Other",
}
SKIP_PAT = re.compile(r"whatsapp|chat con|fattur|invoice|contratt|cv[_ ]|curriculum|"
                      r"passaport|carta[ _]ident|iban|bank|estratto[ _]conto",
                      re.IGNORECASE)

EXTS = {".pdf", ".txt", ".tex", ".md", ".epub", ".odt", ".docx", ".html"}


def clean(t):
    t = unicodedata.normalize("NFKC", t)
    t = re.sub(r"[ \t\xa0]+", " ", t)
    t = re.sub(r"\n{2,}", "\n\n", t)
    return t


def read_pdf(p):
    try:
        r = subprocess.run(["pdftotext", "-q", "-enc", "UTF-8", p, "-"],
                           capture_output=True, timeout=90)
        return r.stdout.decode("utf-8", "ignore")
    except Exception:
        return ""


def read_any(p):
    ext = os.path.splitext(p)[1].lower()
    if ext == ".pdf":
        return read_pdf(p)
    if ext in (".txt", ".tex", ".md", ".html"):
        try:
            return open(p, encoding="utf-8", errors="ignore").read()
        except Exception:
            return ""
    if ext in (".epub", ".odt", ".docx"):
        # both are zip containers of XML; pull the text nodes
        try:
            import zipfile
            z = zipfile.ZipFile(p)
            buf = []
            for n in z.namelist():
                if n.endswith((".xhtml", ".html", ".xml")) and "meta" not in n.lower():
                    raw = z.read(n).decode("utf-8", "ignore")
                    buf.append(re.sub(r"<[^>]+>", " ", raw))
            return " ".join(buf)
        except Exception:
            return ""
    return ""


def paragraphs(text, lo=200, hi=1400):
    """Paragraph-shaped chunks. Prose, not fixed byte slices."""
    out, cur = [], ""
    for para in re.split(r"\n\s*\n", text):
        para = " ".join(para.split())
        if len(para) < 40:
            continue
        if len(cur) + len(para) + 1 > hi:
            if len(cur) >= lo:
                out.append(cur)
            cur = para
        else:
            cur = (cur + " " + para).strip()
    if len(cur) >= lo:
        out.append(cur)
    return out


def main():
    files = []
    for dirpath, dirnames, filenames in os.walk(ROOT):
        dirnames[:] = [d for d in dirnames
                       if d not in SKIP_DIRS and not d.startswith(".")
                       and not SKIP_PAT.search(d)]
        rel = os.path.relpath(dirpath, ROOT)
        top = rel.split(os.sep)[0] if rel != "." else "."
        if top in SKIP_DIRS or SKIP_PAT.search(top):
            continue
        for f in filenames:
            if os.path.splitext(f)[1].lower() in EXTS and not SKIP_PAT.search(f):
                files.append((top, os.path.join(dirpath, f)))
    print(f"{len(files)} candidate files after the exclusion policy")

    seen, n_chunks = set(), 0
    with open(OUT, "w") as fh:
        for i, (top, p) in enumerate(files):
            txt = clean(read_any(p))
            if len(txt) < 400:
                continue
            for c in paragraphs(txt):
                # de-duplicate: ebooks and course notes repeat boilerplate
                key = hash(c[:160])
                if key in seen:
                    continue
                seen.add(key)
                # ASCII-letter ratio filter: drops OCR garbage and formula dumps
                letters = sum(ch.isalpha() for ch in c)
                if letters / max(len(c), 1) < 0.62:
                    continue
                if len(c.split()) < 25:
                    continue
                fh.write(json.dumps({"src": top, "text": c}, ensure_ascii=False) + "\n")
                n_chunks += 1
            if i % 50 == 0 and i:
                print(f"  {i}/{len(files)} files, {n_chunks} chunks")
    print(f"\n{n_chunks} chunks written to {OUT}")
    print("(gitignored: this file holds document text and is never committed)")


if __name__ == "__main__":
    main()
