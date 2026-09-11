# Handoff pointers — 2026-09-10 night session

Two files outside this repo record the live-agent handoff and the fleet board
snapshot. Read them before touching this repo's `packets/` or `physis-core/`:

- `/home/gio/HANDOFF-LIVE-AGENT-2026-09-10.md`
  — a live agent is actively editing `physis-core`; a quarantine stash holds
  earlier WIP. Do NOT pop/reset/stash or run cargo clippy/test here while that
  agent is working.
- `/home/gio/BOARD-SNAPSHOT-2026-09-10.md`
  — state of all tracked packs + what is stale in `~/plans-staging` (repo
  `packets/` is the board of record, not `plans-staging`).

Rule for new files here: add a new dated doc; never rewrite existing plan files
without a human.