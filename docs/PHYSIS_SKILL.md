---
name: physis
description: Judge your own work before claiming it is done. Use when working in physis-pro/physis-core, or any time you are about to (a) claim a capability exists, (b) summarise what a system is, (c) quote a benchmark number as evidence, or (d) report a task complete. Also use when the user says "physis", "check yourself", "did you verify", or pushes back on a claim you made.
---

# Physis: judge your own work

Seven failure modes, each observed repeatedly, each with a mechanical check.
Run the check. Do not reason about whether the check is needed.

Every check in this file is a command. Recitation is not verification, and the
only externally visible difference between an agent that read this skill and one
that ran it is the command output:

```bash
physis-check all [dirs]     # the whole checklist
```

`install.sh` links that onto `$PATH`. If the command is not found, it is
`~/.claude/skills/physis/scripts/physis-check.sh` — run it by path rather than
skipping the check.

Engine-backed steps print `NOT MEASURED` when physis-core is absent rather than
passing quietly. A check that silently skips is worse than no check.

## 1. Declared ≠ called

**A module that compiles and exports is not a module that runs.**

Observed four times in one session, same shape every time:

| symbol | looked like | actually |
|---|---|---|
| `OnnxEmbedder` | production ONNX embedder, exported from `lib.rs` | called by nothing — every benchmark number came from a lexical hash |
| `FitnessShifted`, `OutcomeObserved` | audit event types | emitted by no code path; `replay` disagreed with `list` |
| structural classifier | classified into `DOMAIN×MODE` | built its own random-projection embedder — symbol sequences were noise with a schema |
| `build_structural` ← `ledger` | trace-fed n-gram tables | both halves exist, join absent |

**Before writing "X works" / "X is wired" / "the system does X":**

```bash
physis-check calls SymbolName [dirs]     # one symbol
physis-check sweep [dirs]                # the whole tree
```

`calls` excludes the declaring file and counts test uses separately, because
both are how a dead symbol looks alive: `OnnxEmbedder` had seven "call sites"
and every one was its own `#[cfg(test)]` module. Zero non-test use sites outside
the declaring file ⇒ **it does not run.** Say so.

Rust, Python, TypeScript/JS and Go. The output is a list of questions, not
verdicts — trait dispatch, macros, decorators and dynamic imports are invisible
to a textual sweep, so a listed item may still be reached.

Measured on two real trees:

| tree | files scanned | items | verified |
|---|---:|---:|---|
| TypeScript, `praxis_webapp/src` | 449 | 300 | 28/30 true at whole-repo scope |
| Go, a 390-file plugin | 188 | 55 | **55/55 true** (all of them) |

Two things that measurement changed:

**Test functions are uncalled by construction.** The first Go run returned 1235
items and **1180 were `TestXxx` in `_test.go`** — the runner calls them by name,
like `main`. A list that is 96% noise gets skimmed, which is the same as not
running the check. Test files and `Test*`/`Benchmark*`/`Fuzz*`/`Example*` names
are excluded now: 1235 → 55 items, 9.7s → 3.1s.

**Scope is the false-positive source.** Of 30 TypeScript findings sampled at
random, 28 were dead anywhere in the repo and 2 (`TrendPoint`, `DigestEntry`)
were consumed by a sibling tree — `client/src/` — that was not in the swept
path. The sweep is exactly as correct as the directories you hand it: **pass
every tree that can import the code, or expect one false positive in fifteen.**

300 questions is still past what anyone reads, so on a large tree sweep one
directory at a time and treat the list as a queue, not a report.

## 2. Does the measurement discriminate?

**A benchmark whose score does not move when the thing it measures moves is not
a benchmark.**

`physis-core/benchmarks/ground-truth` scores *identically* under a random
projection hash and a real ONNX model: repeat 100%, anomaly 100%, contradiction
50%, compression ~24% either way. It cannot fail, so it is not evidence.

**Before quoting any number as evidence:** name the arm it would lose to. Run
both arms. If the difference is ~0, the number says nothing.

```bash
physis-check discriminate --a '<the real arm>' --b '<the stupid control>'
```

It runs both, subtracts, and fails on Δ≈0 — the measurement that cannot fail.
Inside the physis-pro tree, `benchmarks/retrieval/run.py --arms rp,semantic` is
the domain-specific version of the same move.

Standing rule, earned seven times in this repo's research track:
**treat any positive as an artifact until a construction-matched control says
otherwise.**

## 3. Do not compress a system to one noun

Three attempts in one session reduced physis to a single noun. All three wrong:
"context compiler" (one operation of six), "epistemic ledger" (a second of six),
"70-symbol vocabulary" (it is **70 tables with subtables**).

Each sounded sharper than the last. Sharpness is not correctness.

