# Delegation planning and agent connections

Core now serves ten MCP tools from the same `Workspace` methods as
`physis system`, using `.physis/system/observations.jsonl`. The MCP server needs
no Pro licence, model or additional runtime package.

## Connect Codex and OpenCode

```sh
cargo build --offline -p physis-core --no-default-features --features cli --bin physis-core
bash scripts/physis-system-mcp.sh
```

The second command waits for MCP messages; stdout contains only protocol JSON.
It selects this checkout's debug binary explicitly. Overrides:
`PHYSIS_CORE_BIN`, `PHYSIS_WORKSPACE`, `PHYSIS_SYSTEM_STATE`. Rebuild after
changing Core and reconnect existing clients to load the new binary.

Project settings: `.codex/config.toml` and `.opencode/opencode.json`. On this
machine Codex also has a user-level `physis_system` entry for sessions launched
from `/home/gio/dev`. Config paths are machine-specific; update them if moving
the checkout. The launcher resolves its checkout from its own location.

```sh
codex mcp get physis_system --json
opencode mcp list
```

Restart Codex sessions opened before the configuration change. Existing Pro
MCP in `.mcp.json` remains separate: these tools expose workspace files and Core
observations, not the Pro coherence graph.

Configuration references: [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli),
[OpenCode local MCP](https://opencode.ai/docs/mcp-servers/).

## Shared development loop

| Tool | Use |
|---|---|
| `physis_capabilities` | Inspect root, store and limits. |
| `physis_inspect` | Inventory files and recent work. |
| `physis_history` | Recall previous attempts before working. |
| `physis_find` | Find files and observations with lexical BM25. |
| `physis_pack` | Read relevant line windows within a Physis token budget. |
| `physis_read` | Follow a path, range, file ID or observation ID. |
| `physis_predict` | Consult process-failure history without executing argv. |
| `physis_remember` | Record outcome, actor and evidence references. |
| `physis_tasks` | List delegation tasks and reservation state. |
| `physis_task` | Read a task, context and proposed invocation. |

Only `physis_remember` writes through MCP. Outcomes remain operator reports.
Command execution and task/reservation writes remain explicit CLI operations.
OpenCode prefixes tool names, e.g. `physis_system_physis_history`.

Verified 2026-09-14: OpenCode 1.18.30 used the catalog-listed free model
`opencode/mimo-v2.5-free` through `serve` to call history → remember → history.
It wrote and retrieved `obs:20` with actor `opencode`. Codex's real app-server
MCP client discovered ten tools and read that record from the same log with no
model turn. Repeat that cross-client check with:

```sh
node scripts/physis-codex-smoke.mjs
```

The script expects the OpenCode smoke note tagged `PHYSIS_WIRING_20260914`.
It uses the installed experimental Codex app-server protocol, which can change.

## Delegation request and registry

The following is the implemented request format. Unknown fields and
caller-supplied IDs are rejected. The broader plan contains additional proposed
execution/report schemas; those are not this command's input format.

```json
{
  "schema": "physis.task.v1",
  "intent": "review workspace MCP argument validation",
  "paths": ["src/system_mcp.rs", "tests/system_mcp.rs"],
  "mode": "read",
  "acceptance": ["Cite findings by file and line; make no edits"],
  "checks": [],
  "worker": "opencode-free",
  "budget": 1200,
  "policy": {"costs": ["free_tier"], "allow_remote_model": true, "timeout_seconds": 300}
}
```

```json
{
  "schema": "physis.workers.v1",
  "workers": [{
    "id": "opencode-free", "kind": "opencode",
    "program": "/home/gio/.local/bin/opencode",
    "provider": "opencode", "model": "opencode/mimo-v2.5-free",
    "cost": "free_tier", "remote_model": true,
    "modes": ["read", "plan", "write"], "max_parallel": 2,
    "server": "http://127.0.0.1:4096"
  }]
}
```

Cost/locality are operator declarations, not live probes. Remote model use
defaults to false; there is no silent paid fallback. For Cline use `kind:
"cline"`, its absolute program path, explicit provider/model, and no `server`.
The live Cline review pilot failed with `session not found`; a generated Cline
command does not establish a successful runtime integration.

Save these examples outside the source scope. For Core tasks use its Git root:

Ready-to-use copies are in `clients/delegation/review-system-mcp.json` and
`clients/delegation/workers.example.json`; their executable/model settings match
the inspected installation and can be edited for another machine.

```sh
./target/debug/physis system --root physis-core delegation plan --task /tmp/task.json --registry /tmp/workers.json
./target/debug/physis system --root physis-core --json delegation create --task /tmp/task.json --registry /tmp/workers.json
./target/debug/physis system --root physis-core delegation list
./target/debug/physis system --root physis-core --json delegation show task-ID
./target/debug/physis system --root physis-core delegation reserve task-ID --seconds 900
./target/debug/physis system --root physis-core delegation release task-ID --token TOKEN
./target/debug/physis system --root physis-core delegation export task-ID /tmp/new-task-folder
```

`plan` creates no state and runs no worker. It reads Git state, fingerprints
visible scoped files, recalls matching prior work, and packs only allowed paths
using Core's BM25 window packer. Identical unchanged inputs yield the same
preview ID. `create` assigns a fresh task/correlation ID and appends its plan.
Repeated requests receive different IDs and name earlier tasks in `duplicates`.

Reservations name future worktrees without creating them. A cooperative lock
serializes read/check/append across broker processes. The full log reconstructs
slots, so history-tail limits cannot forget an older lease. Corrupt log lines
fail closed. A stale lock after process death requires inspecting its owner;
automatic stale-lock recovery is not implemented.

Reservation checks reject changed source state, occupied task/worktree slots
and exhausted worker capacity. Only the current token releases a reservation.
TTL expiry frees **unstarted** slots. This must be extended before workers run
under those leases. Scope bounds context reads, not worker filesystem/network
access; a worktree is not an OS sandbox.

The report prints literal argv for Cline or OpenCode. The next phase must create
the worktree, carry relevant uncommitted changes, write `.physis-task.json`,
enforce runtime limits and record independent checks. Automatic worker dispatch,
worktree creation and acceptance/merge are not implemented by these commands.

## Verification

```sh
cargo test --offline -p physis-core --no-default-features --features cli --test system_delegation --test system_mcp --test system_interface
cargo clippy --offline -p physis-core --no-default-features --features cli --lib --bin physis-core --test system_delegation --test system_mcp --test system_interface -- -D warnings
```
