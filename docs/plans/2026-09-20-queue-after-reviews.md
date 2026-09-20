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

### 1b. What the follow-up pass changed (later 2026-09-20)

| packet | before | after | evidence |
|---|---|---|---|
| PH-004 semantic self-test | `claimed`, gate filter matched **zero** tests and exited 0 | **`done`** — filter now matches two; real-weights skip asserts its own precondition; CLI exit code pinned; independently re-run | `packets/reports/PH-004.md`; commits `5e074ce`, `4fe970e` |
| MS-002 mutation replay | `review` | **`done`** — accepted report-only; citations re-checked at `ed8dc29` and `54b8a8d` | `packets/reports/MS-002.md` §Verification note; commit `59f50d7` |
| MS-003 relation survival | `review` | **`todo`** — returned: gate described wrongly, PH-051 misquoted | `packets/reports/MS-003.md` §Verification note; commit `d893d3d` |
| MS-002-harness | did not exist | **`review`** — built the same day: 7 arms green, gate run twice, negative arm proven to fail | `packets/reports/MS-002-harness.md` |

The MS-003 return is the pass that earned its keep: a survey's line citations
had drifted by one or two lines in three places (harmless), but its headline
acceptance-gate claim named three dimensions that do not exist in
`Transform::apply` and said all five must be `1.0`, when the code hard-requires
two. A successor building a relation-survival metric on that sentence would
have gated on the wrong thing.

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

## 3. Eligible now, in order (re-measured after §1b)

1. **MS-002-harness review** — built (`physis-core/tests/replay_contract.rs`,
   7 arms green, gate run twice, the negative arm proven to fail). Needs an
   independent re-run before it can be `done`. Two corrections came out of the
   build: the oracle cannot be "affected == changed" (the wave's cutoffs and
   `compute_coherence`'s global scan make a full recompute a superset), and the
   packet's P1 compared a stored score against a recomputed one. MS-004–MS-008
   rest on this oracle.
2. **MS-003 corrections** — four citation fixes to the existing report; no
   re-execution. Then MS-003 can go to `review` again, which unblocks MS-005.
3. **PH-023** — local NLI cross-encoder as the contradiction discriminator.
   Deps met. Gate is a measurement, not a build: precision ≥ 0.90 **and**
   recall > 0.50 on the 86-pair set, and the incumbent rule is precision
   1.000 / recall 0.50, so precision is the binding constraint.
4. **The review backlog** — PH-CORE-A4 / CLASSIFY / CLEANUP / DOCS /
   RETIREMENT, PH-ORCH-ROUTER, PH-013, PH-015, PH-001 are all `review` and
   each needs an independent re-run before it can be `done`. PH-CORE-RETIREMENT
   is a human decision and cannot be closed by a reviewer.
5. **PH-011 (recurring)** — the monthly docs staleness sweep. This document is
   itself an input to it.

## 4. Blocked, and on what

| packet | blocked by |
|---|---|
| PH-006 pro-tier gating | the **human** features decision. The analysis it was waiting for now exists (`packets/reports/PH-001.md` §Item 3): keep `mcp` out of `default`, recommended on measured cost (+35.9 MiB, +31 s). Nothing executes it automatically. |
| MS-004 | MS-002-harness (the oracle its credibility rests on) |
| MS-005 | MS-003 returned to `todo` — its report is the relation-family baseline |
| MS-006 | MS-005; observational only |
| MS-007 | real task outcomes — none banked |
| MS-008 | conditional review, not a permission |
| PH-CORE-RETIREMENT | **human** decision: correct the retirement docs or move the engine to a successor |

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
- **Two sessions can share one branch.** On 2026-09-20 the review of PH-004
  landed (`4fe970e`, 17:13:37) *between* two commits of the session that wrote
  the PH-004 implementation (`5e074ce`, then `e4b5d12`), because both were
  committing from the same checkout with HEAD on the same branch. Nothing was
  lost, but branch-per-packet is not enough on its own when HEAD itself is
  shared: re-read `git log --oneline -5` and `git status` immediately before
  every commit, and never `git add -A`.

## 6. Next hygiene step

PH-004 has been `claimed` since 2026-09-12 with a design report
(`packets/reports/PH-004-design.md`) and no status move — the same shape PH-001
had before this session. Per `PROTOCOL.md` §7, a claim older than seven days
with no commits reverts to `todo`; the cheap correct action is a status pass,
not a re-execution: read the design, re-measure its claims against the current
tree, and either finish or revert the claim.

**Resolved 2026-09-20 (later).** The claim was re-made at `497b91c` and is now
discharged: PH-004 is `done` with `packets/reports/PH-004.md`. The design's
three holes are closed — the gate filter `cargo test semantic_self_test` went
from matching zero tests to two, the real-weights arm's skip asserts its own
precondition instead of printing into the void, and the CLI exit code is
pinned by a child-process test. The design's claim that `models/model.onnx`
exists was stale; the flat export is absent, so the conditional arm is the
live one. Full gate green (`clippy -D warnings`, suite 0 failed).
