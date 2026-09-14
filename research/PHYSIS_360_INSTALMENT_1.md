# Physis 360° empirical investigation — instalment 1

Started 2026-09-14. This covers Phase 1 (reconnaissance), Phase 2 (baseline),
and the first three falsification experiments, which happen to hit Phases 7–9,
11, 13 and 14. Every claim carries its evidence class. Phases not listed at the
end have **not been run**, and nothing here should be read as covering them.

## Rule Zero classes used below

`[CODE]` observed in the source · `[EXPERIMENT]` observed by running something ·
`[INFERENCE]` reasonable but untested · `[PHILOSOPHY]` interpretation.

---

## Phase 1 — reconnaissance

`[CODE]` Scale: 83121 lines across two crates; 60 modules in `physis-core`
(Apache-2.0), 87 in `physis_pro` (proprietary), plus 5 binaries and ~40 CLI
subcommands.

### Three fields that name the hypothesis and are never read

The node record (`physis-core/src/models.rs`) carries exactly the fields a
conatus argument would want. `[CODE]` **Three of them are written at
construction and read nowhere in either crate:**

| field | declared | ever read? | the source's own comment |
|---|---|---|---|
| `preference: Option<f32>` | models.rs:74 | **no** | *"Enables preference-aware dream seeding and volitional analogue production. Level 4 (preference) → Level 5 (volition) transition point."* |
| `self_model_version: u32` | models.rs:69 | **no** | — |
| `previous_coherence_state: Option<f32>` | models.rs:66 | **no** | — |

Each appears exactly twice in 83k lines: the declaration and the initialiser
(`None`, `None`, `0`). Grep across `physis-core/src` and `src` finds no reader,
no comparison, no branch. The field the source itself calls the preference →
volition transition point has no consumer.

By contrast `cell_pin` (15+ read sites across `core.rs`, `delta_engine.rs`,
`atlas.rs`) and `asserted` (35+) are live and do reach decisions.

`[CODE]` `grid_fitness::run` — the D4 "cells are proposals with a fitness"
mechanism — is called from **three examples and nothing else**. Cell fitness is
a research instrument, not a selection pressure in any running path.

`[CODE]` Hypothesis fitness *is* consumed: `src/ai/harness.rs:327` gates
`Approved / Warning / Rejected` on `composite_fitness >= coherence_threshold &&
contradictions.is_empty()`, and `src/web/coherence.rs:882` filters by it. That
is one real decision path, in the Pro crate, behind the licence gate.

---

## Phase 2 — baseline

`[EXPERIMENT]` `cargo test -p physis-core --no-default-features --features cli
--lib`: **307 passed, 0 failed.**

`[EXPERIMENT]` `cargo test` for the same crate **fails to build**: the example
`impossible_machine_experiment` has its `main` behind `#[cfg(feature =
"embed-onnx")]`, so E0601 (`main` not found) stops the whole suite. The library
is green; the suite as a whole is not runnable in that configuration without
knowing to pass `--lib`. Recorded as **PARTIAL**, not PASS.

---

## E75 — is history storage, or does it constrain the future?

`benchmarks/conatus/e75_history.py`. One hypothesis, six pieces of evidence
(4 supporting, 2 contradicting, mixed weights), fed to isolated stores in
different arrival orders. Exact values read from `nodes.json`, because
`hypothesis explain` prints two decimals and would have hidden an effect below
0.005.

| arm | fitness (exact) | status |
|---|---:|---|
| ordered | 0.33125 | Contradicted |
| shuffled, seeds 0–4 | 0.33125 (all five) | Contradicted |
| blocked (all supporting, then all contradicting) | 0.33125 | Contradicted |
| a fresh claim in the same store | 0.5 | Candidate |

`[EXPERIMENT]` **Order spread: 0.000000 over six orderings, at f32 precision.**
Hypothesis state is a function of the evidence *multiset*. And a new claim about
the same subject, in a store that already holds all that evidence, starts at the
default 0.5 — the store's history does not reach it.

