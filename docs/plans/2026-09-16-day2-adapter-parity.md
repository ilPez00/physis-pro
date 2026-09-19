# Day 2 — adapters moved, state preserved, boundary enforced (2026-09-16)

Continues `docs/plans/2026-09-16-day1-migration-completion.md`. Day 2 of
`docs/plans/2026-09-15-core-pro-development.md` asks for one thing in the plan's
words: **"Extract adapters and preserve state — Core standalone build; restart
and migration parity."** This session verified the extraction was already
complete, found and fixed the one way it was not, proved parity by running the
shipped binary, and turned the boundary into a gate.

Nothing here deletes a feature.

## 1. What was already done when this session started

The watchers (`watch_fs`, `watch_git`, `watch_shell`, `watch_agent`,
`watch_proc`, `watch_browser`) were already relocated to
`src/ported/observe_adapters.rs` (committed `bc36b19`), and
`physis-core/src/observe.rs` already kept the record, the JSONL format and the
reader API (`append`/`read`/`read_tail`/`last_seq`/`by_source`/`concurrent`/
`promote`/`cited_by`) with a comment recording the move. So Day 2 was not a
move; it was a **proof that the move preserved state**, which is the part the
plan actually gates on.

## 2. The one way it was not preserved — found by running it, not reading it

`observe::append` opened the log in append mode and wrote directly onto whatever
was there. A process killed mid-write leaves a final line with **no newline**.
The next append then concatenated its first record onto that fragment; `read`
drops the whole line; and both the interrupted record and the first record of
the new batch vanished — a **skipped record and a gap in the sequence**, which
is the one thing an append-only log must never do.

Reproduced against a truncated fixture (3 complete records + a partial 4th) and
one `observe`: the writer reported `log now holds 203`, a fresh reader saw
`202`. The seq-3 record was gone.

**Fix (`physis-core/src/observe.rs`):** before writing, inspect the final byte;
if the file is non-empty and does not end in `\n`, emit the missing newline
first. The fragment becomes a line of its own that `read` already drops by
design, and the new records land intact. Nothing already written is rewritten or
removed, so the append-only guarantee holds. The handle is opened
`.read(true)` as well — an append-only handle is write-only by default, and
reading the tail byte from it fails with `EBADF` (found by the test, not by
reasoning).

Unit coverage: `appending_after_a_truncated_line_keeps_the_new_record`
(`cargo test -p physis-core --lib observe::` → 9 passed).

## 3. Parity, proved from the product side

