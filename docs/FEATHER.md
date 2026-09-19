# Feather agent guide — install the current physis-pro build

You are on **feather** (weak client, disk ~94% full, glibc 2.41, ollama has
`qwen2.5-coder:3b` + `qwen3:4b`, **no** `nomic-embed-text`). Never build here:
no disk, no toolchain margin. Pansa builds for you; you install + smoke-test.

Current version: branch **`agent/MS-005-q6`** on origin (7 commits over
`ac51db6`: HUD edge toggle, adaptive-doc export, Q6 rerun, HUD polish,
3× wip groups with other agents' packets/code/docs).

## 1. Get the source (preferred: fetch; fallback: patches)

```bash
cd ~/dev/physis-pro          # or ~/code/physis-pro — check both, keep the newer
git fetch origin
git checkout agent/MS-005-q6 # or: git checkout -b fe-test origin/agent/MS-005-q6
```

Fallback — patch files pushed to `~/physis-diffs/` (0001..0007, in order):

```bash
cd ~/dev/physis-pro
git checkout <base>          # ac51db6 unless told otherwise
git am ~/physis-diffs/*.patch
```

## 2. Install the binaries (no build — copy, don't compile)

Pansa pushes trixie-linked binaries (glibc 2.41-compatible) to
`~/.local/bin/`. Verify they are the pushed ones, never rebuild:

```bash
ls -la ~/.local/bin/physis-pro ~/.local/bin/physis-hud
physis-hud --help | head -3   # must print flags, not a GLIBC error
```

If `GLIBC_2.4x not found` appears, the binaries are the wrong build — stop
and report which glibc your `ldd --version` shows. Do not `cargo build` here.

## 3. Licence (dev key, already placed — check it exists)

```bash
ls ~/.physis/license.key
PHYSIS_ALLOW_DEV_LICENCE=1 physis-pro license 2>&1 | head -2
```

If the licence is missing, ask pansa to re-push it (`physis-dev-license`
crate mints it; the private half is public — dev only, never ship).

## 4. Smoke test (fast, offline, honest-degraded)

Feather has no `nomic-embed-text`, so force the lexical embedder. The
`LEXICAL ONLY` header is expected, not a failure.

```bash
mkdir -p /tmp/physis-smoke
printf '# hello\n\nsmall test file about retrieval and packing\n' > /tmp/physis-smoke/a.md
printf 'fn main() {}\n' > /tmp/physis-smoke/b.rs

export PHYSIS_ALLOW_DEV_LICENCE=1 PHYSIS_EMBEDDER=random-projection

physis-hud /tmp/physis-smoke --prescan-only
# expect: "nav index: 4 objects … +4 embeddings in …ms"

physis-pro pack /tmp/physis-smoke "retrieval packing" --budget 500 --top-k 5
# expect: ranked chunks a.md / b.rs with token counts

printf 'what does this project do?\ninspect\ncontext\nbench\nquit\n' \
  | physis-pro /tmp/physis-smoke
# expect: PHYSIS banner (files/objects/sample map/index ready), PROJECT tree
# with why: breakdowns, CONTEXT packet with token math, BENCH numbers
```

## 5. Optional: index the real clone (slow first time)

```bash
export PHYSIS_ALLOW_DEV_LICENCE=1 PHYSIS_EMBEDDER=random-projection
physis-hud ~/dev/physis-pro --prescan-only   # thousands of files; background it
```

First index embeds every chunk (minutes); later starts are incremental.
One writer per repository: never run two loops/HUDs on the same root
(`could not acquire lock … index.sled/db`).

## Rules that already cost someone a session

- Services bind the Tailscale IP, never loopback.
- Disk is nearly full: no `target/`, no `node_modules`, no model pulls.
- `random-projection` matches strings, not meaning — every telemetry block
  prints the provider, so say which embedder your numbers came from.
- Report back: binary versions (`ls -la`), smoke outputs, embedder used.
