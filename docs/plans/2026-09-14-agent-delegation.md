# Agent delegation through Physis

Date: 2026-09-14. Status: implementation plan.

Implementation update: D0/D1 now provide strict task/worker inputs, scoped
context packs, recorded plans, stale-source checks, unstarted reservations,
list/show and folder exports. They do not launch workers or create worktrees.
Core MCP is connected to Codex and OpenCode; a real OpenCode note was retrieved
through Codex's MCP client. See [the implemented contract and checks](../SYSTEM_DELEGATION.md)
for exact JSON input and commands; later execution schemas below remain proposals.

## Decision in one paragraph

Physis should be the shared work ledger and context compiler for a pool of
coding agents. Cline and OpenCode are worker adapters, not peers that each keep
their own truth about the repository. A human or supervising agent creates a
typed task; Physis checks scope, packs the relevant context, leases an isolated
workspace, dispatches to a selected worker, records the worker's events and
artifacts, and requires an independent check before a result can be accepted.
OpenCode's headless `serve` is the preferred long-lived adapter. Cline's CLI is
the second adapter and is useful when its worktree isolation, ACP, or provider
configuration is the better fit. “Free agents” means locally runnable agents
or a provider/free-tier configuration available to the operator; it must never
be interpreted as unlimited capacity, free model usage, or permission to send
repository contents to an unapproved service.

This plan deliberately separates three things that are easy to conflate:

1. **Delegation** — starting a worker with a bounded task.
2. **Observation** — recording what the worker actually did.
3. **Acceptance** — independently deciding whether the requested outcome is
   supported by evidence and checks.

The existing OpenCode subprocess tool and observer plugin cover parts of the
first two. This plan makes their contracts explicit and adds the missing broker
and acceptance layer without replacing the working `system` interface or MCP
server.

## 1. Existing surfaces to preserve

| Surface | Current role | Keep / change |
|---|---|---|
| `physis system` | Human/agent CLI for capabilities, inspection, packing, history, memory, and recorded runs | Keep as the lowest-level local interface; add delegation records, not a second log. |
| `physis-pro serve --stdio` | MCP server for OpenCode, Cline, and other MCP clients | Keep as the standard tool transport. Add broker tools only after the local contract is tested. |
| `src/web/agent.rs` `opencode` tool | Runs `opencode run` in a scan-root-confined subprocess with cancellation | Keep for compatibility; route new dispatch through the broker so direct use is visibly legacy/direct mode. |
| `AgentRuntimeConfig` | OpenCode binary/model, round and token limits, harness gate | Extend with worker policies and allowlists; retain safe defaults. |
| `clients/opencode-physis-observer` | Records OpenCode tool calls and turns as `unverified` observations | Keep as a passive observer. Add task/run IDs to records; do not make it judge success. |
| `.mcp.json` | Starts the local Physis MCP server | Keep. Both workers should point at the same workspace root and data directory. |
| `.opencode/opencode.json` | OpenCode model and primary agent configuration | Add only documented broker/observer settings; do not encode task state here. |
| Cline CLI | `cline [options] [prompt]`, `--cwd`, `--json`, `--plan`, `--worktree`, ACP, and MCP management | Use an adapter process; do not assume undocumented daemon semantics. |
| OpenCode CLI | `opencode run`, `opencode serve`, ACP, MCP, and HTTP attachment | Use `serve` for a reusable worker pool when its API is pinned; retain `run` as a fallback. |

Before implementation, run `physis system inspect` on each surface and record
the revision plus working-tree state. The plan must not silently treat stale
documentation or a binary from another checkout as the implementation.

## 2. The delegation contract

Introduce a versioned envelope. JSON is the wire representation; the CLI may
render a compact human form. Every envelope carries a correlation ID so that a
single user request, worker session, process, commit, check, and outcome can be
joined without parsing transcripts.

### Task request

```json
{
  "schema": "physis.task.v1",
  "task_id": "task-…",
  "intent": "add a regression test for the ranged system read",
  "acceptance": [
    "the focused test passes",
    "cargo clippy for the touched target passes with -D warnings",
    "no unrelated files are changed"
  ],
  "scope": {
    "root": "/workspace/physis-pro",
    "paths": ["physis-core/src/system.rs", "physis-core/tests/system_interface.rs"],
    "mode": "write"
  },
  "context": {
    "query": "system interface ranged read tests",
    "budget": 2400,
    "references": ["obs:…", "goal:…"]
  },
  "worker": { "kind": "auto", "model": null },
  "limits": {
    "timeout_seconds": 900,
    "max_rounds": 12,
    "network": "deny",
    "secrets": "deny"
  },
  "parent": null
}
```

