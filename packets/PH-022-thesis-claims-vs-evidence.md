---
id: PH-022
track: physis-pro
phase: 1
status: todo
rung: 2
depends: [PH-020]
gate: "cd ~/dev/physis-pro && cargo test --release --test epistemic_evolution && ./scripts/check_thesis_claims.sh"
commit: "branch agent/PH-022; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
---

# The thesis's "Empirical Verification" is one test with a 64-dimension hash embedder

## Why (the failure this prevents)

`PHYSIS_THESIS.md` §8 is a seven-row table, every row marked **VERIFIED**, presented as the
empirical basis of the whole project. It rests on a single test function that builds its state
with `RandomProjectionEmbedder::new(64)` — SHA-256 n-gram feature hashing into 64 dimensions, no
trained model of any kind. Every row is N = 1 and hand-constructed.

This is the document that goes to customers, referees, and investors. Anyone who opens the cited
file sees the gap in under a minute, and at that point every other claim in the repository becomes
suspect too — including the ones that are true, like the 0.873/0.727 classification accuracy. The
cost of overclaiming here is not the overclaim; it is the credibility of the real results.

## Context (measured 2026-09-06 on physis-pro `13d8cdf` + physis-core `9d43821`; verify before relying on it)

`PHYSIS_THESIS.md:174` cites `physis-core/tests/e2e_verification.rs`. That file is 252 lines and
contains **one** `#[test]`. `e2e_verification.rs:22` and `:185`:
`RandomProjectionEmbedder::new(64)`.

Specific defects in the §8 table:

- **"Cosine = 1.088."** Cosine similarity cannot exceed 1.0. This is a nearest-entry score
  reported as a cosine.
- **"Quality Penalty ... Raw score 1.0884 → 1.0000 adjusted."** That is the `clamp(0.0, 1.0)` in
  `physis-core/src/quality.rs:189`, not a learning effect. The penalty did not do this.
- **"Truth Maintenance ... contextually resolved as `BPreferred`."** The test *assigns*
  `resolution = ResolutionStatus::BPreferred` and then asserts it was stored. Nothing resolved
  anything.
- **`PHYSIS_THESIS.md:174` links `file:///home/gio/dev/physis-pro/...`** — an absolute path on one
  developer's machine, in the flagship document.

The same pattern in the test suite:

- `tests/epistemic_evolution.rs::test_peculiarity_benchmark_comparison` is named as a baseline
  comparison. At lines 564–566 it writes `semantic.score = 0.85`, `logical.score = 0.90`,
  `empirical.score = 0.30` by hand, then asserts a relation over the composite of those constants
  (`assert!(coh_before_feedback < pure_sim_score + 0.5)`). It compares nothing to nothing.
- `tests/epistemic_evolution.rs::test_1_contradiction_preservation` constructs the contradiction,
  sets its resolution and explanation, and asserts the store retained it. That is a persistence
  test named as an epistemology test.

Doc drift found alongside, in scope here: `physis-core/src/lib.rs:20` documents **five** coherence
dimensions (`Semantic, Ontological, Logical, Empirical, Process`); the code has six and shares
only three of those names (PH-017 fixes the code side; this packet fixes the claim side).
`AGENTS.md:12` states all development happens on `pansa` via ssh — this audit built and tested on
a different host without incident.

## Steps

1. **Audit every claim.** Produce `docs/CLAIMS.md`: one row per assertion in `PHYSIS_THESIS.md`
   and `README.md`, with columns `claim | evidence | file:line | N | status`. Status is one of
   `measured | demonstrated | aspirational`. Be strict:
   - **measured** — a number from a labelled dataset with a baseline and a stated N.
   - **demonstrated** — the mechanism runs end-to-end on a real input, but nothing is measured.
   - **aspirational** — the design intends it; no evidence exists.
   On today's code, most of §8 is `demonstrated` at best and several rows are `aspirational`.
2. **Rewrite `PHYSIS_THESIS.md` §8** against that audit. Replace "VERIFIED" with the honest status
   per row. Where a real number exists, use it and cite it — the classification accuracy
   (CORE domain 0.873 / cell 0.727, ITALIAN 0.867 / 0.800 on 79 fixtures under bge-base,
   `tests/classification.rs::accuracy_floor`) is a genuine measured result and should be the
   table's anchor. Delete the `1.088` "cosine" and the clamp-as-learning row outright; they are
   not weak evidence, they are wrong.
3. Fix `PHYSIS_THESIS.md:174` to a repo-relative path.
4. **Fix the mislabelled tests.** Rename `test_peculiarity_benchmark_comparison` to what it
   actually asserts (it is a composite-arithmetic test), or delete it and let PH-020's ladder be
   the comparison. Do the same for `test_1_contradiction_preservation` → it tests persistence, so
   name it that. Keep the tests; correct the names. A test whose name claims more than it asserts
   is a documentation defect that ships inside the binary.
5. **Re-point §8 at PH-020's output** once that packet lands. `physis-core/docs/EXPERIMENTS.md` —
   the 26-iteration record with its controls, its five exhausted signal families, and its verdicts —
   is what §8 should have been. Cite it. Where an iteration found a mechanism **not supported**,
   §8 must say so; a claim table that reports only the supported results repeats this packet's
   defect in a new place.
6. **Add a claim-drift guard.** `scripts/check_thesis_claims.sh`: fail if `PHYSIS_THESIS.md` or
   `README.md` contains "VERIFIED" (or a configurable claim-word list) without an adjacent
   `docs/CLAIMS.md` row, and fail on any `file:///` link in tracked markdown. Wire into CI beside
   the existing clippy gate. This is what stops the drift recurring.
7. Fix `physis-core/src/lib.rs:20` to list the dimensions the code has (coordinate with PH-017 —
   if PH-017 has landed, the correct list is seven).
8. Correct `AGENTS.md:12`: the build is not host-bound. Per repo rule, rewrite the comment to
   describe the new reason rather than deleting the note.

## Tests you must add

- **Script test** (`scripts/check_thesis_claims.sh` + a fixture): a markdown file containing
  "VERIFIED" with no CLAIMS.md row exits non-zero; one with a row exits 0. A file containing a
  `file:///` link exits non-zero.
- **Unit:** the renamed tests from step 4 must still pass under their honest names — the assertions
  do not change, only what they are called.
- **Doc test / integration:** assert `docs/CLAIMS.md` parses as the expected table shape, so a
  malformed row cannot silently disable the guard.
- A documentation fix without a guard against its recurrence is not done.

## Gate

```bash
cd ~/dev/physis-pro
cargo test --release --test epistemic_evolution
./scripts/check_thesis_claims.sh
cargo clippy --all-targets -- -D warnings
```

## Commit strategy

Separate commits: the claims audit (`docs/CLAIMS.md`); the thesis rewrite; the test renames; the
CI guard. The audit must land first — it is the evidence for every edit that follows, and a
reviewer needs to check the audit independently of the rewrite.

This packet touches customer-facing documents. A human reviews before merge, per PROTOCOL §7 — do
not self-approve.

## Report

Append `packets/reports/PH-022.md` per PROTOCOL §5. Include the counts: how many claims audited,
how many landed in each of the three statuses. That ratio is the honest headline state of the
project and belongs in the report.

## Never

- Never mark a claim "verified" on the strength of a test that constructs its own answer and
  asserts it was stored.
- Never present a single hand-built scenario as an empirical result.
- Never report a similarity above 1.0, or a clamp as a learning effect.
- Never downgrade a claim by deleting it silently — move it to `aspirational` in `docs/CLAIMS.md`
  so the history of what was claimed stays legible.
