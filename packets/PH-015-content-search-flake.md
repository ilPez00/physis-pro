---
id: PH-015
track: physis-pro
phase: 1
status: todo
rung: 2
depends: []
gate: "cd ~/dev/physis-pro && for i in 1 2 3 4 5; do cargo test || exit 1; done"
commit: "branch agent/PH-015; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
---

# Diagnose the content-search flake under full-suite concurrency

## Why (the failure this prevents)

A CI run that fails one run in three on an unrelated PR trains reviewers to
re-run CI instead of reading the failure — which is how a real regression
eventually slips through disguised as "probably the flaky one again."

## Context (measured 2026-09-05, verify before relying on it)

`tests/http_surface.rs::content_search_finds_sections_cites_them_and_reuses_its_index`
failed twice in this session, both times as part of a full `cargo test` run
(89 passed / 1 failed), with this panic:

```
both files mention bearings: {"hits":[{"path":"/tmp/.tmpXXXXXX/line-2/maintenance.md", ...}], "matched":1, ...}
```

i.e. the test expects hits from **two** files but the search only returns
one. Both times it was run in isolation immediately after
(`cargo test --test http_surface content_search_finds_sections_cites_them_and_reuses_its_index`)
it passed. This was on unrelated commits (a clippy-only cleanup and, before
that, a physis-core version bump) — not caused by either change.

Leading suspects, cheapest to check first:
- `tempfile::tempdir()` naming collision or content bleed between two
  concurrently-running tests that both build a small corpus under `.tmp*`.
- A shared/global embedder or content-index cache keyed on something that
  collides across parallel test threads (`RandomProjectionEmbedder` is
  supposed to be deterministic per-input, but check whether it or the
  content index has any process-global mutable state).
- Genuine ordering dependency: does this test rely on another test's side
  effect that only sometimes runs first?

## Investigation 2026-09-13 — three suspects ruled out, one real defect found

Read-only pass, no test run (the Tier-1 gate held the target lock). Recorded
because eliminating a suspect is information, and the packet's original list
sent me at two things that cannot be the cause.

**Ruled out — environment racing.** This was the top suspect by analogy: PH-020
turned out to be exactly that, a process-global `PHYSIS_DIRECTION_METRIC` read
inside a hot function while one test set it. `http_surface` does set
process-global env — `app_on` (`tests/http_surface.rs:37`) sets
`PHYSIS_SCAN_ROOT`, `PHYSIS_DATA_DIR`, `PHYSIS_EMBEDDER` — **but it takes
`ENV_LOCK` first and returns the guard to the caller**, so the lock is held for
the calling test's whole body. Every call site binds it:

```
64  let (router, _dir, _env) = app(
14  let (router, _env)       = app_on(
 5  let (router, dir, _env)  = app(
 2  let (router, env)        = app_on(
 2  let (router, _dir, _guard) = app(
```

87 of 87 bind to a named binding. None binds to plain `_`, which would drop the
guard immediately and leak the env — that was the specific thing worth
checking, and it is not present. Within this binary, the tests are serialised.

**Ruled out — a shared index across tests.** `content_index` is
`Mutex<Option<Arc<ContentIndex>>>` constructed fresh per `build_app()`
(`src/web/mod.rs:2582`), so each test's router owns its own. Not a process
global; there are no `Lazy`/`OnceLock` statics in `web/mod.rs`, `content.rs` or
`ingest.rs`.

**Ruled out — cross-root reuse.** `ensure_index` only reuses a cached index
when `idx.root == root_buf` (`src/web/content.rs:248`), so one test's tempdir
cannot serve another's query.

**Found, and it is a genuine defect whether or not it causes this flake:**
`scan_signature` (`src/content.rs:645`) summarises a tree as
`IndexSignature { files, bytes, newest }`, where `newest` is
`d.as_secs()` — **whole seconds**. The signature is what decides "the index is
current". So a tree that changes without changing its file count or total byte
count, within the same second as the previous build, produces an identical
signature and the stale index is reused. Same-size edits are not exotic: a
fixed-width field, a status flag, a timestamp rewrite.

This is worth fixing on its own terms. It is *not* yet shown to be the cause
here, because in the failing test both files exist before the first query, so
the first build should see both.

**Where to look next, given the above.** The remaining hypothesis is that
interference comes from *another test binary*, not another test — `cargo test`
runs binaries in parallel, and `ENV_LOCK` is per-process. That points at
something shared outside the process: the filesystem, or CPU-timing-dependent
behaviour in ingest. Worth running `cargo test --test http_surface` in a loop
while a second `cargo test --lib` runs, and seeing whether the flake rate moves.

## Steps

1. Reproduce reliably: run `cargo test` (not scoped to one test — isolation
   doesn't reproduce it) 5-10 times, or with `--test-threads=1` vs default,
   to establish whether thread-count correlates with failure rate.
2. Once reproducing, bisect: run with `--test-threads=1` (should always
   pass if it's concurrency) then binary-search which other test(s) running
   concurrently trigger it (e.g. `cargo test <this> <suspect>` with two
   explicit filters, `--test-threads=2`).
3. Find the shared resource and fix the isolation (usually: make the tempdir
   name include a random/unique component if it doesn't already, or scope
   whatever cache is shared per-test rather than per-process).
4. If it turns out to be a genuine logic bug in content search under
   concurrent access (not test isolation), fix that instead and say so.

## Tests you must add

- Whatever the root cause turns out to be needs a test that fails before the
  fix and passes after — if the fix is test isolation, a test that asserts
  the fixed invariant (e.g. two instances of the search index in the same
  process don't share the leaking state).

## Gate

```bash
cd ~/dev/physis-pro
for i in 1 2 3 4 5; do cargo test || exit 1; done
```

## Report

Append `packets/reports/PH-015.md` per PROTOCOL §5.
