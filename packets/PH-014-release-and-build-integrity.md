---
id: PH-014
track: physis-pro
phase: 0
status: todo
rung: 1
depends: []
gate: "cd ~/dev/physis-pro && cargo test --workspace && git ls-files -s | awk '$1==\"100644\"' >/dev/null && ./scripts/check-release-integrity.sh"
commit: "branch agent/PH-014; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
---

# Release and build integrity: three defects from trees that did not match

## Why (the failure this prevents)

Three separate incidents on 2026-09-04/05 share one root cause: **something was
committed or published from a working tree that did not match what was recorded.**
`main` stopped compiling and nobody noticed for a day; a crates.io version went out
that corresponds to no commit and can never be reproduced; and a 435 MB binary was
committed under a message asserting it had been excluded. Each was silent. The
build one cost a full working session to diagnose because the symptom (missing
methods) appeared nowhere near the cause (a struct deleted in an unrelated commit).

Left alone, the next release is equally unreproducible and the next `main` break
is equally invisible until someone tries to build.

## Context (measured 2026-09-05, verify before relying on it)

**Environment traps on feather — read `mesh mem recall` or these will bite you:**
- `grep` is aliased to `rg`. `--include=…`, and some `-A/-B` forms, fail with
  `no matches found`. Use `rg -g '*.rs'` or `command grep`.
- `ls` is `eza`, `cp` is `cp -i` (hangs with no tty). Use `command ls` / `cp -f`.
- `cat` was aliased through a shadowed `bat` to `upower` — it printed battery info
  and `cat >> f <<EOF` silently wrote **nothing**. Fixed 2026-09-05 in
  `~/.bash_aliases` (backup: `~/.bash_aliases.bak-before-cat-fix-20260905`).
  If you see a write "succeed" and a later read show it missing, suspect an alias.
- `physis-core/` is a **git submodule** with its own remote (`ilPez00/physis-core`,
  branch `master`). Running `git rebase` in physis-pro checks the submodule back
  out at the recorded commit and **silently reverts uncommitted work inside it**.
  Always commit *and push* inside `physis-core/` first, then touch the superproject.

### Item A — `origin/main` does not compile

**Scope note (2026-09-05, after a deliberate reset):** the local `main` branch was
reset to `77d5e13`, which is based on `db70406` and does **not** contain `bb469ea`.
So the local tree compiles and carries the typed-object layer natively. This item
is about **`origin/main`**, which is 2 commits ahead of local and does contain the
break. Whoever picks this up must decide first whether `bb469ea` is being kept at
all — if it is reverted upstream instead, only Items B and C remain.

Commit `bb469ea` ("feat(plan-11): enhanced ingestion, structured templates, …")
did two incompatible things at once:

1. It **deleted** from `src/models.rs`: the `kind` and `properties` fields on both
   `CoherenceNode` and `NodeView`, the builders `with_kind` / `with_properties` /
   `property`, and `OBJECT_TYPES`, `object_type_for`, `is_valid_object_kind`,
   `ActionKind`, `TypedAction`, `typed_action_for` — while leaving every caller
   in place. Result: 37 compile errors.
2. It **added** 18 routes `/api/v1/machines/<name>/register` in `src/web/mod.rs`
   wired to `machines::machines_<name>_register_handler` functions that it never
   added and that exist in **no commit in this repository**
   (`git log --all -S 'machines_chronos_register_handler'` returns only `bb469ea`).

A fix was worked out on 2026-09-05 against a tree that had `bb469ea` merged in,
then discarded by the reset. It is reproducible and worth knowing before you start:

- The deleted items restore **verbatim from `bb469ea^`** — recovered text, not a
  reconstruction. `git show bb469ea^:src/models.rs` has all of them.
- `src/content.rs`'s test helper needs `template: None`, which `bb469ea` made
  mandatory on `IngestedDoc`.
- With those two done, the only errors left are the 18 missing handlers.
- `/api/v1/coherence/register` looks like the same defect but is **not** — its
  handler (`coherence_register_handler`, `src/web/coherence.rs:456`) exists. Do
  not disable it.

**What remains:** write the 18 handlers, then re-enable their routes.

The intent is unambiguous from the surrounding code: each `/register` endpoint runs
the same machine as its sibling and registers the result as a **typed**
`CoherenceNode` — which is exactly why `bb469ea` needed `kind` / `properties`, and
exactly what it deleted. The 18 machines are:

    apodeixis  chroma   chronos  elenchus  empedocles  erosa
    erosp      euclid   harmony  kinesis   krisis      lex
    pathos     plutos   polis    soma      techne      topos

- Sibling to copy shape from: `machines_<name>_handler` in `src/web/machines.rs`
  (each has its own `<Name>Request` type — do not invent a shared one).
- Reference implementation for the *register* half:
  `coherence_register_handler`, `src/web/coherence.rs:456`.
- Use `CoherenceNode::new(embedding).with_kind(…).with_properties(…)`, and pick
  `kind` from `OBJECT_TYPES` / `object_type_for` — `is_valid_object_kind` exists to
  reject anything else. Do not stamp a `kind` that is not in the vocabulary.

If any machine genuinely has no sensible typed-object mapping, **say so in the
report and leave that route commented** with a one-line reason. Eighteen guessed
mappings are worse than fifteen correct ones and three flagged questions.

### Item B — crates.io `physis-core` 0.1.12 corresponds to no commit

| version | published | reproducible from git? |
|---|---|---|
| 0.1.11 | 2026-08-30 16:26 | yes |
| **0.1.12** | **2026-09-05 05:00** | **no — matches no commit; its `src/hypothesis.rs` is the pre-resolution 459-line file** |
| 0.1.13 | 2026-09-05 07:08 | yes — tag `v0.1.13`, published from a clean clone |

