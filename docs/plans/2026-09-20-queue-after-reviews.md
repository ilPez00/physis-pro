# Queue after the 2026-09-20 review pass

State of the work board after an independent review session on pansa
(main `e466d4e`, submodule `physis-core` `ed8dc29`), and what may run next.
This is a status-and-queue document, not a capability list: every claim below
was produced by running the thing, and the commands are in the named reports.

## 1. What the review changed

| packet | before | after | evidence |
|---|---|---|---|
| PH-015 content-search "flake" | `todo`, blamed on concurrency | `review` — **root-caused as a deterministic ingest bug**, fix + test | `packets/reports/PH-015.md`; commit `5ffa183` |
| PH-001 three drift fixes | `claimed` for 9 days, work committed, no report | `review` — report written, gate run in full | `packets/reports/PH-001.md`; commit `fca6a82` |
| MS-001 archaeology | `todo`, awaiting review | **`done`** — accepted with amendment A1 | `packets/reports/MS-001.md` §Verification note; commit `241e602` |
| MS-002, MS-003 | `blocked` on MS-001 | **`todo`, released** to run in parallel | packet headers carry A1 |
| PH-021 structural machines | `review` | **`done`** — gate holds in full | `packets/reports/PH-021.md`; commit `74dbe05` |
| PH-022 `number_clash` | `review` | **`done`** — gate holds in full | `packets/reports/PH-022.md`; commit `74dbe05` |
| PH-023 NLI discriminator | `todo`, dep unmet | **`todo`, dep met** | PH-021 done |

Two of these turned on a stale caveat rather than a stale claim: both core
reports said their clippy leg failed on "2 pre-existing `notebook.rs` errors".
Those errors are gone, so both gates now pass *in full*. A report that
honestly records a caveat still needs the caveat re-measured later; the
caveat is a fact with a date on it.

## 2. The one amendment that binds future work

**A1 (MS-001).** The report called `evaluate_mutation` "deterministic given
base nodes/hypotheses/edges + mutation". Values yes, order no:
`EvaluationContext::find_neighbors` (`delta_engine.rs:252-298`) returns
HashSet iteration order with no sort, so the DFS wave's
`OntologyDeltaReport.node_deltas` order — and `net_coherence_delta`'s float
summation order — is unspecified. Two identical element sets iterate
differently within one process and across processes (std-only probe in the
report).

Consequence: MS-002/MS-003 compare `node_deltas` as a set sorted by `node_id`,
and floats to epsilon 1e-6, never positionally or bit-exact. Sorting the wave
is a **core source change** and needs its own packet — it is not an amendment
to a report-only packet, and no packet currently owns it.

## 3. Eligible now, in order

1. **MS-002** — mutation replay and incremental correctness. Heaviest, and the
   one every later MS packet's credibility rests on. Own packet report plus
   `research/multiscale/MS-002/` only; no production changes.
2. **MS-003** — linguistic scales and relation survival. Independent of
   MS-002, same contract, can run in parallel in an isolated workspace.
3. **PH-023** — local NLI cross-encoder as the contradiction discriminator.
   Deps met. Gate is a measurement, not a build: precision ≥ 0.90 **and**
   recall > 0.50 on the 86-pair set, and the incumbent rule is precision
   1.000 / recall 0.50, so precision is the binding constraint.
4. **PH-013** — tenant isolation and authz threat model + enforcement matrix.
   Rung 3 (design doc first), 181 routes to audit.
5. **PH-011 (recurring)** — the monthly docs staleness sweep. This document is
   itself an input to it.

## 4. Blocked, and on what

| packet | blocked by |
|---|---|
| PH-006 pro-tier gating | the **human** features decision. The analysis it was waiting for now exists (`packets/reports/PH-001.md` §Item 3): keep `mcp` out of `default`, recommended on measured cost (+35.9 MiB, +31 s). Nothing executes it automatically. |
| MS-004, MS-005 | MS-002 / MS-003 reports reviewed |
| MS-006 | MS-005; observational only |
| MS-007 | real task outcomes — none banked |
| MS-008 | conditional review, not a permission |
| PH-004 | `claimed` since 2026-09-12 (see §6) |

## 5. Standing hazards for whoever picks these up

- **The tree is shared.** During this session the checkout did not compile on
  arrival: another agent's half-finished `EmbedCache::shared` WIP
  (`src/main.rs` and eight other files) was in the worktree. Check
  `git status` *and* `cargo check --all-targets` before believing any
  measurement, and never commit another agent's dirty files — five commits
  this session were branch-per-packet for that reason.
- **Binaries are shared too.** Rebuilding `target/debug/physis-pro` under a
  running MCP client swaps the binary mid-call; a restore via `cp` fails with
  `ETXTBSY` (use `mv`). Both were hit here.
- **A gate that forbids a string punishes the file that explains it.**
  PH-001's gate is `! grep -rn 'PHICS_LOG_LEVEL' .env*`; writing the typo into
  an `.env` comment turns it red. Keep the literal in the artifact the gate
  does not read (`src/web/mod.rs:3729`).
- **A dead condition is not dead code.** PH-022's `number_clash` compiled,
  was called, and could never be true. `physis-check`-style "is it called?"
  checks cannot see this; a test that asserts the branch fires can.

## 6. Next hygiene step

PH-004 has been `claimed` since 2026-09-12 with a design report
(`packets/reports/PH-004-design.md`) and no status move — the same shape PH-001
had before this session. Per `PROTOCOL.md` §7, a claim older than seven days
with no commits reverts to `todo`; the cheap correct action is a status pass,
not a re-execution: read the design, re-measure its claims against the current
tree, and either finish or revert the claim.
