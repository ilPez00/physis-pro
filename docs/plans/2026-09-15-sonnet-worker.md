# Sonnet 5 worker: complete the first Core/Pro milestone

Assigned 2026-09-15 by Codex at the user's request. Required model:
`claude-sonnet-5`; do not switch models or spawn other workers.

## Read first

- `AGENTS.md` and any applicable instructions in the files you touch.
- `docs/WHAT_PHYSIS_IS.md`.
- `docs/plans/2026-09-15-core-pro-development.md`: the complete approved
  development direction, including research gates and known experiment results.
- `physis-core/docs/CORE_AUSTERITY_AUDIT.md`.

The user wants all application features in Pro and Core to remain the CLI
brain. There are two product repositories/layers, not a new third Product repo.

## Implement now: Day 1

Finish the in-progress relocation into `src/ported` and wire product entrypoints
in Pro. Inspect first: the working tree already contains substantial staged,
unstaged and untracked changes in both Pro and the Core submodule. Preserve
those changes and source provenance. Do not reset, clean or discard work.

1. Inventory relocated modules, callers, assets, feature requirements and tests.
2. Wire Pro CLI routes to the moved workspace service, MCP adapter, notebook,
   action execution and Studio, reusing existing Pro routes where appropriate.
3. Fix feature gating and imports so optional web/TUI/MCP dependencies stay in
   Pro and Core has no product dependency.
4. Update moved integration tests: they currently import Pro modules while
   invoking `CARGO_BIN_EXE_physis-core`. Test the new Pro executable paths.
5. Complete removal of product dispatch from Core where needed for this gate.
   Do not add Pro-aware dispatch back into Core. Preserve mechanism commands.
6. Preserve observation schemas, IDs, error behavior and existing data paths.
   Do not merge or migrate the user's runtime stores during this task.
7. Verify with fresh builds and focused integration tests. An old target binary
   is not evidence that the changed source compiles or routes correctly.

## Boundaries

Work in `/home/gio/dev/physis-pro`, including its Core submodule. This host
reports hostname `pansa`; follow the repository's development-host requirements
and report an actual execution constraint rather than modifying host setup.
Primary scope: `Cargo.toml`/lockfile, `src/main.rs`, `src/cli.rs`, `src/lib.rs`,
`src/ported`, necessary Pro binary adapters, moved integration tests, and Core
manifest/CLI/export changes required to finish the existing move.

Do not implement Days 2–10 yet; hand the first milestone back for review.
No model training, new product features, new connectors, deployment, publishing,
repository archival, commits or pushes. Do not read credential files or send
messages through external integrations. Use the current Claude model only.
Do not bypass permission controls. If a required command is denied, record the
exact command and blocker in the handoff. Avoid unnecessary full test reruns.

## Acceptance

- Pro CLI and MCP exercise the same moved workspace service with write/read
  round trips, valid JSON/protocol results and correct error behavior.
- Notebook, action and Studio adapters have actual dispatch paths; explicitly
  list any unimplemented route instead of calling the move complete.
- Core builds independently and exposes no restored application surface.
- Appropriate focused suites pass against freshly built targets, including
  moved workspace, MCP and delegation tests where their features are enabled.
- Existing uncommitted changes remain intact; your own changes are identified.

## Handoff

Write `docs/plans/2026-09-15-sonnet-day1-handoff.md` with:

1. Source revisions and initial dirty-tree state.
2. Changed paths and reasons, distinguishing inherited work from your edits.
3. Commands executed, feature combinations, pass/fail counts and evidence.
4. Remaining defects or unmeasured runtime paths.
5. Whether the Day 1 acceptance gate passed and the exact next step.

Record a concise development outcome through the available workspace history
service if possible; do not use classification-quality penalties for build
results. Return a short summary of the handoff when finished. Report honestly
if a permission or build environment prevents completion.