**Before summarising any large system:** generate its module map
(`physis-check map [dirs]`) and read that. Describe the
system **structurally** — its coordinate system and the operations over it —
never as one noun. In a physis tree, `docs/WHAT_PHYSIS_IS.md` is the canonical
version.

Specific trap: **`judge` is not `propose`.** The research track refuted
*certification* (does this belong here → chance). It did **not** refute
*proposal* (which few should a person look at → top-3 0.712 vs null 0.136).
Writing "the grid is not a discovery mechanism" is the error this rule exists
to prevent.

## 4. Read before grepping; recall before working

A module map — one line per module, taken from each module's own `//!` header —
answers "which module does X" in one read. Three separate times an agent rebuilt
that inventory by hand at real token cost because it was not on disk.
**Generate it once, read it first:**

```bash
physis-check map [dirs]     # Rust //! · Python docstring · Go package · JS/TS header
```

Measured on physis-pro: the map is ~4.4k tokens and pays for itself immediately.
A full *symbol* index was also built and measured — 3996 lines, ~26k tokens —
and deliberately discarded: it answers exactly what `grep -rn "fn foo"` answers
for ~50 tokens against fresher data. **A wiki page earns its keep only when
reading it is cheaper than the search it replaces.**

Then recall before working — has this been tried and already failed?

```bash
physis-check recall "<task>"                        # failure here = already tried
# ... work ...
physis-check verdict "<outcome>" success|inert|failure
```

