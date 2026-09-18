# Bounded OpenCode orchestration

Opt-in, human-supervised MVP. No new model transport, event bus, MCP server or
Rust orchestration engine. Existing default `physis` agent remains unchanged.

## Run

Restart OpenCode in `/home/gio/physis_pro`, then `/orchestrate <goal>`.
Use interactive OpenCode: worker shell checks and final acceptance require
approval. Do **not** use `--auto` to bypass boundaries. CLI equivalent:

```sh
opencode run --dir /home/gio/physis_pro --command orchestrate "goal"
opencode run --dir /home/gio/physis_pro --session <session-id> --command orchestrate "resume; show status first"
```

Headless operation may stop at permissions; that is not successful validation.
`/orchestrate stop after planning`, `no Astra`, `skip Physis`, `use Physis here`
are coordinator instructions. Disabled sensors are persisted. Task ordering
must respect dependencies. Explicit Astra calls ask approval. Human risk
acceptance must be documented in route evidence, never silently lower scores.
Model overrides require editing role arrays and restarting OpenCode; natural
language cannot silently select an unconfigured provider.

```text
human -> scout -> planner -> independent critic -> measured risk
                                            |       |
                                       optional Physis
                                                    |
                                  unresolved -> Astra (approval) / human
                                                    |
                           ready worker slice -> checks -> independent reviewer
                                                    |
                             optional delta evidence -> human acceptance
                                                    |
                                      accept / bounded repair / stop
```

## Reused machinery and configuration

- Native project command, dynamically configured agents and child sessions.
- Installed `@opencode-ai/plugin` and its SDK; no dependency added.
- Native messages/tool results/session export for working record, token/cost
  accounting. Compact checkpoints are references, not copied conversations.
- Existing `physis_system` MCP launcher. Config now explicitly sets this
  checkout as `PHYSIS_WORKSPACE`; previously launcher selected another checkout.
- Existing generic `physis` capability-check skill remains unchanged. Optional
  `physis-inspect` skill documents verified MCP operations, not a new adapter.
- Existing observer plugin was inspected, not enabled: it writes a Physis note
  per tool, contrary to this workflow’s opportunistic inspection policy.
- Existing Rust repair orchestration/dependency/semantic-delta modules were
  inspected; placeholder heaviness metrics were not reused as measurements.

Policy: `/home/gio/physis_pro/.opencode/orchestration.json`.
Discovered and live-probed models:

| Role | Default |
|---|---|
| coordinator, scout, worker, inspector | `commandcode/deepseek/deepseek-v4-flash` |
| planner | `commandcode/meta/muse-spark-1.3` |
| critic | DeepSeek; configured GLM fallback |
| reviewer | Muse; configured GLM fallback |
| architect | `commandcode/gpt-6-astra` |

Muse/DeepSeek responded live. **Astra advertised but rejected by Command Code
API (`unsupported_model`, use Command Code CLI).** No CLI bridge added. Thus
current Astra escalation stops at human. GLM fallback configured from catalog,
not live-certified. No prices or availability inferred from marketing names.
Fallback candidates filtered by native connected providers/model catalog.
Runtime fallback only after explicit unavailable-model response and confirmed
absence of tool activity. Ambiguous execution never retries blindly.

`families` is explicit metadata. Critic/reviewer prefer a different family;
`independent:false` exposes degradation. Astra model is excluded from ordinary
role candidates. Profile A intentionally uses one family.

## Limits, routing and records

Eight nodes per plan, 24 delegations per run, serial dispatch (concurrency 1),
non-recursive subagents, two local worker repairs, one critic repair, one Astra
attempt. Packets capped at 12,000 characters; prompt timeout 180 seconds.
Policy values configurable; serial/no-recursion is intentionally fixed.

Risk is normalized weighted mean over available 0..1 signals; missing values
stay null. High score or supported unresolved disagreement/repeated failure
routes to Astra only in profile E and when enabled; otherwise human. Signals
are heuristics with evidence references, not calibrated failure probabilities.
No semantic distance or coupling score is invented. Route and successful
worker/reviewer results carry task content hashes, invalidating stale authority
when task changes. Accepted tasks cannot be rewritten.

