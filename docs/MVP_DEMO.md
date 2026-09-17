# MVP demo — the closed loop, reproducible

One command, one repository, one transcript:

```bash
physis-pro <directory>          # the loop (REPL on stdin/stdout)
physis <directory>              # same loop through the front door, when installed
scripts/mvp-demo.sh <directory> # the same loop, scripted, transcript captured
```

All three are the same binary and the same session; the script only fixes the
input lines and tees the output, so the demo cannot drift from the product.
(`physis <dir>` is verified against the local build with
`PATH=target/debug:$PATH`; the front door forwards unparsed arguments.)

---

## What the demo shows (≤2 minutes)

```text
real directory/repository
  → scan + structural/semantic map
  → human natural-language intent
  → retrieve smallest relevant world slice
  → inspect / answer / plan
  → optional agent action
  → real filesystem change
  → tests/build/observations
  → semantic Δ
  → update affected map
  → show consequences
```

The claim being demonstrated is not "Physis has an agent". It is:

> A human navigates and commands a **map of a real repository** in natural
> language. A model receives **the relevant portion** rather than the
> undifferentiated world. Actions change the real territory; Physis observes the
> consequences and updates its map.

---

## Run it

```bash
# 1. build (no model files needed for the offline mode)
cargo build --bin physis-pro --features cli

# 2. run the scripted demo; the transcript lands in <repo>/.physis/
PHYSIS_EMBEDDER=random-projection scripts/mvp-demo.sh /path/to/repo

# or drive it by hand, same loop, same commands:
PHYSIS_EMBEDDER=random-projection ./target/debug/physis-pro /path/to/repo
```

Interactive commands (plain language also works for anything not in the list):