Invariants:

- `task_id` and `correlation_id` are generated by Physis, never by a worker.
- `scope.paths` is an allowlist when present. A missing allowlist means the
  task is still confined to a dedicated worktree, not the shared checkout.
- `mode` is one of `read`, `plan`, `write`, or `execute`; `execute` is reserved
  for an explicitly configured machine adapter and is not enabled by default.
- Acceptance criteria are data, not prose that a worker may rewrite.
- The context pack contains paths, line ranges, record IDs, and freshness. It
  is a bounded pointer bundle, not an unreviewed prompt dump.

### Worker report

```json
{
  "schema": "physis.worker-report.v1",
  "task_id": "task-…",
  "worker": { "kind": "opencode", "session": "…", "version": "…" },
  "workspace": { "kind": "worktree", "path": "…", "base": "…" },
  "status": "completed",
  "summary": "…",
  "changed_paths": ["…"],
  "commands": [{ "argv": ["cargo", "test", "…"], "exit": 0 }],
  "artifacts": [{ "path": "…", "sha256": "…", "kind": "patch" }],
  "observations": ["obs:…"],
  "claims": [{ "text": "…", "evidence": ["obs:…"], "verdict": "unverified" }],
  "failure": null
}
```

`completed` means the worker stopped and emitted a report. It does not mean the
task passed. Acceptance creates a separate `accepted`, `rejected`, or
`needs-review` outcome after checks and a reviewer decision.

## 3. Broker architecture

```text
human / supervising agent
          |
     task request
          v
  Physis delegation broker
   | scope + policy + pack
   | lease + workspace
   | dispatch + heartbeat
   | observe + reconcile
   v
 Cline adapter       OpenCode adapter
 (CLI/ACP/MCP)       (serve/HTTP, run fallback)
          |
     isolated workspace
          v
  independent checks + reviewer
          |
  accepted patch / rejected report
          v
 system observations + causal ledger + artifacts
```

The broker owns task state, leases, cancellation, and final status. Adapters
only translate a task into a worker-specific invocation and normalize events
back into the report schema. The repository and `.physis` store remain the
authoritative state; worker transcripts are evidence references and debugging
material, not the system of record.

### Broker states

`created → packed → leased → dispatched → running → reported → checking →`
`accepted | rejected | needs-review | cancelled | expired`.

Every transition appends an observation. A crash or lost worker leaves the task
in `running` until a heartbeat timeout turns it into `expired`; it must not be
silently retried as a new task. A retry creates a child task with `parent` set
to the expired task and a reason recorded.

### Concurrency rules

- One write lease per worktree. Read-only tasks may share a checkout only when
  their requested paths are disjoint and the policy explicitly permits it.
- Never dispatch two writers against the same shared checkout.
- A worker may not widen its path allowlist, network policy, or secret policy.
- The broker records the base revision and dirty paths before dispatch and
  compares them after completion. Pre-existing user changes are preserved and
  cannot be included in an acceptance patch by accident.
- Merge is a human/reviewer action in the first release. Automatic merge is a
  later experiment with an explicit rollback and conflict policy.

## 4. Cline adapter

Cline's currently installed CLI exposes the following useful contract:

```text
cline --cwd <worktree> --json --auto-approve false --timeout <seconds> <prompt>
cline --cwd <worktree> --plan <prompt>
cline --cwd <worktree> --worktree <prompt>
cline --acp …
cline mcp add …
```

The adapter should:

1. Create or obtain the leased worktree before starting Cline. Prefer the
   explicit broker worktree over Cline's implicit path so Physis can name and
   clean it up deterministically.
2. Write the task envelope and context pack to a broker-owned, ignored file
   inside the worktree (for example `.physis/task.json`). The prompt points to
   that file and states the acceptance criteria verbatim.
3. Run plan mode for `plan` tasks and JSON/controlled mode for `write` tasks.
   Do not turn on `--auto-approve` for a write task unless the policy explicitly
   says so and the worktree is disposable.
4. Pass the same `.mcp.json`/Physis MCP server configuration to Cline. The
   adapter must verify that `PHYSIS_SCAN_ROOT` is the leased worktree and that
   `PHYSIS_DATA_DIR` points at the shared project store or the approved
   per-task overlay.
5. Capture stdout/stderr, exit code, session ID if emitted, changed paths, and
   the final JSON message. Record each as an observation before producing the
   normalized worker report.