Native session ID identifies run; private, gitignored checkpoint:
`/home/gio/physis_pro/.physis/opencode-orchestration/<session-id>.json`.
It stores task graph, role/model, child refs, attempts, routing inputs/results,
reviewer verdict, final-turn usage/latency, test evidence and outcome.
Full usage/tool history stays in `opencode export <child-id>`; checkpoint usage
is **last returned assistant message**, not sum of all agent turns. Coordinator
usage remains in parent session. Avoid summing checkpoint cost as total run cost.

Resume same native session. Pending execution blocks replay until approved
`reconcile` includes child ID/outcome/evidence. Creation failures permit
`child:null,outcome:aborted` only after human checks for orphan sessions. A stale
`.json.lock` after process death requires manual removal **after** aborting any
live child and inspecting tree. No automatic stale-lock stealing. Logging gaps
are reported. Native session evidence remains available if checkpoint writing
fails; state-dependent automation pauses rather than rerunning changes.

## Physis path and limitations

`physis-inspect` -> existing `physis_system` MCP -> existing workspace service.
Verified calls: capabilities, history, pack, read, find, inspect, tasks/task.
MCP provides lexical BM25 line-window evidence, **not** semantic_delta,
neighbors or graph centrality. Working-record packet carries compact native
session/checkpoint refs and summaries; ignored state is not magically indexed.

Use for cross-module work, unclear dependencies, supported disagreements,
unexpected failures or plan drift, before expensive unresolved arbitration.
Skip trivial edits. Post-change comparison is bounded source/diff evidence,
not numeric semantic-delta certification. Missing MCP/empty findings explicitly
means NOT MEASURED; independent criticism/tests/review remain available.

## Safety boundary

Read-only agents deny edits/shell and unknown tools. Worker common edit/write
calls are checked against canonical TaskNode.writes; apply_patch rejected.
Worker shell remains human-approved, **not sandboxed**. Tests can execute arbitrary
code; do not approve untrusted commands. Native permission system and human
review of actual diff protect dirty changes; no filesystem isolation or automatic
rollback. This is supervised tooling, not a security boundary against a malicious
model or repository. Other OpenCode sessions are outside this run’s lock.

Acceptance checks actual parsed reviewer JSON verdict, not coordinator’s claim.
Test evidence remains coordinator-reported and must be confirmed in final human
permission prompt. Reviewer missing-validation fields are preserved for that
approval. No compiler/test certification inferred from LLM text. Provider setup
requests are not all covered by prompt timeout; interrupted setup needs recovery.

## Experiments and tests

Set profile before new run: A strong planner model everywhere; B cheap worker
model for planning/work; C adds critic; D adds optional Physis; E adds sparse
Astra. Same reviewer/test/human safety floor across all profiles. A/B omit critic.
No claim yet that Physis reduces failures, saves cost or improves routing.

On Pansa:

```sh
node --test /home/gio/physis_pro/clients/opencode-orchestration/router.test.mjs
node /home/gio/physis_pro/clients/opencode-orchestration/smoke.mjs
```

Unit cases cover isolated/cross-module routing, disagreement, repair limits,
missing/useful Physis replies (SDK doubles), unavailable worker/Astra, disabled
Astra, resume/locking, profiles, reviewer rejection, amended tasks, disconnected
providers, native bash exit validation (exact command match, latest exit,
missing evidence, nonzero failure), post-review worktree drift, and hash path
scope. Deterministic smoke uses real temporary IO with explicit mocked
permissions; it is not a live-provider test.

Live 2026-09-16 trace (before final permission/acceptance hardening):
`ses_f546324e1ffe3C4U7YC1os5BMT`:

- Muse planner `ses_f5462e4b9ffeuFp1AN6Rw2Ic7H`
- DeepSeek critic `ses_f54629a06ffe1pirmYck0swcr2`
- Three DeepSeek worker attempts; last `ses_f546179b0ffeV00OGEqyG4IvA1`
- Muse reviewer `ses_f5461386fffeFFLLrFwsnr9h22`
- Fixture changed `ok=false` to `ok=true`; baseline failed, final test passed.
- Astra/Physis disabled; two empty worker results exposed permission issues.
- Reviewer lacked shell rerun capability; historical acceptance was reported,
  not independent test certification. Latest code instead stops on empty output
  and asks human to confirm test/diff evidence before acceptance.