0.1.12 was published from a working tree whose version bump was never committed.
crates.io versions are immutable, so the number cannot be reused or corrected —
only yanked. Cargo credentials live on **omo** and **pansa**, not feather.

### Item C — a 435 MB binary was committed under a message denying it

An unpushed local commit added `models/bge-base-en-v1.5/onnx/model.onnx`
(435,811,539 bytes) with the message "…model.onnx excluded via gitignore". The
pattern was `models/*.onnx`, which does not match a nested path. GitHub rejects
files over 100 MB, so the push would have failed outright.

Already done: the blob was unstaged (**the file remains on disk**) and `.gitignore`
now uses `models/**/*.onnx`, plus `**/*.safetensors` and `**/*.bin`. Verified with
`git check-ignore -v`.

**What remains for you:** the guard, so this cannot recur silently.

## Steps

1. **Decide the branch question first.** Local `main` (`77d5e13`) does not contain
   `bb469ea`; `origin/main` does. Confirm with the owner whether `bb469ea` is being
   merged forward or reverted. If reverted, Item A is closed — skip to Item B.
   Then confirm your starting tree compiles: `cargo check --workspace` must be clean
   before you change anything.
2. **Item A** (only if `bb469ea` is being kept). Restore the deleted `models.rs`
   items from `bb469ea^` and add `template: None` to the `IngestedDoc` helper in
   `src/content.rs`. Then implement the 18 `machines_<name>_register_handler`
   functions in `src/web/machines.rs`, enabling each route in `src/web/mod.rs` only
   as its handler lands — never wire a route to a function you have not written.
3. **Item B.** Add `scripts/check-release-integrity.sh` that exits non-zero when
   any of these is true, and wire it into CI on `main`:
   - the working tree is dirty (`git status --porcelain` non-empty);
   - `HEAD` is not tagged `v<version>` where `<version>` is the `version =` field of
     the crate being published;
   - that version already exists on crates.io.
   Make `cargo publish` unreachable except through it (a `just release` / `make
   release` target that calls the script first is enough — document it in
   `docs/RELEASING.md`, which does not yet exist).
4. **Item B, decision required — do not guess.** Ask the owner whether to
   `cargo yank --version 0.1.12 physis-core`. Yanking stops new dependents without
   breaking existing lockfiles. It is reversible (`--undo`). Record the answer in
   `docs/RELEASING.md` either way, with the reason.
5. **Item C.** Add a pre-commit hook (and the same check in CI, because hooks are
   per-clone — see the `aion repo guards` memory) that refuses any staged blob over
   25 MB, naming the file and suggesting the `.gitignore` pattern. Verify the repo
   is currently clean of them:
   `git rev-list --objects --all | git cat-file --batch-check='%(objecttype) %(objectname) %(objectsize) %(rest)' | awk '$1=="blob" && $3>25000000'`
   If that command reports anything already in history, **stop and report** — a
   history rewrite is the owner's decision, not yours.
6. **Item A regression guard.** Add a CI job that runs `cargo check --workspace`
   on every push to `main`. Item A went unnoticed for a day only because nothing
   checked. This is the step that makes the other two unnecessary next time.

## Tests you must add

- **Unit** (`src/models.rs`, existing `#[cfg(test)]` module): `object_type_for`
  returns a member of `OBJECT_TYPES` for every `(domain, mode)` pair it accepts,
  and `is_valid_object_kind` rejects a kind not in the vocabulary. These pin the
  restored code so a future rewrite cannot delete it silently again.
- **Unit** (`src/models.rs`): a `CoherenceNode` built with `.with_kind(…)` and
  `.with_properties(…)` survives a `serde_json` round-trip with both fields intact,
  and `observable()` propagates both to `NodeView`.
- **Integration** (`tests/http_surface.rs`, which already drives the router
  in-process via `ServiceExt::oneshot`): for at least three of the eighteen
  machines, `POST /api/v1/machines/<name>/register` returns 200 and the node it
  created has a non-empty `kind` drawn from `OBJECT_TYPES`.
- **Script test:** `check-release-integrity.sh` exits non-zero on a dirty tree and
  on an untagged HEAD. A guard nobody proved fails is not a guard.
- A bug fix without a regression test is not done.

## Gate

```bash
cd ~/dev/physis-pro
cargo check --workspace
cargo test --workspace
# every machines/*/register route resolves to a real handler
for m in apodeixis chroma chronos elenchus empedocles erosa erosp euclid harmony \
         kinesis krisis lex pathos plutos polis soma techne topos; do
  rg -q "pub async fn machines_${m}_register_handler" src/web/machines.rs || exit 1
done
# no oversized blob anywhere in history
[ -z "$(git rev-list --objects --all \
    | git cat-file --batch-check='%(objecttype) %(objectname) %(objectsize) %(rest)' \
    | awk '$1=="blob" && $3>25000000')" ]
# the release guard exists and rejects a dirty tree
./scripts/check-release-integrity.sh --self-test
```

## Commit strategy

One commit per item (A, B, C) on `branch agent/PH-014`, so Item A can be reverted
without losing the guards. **Do not touch `physis-core/`** — it is a submodule at
`789e6df` (`v0.1.13`) and nothing in this packet requires changing it. If you find
you need to, commit and push inside the submodule *first*, then stage the pointer.

## Report

State plainly, for each item: done / partially done / not done, and for Item A how
many of the eighteen handlers you wrote versus flagged as having no sensible typed
mapping. Name any machine whose `kind` you were unsure of rather than picking one
to make the count look complete. If the owner has not answered the yank question in
step 4, say the item is blocked on them — do not decide it yourself.
