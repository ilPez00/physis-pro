# 2026-09-05 — M0: make the loop runnable

> File-integrity note: Steps 1–3 below were written at the times stated, but the
> file was truncated mid-session when the root filesystem hit 0 bytes free and
> was then cleared. The text is restored verbatim from the session transcript.
> Nothing was revised after the fact — in particular Step 1's confidence number
> and its stated reasoning are as originally written, which matters because
> Step 5 marks that reasoning wrong.

## Step 1 — Pre-registration

Written before any query against the Physis ledger. Code was read; the ledger was not.

### Task

Build order M0: "whatever minimal CLI or API is needed to query and write
hypotheses from a dev session." Until this exists there is no measurement, so
nothing else in the directive can be honestly executed.

### What I intend to do

Reading `physis-core/src/hypothesis.rs` and `src/main.rs`, the epistemic model is
substantially complete and the *write path* is where the loop breaks. Specifically
I predict these five gaps, and intend to close the first four:

1. **`hypothesis resolve` does not exist.** `Prediction.correct` is `Option<bool>`
   and `recompute_fitness()` already reads it (`pred_success`,
   `failed_prediction_penalty`), but nothing in the CLI can ever write it. Step 5
   of the loop ("resolve it — set `Prediction.correct`, let fitness update") is
   therefore currently impossible. This is the single highest-value gap: the
   machinery exists and is unreachable.
2. **No topic query.** `HypothesisCmd::List` prints every hypothesis, unranked.
   Step 2 ("query Physis for anything relevant") degrades to reading the whole
   graph, which does not scale past ~30 claims and will silently stop happening.
   I intend `hypothesis query <topic>` ranked by embedding similarity, printing
   status + fitness per claim.
3. **ID friction.** `List` prints `&h.id[..8]`; `Evidence`/`Predict`/`Transition`/
   `Explain` all do `hypotheses.get_mut(&id)` on the full 36-char UUID. Nothing
   printed by the tool can be pasted back into it. I intend unambiguous prefix
   resolution.
4. **No open-predictions surface.** `pending_predictions()` exists as a count
   only. M4 needs the list; the loop needs it at Step 5 to know what to resolve.
5. **`sessions/` and `metrics/` do not exist** and Part 4 mandates
   `metrics/ledger.jsonl`. I intend a small script rather than Rust, because the
   ledger is about the *developer's* process, not about Physis's model, and
   putting it in the engine would be the scope creep Part 1 warns against.

I intend to touch only `physis-core/src/hypothesis.rs` (add `resolve_prediction`)
and `physis-core/src/main.rs` (four subcommands + prefix resolution), plus new
non-Rust tooling. No change to the persisted schema — `correct` is already a
serialised field, so existing graphs stay readable.

### What would make this wrong

- If a topic query already exists somewhere I did not grep (studio, MCP, web).
- If `resolve` exists under a different name (`outcome`, `observe`).
- If hypotheses are not actually persisted by `core.persist()` in a form where
  mutating a nested `Prediction` round-trips.
- If the crate does not build, or `--features cli` is broken, in which case M0 is
  a build task and not a feature task at all.

### Confidence

**0.6** that this approach survives contact with the code.

Lower than it feels, for one reason: I have not built the crate. The repo has
`Cargo.toml.bak`, a 3-branch merge in recent history, and a `models/` directory
that memory says needs weights pulled from HuggingFace. My prior is that
`cargo build -p physis-core --features cli` is more likely to be the actual
obstacle than any of the five gaps above.

---

## Step 2 — Query

Nothing to query. Recorded in full because a null retrieval is the result.

| location | contents |
|---|---|
| `feather:~/.physis-core/` | directory exists, **zero files** |
| `pansa:~/.physis-core/` | does not exist |
| `omo:~/.physis-core/` | `nodes.json` — 83 coherence nodes, bare-array legacy format; `quality.json` — 2 failures; `custom_ontology.json` — empty |
| anywhere | **zero hypotheses, zero contradictions, zero audit events** |

omo's `nodes.json` is a JSON array, which `from_json` accepts as the pre-upgrade
format — so it deserialises with `hypotheses`, `contradictions` and
`epistemic_audit` all empty, and there is no earlier state that was lost. There
has never been a hypothesis in this system.

The query I intended to run (`hypothesis query <topic>`) does not exist, but its
absence did not cost me anything this session: `hypothesis list` would have
returned zero rows just as fast.

### One pre-registered risk retired

I listed "hypotheses are not actually persisted" as a way the plan could be
wrong. It *was* wrong, and it has already been fixed: `to_json` once serialised
only `nodes`, so every save discarded hypotheses, contradictions and the audit
trail — `hypothesis create` printed an ID and `hypothesis list` immediately
reported zero. `core.rs` now writes `PersistedCore` and carries the regression
test `hypotheses_contradictions_and_audit_survive_a_save`, plus a bare-array
compatibility test so omo's 83 nodes are not wiped on upgrade. This is why the
graph being empty is not evidence of that bug still being live.

## Step 3 — The binary

    plan_changed: false
    changed_by_claim_id: null

No retrieved claim altered any decision, because no claim was retrieved. This is
the trivial `false` — the ledger was empty — and it is the expected value for
the first session. It is recorded as `false`, not rationalised into anything else.

---

## Step 4 — Execute

### `physis-core/src/hypothesis.rs`

- **`resolve_prediction(index, actual, correct) -> bool`** — the write path that
  did not exist. Sets `actual_outcome`, `observed_at`, `correct`, then
  `recompute_fitness()` and appends a revision naming which way it went. Refuses
  an out-of-range index and refuses to re-resolve: a record whose outcome can
  change on a second call is not a record of anything.
- **`open_predictions() -> Vec<(usize, &Prediction)>`** — carries the index,
  because that index is what the CLI asks you to type back.
