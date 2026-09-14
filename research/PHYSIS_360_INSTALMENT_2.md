# Physis 360° investigation — instalment 2: fixes, and Phases 5–6

Continues `PHYSIS_360_INSTALMENT_1.md`. Two defects from Phase 2 are fixed, the
fix plan for the rest is written down with what each one is waiting on, and the
temporal experiment (Phases 5 and 6) has run.

## Fixes applied

**1. `cargo test` could not build the crate.** Two research examples
(`impossible_machine_experiment`, `experiment48_exponent_necessity`) have their
only `main` behind `#[cfg(feature = "embed-onnx")]`. Without that feature the
file has no `main`, which is E0601, and cargo builds examples as part of
`cargo test` — so the library's tests were green and unreachable behind a
compile error in code no test touches. Both now carry a stub `main` under the
inverse cfg that prints what the example needs.

**2. A shell snippet was compiled as Rust.** `config_run`'s module docs put two
`physis-core run --config …` lines in an indented block, which Rust treats as a
doctest. Fenced as `text`.

`[EXPERIMENT]` After both: **375 passed, 0 failed** across the crate's unit,
integration and doc tests (`--no-default-features --features cli`), up from
"307 if you know to pass `--lib`, otherwise a compile error".

## Fix plan for what instalment 1 found

Split by what each change is waiting on, because two of them would otherwise be
the architecture being edited to make the hypothesis true.

| finding | fix | status |
|---|---|---|
| `cargo test` broken | stub mains | **done** |
| doctest compiled shell | fence it | **done** |
| `preference`, `self_model_version`, `previous_coherence_state` never read | either delete them or wire them — **not both silently** | blocked on Phase 10/12: if directedness and goals turn out to be inert too, the honest fix is deletion plus a note, not new plumbing |
| `grid_fitness` called only from examples | leave it; it is an instrument | no change — the defect was the *claim*, not the code |
| node records store no content | decide whether the store is an index or a store | blocked on Phase 3: if retrieval quality needs content, it is a gap; if labels suffice, it is a design |
| nothing restores state after perturbation | do not add a repair loop yet | blocked on Phase 16: an ablation has to show that the absence costs something measurable |
| no temporal semantics (below) | minimal `observed_at` on evidence | **deficiency now demonstrated** — see the last section |

## E77 — timestamps, or temporal semantics? (Phases 5 and 6)

`benchmarks/conatus/e77_temporal.py`. A machine heats up: 60° on 2026-01-01,
80° on 2026-06-01, 95° on 2026-12-01. Claim: *"machine temperature stays below
70"* — true at t0, false afterwards. Three pieces of evidence carry those dates
in their text. Then the engine is asked to replay the belief at each date.

| replayed at | forward arrival | reverse arrival |
|---|---|---|
| 2026-01-01 | *no recorded belief* | *no recorded belief* |
| 2026-06-01 | *no recorded belief* | *no recorded belief* |
| 2026-12-01 | Contradicted | Contradicted |

What replay actually printed at t1:

```
[63e892fa…] is known but had no recorded belief at 2026-06-01 00:00:00 UTC
```

`[EXPERIMENT]` The engine holds evidence stating the machine ran at 80° on
2026-06-01, and reports no belief at that date. The split it *does* make is
before/after the moment the rows were written (this run, 2026-09-14) — January
and June precede the write, December follows it.

**Replay is indexed by write time. The world times inside the evidence are
text.** A claim that is true at t0 and false at t1 cannot be represented in this
path: there is one current belief, and a history of when rows were inserted.

`[CODE]` This is consistent with the field survey: `TemporalValidity` (with
`valid_from` / `valid_until` / `is_valid_at`) exists and is live in `process.rs`
and `contradiction.rs`, but nothing on the hypothesis/evidence path carries an
`observed_at` distinct from the insertion time.

**Verdict — temporal semantics: NOT DEMONSTRATED. Timestamps: DEMONSTRATED
(ingestion only). Temporal arrow: NOT DEMONSTRATED.**

## Phase 25 — the minimal addition this justifies, not implemented

Phase 25 says: propose only if an experiment demonstrates that state-only
representation prevents something. E77 demonstrates it, and the minimum is
smaller than a `Transition` object:

> **WHY** a claim cannot be true at one time and false at another, because
> belief is indexed by when a row was written.
> **WHAT SHOWED IT** E77: evidence dated 2026-06-01 is held, and replay at
> 2026-06-01 reports no belief.
> **WHAT CANNOT BE REPRESENTED** the distinction between *when this was
> observed* and *when we were told*.
> **MINIMAL FIX** one optional `observed_at` on evidence, defaulting to the
> insertion time, and `replay` filtering on it. No transition object, no
> alternatives, no selection criterion — those need Phases 10 and 12 first, and
> if directedness turns out inert, they would be plumbing for a capability that
> does not exist.

Not implemented in this instalment, deliberately.

## Verdicts after instalment 2

| capability | verdict |
|---|---|
| baseline library functionality | **DEMONSTRATED** (375/375, after two fixes) |
| persistent identity | PARTIALLY SUPPORTED (append-only storage) |
| perturbation resistance | NOT DEMONSTRATED |
| self-maintenance | NOT DEMONSTRATED |
| preference | REFUTED as implemented |
| self-reference | NOT DEMONSTRATED |
| learning / path dependence | REFUTED at the hypothesis level |
| **temporal semantics** | **NOT DEMONSTRATED** — ingestion timestamps only |
| **temporal arrow** | **NOT DEMONSTRATED** |
| computational conatus | NOT DEMONSTRATED on this evidence |

Still not run: Phases 3, 4, 10, 12, 15–22.