`tests/observe_migration.rs` (new, 4 tests) runs the shipped `physis-pro-world`
binary against a sandboxed `PHYSIS_CORE_DIR` **and** `HOME` (the shell watcher
reads `$HOME/.zsh_history`; the operator's history is not a fixture). It asserts
the four acceptance conditions, not the presence of code:

| acceptance | test | what it asserts |
|---|---|---|
| legacy logs retain IDs, order, provenance | `legacy_ids_order_and_provenance_survive_the_move` | `trace` reads back `seq 1 · fs`, `by watch_fs`, the recorded time and subject; the truncated line costs one record, not the log |
| retries do not duplicate | `a_retry_appends_nothing_and_duplicates_nothing` | a second `observe` prints `nothing new` and the log is byte-identical |
| interrupted ingestion does not skip | `an_interrupted_append_does_not_skip_the_next_record` | the count the writer reports equals the count a fresh reader sees, over a truncated fixture |
| copied fixture answers the same after restart | `a_copied_log_answers_identically_after_restart` | `locate`/`inspect` are identical on a second process and on a byte-copy of the log in a fresh directory |

The default representation is the counted word table (no model, no clock in the
answer), so parity is exact rather than approximate.

## 4. The boundary, as a gate

`scripts/check-core-boundary.sh` (new) implements the plan's `CI.boundary()`:
dependency direction (no `use physis_pro` / `physis_pro::` / manifest dep in
Core), no moved product module exported by `physis-core/src/lib.rs` (19 names,
anchored so `act` does not match `act_recall`), the CLI surface (no product
verb; 10 mechanism verbs still present), and a removed verb failing with
`unrecognized subcommand` and a nonzero status. `worldstate`, `oracle` and
`config_run` are deliberately **not** on the moved list — Core keeps the
structural primitives those names refer to.

`.github/workflows/physis-core.yml` gains that step plus the plan's fourth part,
an **isolated build**: `physis-core` is copied (via `tar --exclude=target`) to a
temp dir and `cargo check --no-default-features` runs there, proving Core builds
with no Pro checkout at all. `cp -a` was rejected: the submodule carries a
1.3 GB stale `target/`.

## 5. Evidence

| check | command | result |
|---|---|---|
| Core standalone, no defaults | `tar -C physis-core --exclude=target -cf - . \| tar -C /tmp/… -xf -` then `cargo check --no-default-features` | exit 0 |
| Core standalone, default bin | `cargo build --bin physis-core` (isolated target) | exit 0 |
| Core standalone, all features | `cargo check --all-features` (isolated target) | exit 0 |
| Core observe tests | `cargo test -p physis-core --lib observe::` | 9 passed, 0 failed |
| migration + parity | `cargo test --test observe_migration` | 4 passed, 0 failed |
| Day 1 routes still green | `cargo test --test ported_routes` | 10 passed, 0 failed (40.60s) |
| boundary gate | `scripts/check-core-boundary.sh` | PASS |
| Pro lint | `cargo clippy --all-targets -- -D warnings` | exit 0 |
| Core lint (lib + bins) | `cargo clippy -p physis-core --lib --bins --all-features -- -D warnings` | exit 0 |
| Pro full suite | `PHYSIS_EMBEDDER=random-projection cargo test` | **1202 passed, 0 failed** (1 ignored) |
| Core full suite | `PHYSIS_EMBEDDER=random-projection cargo test -p physis-core` | **273 passed, 0 failed** (1 ignored) |

The env var is not a shortcut around the tests: `PHYSIS_EMBEDDER=random-projection`
is the documented offline mode `select_embedder` honours *before* the model
cascade, and it matches what CI runs — CI's `test` job downloads no model files,
so its cascade lands on random projection anyway. Without it this box takes the
full ONNX cascade, and the `src/main.rs` CLI tests then spend ~480 s **each**
re-embedding the ontology; the suite does not finish in half an hour. See item 6
below — that is a real trap, not merely a slow machine.


Manual, sandboxed: a truncated legacy log + one `observe` now leaves the reader
and the writer agreeing on the count, where before the reader was short by one.

## 6. Also in this session (fleet done-gate, not plan scope)

`cargo clippy --all-targets -- -D warnings` was RED on `physis-pro` (~30
warnings, all pre-existing, none in migration code: `dependency`,
`orchestration`, `staleness`, `semantic_delta`, `repair_regions`, `metrics`,
`registry`, `intent_ir`, `tests/adaptive_document`). Fixed with
`cargo clippy --fix` plus manual edits for the non-mechanical ones
(`DependencyGraph::default`, the `unwrap`-after-`is_none` in `intent_ir`, the
`clamp` in `semantic_delta`, and making `choose_worker` `pub` — the routing
entry point of a public module that nothing could reach). The diff is
behaviour-preserving; `choose_worker`/`compute_heaviness`/`CheapWorker`/
`StrongWorker` were kept, not deleted.

## 7. Remaining, explicitly not done

1. **Core research-example clippy is still RED** (~28 lints in
   `physis-core/examples/perspective_invention_experiment.rs` and
   `impossible_machine_experiment.rs`: `too_many_arguments`, `type_complexity`,
   `excessive_precision`, needless-range-loop, dead fields). Not touched: these
   are experiment apparatus, and `excessive_precision` fixes can change the
   numbers an experiment produces. `physis-core.yml`'s clippy step
   (`--all-targets --all-features`) therefore still fails. Fix as its own change
   with the experiments re-run, or scope that step to `--lib --bins`.
2. **`vault`/`history`/`praxis` remain two implementations each** (carried from
   Day 1). The plan's own rule — do not move data paths in this gate — still
   applies.
3. **`physis-pro-system-tui` is unverified beyond `--help`** (needs a real TTY).
4. **`act-recall`'s artifact path is CWD-relative**, as in Core.
5. **The stale standalone Core build tree** at `physis-core/target/` (1.3 GB)
   still exists; never cite it as evidence.
6. **The `cli_app` test helper does not actually force random projection.**
   `src/main.rs` sets `embedders.primary = RandomProjection` "for speed", but
   `select_embedder` runs every primary through `semantic_self_test`, which
   random projection fails *by design* — so the cascade walks past it to
   `bge-base` (768-dim ONNX) whenever that model is on disk, and each CLI test
   pays a full ontology embedding pass (~480 s here). Fixing it means teaching
   `select_embedder` that an explicitly-configured primary is a decision, not a
   candidate — or setting `PHYSIS_EMBEDDER=random-projection` in the test. Not
   changed here because it is a test/runtime-contract question, not a migration
   one, and changing embedder selection during a parity gate would confound the
   parity evidence.

## 8. Gate status

Day 2's acceptance is met as written: Core builds standalone (no defaults,
default bin, all features) with no Pro checkout; a legacy log keeps its IDs,
order and provenance; retries do not duplicate; an interrupted append no longer
skips a record; and a copied log answers identically after a restart. The
boundary is now a gate rather than a habit.

Next per the plan: **Day 3 — repair retrieval using an equal-budget
experiment.** Freeze query/evidence-span cases, report actual tokens, and
compare the strongest simple baseline at the same budget before any token claim.

**Actor**: continuation session (OpenCode)
**Date**: 2026-09-16
**Revisions**: Pro `96a98c4` (branch `main`), Core `d65858b` (branch `master`), both dirty
