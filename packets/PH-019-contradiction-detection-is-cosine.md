---
id: PH-019
track: physis-pro
phase: 0
status: todo
rung: 2
depends: [PH-017]
gate: "cd ~/dev/physis-pro && cargo test --release --test contradiction_detection && cargo test --release && cargo clippy --all-targets -- -D warnings"
commit: "branch agent/PH-019; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
---

# Contradiction detection is cosine similarity — the exact error the thesis is written against

## Why (the failure this prevents)

`PHYSIS_THESIS.md` §1.1 is titled "The Cosine Fallacy". It states that a claim *A* and its
negation *¬A* "frequently share high cosine similarity (≥ 0.85) due to shared lexical and semantic
context", and presents this as the defect Physis exists to correct. Three of the four contradiction
paths in this codebase are cosine thresholds; the fourth performs no comparison at all.

If a reviewer, a customer, or a referee reads §1.1 and then reads `harness.rs:199`, the project
loses its central claim in one minute. Worse operationally: a maintenance note that *agrees* with
a known-failed hypothesis is currently flagged as contradicting it, so the system raises conflicts
on corroboration and stays silent on genuine negation.

## Context (measured 2026-09-06 on physis-pro `13d8cdf` + physis-core `9d43821`; verify before relying on it)

Four detection paths, all defective:

**1. `src/ai/harness.rs:196` — similarity to a failed claim treated as contradiction:**

```rust
for h in core.hypotheses.values() {
    if h.status == HypothesisStatus::Contradicted || h.status == HypothesisStatus::Failed {
        let sim = cosine_sim(&embedding, &h.embedding);
        if sim > 0.68 { contradictions.push(ContradictionSummary { severity: sim, .. }); }
    }
}
```

**2. `src/ai/harness.rs:216` — similarity to *both* sides of an open contradiction:**

```rust
if sim_a > 0.70 && sim_b > 0.70 { /* flagged */ }
```

Being close to both *A* and *¬A* is exactly the symptom §1.1 predicts, used here as the signal.

**3. `physis-core/src/core.rs:606` — no detection whatsoever:**

```rust
pub fn detect_hypothesis_contradiction(&self, a: &str, b: &str) -> Option<Contradiction> {
    let ha = self.hypotheses.get(a)?;
    let hb = self.hypotheses.get(b)?;
    // ...construct two parties from the two statements...
    Some(Contradiction::new(party_a, party_b))
}
```

Any two hypotheses that exist are "contradictory". The only `None` is a missing id.

**4. `physis-core/src/core.rs:378` — `detect_contradictions()` returns nodes with
`coherence_score < 0.3`.** That is vector-space outlier detection under a contradiction name; the
return type is `Vec<IsolatedBranch>`, which is at least honest, but the function name is not.

What is **not** broken and must be preserved: the contradiction *model*
(`physis-core/src/contradiction.rs`) is good. `Contradiction`, `ContradictionParty`
(`authority`, `confidence`, `timestamp`, `context`), `ResolutionStatus`
(`APreferred | BPreferred | Open | Contextual | Superseded`), and `TemporalValidity` on the
contradiction itself give everything needed for non-destructive tracking. `record_contradiction`
(`core.rs:588`) writes an `EpistemicEventType::ContradictionDetected` to the append-only audit
trail. The storage layer is finished; only the detector is missing.

Note `ContradictionParty` has an `embedding` field that is currently unused by every caller —
available if a detector wants it.

## Steps

1. **Rename before fixing, so nothing keeps the wrong name.** `core.rs:378`
   `detect_contradictions` → `detect_isolated_branches`. Keep a `#[deprecated]` alias for one
   release since it is public API. Update the four call sites (`rg 'detect_contradictions'`).
2. **Decide the detector and write the decision down before coding.** Three viable options,
   ordered by cost; pick one and record why in the report:
   - *(a) Structural/ontological*: two hypotheses whose `ontology_refs` name the same entity but
     assign mutually exclusive cells or states, or whose `TemporalValidity` windows overlap while
     asserting incompatible values. Cheapest, fully deterministic, needs no model, and exercises
     primitives that already exist and are currently never read by any scorer.
   - *(b) Polarity/negation*: lexical negation and antonym detection over the two claims, gated on
     high topical similarity. Catches the *A* / *¬A* case §1.1 names. Language-dependent — note the
     Italian fixtures in `tests/classification_fixtures.json`.
   - *(c) NLI entailment*: a small cross-encoder producing `entails | neutral | contradicts`. Most
     accurate, adds a model dependency and inference cost. **If chosen, it must be a separate
     optional feature** — it cannot land in the default embedder cascade (see
     `packets/README.md`: adding a model to `models/` can silently swap the primary embedder).
   Recommended: implement **(a) first**, ship it, and only then evaluate whether (b) or (c) adds
   measurable recall. (a) is the option that makes Physis structurally different from a vector
   store, which is the whole argument.