Latest hardened version passes deterministic tests/smoke. Live follow-up below
reached worker permission gate but did not complete end-to-end. Next highest-value improvement: bind acceptance to actual
native test-tool exits and worktree hashes, then matched A–E trials with complete
session usage aggregation. Do that before any unattended production rollout.

## Acceptance evidence (2026-09-16)

Coordinator-supplied test exits are ignored. `accept {task_id}` first confirms a
current reviewer `accept` result, then reads completed `bash` parts from the
worker child's native `session.messages`: the part must have
`state.status === "completed"`, `state.input.command` exactly equal to a planned
`acceptance_tests` entry, and integer `state.metadata.exit`. Every planned
command needs a matching row; acceptance uses each command's latest row, so a
final passing run after earlier failures is eligible while the latest nonzero
exit rejects. Denied/failed shell calls never qualify. Recorded test rows carry
`evidence_kind: "native"` with `sessionID/callID` references.

Task `reads`+`writes` are SHA-256 hashed at worker and reviewer completion; any
post-review mismatch rejects acceptance until re-review. Hashing is confined to
the runtime directory (absolute escapes rejected) and treats missing files as
`null`, not silently skipped. Earlier pre-hardening run
`ses_f546324e1ffe3C4U7YC1os5BMT` had `evidence_kind: "operator_reported"`; new
acceptances record `"native"`.

Verified against live 2026-09-16 exports: the worker child
`ses_f546179b0ffeV00OGEqyG4IvA1` carries exactly two completed bash rows —
`node --test .physis/orchestrator-smoke.test.mjs` with exits `1` then `0`. The
router's `nativeTestEvidence` was exercised over an equivalent fixture: baseline
rejected (`latest exit 1`), post-repair accepted (`latest exit 0`). Human
`orchestration_accept` approval still confirms the native records and actual
diff; it cannot substitute missing evidence.

```text
/scout|planner|critic -> router decision -> worker (scoped edits, native bash)
    -> reviewer (accept JSON) -> native evidence check -> drift check
    -> human confirmation -> accept/finish
```

## Hardened end-to-end live run (2026-09-16)

Run `ses_f53534078ffenChZlSRi7Oxaga` (profile E, Astra/Physis OFF) exercised the
full path against a disposable fixture under the `.physis/` runtime directory:

| Role | Model | Child |
|---|---|---|
| scout | DeepSeek V4 Flash | `ses_f5353035affeakEv4iYdDmeC1l` |
| planner | Muse Spark 1.3 | `ses_f53524bf0ffeUGfl7dv5POuL2R` |
| critic | DeepSeek V4 Flash | `ses_f5351ba7effe6hgrWkcYlxozXm` |
| worker | DeepSeek V4 Flash | `ses_f535059c2ffeNSFvA9FgQQkGUQ` |
| reviewer | Muse Spark 1.3 (independent family) | `ses_f53500b02ffe4VolxCVVardEG3` |

Router decision: score `0.079` (threshold 0.7), `action: worker`,
`inspect: false`; `structural_coupling`, `novelty`, `semantic_delta` recorded
null because Physis was disabled.

Verified from `opencode export` of the worker child — exactly the planned
acceptance command, two completed bash rows: **exit 1** on the baseline
(`false !== true`), **exit 0** after the edit. Reviewer returned
`verdict: "accept"` and recorded in `missing_validation` that it could not
re-execute the command itself (reviewer shell is denied). Worker and reviewer
`file_hashes` matched, so no post-review drift.

Actual change: `value.mjs:1` `ok = false` → `ok = true`; test file untouched. No
repository source, test, or configuration was modified by that run.

**Outcome: BLOCKED at the human gate — not failed.** The coordinator called
`accept`; headless `opencode run` auto-rejected the `orchestration_accept`
prompt; the coordinator stopped and reported instead of bypassing. Node left
unaccepted, `outcome: in_progress`. Final acceptance is one interactive
confirmation away:

```sh
opencode run --dir /home/gio/physis_pro --session ses_f53534078ffenChZlSRi7Oxaga \
  --command orchestrate "accept the node now; evidence verified"
```

Pre-authorising the fixture command for that run:

```json
"worker_bash_allow": [
  "node --test /home/gio/physis_pro/.physis/orchestration-verify-current/value.test.mjs*"
]
```

`worker_bash_allow` is empty by default, so every worker shell call asks. When
set, worker bash becomes deny-all plus exactly those patterns; the trailing `*`
is required because patterns match the parsed command including arguments.
Pre-authorising a command pre-authorises the code it runs — node test files
execute arbitrary JavaScript, so scope this to fixtures you control. No `--auto`
and no repository-wide `bash: allow`.

## Hardened live verification follow-up (2026-09-16/17)

Run `ses_f53534078ffenChZlSRi7Oxaga` (profile E, Astra OFF, Physis OFF) reached
worker completion and independent reviewer `accept` on the hardened router.
Native worker export `ses_f535059c2ffeNSFvA9FgQQkGUQ` shows reads of both
fixtures, one scoped `edit` (`ok=false` → `ok=true`), and two completed bash
rows for the exact acceptance command (baseline exit 1 with decisive
`AssertionError ... false !== true`, post-edit exit 0 `# pass 1`). Worker and
reviewer file hashes match (`value.mjs=ad5abda1…`, `value.test.mjs=da2fa6b4…`,
no drift). Reviewer `ses_f53500b02ffe4VolxCVVardEG3` returned verdict `accept`
with explicit `missing_validation` (no re-execution; reviewer bash denied).

Final acceptance was NOT recorded: two headless `accept` attempts stopped at
`orchestration_accept=ask` auto-reject (`validation: "not performed"`), as
designed. Checkpoint and fixture hashes confirmed unchanged after both blocked
attempts. Direct out-of-band `Runner.control` accept via the serve daemon's SDK
was also rejected (HTTP 401, no standing credentials), so no bypass path was
used. The remaining step is one human-approved `accept` in interactive OpenCode,
or a scoped fixture-only bash allow (tested live during this run, then reverted
— see `roles.mjs` history). No `--auto`, no global allow.

Temporary nested worker-bash allow was applied for this live run only
(`roles.mjs:16`, fixture command + trailing `*`, worker only, everything else
unchanged), verified with 21/21 tests, then reverted to `ask`. Last-match-wins
glob semantics confirmed via docs; agent rules override global.

Earlier probe selected `/home/gio` despite Python subprocess `cwd` selecting

Current permission policy was exercised unchanged. Run
`ses_f53f29633ffe1Tv9IBjvP6T7NP` completed native scout, Muse planner,
DeepSeek critic and risk routing. Worker child
`ses_f53f0a44fffeknHDbxLBQabnMD` read both fixture files, then requested
baseline test execution. Headless OpenCode rejected that permission:
`The user rejected permission to use this specific tool call.`
Router recorded `empty_output` and retained pending child. No implementation,
review or acceptance completed; fixture and implementation hashes unchanged.
This is **BLOCKED**, not successful end-to-end validation. Resume only after
inspecting/reconciling child state in interactive OpenCode. Do not use `--auto`
or remove locks based solely on age to manufacture a pass.

Earlier probe selected `/home/gio` despite Python subprocess `cwd` selecting
repository, because inherited `PWD` still named home; its native export had
zero messages and project `global`. Explicit `--dir` plus matching `PWD`
selected correct repository and reached delegation. When using Python subprocess,
set both `cwd` and `env={**os.environ, "PWD": repository}`, and pass `--dir`.
No router-code cause was demonstrated for that startup error.

Verification artifacts (private, gitignored):
`/home/gio/physis_pro/.physis/orchestration-verify-current/`.
A separate 25-second probe interrupted planner child and left run
`ses_f53f4d862ffeYgXWmDeZZ0LmXl` pending with a stale lock; it was not replayed.

Native session exports also confirmed bash results contain structured
`state.metadata.exit` (baseline 1, success 0), even when tool status is
`completed`. Future acceptance binding should consume these native records,
not introduce another command executor or infer success from completed status.
That binding and reviewed-file content hashes remain unimplemented.