6. Run the acceptance checks outside Cline. A worker cannot certify its own
   result merely by printing “done”.

Cline MCP registration is a setup operation, not a task operation. The first
adapter may generate a checked-in snippet for `cline mcp add`; it must not
mutate a user's global `~/.cline` configuration without explicit opt-in.

## 5. OpenCode `serve` adapter

OpenCode is the preferred long-lived worker because `opencode serve` provides a
headless server and `opencode attach`/HTTP can reuse a process. The adapter must
pin the OpenCode version and discover the actual server API at startup; it must
not infer endpoint names from a moving CLI help page.

Startup contract:

```sh
opencode serve --hostname 127.0.0.1 --port <allocated-port> --print-logs
```

The broker then performs a health/version handshake, registers the leased
workspace and task envelope, and submits one task. If the server API is not
available or the handshake fails, fall back to the already implemented
`opencode run --model … -- <task>` subprocess path. The fallback remains
scan-root confined and cancellable.

The adapter must associate every request with `task_id`, `session_id`, and
`workspace_id`; a shared server must never let one task's session receive
another task's prompt or tool output. On cancellation, stop accepting new work,
cancel the session if supported, then terminate the process only after a grace
period. Record whether the worker acknowledged cancellation or was killed.

The existing OpenCode observer should receive the task and correlation IDs in
its subject lines. It continues to record `unverified` tool outcomes and never
blocks the worker. A broker-side observer records stronger lifecycle events
(lease, dispatch, report, check) so a missing plugin cannot make delegation
look successful.

## 6. Free/local worker policy

Create a worker registry with explicit policy fields:

```text
kind: cline | opencode
transport: cli | acp | serve
provider: local | configured-provider-id
cost_class: local | free-tier | paid
network: deny | allowlist
model: optional display name
max_parallel: integer
capabilities: read,write,test,execute
```

Selection order for `worker.kind = auto`:

1. Local OpenCode `serve` when healthy and policy-compatible.
2. Local Cline CLI when installed and policy-compatible.
3. A configured provider only if the task explicitly permits its
   `cost_class` and data egress.
4. Otherwise return `no eligible worker`; never silently substitute a paid or
   remote model.

“Free” is therefore an auditable label, not a promise. The report includes the
resolved provider, model, network mode, and an operator-supplied cost class.

## 7. Physis-native context and review loop

Before dispatch, the broker performs this deterministic sequence:

1. `system history` / MCP recall for prior attempts on the same intent and
   paths. Duplicate or recently failed tasks are shown to the supervisor.
2. `system pack <query> --budget N` over the allowlisted source tree. Every
   chunk retains path and line range.
3. `system inspect` of the target symbols and current dirty state.
4. `system remember` for the task intent, scope, selected worker, and parent
   observations.
5. Lease creation and worker dispatch.

After dispatch:

1. Observer events are appended as they arrive; payloads are summarized and
   bounded, with full artifacts kept by path/hash.
2. The broker records commands and exit statuses, but treats them as
   observations, not proof.
3. Independent checks run in the leased worktree: focused tests, formatter or
   lint gates, capability/call checks where applicable, and a changed-path
   audit.
4. A reviewer (human or separately selected worker) receives only the task
   envelope, patch/artifacts, checks, and evidence references. It does not
   inherit the worker's unverified “done” claim as truth.
5. The broker writes `accepted`, `rejected`, or `needs-review` to the shared
   history and exports a compact report for the supervisor.

Use Physis to develop Physis: the first broker tasks should be bounded changes
to `physis-core`'s system interface, and the broker itself should be exercised
through `system run`, with its actual check outcomes remembered in the same
workspace. This creates dogfood evidence instead of a plan that has never
survived its own workflow.

## 8. Implementation phases and gates

### Phase D0 — Inventory and contract (one focused change)

- Add the envelope schemas and state-transition table as Rust/JSON fixtures.
- Add a worker registry parser with safe defaults (`network=deny`, no execute,
  no automatic merge).
- Add `system delegation list/show` read-only views backed by the existing
  observation store or a clearly linked delegation ledger.
- Gate: schema round-trip tests; path confinement tests; no worker process is
  started by a read-only command.

### Phase D1 — Dry-run broker

- Implement task creation, context packing, duplicate detection, leases, and
  deterministic worktree allocation.
- Add `system delegation plan --worker cline|opencode` that emits the exact
  command/request without launching it.
- Gate: two simultaneous write requests cannot obtain the same lease; a task
  with an out-of-root path is rejected; pre-existing dirty files are listed.