**Verdict — path dependence at the hypothesis level: REFUTED.** History here is
storage plus a scoring function. Nothing in this path is learned, preferred, or
carried forward.

---

## E76 — does it resist perturbation, or absorb it?

`benchmarks/conatus/e76_perturbation.py`. Four-document corpus scanned into a
store, then one document rewritten into unrelated text (a cake recipe) and a
claim added that contradicts the corpus, then every maintenance operation the
engine exposes.

| stage | coherence index | nodes |
|---|---:|---:|
| S0 | 0.928 | 4 |
| after perturbation | 0.922 | 5 |
| after `dream` + `quality` | 0.917 | 5 |

`[EXPERIMENT]` The perturbation **added** a node; all four originals were
untouched. `dream` answered *"No nodes need replaying right now."* Coherence
moved −0.006 and then −0.005 further — away from S0, not back. Nothing was
restored, isolated, rejected or repaired.

**Verdict — self-maintenance: NOT DEMONSTRATED. Perturbation resistance: NOT
DEMONSTRATED in the storage sense that matters.** What the store exhibits is
append-only persistence: old state survives because nothing ever overwrites it,
which a read-only file also achieves. That is Phase 9's passive arm, and
Physis's behaviour here is indistinguishable from it.

### Two measurement bugs this experiment produced first, both worth recording

1. The first parser matched "the first number after the word coherence", which
   picked up the node count and reported 4.0 → 5.0 as a *coherence* rise. The
   parser was doing the measuring.
2. The first restoration check compared node content before and after — and node
   records **store no content** (`id, embedding, coherence_score, asserted,
   label, cell_pin, embedder, previous_coherence_state, self_model_version,
   preference`). Every comparison was `"" == ""`, and it reported the perturbed
   node as *restored*. Content-level restoration is **NOT MEASURABLE** from the
   store; only labels, scores and embeddings are.

---

## Verdicts so far

| capability | verdict | basis |
|---|---|---|
| baseline library functionality | **PARTIALLY SUPPORTED** | 307 lib tests pass; full suite does not build |
| persistent identity | **PARTIALLY SUPPORTED** | append-only store keeps prior nodes; identity is the label/ID, nothing reconstructs it |
| perturbation resistance | **NOT DEMONSTRATED** | state absorbed the perturbation; coherence drifted down, not back |
| self-maintenance | **NOT DEMONSTRATED** | the maintenance pass declined to act |
| preference | **REFUTED as implemented** | the field is never read |
| self-reference | **NOT DEMONSTRATED** | `self_model_version` never read |
| temporal arrow (state-before stored and used) | **NOT DEMONSTRATED** | `previous_coherence_state` never read |
| learning / path dependence | **REFUTED** at the hypothesis level | six orderings, spread 0.000000 |
| coherence as a selection pressure | **NOT TESTED YET** | one real gate exists (`ai/harness.rs:327`), in the licensed crate |
| computational conatus | **NOT DEMONSTRATED** on this evidence | three of its four named ingredients are inert fields |

---

## Not yet run

Phases 3 (semantic benchmark), 4 (ontology stress), 5–6 (temporal semantics and
the transition arrow), 10 (directedness), 12 (goal ablation), 15 (failure and
recovery), 16 (counterfactual ablations), 17 (composite conatus measures), 18
(semiotic), 19 (long-horizon trajectories), 20 (comparative baselines), 21
(adversarial), 22 (LLM comparison). Phase 25 is not reachable until 5–6 have run.

## Reproduction

```sh
cargo test --offline -p physis-core --no-default-features --features cli --lib
cargo build --offline --release -p physis-core --no-default-features --features cli
python3 benchmarks/conatus/e75_history.py --out benchmarks/conatus/e75-results.json
python3 benchmarks/conatus/e76_perturbation.py --out benchmarks/conatus/e76-results.json
```

Both harnesses build their own isolated `PHYSIS_CORE_DIR`, print their
denominators, and write machine-readable results. Engine commit: see
`git -C physis-core rev-parse HEAD`.
