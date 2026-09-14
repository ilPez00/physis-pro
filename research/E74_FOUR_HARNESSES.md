# E74 — four harnesses, three machines, and when a borrowed prior helps

Run 2026-09-14, extending E73 from one agent to four. Extractors were run **on**
each machine and only derived action records fetched back; no transcript, source
file or command output left the machine it was written on.

## The corpora

| harness | where it lives | sessions | actions | label |
|---|---|---:|---:|---|
| Claude Code | `~/.claude/projects/*.jsonl` | 42 | 14457 | harness `is_error` |
| Codex | `~/.codex/sessions/**/rollout-*.jsonl` | 3 | 662 | real `exit_code` |
| Cline | `~/.cline/data/sessions/*/*.messages.json` | 69 | 12354 | `is_error` (rare) + markers |
| OpenCode | `~/.local/share/opencode/opencode.db` (SQLite `part`) | 288 | 48460 | `state.status` |
| **merged, deduplicated** | pansa · omo · feather | **402** | **75933** | |

188 duplicate session records were dropped: transcripts sync between machines,
and a byte-identical session present on three of them would otherwise be counted
three times.

Two extraction defects worth recording, both found by looking at the rates
rather than at the code:

* **Cline's shell results are batches.** Reading only `text` keys returned
  nothing for shell actions, so the failure rate came out at 0.005 over 8301
  actions — which reads as a very reliable agent rather than as a parser that
  found no output. Reading `result`/`output`/`error` too puts it at 0.070, in
  line with every other harness. Its commands also arrive as a list under
  `run_commands`, so 6602 actions were one kind until the first command was
  parsed out; now 112 kinds.
* **The merge labelled harness from the wrong field.** Every Cline and OpenCode
  session was tagged `claude-code` because the label was derived from a
  `project` field rather than from the corpus stamp. The transfer table below
  answered a smaller question than the data supported until that was fixed.

## E71, on a stratified sample of all four

Leave-one-session-out refits per held-out session, so the full corpus is hours;
this is ≤ 20 sessions per harness (63 sessions), which the harness prints.

```
deform vs base-rate    Brier -0.00613 [-0.01000, -0.00285]  beats
deform vs kind-prior   Brier -0.00309 [-0.00587, -0.00059]  beats
deform vs frozen       Brier -0.00172 [-0.00355, -0.00018]  beats
deform vs deny         Brier -0.00474 [-0.00817, -0.00193]  beats
deform vs shuffled     Brier -0.00052 [-0.00142, +0.00040]  TIES
deform vs frozen       AUC   -0.0613 [-0.0872, -0.0376]     LOSES
```

Unchanged across four agents: **deny loses** (fourth independent replication),
**shuffled ties** (fifth time order carries nothing), and online deformation
still **loses at ranking** to a model fitted on other sessions even where it now
wins slightly on Brier.

## Transfer, and the rule it produces

Per-session AUC with a bootstrap over target sessions, `markers` label:

| source → target | transferred | target's own | coverage |
|---|---|---:|---:|
| opencode → cline | **0.648 [0.583, 0.707]** | 0.786 | 0.743 |
| opencode → codex | **0.654 [0.631, 0.678]** | 0.494 | 0.500 |
| cline → codex | **0.640 [0.639, 0.642]** | 0.494 | 0.464 |
| claude-code → codex | **0.635 [0.606, 0.664]** | 0.494 | 0.470 |
| cline → opencode | 0.576 [0.554, 0.598] | 0.695 | 0.375 |
| claude-code → opencode | 0.530 [0.507, 0.552] | 0.695 | 0.398 |
| claude-code → cline | 0.548 [0.480, 0.614] | 0.786 | 0.645 |
| opencode → claude-code | 0.510 [0.464, 0.557] | 0.631 | 0.402 |
| cline → claude-code | 0.519 [0.476, 0.562] | 0.631 | 0.392 |
| codex → anything | 0.524–0.546 | — | 0.17–0.35 |

Same-harness across machines (E73) remains the strongest: feather → pansa 0.644
against an in-corpus 0.642, pansa → feather 0.677 against 0.612.

**The rule, stated so it can fail:**

> A borrowed prior beats your own when your own record is thin, and loses to it
> when your record is thick. Coverage sets the ceiling either way.

Codex has three sessions and an in-corpus prior of 0.494 — chance. Every other
harness's prior beats that on Codex's own failures (0.635–0.654). In the other
direction, no borrowed prior beats a target whose own corpus is substantial:
cline's own 0.786 and opencode's own 0.695 are never matched by an import. And
every pair above 0.6 has ≥ 0.46 coverage while every pair at ~0.51 sits at
0.17–0.40.

That is exactly the back-off `physis system predict --prior-from` implements:
the workspace's own record first, a borrowed one only for a kind the workspace
has never run — and only above a coverage gate, or on a cold start where the
local log is empty. The gate was set at 0.90 from E73's machine pairs; these
harness pairs transfer usefully from 0.46 up, so the gate is conservative rather
than tight, and that is a choice, not a measurement.

## What is in the repository, and what is not

`benchmarks/behaviour/corpus-redacted.json.gz` (796 KB) carries all 402 sessions
and 75933 actions with every target replaced by a salted digest, so
`target_failed` still means "this exact target failed before" and no recorded
command is readable. The raw extracts stay on the machines that produced them
and are gitignored: a scan of the merged corpus found an API key and a bearer
token inside recorded commands, which is a good argument against ever committing
agent transcripts, redacted or not, without looking first.

## Limits

One person's fleet, three reachable machines, a fortnight. The four labels are
not the same measurement: Codex's is a real exit code, OpenCode's and Claude
Code's are harness judgements, Cline's is mostly output markers. Cross-harness
comparisons therefore mix a little label drift into every number, which is why
the rule above is stated in terms of thin-versus-thick rather than as a constant.