### Phase D2 — Cline CLI adapter

- Launch Cline in a disposable leased worktree with the envelope file and MCP
  configuration.
- Normalize JSON/exit output and record observations.
- Gate: a no-op read task completes; a bounded test-writing task produces a
  patch; a failing task is `rejected` or `needs-review`, never `accepted`.

### Phase D3 — OpenCode serve adapter

- Start and health-check a loopback `opencode serve`; pin the observed API
  handshake in an integration fixture.
- Submit one isolated task; implement cancellation, timeout, and fallback to
  the existing `opencode run` path.
- Gate: two sessions cannot cross-talk; cancellation leaves no live child;
  server unavailability selects the documented fallback.

### Phase D4 — Independent acceptance and review

- Run checks outside the worker and attach evidence to the report.
- Add a reviewer adapter with read-only access to the leased patch and task
  criteria.
- Gate: a worker can claim success while the broker still rejects it when a
  check fails or changed paths exceed scope.

### Phase D5 — Observer and fleet memory

- Add task/session/correlation fields to the OpenCode observer.
- Add a Cline hook/adapter observer using the same `system remember` path.
- Add duplicate-work and collision dashboards from history, without claiming
  productivity gains until measured.
- Gate: smoke tests show records from both workers can be queried together;
  failed recording is visible and never blocks a worker.

### Phase D6 — Controlled merge and machine handoff

- Only after D0–D5 are stable, add a human-approved patch/merge command.
- Keep machine `execute` separate: require a typed adapter, current-state
  recheck, explicit approval, and reconciliation of the observed outcome.
- Gate: no task can reach machine execution or automatic merge by merely naming
  those words in its prompt.

## 9. First delegation pilots

Use small tasks that can fail cleanly and exercise the interface itself:

1. **Read-only orientation:** locate the `system pack` implementation, return
   file/line evidence, and make no edits.
2. **Test addition:** add one regression test to `system_interface.rs`, limited
   to named paths, then run the focused test and Clippy.
3. **Documentation correction:** reconcile the number of MCP tools in one doc
   against a live `tools/list` result; require a cited diff.
4. **Cross-worker review:** Cline writes the test in one worktree; OpenCode
   reviews the patch in a second worktree; a human decides whether to apply it.
5. **Failure recovery:** intentionally give a worker an invalid acceptance
   criterion and verify the broker records `needs-review` without retrying
   indefinitely.

Do not start with physical machine commands, broad refactors, automatic merges,
or tasks whose success cannot be checked without trusting the worker.

## 10. Measurement, not anecdotes

For each pilot record:

- eligible workers and the selected worker;
- context tokens, startup latency, task latency, and check latency;
- task success by independent acceptance, not worker exit code;
- scope violations, collisions, retries, cancellations, and expired leases;
- duplicate-work rate before and after history consultation;
- human review time and rejected/accepted patch rate;
- local/free/paid class and network egress;
- whether Cline/OpenCode serve actually reduced cost or latency relative to the
  existing direct subprocess path.

Pre-register a construction-matched baseline: direct `opencode run` without
the broker, and Cline without Physis context. A positive result is only useful
if both workers receive comparable tasks and the same acceptance gates. Do not
claim that delegation improves agents until the baseline and broker arms are
run on the same task set.

## 11. Security and failure policy

- Default to loopback transports, scan-root confinement, no secrets, and no
  network. An allowlist is required for any exception.
- Treat prompts, repository files, worker output, and tool summaries as
  untrusted input. They cannot change broker policy or acceptance criteria.
- Keep task and observation payloads bounded; store large output as artifacts
  with hashes and explicit retention rules.
- Never expose one tenant's task, session, path, or observations to another.
- Make cancellation idempotent. Reap child processes and close server sessions
  on timeout.
- Preserve `unknown`, `unverified`, and `needs-review` states. Do not convert a
  missing observation into success, and do not let a model-generated summary
  overwrite raw command evidence.
- Before claiming the broker works, run the physis mechanical checks: call-site
  checks for each dispatch path, capability write/read checks, focused tests,
  and a real smoke task through each configured adapter.

## 12. Immediate next action

Implement D0 and D1 in the existing `physis system` surface first. Do not add a
new daemon yet. The first useful artifact is a dry-run delegation report that
answers, for one task: what prior work was found, what context was packed, what
worker would be selected, which paths would be leased, what exact command or
serve request would be sent, and what checks would decide acceptance. Once that
report is reproducible, launch one Cline task and one OpenCode task and compare
their independently checked reports.
