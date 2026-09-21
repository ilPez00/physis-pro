# Feather agent guide — install the current physis-pro build

You are on **feather** (weak client, disk ~94% full, glibc 2.41, ollama has
`qwen2.5-coder:3b` + `qwen3:4b`, **no** `nomic-embed-text`). Never build here:
no disk, no toolchain margin. Pansa builds for you; you install + smoke-test.

Last push: **2026-09-21 ~13:37** from pansa — **ROLLED BACK the same hour**.
The `cli,tui,web,embed-onnx` rebuild (glibc 2.43 host) produced binaries feather
(2.41) refuses to start (`GLIBC_2.43 not found`, both `physis-hud` and `physis-pro`).
A `rust:1-bookworm` container rebuild was attempted for the older glibc but fails
at link time: bookworm's `libstdc++` lacks `_M_replace_cold` + `__isoc23_strtol`
symbols the vendored `ort_sys` needs. Feather's `~/.local/bin` therefore still
holds the **2026-09-18 full-feature build** (`physis-pro` 59 MB, no `nav`
subcommand — `error: unrecognized subcommand 'nav'`). The PH-028 `nav
export/import/retag-store/stamp` surface has **never** run on feather. (An older
line here claimed a 06:11 lean push of a 14.9 MB binary — no such binary was
ever confirmed on the client; treat it as stale.)

Branch delivered: **`agent/PH-028`** on origin (this branch — PH-028 nav
portability: `nav export/import/retag-store/stamp`, embedder stamp, remote-embed
label; built on PH-029's reducer + qualified symbol extraction). **Source only**:
feather fetched `origin/agent/PH-028` @ `f85d278` but no PH-028 binary runs there
yet (see "Last push" above). Feather's own checkout (`~/dev/physis-pro`) sits on
`main` + local wip — PH-028 code reaches it only via `git fetch origin` +
checkout, never via its `~/.local/bin`. The HUD now labels a lexical index `LEXICAL ONLY` and
prints `semantic: false` in the index summary. The previous binary
(2026-09-18) was built from `agent/MS-005-q6`, whose `src/nav/workspace.rs` held
`(n, true, kind)` at line 140 and the component-presence test at line 201 — the
same two lines this push fixes — so that build reported `semantic: true` and
`[semantic+lexical]` on this box while running the identical string-matching
fallback. (Source-level verification, not a re-run of the old binary: the push
replaced it.)

## 0. Reaching the fleet (measured 2026-09-21)

`feather` (100.122.202.38) is **offline**; the live node is **`feather-1`**
(100.94.69.0 / LAN 192.168.1.14) and the `feather` alias points at it. Tailscale
SSH answers with `# Tailscale SSH requires an additional check` plus a browser
URL, so an unattended `ssh feather …` **cannot** be approved by an agent. The
LAN path uses the normal key auth and works:

```bash
ssh gio@192.168.1.14                      # works with keys, no browser step
```

Pushing from pansa therefore needs the LAN host:

```bash
PHYSIS_FEATHER_HOST=gio@192.168.1.14 \
PHYSIS_TARGET_DIR=target-trixie-mine/release \
  scripts/push-feather.sh physis-hud
```


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
`~/.local/bin/` — `scripts/push-feather.sh` on pansa does the staging + atomic
swap, and takes an optional list of names (`scripts/push-feather.sh
physis-studio`). Verify they are the pushed ones, never rebuild:

```bash
ls -la ~/.local/bin/physis-pro ~/.local/bin/physis-hud ~/.local/bin/physis-studio
physis-hud --help | head -3   # must print flags, not a GLIBC error
physis-studio --snapshot STATUS | head -5   # one real frame, no TTY needed
```

The HUD snapshot matrix — the check every HUD packet must re-run here:

```bash
export PHYSIS_ALLOW_DEV_LICENCE=1 PHYSIS_EMBEDDER=random-projection
export PHYSIS_DATA_DIR=/tmp/hud-feather-test/.physis
mkdir -p /tmp/hud-feather-test
printf '# notes\n\nretrieval and packing\n' > /tmp/hud-feather-test/notes.md
printf 'fn retrieval_notes() -> usize { 3 }\n' > /tmp/hud-feather-test/retrieval.rs
physis-hud /tmp/hud-feather-test --prescan-only     # "semantic: false"
for s in 60x18 100x30 160x50; do for v in world graph; do
  physis-hud /tmp/hud-feather-test --snapshot=$s --query=retrieval --view=$v | head -1
done; done
# every frame must print: PHYSIS NAVIGATOR  [LEXICAL ONLY] embedder: random-projection
```

Use a throwaway `PHYSIS_DATA_DIR` like the above: `~/dev/physis-pro` is a real
clone another session may be indexing, and the index is single-writer.

Installed set: `physis` (front door), `physis-pro`, `physis-pro-web`,
`physis-pro-tui`, `physis-pro-wiki`, `physis-studio`, `physis-hud`.

`physis` is now the **real front-door binary**, not a local shim (a 467-byte
`physis` wrapper used to sit here; it was replaced by the pushed binary on
2026-09-18 and no copy of it survives). It runs fine on feather as long as the
licence + embedder env are exported, which the smoke below does — its banner,
`inspect`, `context` and `bench` all work with `PHYSIS_EMBEDDER=random-projection`.

**Not** in this build: `ocr` and `ingest` features. They pull `tesseract` →
`leptonica-sys`, which needs `libleptonica-dev` + `libtesseract-dev` in the
build image and several MB of native libs on the target — wrong for a client
whose disk is 96% full. Consequence to state plainly when reporting: OCR of
scanned documents and spreadsheet extraction are unavailable here; document
ingestion still handles text/markdown/PDF/mail.

If `GLIBC_2.4x not found` appears, the binaries are the wrong build — stop
and report which glibc your `ldd --version` shows. Do not `cargo build` here.

## 3. Licence (dev key, already placed — check it exists)

A dev-build binary verifies against the dev fallback key (all-zeros seed),
NOT the vendor key in `~/.physis-vendor`. A vendor-signed licence fails with
`BadSignature` here. The valid key is minted by the `physis-dev-license`
crate (private half is public — dev only, never ship):

```bash
ls ~/.physis/license.key
PHYSIS_ALLOW_DEV_LICENCE=1 physis-pro license 2>&1 | head -2
# expect: ENTERPRISE / * — if BadSignature, the key is vendor-signed: ask
# pansa to re-push it with `(cd physis-dev-license && cargo run -q)`
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