- Seven unit tests in a new `resolution_tests` module.

### `physis-core/src/main.rs`

- **`hypothesis query <topic>`** — ranks by cosine against the topic embedding,
  prints status, fitness, evidence counts and pending predictions, and hides
  dead claims (`Failed`/`Inert`/`Superseded`) unless `--include-dead`.
- **`hypothesis resolve <id> <n> "<actual>" --correct|--wrong`** — refuses when
  neither or both verdict flags are given, rather than defaulting. A default
  either way lets a wrong prediction be recorded as right by omission, which is
  the single thing this command exists to prevent.
- **`hypothesis open [--older-than N]`** — the M4 surface, oldest first, each row
  carrying the exact command that resolves it.
- **`require_hypothesis_id`** — see Step 5.

### `bin/physis-session`

One command per step of the loop, plus `report`. Two refusals in it:
`start` will not overwrite an existing pre-registration, and `end` will not write
a ledger row whose `plan_changed` it had to guess, or a `true` with no claim id.

### `sessions/`, `metrics/ledger.jsonl`

Created. They did not exist.

---

## Step 5 — Outcome

### Did the Step 1 approach survive?

**Yes, with one correction and one wrong reason.**

Four of the five pre-registered gaps were real and closed exactly as described.
The fifth — `sessions/`/`metrics/` — was real and is now scaffolded.

**The correction.** I pre-registered "no prefix resolution" as gap 3 and started
writing `resolve_hypothesis_id` from scratch. It already existed, 400 lines
further down, returning an `IdMatch` enum — and was wired into exactly one
command, `replay`, whose author had hit the same wall and solved it locally. The
compiler caught the duplicate. I deleted mine and wrapped theirs
(`require_hypothesis_id`). The gap was not "prefix resolution is missing"; it was
"prefix resolution exists and five of six commands do not use it", which is a
different and more interesting kind of defect — the fix had already been found
and not generalised.

**The wrong reason.** My confidence was 0.6, and I said so specifically because I
expected the build to be the obstacle: a `Cargo.toml.bak`, a three-branch merge in
recent history, model weights that need pulling from HuggingFace. `cargo build -p
physis-core --bin physis-core --features cli` finished in **19.78 seconds** from
cold, and the CLI's embedder is `RandomProjectionEmbedder`, which needs no
weights at all. The number was defensible; the reasoning under it was not, and
that gap is worth more than the number.

`prereg_survived: true`. 125 lib tests pass, up from 118; the full suite is green.

### What resolving actually does

Measured, not asserted:

    fitness 0.500 -> 0.225   after one prediction resolved WRONG

A quarter of the composite (`predictive_success`) plus a `failed_prediction_penalty`
had been unreachable since the type was written. Every hypothesis in every graph
has carried a `predictive_success` of exactly 0.5 because nothing could ever
write the field it reads.

### An honesty problem found while testing

`hypothesis query "stripe payment webhooks in the praxis backend"` scored **0.830**
against a claim about `OntologyDeltaReport` and ranked it first. `load_embedder`
returns a `RandomProjectionEmbedder` unconditionally — hashed bag-of-words, no
trained weights — and its cosine is high between arbitrary sentences.

The ordering is usable. The number is not. Printing it bare is describing the
system as more capable than the code makes it, which Part 6 forbids, so `query`
now says so in its own output rather than in a doc nobody reads at the moment
they are trusting the score. This is not a fix; it is a label on a known defect.

### Predictions this task resolved

None. There were none to resolve — the graph was empty when the task started.

### Things found that this task does not fix

Nothing was blocked by them, so they are recorded rather than acted on:

- **The graph is per-machine and already divergent.** `omo:~/.physis-core/nodes.json`
  holds 83 coherence nodes; feather's is empty; pansa has none. There is no
  notion of a canonical ledger across the fleet, so "query Physis" returns a
  different answer depending on which box the session runs on — and "did
  retrieval change the decision" becomes partly an artifact of ssh destination.
  Worth a hypothesis once there is enough of a ledger for it to matter.
- **`Hypothesis::revise` records `previous_status` and `new_status` as the same
  value** — it reads `self.status` after the caller has already mutated it. Every
  revision except those routed through `transition_to` therefore shows no
  transition. It does not affect resolution, which is why it is a note and not a
  patch.

## Step 6 — Write back

Two hypotheses, both falsifiable by the ledger itself and both load-bearing on
whether M1–M4 are worth building. No others: the graph being empty is a
temptation to populate it, and Part 6 names that as the anti-pattern.

- **`f8fcf49e`** — *Retrieval, not the write path, is the binding constraint:
  random-projection relevance is too weak to surface the right prior claim, so
  rediscovery will keep happening even once claims are registered.*
  Evidence attached: the 0.830 unrelated-query score, weight 0.7 — which is why
  it is already `Supported` rather than `Candidate`.
  Prediction: *across the first 20 tasks, rediscovery is recorded true more often
  than plan_changed.*
  If it holds, M0 is not finished until a real embedder is wired in, and no
  amount of M2/M3 work will move the headline metric.

- **`1bf9fec4`** — *The ledger's value will come from resolved predictions rather
  than from retrieval: plan_changed stays under 15% while the record of what was
  predicted and missed is what changes behaviour.*
  Prediction: *at task 20, plan_changed is under 15% AND at least one
  resolved-wrong prediction is cited in report-1.md as having changed a decision.*
  This one is deliberately uncomfortable: it predicts that the project's own
  headline metric comes in under the threshold the directive sets for "say so
  plainly". If it holds, the honest report is that the premise as stated is
  wrong and the value is somewhere adjacent to it.

Both are `pending`. `physis-session open` will keep putting them in front of me.