| command | what it does |
|---|---|
| `what does this project do?` | scope an intent → map + slice + grounded answer |
| `where is retrieval implemented?` | same path; retrieval ranks by meaning + lexical + recency |
| `inspect` / `inspect <path>` | provenance, semantic parents, relations, exact source |
| `dependencies [path]` | files that mention the focus's stem or symbols |
| `contradictions` | contradiction candidates inside the current slice |
| `history` | git log, or the mtime-ordered snapshot when there is no git |
| `differences` | sample-map difference count |
| `related` | files related to the focused object |
| `context` | deterministic packet: INTENT/TARGETS/DEPENDENCIES/CONSTRAINTS/RECENT DELTA/snippets/EXCLUDED + measured costs |
| `focus [query]` | re-scope + HOT/WARM/COLD attention bands (pins never demote) |
| `edit [path]` | open focus in `$EDITOR`, then rescan → Δ (undo is the editor's, before save) |
| `rescan` | observe whatever changed on disk → Δ → incremental remap (no validation) |
| `bench` | measured offline numbers: scan/index size/query+propagation latency/slice/generator/network |
| `improve … --yes` | plan → validation → Δ → remap (no `--yes` = plan only) |
| `help`, `quit` | |

Environment:

| variable | meaning |
|---|---|
| `PHYSIS_EMBEDDER=random-projection` | deterministic offline embedder (no model files, no network) |
| `PHYSIS_MODEL=none` | no generator: deterministic/manual path (default). `local` = optional offline worker via `PHYSIS_ORACLE_*` → localhost; packet stays complete without it |
| `PHYSIS_DATA_DIR` | where the index, caches and logs live (default `<repo>/.physis`) |
| `PHYSIS_MVP_VALIDATE` | the validation command the action step runs (default `cargo test --quiet`) |
| `PHYSIS_MVP_TIMING=1` | phase timings on stderr, for diagnosing a slow start |

**First run vs later runs.** The first start builds the index and embeds every
chunk; later starts are incremental. The same is true of the first *query*
(which fills the shared embedding cache). The numbers below are from both.

---

## Measured results

### `physis-core` — 2026-09-17 (339 files, 2 992 index objects)

```bash
PHYSIS_MVP_VALIDATE='cargo check --quiet 2>&1 | tail -3' \
  scripts/mvp-demo.sh physis-core
```

| measurement | value |
|---|---|
| startup, first ever run (index build) | 78.7 s (of which 71.8 s embedding 2 992 objects) |
| startup, incremental (persisted index + sample-map cache) | 1.7 s |
| first query (fills the pack embedding cache) | 70.7 s |
| second query (cache hit) | 1.5 s |
| repository objects considered | 2 992 |
| candidate/raw context tokens | 1 204 946 |
| final context tokens | 743–833 of a 2 000 budget |
| **context reduction** | **99.93 %** (1 204 946 → ~800) |
| objects selected into the slice | 6–8 chunks over 4–8 files |
| validation | `cargo check --quiet` (exit 0), recorded in the Δ |
| Δ after the action | source unchanged by `cargo check` (0 objects remapped) — stated as such, not dressed up |
| transitions recorded | 10, one per interaction |
| whole demo wall time | 80 s |

The transcript is `physis-core/.physis/MVP_DEMO_TRANSCRIPT.md` (rewritten by each
run) and the raw transition log is `physis-core/.physis/mvp_transitions.jsonl`.

### `physis-core` (Physis itself, dogfood) — 2026-09-17, real bounded edit

The dogfood scenario edits a real file instead of only building:

```bash
PHYSIS_MVP_VALIDATE='printf "\n// bounded demo edit\n" >> src/act_recall.rs \
                     && cargo check --quiet --lib 2>&1 | tail -3' \
  scripts/mvp-demo.sh physis-core
```

| measurement | value |
|---|---|
| startup (index + caches warm) | 1.4 s |
| whole demo wall time | 14 s |
| intent → slice latency | 1 257 ms (first query) / 1 370 ms (second) |
| candidate/raw context tokens | 1 204 946 |
| final context tokens | 2 000 of 2 000 (budget-capped), then 743 |
| **context reduction** | **99.83 – 99.94 %** |
| action | real append to `src/act_recall.rs`, then `cargo check --quiet --lib` |
| Δ CHANGED | `src/act_recall.rs` |
| Δ AFFECTED (6) | `src/lib.rs`, `src/main.rs`, `src/act.rs`, `src/claim_identity.rs`, `examples/claim_identity.rs`, `examples/floor_spread.rs` |
| Δ VALIDATION | `exit Some(0)` in 1 586 ms (the command is printed in full, so a transcript can never claim a validation that did not run) |
| Δ MAP | 1 object remapped (2 992 → 2 992), 6 semantic neighborhoods changed — incremental, not a full rescan |
| transitions recorded | 10, one per interaction |
| revert | `git -C physis-core checkout -- src/act_recall.rs` (the demo edits nothing else) |

Transcript: `physis-core/.physis/MVP_DEMO_TRANSCRIPT.md`; raw records:
`physis-core/.physis/mvp_transitions.jsonl`.

**The AFFECTED list is the result of a measured correction, not a lucky guess.**
The first rule ("does this file contain the symbol name?") returned **261**
dependents for that one-file append — `run`, `Report` and `Source` occur in
prose and in benchmark outputs. Requiring code-shaped references and only
symbols the file *defines* (not the types it implements — `impl VectorEmbed`
alone accounted for 79 false hits) brought it to **6**, each one checkable by
hand. The rule, and the measurement behind it, are in
`src/mvp/deps.rs::affected_dependents`.

---

## Install on another machine (measured on `feather`)

The loop needs no model files, no network and no licence. It does need a binary
that runs on the target's libc.

```bash
# 1. from the checkout that has the MVP, ship the source delta
git diff -- src/lib.rs src/main.rs src/pack.rs src/scanner.rs src/ported/act.rs > /tmp/mvp-src.patch
tar czf /tmp/mvp-payload.tgz src/mvp src/nav tests/mvp_loop.rs \
        docs/MVP_DEMO.md MVP_STATUS.md POST_MVP.md scripts/mvp-demo.sh
scp /tmp/mvp-src.patch /tmp/mvp-payload.tgz <host>:/tmp/

# 2. on the target: apply, extract, build there
ssh <host>
cd ~/dev/physis-pro
git apply /tmp/mvp-src.patch && tar xzf /tmp/mvp-payload.tgz -C .
cargo build --release --features cli --bin physis-pro -j3   # 2 min, 6 cores, warm cache

# 3. install and run
install -m755 target/release/physis-pro ~/.local/bin/physis-pro
physis ~/dev/physis-pro/physis-core        # front door forwards here
```

Measured on `feather` (Debian 13, 6 cores, 7 GB): cold build **2m 00s** from the
existing `target/` cache; startup after the first index **instant**; the scripted
demo **3 s** wall with latencies **412 ms / 334 ms** and the same numbers as
`pansa` (339 files, 2 992 objects, 1 204 962 candidate tokens → 2 000 / 743).

Two gotchas, both hit for real:

* **glibc.** A release binary built on `pansa` (glibc 2.43) does **not** run on
  `feather` (glibc 2.41): it links `GLIBC_2.43`. Build on the target, or ship a
  musl-static build.
* **Missing untracked modules.** `src/nav/` was never committed in the source
  checkout, so the first build on the target failed with
  `file not found for module nav`. The payload above includes it.

State lives in `<repo>/.physis` by default (`PHYSIS_DATA_DIR` overrides), so
`physis <repo>` run from any directory keeps one index, one transition log and
one cache per repository.

### Recording the demo

```bash
# transcript + timings, no recorder needed
cd ~/dev/physis-pro
PHYSIS_BIN=$HOME/.local/bin/physis-pro PHYSIS_EMBEDDER=random-projection \
  scripts/mvp-demo.sh physis-core /tmp/DEMO.md

# or record the interactive session
asciinema rec -c "physis ~/dev/physis-pro/physis-core" demo.cast
#   then type: what does this project do? / where is retrieval implemented? /
#              differences / inspect / improve … --yes / quit
```

Warm the target first (`rm -rf <repo>/.physis` is the clean state; the first run
builds the index and says so).

## Reproducing exactly

```bash
# clean state for the demo repository
rm -rf <repo>/.physis

# first run: builds the index, slow, says so
PHYSIS_EMBEDDER=random-projection scripts/mvp-demo.sh <repo>

# second run: incremental, fast — this is the ≤2-minute demo
PHYSIS_EMBEDDER=random-projection scripts/mvp-demo.sh <repo>
```

Two runs from a clean checkout are what the acceptance gate requires, and the
same two runs are asserted by `tests/mvp_loop.rs`
(`the_loop_runs_twice_from_a_clean_state`).