Both halves must hit the **same store**, and two traps make that easy to get
wrong with no error either way: physis-pro and physis-core keep separate graphs
(a `note` written to one is invisible to the other's `node-search`), and the two
binaries read different model variables — `PHYSIS_MODEL_DIR` for physis-core,
`PHYSIS_MODELS` for physis-pro. A note written under the fallback embedder is
stored, unrecallable, and reported as success. `physis-check` sets both
variables and keeps both halves on one binary; if you call the engines directly,
check the `embedder` line and the `restored N nodes` line.

In a physis-pro tree these are `just recall` / `just dev-loop`
(`docs/PHYSIS_DEV_LOOP.md`).

MCP (30 tools, `.mcp.json`): `physis_recall` → `physis_pack` / `physis_find` →
`physis_classify` → `physis_remember` + `physis_serve`.

Needs a dev licence signed for *this* build — the gate verifies signatures even
in dev builds:

```bash
(cd physis-dev-license && cargo run -q) > "$PHYSIS_DATA_DIR/license.key"
```

## 5. Register claims that can be wrong

A claim worth making is worth recording so it can be refuted later:

```bash
physis-check claim "<claim>" 0.6        # wraps: physis-core hypothesis create
physis-core hypothesis evidence <id> "<measurement>" --polarity contradicting --weight 0.9
physis-core hypothesis transition <id> Contradicted --reason "<control that killed it>"
physis-core replay --subject <full-uuid> --at <ISO8601>   # belief state at T
physis-core hypothesis open                                # predictions never resolved
```

This is how a 7-day-old hypothesis about retrieval was found already predicting
a finding being independently rediscovered from scratch.

**PH-017 closed 2026-09-12.** `Hypothesis::revise` read `self.status` for both
ends of every transition, and every caller mutates status *before* revising — so
each revision recorded `previous == new` and the history asserted nothing ever
changed. `replay` was blind by construction while `list` derived standing from
fitness. Events were not missing; the recorded transitions were no-ops.

`revise` now takes the prior status as an argument (the compiler requires the
caller to have captured it), and `PhysisCore::project_revisions` makes the audit
trail a **view** over `revision_history` rather than a second record kept in sync
by discipline — discipline is what failed.

Verified end to end 2026-09-13: a hypothesis transitioned to `Contradicted`
reads `Contradicted` from both `list` and `replay`. Claims that predate the fix
keep permanently no-op revisions and are **not** backfilled — `replay` prints
both values and the reason. A gap in the record is information; a fabricated
record is not.

## 6. Exit 0 is not a result

**An empty result and a check that never ran look identical, and both succeed.**

Observed, all of them while building the checks in this file:

| what happened | what it looked like |
|---|---|
| `declared-never-called.sh` hit `set -e` on the first symbol-less file | a complete clean sweep |
| `ls` was shell-aliased to `eza`, so a `find`-fed loop got no files | "no items found" |
| a `str.replace` patch missed by one trailing space | the script ran, the new check simply was not in it |
| `decisions-mine` parsed 622 events and mined 0 decisions | a working pipeline, exit 0 |
| `note --verdict -1` was read by clap as a flag, not a value | verdict recording worked — for `success` and `inert` only |
| a verification grep searched `*.go` for symbols declared in `sample.py` | three findings "disproved" — the check was wrong, not the tool |
| `calls` armed its Rust `#[cfg(test)]` rule on a shell script that *mentions* `#[cfg(test)]` in a comment | a live dispatch table reported as test-only dead code |

**Every measurement prints its denominator.** "0 problems over 0 files scanned"
is not a pass; the checks here exit 2 on it and say so. When you add a check,
add the count of what it looked at — and when you patch a file by string match,
re-read the file and confirm the patch is in it.

## 7. Judge the session, not only the artifact

The code can be right and the reporting still wrong. A claim is cheap; a tool
call is not. A sentence asserting something about the repository with no tool
call between it and the previous claim came from the model's prior, not from the
repository.

```bash
physis-check flow <session-id|transcript.jsonl>
```

With no argument it takes the newest transcript on disk, which is **not
necessarily this session** — another agent's file may be newer. It prints the
path it chose; pass your own session id when it matters.

It extracts every claim, checks each for adjacent evidence, and then — with
physis-core present — runs the claims through `chain`, which reports structure
(repeats, differences, contradictions), coverage against the grid, drift from
the original ask, **and the label-permuted control, in the same pass**.

Read the control line first, and the embedder line before that:

- `embedder random-projection` ⇒ the geometry is a lexical hash. Nothing below
  it is about meaning. (Rule 1 applies to the judging tool too — `chain` runs
  perfectly happily on the fallback embedder, and the model path resolves
  relative to the current directory, so the same command is semantic in one
  directory and a hash in another, with no error either way.)
- `NOT ABOVE THE NULL` ⇒ shape, not knowledge. Report nothing from the cells.
- `CONTROL not run` ⇒ `NOT MEASURED`, never a pass.

**Measured.** On every *prose* corpus tried, the discovery arm sat at the null:
session claims (28 docs) +0.0000, `docs/` (86) +0.0005, `research/` (92)
+0.0007, `packets/` (32) control not run. Several were identical to four
decimals, which looks exactly like an invariant statistic — the failure mode
this control's own source documents twice.

It is not invariant. On corpora whose topics share no content word it clears
the bar every time there is anything to score:

| topics | docs | candidates | real | null | Δ |
|---:|---:|---:|---:|---:|---:|
| 2 | 12 | 1 | 0.0000 | 0.0000 | — nothing to score |
| 3 | 24 | 2 | 0.0655 | 0.0000 | **+0.0655** |
| 4 | 24 | 2 | 0.0976 | 0.0017 | **+0.0959** |
| 8 | 48 | 2 | 0.0710 | 0.0000 | **+0.0710** |

Verdict line on all three: `The shortlist carries signal the permuted arm does
not.` The 2-topic row is the vacuous case, correctly reported as `CONTROL not
run` rather than as a loss.

What separates the two regimes is shared vocabulary, not corpus size: an earlier
version of the 3-topic corpus differed only by one boilerplate sentence repeated
in every document, and it scored real 0.0223 vs null 0.0223 — bit-identical in
f32.

So the honest reading of a Δ≈0 on prose is **"this corpus has no grouping this
method can find"**, not "the method cannot find groupings". Treat the shortlist
as unproven on that corpus and read the halves the control does not gate —
structure, token compression, unplaced records — which are deterministic rather
than discovered.

A first pass here published the negative as a property of the method after five
prose corpora. One constructed positive refuted it. **Five agreeing corpora are
five samples from one regime, not a law.**

The winning branch of that control had no test at all until this was checked —
`discriminates() == true` was indistinguishable from unreachable code. It is
covered now (`chain::tests::beating_the_control_reaches_the_output`), and the
branch does print. Rule 1 reaches verdict paths, not only capabilities.

Rule 2 applies to this check as well — the arm it must beat is chance
clustering of claims and tool calls:

```bash
scripts/flow-null.py <transcript.jsonl>   # same rule, event order shuffled 200x
```

Measured on two real sessions:

| session | claims | ungrounded | shuffled null | Δ |
|---|---:|---:|---:|---:|
| 737 events | 28 | 7 (0.250) | 1.3 (0.048) sd 1.19 | **+0.202** |
| 383 events | 13 | 9 (0.692) | 1.0 (0.080) sd 1.01 | **+0.612** |

The flagged claims are not an artifact of sequence shape. Chance would flag
about one. The geometry half of the same run did **not** beat its null — read
the two verdicts separately.

## The checklist

Before reporting work done:

```bash
physis-check all [dirs]
```

- [ ] Every capability I claimed — `physis-check calls`, non-test sites only?
- [ ] Every number I quoted — `physis-check discriminate` against its control?
- [ ] Every summary I wrote — structural, not one noun?
- [ ] `judge` vs `propose` kept distinct?
- [ ] Every check I ran — did it print a denominator, or just exit 0?
- [ ] Every claim I made in this session — `physis-check flow`, evidence adjacent?
- [ ] Anything I could not verify — labelled `NOT MEASURED`, not implied?
- [ ] Outcome recorded with a verdict — `physis-check verdict`?

The checklist is the exit condition, not the report. Running it and reporting
what it said is the work; reporting that you followed it is not.
