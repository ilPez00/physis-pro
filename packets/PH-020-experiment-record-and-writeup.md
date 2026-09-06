---
id: PH-020
track: physis-pro
phase: 1
status: todo
rung: 3
depends: []
gate: "cd ~/dev/physis-pro/physis-core && cargo run --release --features embed-onnx --example experiment16_ablation_matrix -- --json > /tmp/exp16.json && test -s /tmp/exp16.json && cd ~/dev/physis-pro && test -s docs/benchmarks/EXPERIMENT_SERIES.md"
commit: "branch agent/PH-020; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
---

# The 26-iteration experiment series has no results record and is invisible from physis-pro

## Why (the failure this prevents)

`physis-core` origin/master carries 22 experiment programs implementing 26 numbered iterations —
ablation matrices, cross-embedder ARI controls, blind conditions, and five signal families tried
and abandoned with the failures recorded as failures. It is the most rigorous work in this project
and by a wide margin the most publishable.

**Its results exist only in commit messages and program stdout.** Nothing records what each
iteration measured. To read the series today you must fetch models separately and re-run 22
programs. That means: the findings cannot be cited, a regression in any of them is undetectable,
and the moment someone reruns an experiment against a newer embedder the earlier number is gone.
A research program whose results are not written down decays into a directory of scripts.

Second failure this prevents: physis-pro pins physis-core at `9d43821` (v0.1.14), which predates
all of it. Nothing in physis-pro's docs points at the series. The audit that produced this packet
initially concluded the project had *no* experimental discipline, because from physis-pro's tree
it has none visible. A reviewer, a referee, or the next agent will reach the same wrong conclusion.

## Context (measured 2026-09-06; verify before relying on it)

**Do not start this packet by writing experiments. They already exist.** Read them first.

`physis-core` origin/master = `f4dd2a1`, v0.1.15, **28 commits ahead** of the pin physis-pro uses.
`git log --oneline 9d43821..origin/master` in the submodule lists the series. The 22 programs are
in `physis-core/examples/`:

```
experiment6_dataset_a_hierarchy        experiment17_jepa_ontology_binding
experiment7_recursion                  experiment18_multi_embedder_comparison
experiment8_dataset_d_crosscutting     experiment19_cross_embedder_corroboration
experiment9_certification_gate         experiment20_fixed_point_discovery
experiment10_integration               experiment21_fixed_cell_linkage
experiment11_generalized_certification experiment22_wordnet_discovery
experiment12_integrated_synthesis      experiment23_wordnet_disambiguated
experiment13_local_calibration_...     experiment24_cross_embedder_split_agreement
experiment14_borderline_item_detector  experiment25_capacity_constrained_loss_gate
experiment15_real_ontology_prevalence  experiment26_persistent_registry
experiment16_ablation_matrix           ontology_metric_learning_experiment
                                       perspective_discovery_experiment{,2,3,5_context}
                                       probe_embedding_determinism  embedder_bench
```

Verified by direct reading, so you know the quality bar you are recording:

- `experiment16_ablation_matrix.rs` — six semantically distinct configurations over one ontology,
  **coarse assignment held identical across all six** so each mechanism's marginal contribution is
  isolated; scoring at coarse or fine level depending on whether the config recursed; averaged over
  20 items; run for both bare and contextual representations. Header quotes the governing rule:
  *"if raw cosine plus thresholding performs equally well, say so."*
- `experiment24_cross_embedder_split_agreement.rs` — compares a given partition against a second,
  architecturally different embedding space's **independently discovered** partition via Adjusted
  Rand Index, the second embedder "never told what MiniLM's given partition looked like".
  Explicitly distinguishes itself from the adjacent question tested in Iteration 19.

Run form is uniform: `cargo run -p physis-core --features embed-onnx --release --example <name>`.
They need real weights (`models/`), so they cannot run in CI as-is.

`physis-core/src/experiments.rs` is a separate, unrelated thing: 62 lines of doc comment, **zero
lines of code**, on both `9d43821` and `f4dd2a1`. It specifies five *different* experiments
(Pearson *r* on coherence/survival; pinned-vs-unpinned survival ratio; goal-seeding chi-square;
decay-rate λ fit; Spearman *ρ* on evidence/fitness). Decide its fate in step 6 — do not assume it
must be filled in.

Also relevant: `physis-core` origin/master commit `041b401` fixes non-deterministic
`classification_domains()` ordering, described as "the real cause of run-to-run drift". Any result
recorded from a run before that commit may not reproduce. Record the core commit with every number.

## Steps

1. **Read the series before writing anything.** Walk `git log 9d43821..origin/master` and the 22
   programs. Write a one-paragraph summary per iteration: the question, the control, the verdict,
   and what it ruled out for the next one. This is the packet's real work and its main deliverable.
