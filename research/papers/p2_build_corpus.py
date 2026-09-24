#!/usr/bin/env python3
"""P2 2A: build the external trajectory corpus.

The pilot was one repository, six Rust files, one embedder, 8-27 revisions --
nowhere near enough for a broad claim about evolving software objects. This
builds the replication corpus: mature public repositories, several languages,
hundreds of files, each with enough revisions to form a trajectory.

Design choices, and why:

* **No --follow.** Rename tracking would recover more history but costs a full
  walk per file. Without it, a renamed file's trajectory restarts at the rename.
  Accepted and disclosed; it biases toward *shorter* trajectories, not longer.
* **Dedupe consecutive identical content.** A commit that touches a file without
  changing it is not a revision of the object, it is bookkeeping.
* **Partial clone (blob:none).** We only need the blobs we actually read, and
  `git cat-file --batch` fetches them in one pass per repo.
* **Incremental output.** One JSONL shard per repo, so a timeout loses at most
  the repo in flight rather than the whole corpus.

Blob text file layout (one line per revision, JSON):
  {"repo","path","lang","rev","commit","ts","n_rev","text"}

REPRODUCTION: python3 research/papers/p2_build_corpus.py [--repos N] [--files-per-repo N]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(REPO, "research", "trajectories_external")
WORK = os.path.join(OUT, "work")
SHARDS = os.path.join(OUT, "shards")

# This box's global git config sets `url.git@github.com:.insteadOf =
# https://github.com/`, so every HTTPS github URL is rewritten to SSH -- and SSH
# auth is denied for these public repos. Worse, the rewrite also breaks the
# partial clone's *lazy blob fetch*, so a --filter=blob:none clone "succeeds"
# and then fails at checkout. Public repos need no auth over HTTPS, so
# neutralising global git config for everything we shell out to at once is the
# root-cause fix rather than patching each call site.
os.environ["GIT_CONFIG_GLOBAL"] = "/dev/null"

# Diverse by language, mature enough to have real histories, small enough to
# clone and walk inside a reasonable budget. `lang` is the dominant language.
REPOS = [
    ("BurntSushi/ripgrep", "rust"),
    ("serde-rs/serde", "rust"),
    ("psf/requests", "python"),
    ("pallets/flask", "python"),
    ("scrapy/scrapy", "python"),
    ("expressjs/express", "js"),
    ("axios/axios", "js"),
    ("jqlang/jq", "c"),
    ("fmtlib/fmt", "cpp"),
    ("nlohmann/json", "cpp"),
]

EXT_LANG = {
    ".rs": "rust", ".py": "python", ".js": "js", ".ts": "ts", ".c": "c",
    ".h": "c", ".cpp": "cpp", ".cc": "cpp", ".hpp": "cpp", ".hh": "cpp",
    ".java": "java", ".go": "go", ".rb": "ruby", ".php": "php",
}

# Excluded: generated, vendored, minified, locks, snapshots. These are not
# evolving *objects*, they are artifacts of a build or a dependency resolver.
SKIP_DIR = re.compile(
    r"(^|/)(vendor|node_modules|dist|build|target|\.git|third_party|"
    r"external|deps|site-packages|__pycache__|\.venv|venv|testdata|"
    r"fixtures|snapshots?|generated)(/|$)")
SKIP_NAME = re.compile(
    r"\.(min\.js|min\.css|map|lock|pb\.go|pb\.cc|pb\.h|g\.cs|"
    r"generated\.[a-z]+)$|"
    r"(^|/)(package-lock\.json|yarn\.lock|Cargo\.lock|poetry\.lock|"
    r"Pipfile\.lock|composer\.lock|go\.sum|Gemfile\.lock)$")

MIN_REVS = 8          # the brief's floor for a "meaningful trajectory"
# Disk budget, not aesthetics: the root partition on this box is at 100% with
# ~3 GB free, so the corpus must fit in a small footprint. 8 KB of text is far
# more than enough to separate files lexically or semantically, and gzipped
# shards cut what remains by roughly 4x.
MAX_TEXT = 8_000
MAX_FILE_BYTES = 200_000
GZIP_SHARDS = True
KEEP_CLONES = False   # clones are scratch; see clone()


def sh(cmd, cwd=None, timeout=1800, check=False):
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout)
    if check and p.returncode != 0:
        raise RuntimeError(f"{cmd[:3]}... failed: {p.stderr[:400]}")
    return p


def clone(slug, dest):
    """FULL clone, deliberately.

    `--filter=blob:none` looks cheaper and is a trap here: `git cat-file
    --batch` then lazily fetches every missing blob from the promisor remote,
    one network round trip per blob. Selecting ~3,000 revisions serialises into
    tens of minutes with no progress output, which is exactly how the first two
    builds appeared to hang. A full clone pays its cost once, up front, and
    `cat-file` afterwards is local. Clones are scratch: each is deleted after
    its shard is written, so peak disk is one repo, not ten.
    """
    if os.path.isdir(os.path.join(dest, ".git")):
        return True, "cached"
    url = f"https://github.com/{slug}.git"
    p = sh(["git", "clone", "--quiet", url, dest], timeout=1800)
    if p.returncode != 0:
        return False, p.stderr.strip()[:300]
    return True, "cloned"


def head_commit(dest):
    p = sh(["git", "rev-parse", "HEAD"], cwd=dest)
    return p.stdout.strip() if p.returncode == 0 else ""


def candidate_files(dest):
    """Tracked files at HEAD, filtered to plausible evolving source objects."""
    p = sh(["git", "ls-files"], cwd=dest, timeout=600)
    if p.returncode != 0:
        return []
    out = []
    for line in p.stdout.splitlines():
        path = line.strip()
        if not path:
            continue
        if SKIP_DIR.search(path) or SKIP_NAME.search(path):
            continue
        ext = os.path.splitext(path)[1].lower()
        if ext not in EXT_LANG:
            continue
        out.append(path)
    return out


def history_index(dest):
    """One pass over history: path -> [(commit, ts)] oldest-last.

    Doing this per candidate file costs one full history walk each, which on a
    repo with a few hundred candidates dominates the runtime. A single
    `git log --name-only` visit builds the whole index at once.
    """
    p = sh(["git", "log", "--reverse", "--format=%H %ct", "--name-only"],
           cwd=dest, timeout=900)
    if p.returncode != 0:
        return {}
    idx, cur = {}, None
    for line in p.stdout.splitlines():
        s = line.strip()
        if not s:
            continue
        parts = s.split()
        if len(parts) == 2 and len(parts[0]) == 40 and parts[1].isdigit():
            cur = (parts[0], int(parts[1]))
            continue
        if cur is not None:
            idx.setdefault(s, []).append(cur)
    return idx


def path_history(dest, path, limit=400):
    """Commits touching `path`, oldest last. Returns [(commit, unixtime)]."""
    p = sh(["git", "log", "--format=%H\t%ct", f"--max-count={limit}",
            "--", path], cwd=dest, timeout=180)
    if p.returncode != 0:
        return []
    rows = []
    for line in p.stdout.splitlines():
        parts = line.split("\t")
        if len(parts) == 2:
            rows.append((parts[0], int(parts[1])))
    rows.reverse()          # oldest -> newest
    return rows


def batch_blobs(dest, specs):
    """Fetch many blobs in one pass. specs = [(commit, path)] -> [text|None]."""
    if not specs:
        return []
    inp = "".join(f"{c}:{p}\n" for c, p in specs)
    p = subprocess.run(["git", "cat-file", "--batch"], cwd=dest, input=inp,
                       capture_output=True, text=True, timeout=1800)
    if p.returncode != 0:
        return [None] * len(specs)
    # git cat-file --batch emits: "<sha> <type> <size>\n<content>\n" per object
    out, buf, i = [], p.stdout, 0
    for _ in specs:
        nl = buf.find("\n", i)
        if nl < 0:
            out.append(None)
            continue
        header = buf[i:nl]
        i = nl + 1
        parts = header.split()
        if len(parts) != 3 or parts[1] != "blob":
            out.append(None)
            continue
        size = int(parts[2])
        out.append(buf[i:i + size])
        i += size + 1
    return out


def build_repo(slug, lang, files_per_repo, max_rev_per_file):
    name = slug.split("/")[1]
    dest = os.path.join(WORK, name)
    t0 = time.time()
    ok, how = clone(slug, dest)
    if not ok:
        return {"repo": slug, "status": "clone-failed", "detail": how}
    commit = head_commit(dest)
    cands = candidate_files(dest)
    idx = history_index(dest)

    # Rank candidates by revision count so the per-repo cap keeps the longest
    # trajectories rather than the alphabetically first ones.
    scored = []
    for path in cands:
        hist = idx.get(path, [])
        if len(hist) >= MIN_REVS:
            scored.append((len(hist), path, hist))
    scored.sort(reverse=True, key=lambda r: r[0])
    scored = scored[:files_per_repo]

    rows, n_files, n_revs = [], 0, 0
    for n_rev, path, hist in scored:
        hist = hist[-max_rev_per_file:]
        specs = [(c, path) for c, _ in hist]
        blobs = batch_blobs(dest, specs)
        # dedupe consecutive identical content, and drop oversized/binary blobs
        prev = None
        kept = 0
        for (c, ts), text in zip(hist, blobs):
            if text is None or "\x00" in text[:1000]:
                continue
            if len(text) > MAX_FILE_BYTES:
                continue
            if text == prev:
                continue
            prev = text
            kept += 1
            rows.append({
                "repo": slug, "path": path, "lang": lang, "rev": kept,
                "commit": c, "ts": ts, "n_rev": n_rev,
                "text": text[:MAX_TEXT],
            })
        if kept >= MIN_REVS:
            n_files += 1
            n_revs += kept

    os.makedirs(SHARDS, exist_ok=True)
    shard = os.path.join(SHARDS, f"{name}.jsonl.gz")
    import gzip
    with gzip.open(shard, "wt") as fh:
        for r in rows:
            fh.write(json.dumps(r) + "\n")

    # Clones are scratch. Deleting each one after its shard is written keeps
    # peak disk at one repository instead of ten -- which matters on a box with
    # ~3 GB usable.
    freed = 0
    if not KEEP_CLONES:
        try:
            freed = int(sh(["du", "-sm", dest]).stdout.split()[0])
            shutil.rmtree(dest, ignore_errors=True)
        except Exception:
            pass

    return {"repo": slug, "status": "ok", "commit": commit, "how": how,
            "candidates": len(cands), "files": n_files, "revisions": n_revs,
            "shard": os.path.relpath(shard, REPO),
            "clone_deleted_mb": freed,
            "seconds": round(time.time() - t0, 1)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repos", type=int, default=len(REPOS))
    ap.add_argument("--files-per-repo", type=int, default=140)
    ap.add_argument("--max-rev-per-file", type=int, default=40)
    ap.add_argument("--keep-clones", action="store_true")
    a = ap.parse_args()
    global KEEP_CLONES
    KEEP_CLONES = a.keep_clones

    os.makedirs(WORK, exist_ok=True)
    os.makedirs(SHARDS, exist_ok=True)
    manifest, totals = [], {"files": 0, "revisions": 0}
    for slug, lang in REPOS[:a.repos]:
        print(f"=== {slug} ({lang}) ===", flush=True)
        try:
            r = build_repo(slug, lang, a.files_per_repo, a.max_rev_per_file)
        except Exception as e:                       # keep going on one bad repo
            r = {"repo": slug, "status": "error", "detail": repr(e)[:300]}
        manifest.append(r)
        if r.get("status") == "ok":
            totals["files"] += r["files"]
            totals["revisions"] += r["revisions"]
        print(f"    {r.get('status')}  files={r.get('files')} "
              f"revs={r.get('revisions')} {r.get('seconds','')}s", flush=True)

    out = {"built": time.strftime("%Y-%m-%dT%H:%M:%S"),
           "min_revs": MIN_REVS, "repos": manifest, "totals": totals,
           "note": ("No rename tracking: history restarts at a rename. "
                    "Consecutive identical contents deduped."),
           "reproduce": "python3 research/papers/p2_build_corpus.py"}
    path = os.path.join(OUT, "corpus_manifest.json")
    with open(path, "w") as fh:
        json.dump(out, fh, indent=2)

    print(f"\n=== corpus ===\n  repos ok: "
          f"{sum(1 for r in manifest if r.get('status')=='ok')}/{len(manifest)}")
    print(f"  files: {totals['files']}  revisions: {totals['revisions']}")
    print(f"wrote {os.path.relpath(path, REPO)}")
    return 0 if totals["files"] >= 100 else 1


if __name__ == "__main__":
    sys.exit(main())