3. Implement the chosen detector as `physis-core/src/contradiction.rs::detect(a, b) ->
   Option<Contradiction>`, returning `None` when there is no structural conflict. Every returned
   `Contradiction` must carry `resolution: ResolutionStatus::Open` and an `explanation` naming the
   specific conflict found — never a generic string.
4. Rewrite `core.rs:606` `detect_hypothesis_contradiction` to delegate to it. It must be able to
   return `None` for two unrelated hypotheses; today it cannot.
5. Replace the two `harness.rs` cosine blocks. Cosine may remain as a **candidate filter** — a
   cheap prefilter that selects pairs worth testing — but must never itself produce a
   `ContradictionSummary`. Make that explicit in the comment, with the §1.1 reason.
6. Preserve non-destructiveness end to end: a detected contradiction must not delete, downgrade,
   or overwrite either party. Assert this in a test.
7. Update `PHYSIS_THESIS.md` §5 and `docs/COHERENCE.md` to describe the detector that now exists.

## Tests you must add

New file `tests/contradiction_detection.rs`. It must contain a small labelled fixture set —
minimum 20 claim pairs, each labelled `contradicts | compatible | unrelated`, including:

- **The §1.1 case, mandatory:** at least three pairs that are genuine negations *and* have cosine
  ≥ 0.80 ("Valve 4 is open" / "Valve 4 is closed"). These must be detected as contradictions.
  Under the current code they are detected only by accident of a threshold; assert the new
  detector gets them for the right reason.
- **The inverse case, mandatory:** at least three pairs that are corroborating restatements with
  cosine ≥ 0.80 ("Spindle bearing overheating at 95 C" / "Bearing temperature reached 95 degrees").
  These must **not** be flagged. This is the case the current code gets backwards and is the
  clearest before/after demonstration.
- **Temporal disambiguation:** a pair that contradicts only if the validity windows overlap.
  Assert non-overlapping windows yield `None` — this exercises `TemporalValidity`, which no scorer
  currently reads.
- **Unrelated pairs:** must yield `None`, to pin the false-positive rate.

Report precision and recall on the fixture set for both the old cosine path and the new detector,
in the packet report. A detector without a measured false-positive rate is not done.

- **Unit** (`physis-core`): `detect_hypothesis_contradiction_returns_none_for_unrelated` — fails
  before the fix, since the function currently cannot return `None`.
- **Unit**: `detected_contradiction_preserves_both_parties` — neither hypothesis's status,
  evidence, or embedding changes as a result of detection.
- **Unit**: `isolated_branch_detection_is_not_contradiction_detection` — pin the rename.
- A bug fix without a regression test is not done.

## Gate

```bash
cd ~/dev/physis-pro
cargo test --release --test contradiction_detection
cargo test --release
cargo clippy --all-targets -- -D warnings
```

## Commit strategy

Renaming a public physis-core function is breaking: keep the `#[deprecated]` alias, bump the minor
version, push the submodule branch, then bump the pointer in physis-pro. If you take option (c),
the NLI model must be behind a non-default cargo feature and must **not** be placed under
`models/` in a way that enters the embedder cascade — verify with
`GET /api/v1/embedder` → `{"degraded": false, "kind": "bge-base"}` after the change.

## Report

Append `packets/reports/PH-019.md` per PROTOCOL §5. Must include: the option chosen in step 2 with
its justification, and the precision/recall table for old vs. new on the fixture set.

## Never

- Never let a cosine threshold alone produce a `Contradiction`. It may only prefilter candidates.
- Never delete, supersede, or downgrade a hypothesis as a *result of detection*. Detection records
  tension; resolution is a separate, explicit, human- or evidence-driven act.
- Never place a new model under `models/` without checking `src/embed/mod.rs:264` — presence alone
  can swap the primary embedder and orphan every persisted node.