2. **Give every experiment a machine-readable output.** Add a `--json` flag (or emit
   `results/<name>.json` unconditionally) carrying: experiment name, physis-core commit, embedder
   kind and model checksum, seed, N, every metric with its value, and the verdict string. Keep the
   existing human-readable stdout — do not replace it. Do this uniformly; a series where each
   program reports differently cannot be tabulated.
3. **Record the results.** `physis-core/docs/EXPERIMENTS.md`: one row per iteration — question,
   dataset, control, metric, result, verdict (`supported | not supported | mixed`), and the commit
   it was run at. Check in the `results/*.json` alongside it. This is what makes the series citable
   and what makes a regression visible.
4. **Make it reachable from physis-pro.** `docs/benchmarks/EXPERIMENT_SERIES.md`: what the series
   is, where it lives, what it found, how to run it, and the explicit warning that the submodule
   pin predates all of it. Link from `PLAN.md` and `docs/EPISTEMIC_AUDIT_2026-09-06.md`. Nothing in
   physis-pro currently points at this work; that is how the audit initially got it wrong.
5. **Decide the pin.** Either bump the submodule to v0.1.15 (getting `041b401`'s determinism fix
   and the WordNet packaging change — read `98d2341` first, it changes what ships) or record in
   `docs/` why physis-pro deliberately stays on v0.1.14. Do not leave it undecided by default.
6. **Resolve `src/experiments.rs`.** Its five specified experiments ask about the *engine*
   (coherence/survival, fitness/evidence), not about ontology discovery — they are not duplicates
   of the series. Either implement them as `examples/` programs in the same style, or delete the
   module and move its spec into `docs/EXPERIMENTS.md` as future work. A publicly exported module
   that is entirely doc comment is worse than either.
7. **Draft the paper.** `docs/papers/ontology-discovery-negative-results.md`: the series' argument
   in order — what was hypothesised, what was controlled for, what failed, what the five exhausted
   signal families were, and what Iteration 8's "first unambiguous positive result" actually
   showed. Aim it at a negative-results venue. **Do not soften the failures**; they are the
   contribution.

## Tests you must add

- **Regression, per experiment:** each program's `--json` output is checked in as
  `results/<name>.json`; a test re-runs a **fast subset** (small N, one seed) and asserts the
  verdict field has not flipped. Numbers may drift with the embedder; a flipped verdict is a
  regression and must fail.
- **Unit:** `experiment_json_schema_is_uniform` — every `results/*.json` carries the required keys
  (name, core commit, embedder, seed, N, metrics, verdict). This is what keeps the record
  tabulatable as the series grows.
- **Unit:** `probe_embedding_determinism` already exists as an example — promote its assertion into
  a real test so the `041b401` determinism fix cannot regress silently.
- **Doc:** `docs/EXPERIMENTS.md` has a row for every `examples/experiment*.rs`. Fail if a program
  exists with no row, or a row names no program. This is the guard that stops the record from
  drifting out of date again.
- A results record without a drift guard is not done.

## Gate

```bash
cd ~/dev/physis-pro/physis-core
cargo run --release --features embed-onnx --example experiment16_ablation_matrix -- --json > /tmp/exp16.json
test -s /tmp/exp16.json
cargo test --release
cd ~/dev/physis-pro
test -s docs/benchmarks/EXPERIMENT_SERIES.md
cargo clippy --all-targets -- -D warnings
```

The experiments need real weights in `models/` and are slow. They are a Tier-1 gate — run once,
never per iteration. The fast subset from the tests section is what belongs in CI.

## Commit strategy

Most of the work lands in the **physis-core submodule**, which has its own remote and is published
to crates.io. Branch there, commit the `--json` plumbing and `docs/EXPERIMENTS.md` separately from
the results files, push that branch, then commit the physis-pro-side docs and (if step 5 says so)
the submodule pointer bump. Never leave physis-pro pointing at an unpushed submodule commit.

Land steps 1–4 before 5–7. The record is the deliverable; the pin decision and the paper both
depend on it existing.

## Report

Append `packets/reports/PH-020.md` per PROTOCOL §5. Must include the full iteration table from
step 3 — every iteration, its verdict, and what it ruled out — and the step 5 pin decision with its
reasoning. If the fast-subset reruns disagree with any recorded historical result, say so
prominently; that is the most valuable thing this packet can find.

## Never

- Never rewrite an experiment to produce a better result. If a rerun disagrees with the record,
  record the disagreement.
- Never delete or soften a negative result. Five exhausted signal families are the finding.
- Never record a number without its seed, its N, its embedder, and the physis-core commit — results
  from before `041b401` may not reproduce.
- Never bump the submodule pin as a side effect of this packet without step 5's explicit decision
  and a note on what `98d2341` changes about what ships.
