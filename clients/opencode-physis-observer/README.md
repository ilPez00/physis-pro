# physis-observer — one log for a swarm

OpenCode can run several agents against the same working tree. With
[opencode-orchestrator](https://github.com/agnusdei1207/opencode-orchestrator)
that is four roles (Commander, Planner, Worker, Reviewer) with their own
concurrency limits and their own mission ledger. Each agent keeps its own
transcript, so "has anyone already tried this, and what happened?" is
answerable only by reading transcripts — the most expensive way to ask, and
impossible across harnesses.

This plugin appends every tool call and every finished turn to the workspace's
Physis observation log through the supported write path, so the question becomes
one command:

```sh
physis system history 'cargo test'      # what has anyone run here, newest first
physis system find 'calibration'        # files and recorded work, ranked together
physis system pack 'pump calibration' --budget 1200   # the context, budgeted
```

## Install

```sh
ln -s ~/dev/physis-pro/clients/opencode-physis-observer \
      ~/.config/opencode/plugins/physis-observer
node clients/opencode-physis-observer/smoke.mjs      # 3/3 observations recorded
```

`physis` must be on PATH (or set `PHYSIS_BIN`). `PHYSIS_OBSERVE_TOOLS` limits
recording to an allowlist of tools; `PHYSIS_OBSERVE_OFF=1` disables it without
uninstalling.

## What it records, and what it does not

One observation per `tool.execute.after` (tool, session, duration, target, a
240-character summary) and one per user turn (`chat.message`: agent, session,
the request). Outcomes are recorded as `unverified`: a tool's exit condition is
not a claim that the agent's goal was met, and nothing here judges that.

It never blocks or modifies a tool call, never reads agent reasoning, and never
throws into the agent loop — a failed write is counted and reported on dispose
(`3/3 observations recorded`), because a plugin that records nothing and one
that is not installed look identical from inside OpenCode.

## Cost

Each record is one `physis system remember` process (a cooperative lock, one
appended line). Recall is what it buys: 20 records of swarm history cost 1051
tokens to read, against 2620 before the display stopped reprinting each note's
payload, and against the transcripts it replaces.

## Not measured yet

Whether this actually prevents duplicated work in a running swarm. The
mechanism is in place and the log is real; the claim that a Commander consulting
`history` before dispatch reduces repeated attempts has no measurement behind it
and is not made here.
