# Releasing

This exists because three separate incidents on 2026-09-04/05 shared one root
cause: something was committed or published from a working tree that did not
match what was recorded (see `packets/PH-014-release-and-build-integrity.md`
for the full incident writeup). This document is the process that makes the
next release reproducible.

## The rule

**`cargo publish` is never run by hand.** The only path to it is
`just release-core`, which runs `scripts/check-release-integrity.sh` first.
The script refuses to let a publish through unless:

1. the working tree is clean (`git status --porcelain` is empty);
2. `HEAD` is tagged `v<version>` — bare, no crate prefix — where `<version>`
   is the `version =` field in `physis-core/Cargo.toml`. This is checked
   inside the `physis-core` submodule's **own** git repo (it has its own tag
   namespace, e.g. `v0.1.13`), not the superproject's `physis-core-vX.Y.Z`
   mirror tags, which are just a convenience marker in this repo and are not
   what gates a publish;
3. that version is not already live on crates.io.

Prove it works before trusting it: `./scripts/check-release-integrity.sh --self-test`
runs the same three checks against disposable repos and asserts the guard
actually rejects a dirty tree and an untagged HEAD. CI runs this on every push
(`release-integrity-selftest` job in `.github/workflows/ci.yml`).

## Release steps (physis-core)

1. Bump `version =` in `physis-core/Cargo.toml`, commit it **inside the
   submodule**, and push:
   ```
   cd physis-core
   git commit -am "chore: bump physis-core to X.Y.Z"
   git push origin master
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```
   Commit and push the submodule *before* touching the superproject — a
   `git rebase` or checkout in `physis-pro` silently reverts uncommitted
   submodule work back to whatever commit the superproject tree records (see
   `physis-core-is-a-submodule` in project memory).
2. From a **clean clone of the pushed tag** (not your working checkout —
   0.1.12 went out from an uncommitted bump precisely because someone
   published from a dirty tree):
   ```
   git clone --branch vX.Y.Z git@github.com:ilPez00/physis-core.git /tmp/release-check
   cd /tmp/release-check
   cargo publish --dry-run
   ```
3. In the superproject, stage the new submodule pointer and mirror the tag:
   ```
   cd ~/dev/physis-pro
   git add physis-core
   git tag physis-core-vX.Y.Z
   ```
4. Publish, gated:
   ```
   just release-core
   ```
   Credentials live on **omo** and **pansa**, not feather (or any other dev
   client) — run this step from one of those two.

## Known-bad version

**`physis-core` 0.1.12 corresponds to no commit.** It was published
2026-09-05 05:00 from a working tree whose version bump was never committed;
its `src/hypothesis.rs` is the pre-resolution 459-line file, not what shipped
in any tagged commit. crates.io versions are immutable — 0.1.12 cannot be
corrected or reused, only yanked.

**Decision (2026-09-05, owner confirmed): yank it.** Yanking is reversible
(`cargo yank --undo`) and only stops *new* dependents from resolving to an
unreproducible build — existing lockfiles pinned to 0.1.12 keep working. Run
from omo or pansa (credentials, not available on feather):
```
cargo yank --version 0.1.12 physis-core
```
This has not been run yet as of this writing — do it once, then delete this
paragraph and record the date it happened.

## Guards this process depends on

- `scripts/check-release-integrity.sh` — the three checks above.
- `scripts/hooks/pre-commit` — refuses to commit a staged blob over 25MB.
  Hooks are per-clone; run `just install-hooks` once after cloning to enable
  it (`git config core.hooksPath scripts/hooks`).
- CI `large-files` job — the same 25MB check as a backstop for anyone who
  hasn't run `just install-hooks`, or for a commit made outside this clone.
- CI `check-workspace` step (in the `check` job) — `cargo check --workspace`
  on every push, so a break introduced by an ontology/model change in one
  crate and consumed in the other (like `bb469ea`) fails CI immediately
  instead of going unnoticed for a day.
