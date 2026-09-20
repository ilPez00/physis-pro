---
id: PH-004
track: physis
phase: 0
status: review
claimed_by: opencode
claimed_at: 2026-09-20
rung: 3
depends: [PH-000]
gate: "cargo clippy --all-targets -- -D warnings && cargo test semantic_self_test && cargo run --bin physis-pro -- selftest 2>/dev/null; test $? -eq 0"
commit: "branch agent/PH-004; design doc commit, then implementation commits"
---

# The semantic self-test must be able to FAIL (confidence/accuracy guard)

## Why
The probe compares "a photograph of a dog" to "a photograph of a puppy" —
four of five tokens shared, so a bag-of-words hash PASSES it. The
random-projection embedder passes; /health then reports `"semantic": true,
"degraded": false` while the CLI warns the embedder is non-semantic. In
industrial QA this is the worst failure shape: more confident, no more correct.

## Steps
1. RUNG 3 first — write a DESIGN doc (packets/reports/PH-004-design.md) that
   specifies a discriminating self-test. Requirements: must FAIL for
   random projection and any bag-of-words baseline; must PASS for bge-base;
   deterministic; < 2 s on CPU. Suggested shape: paraphrase pairs (high sim),
   unrelated pairs (low sim), negation pairs ("X is present" vs "X is
   absent" — moderate sim, NOT near-duplicate), plus a margin threshold.
2. Implement it as the new self-test; wire its result into /health's
   `semantic` and `self_test` fields so they can finally lie FALSE.
3. Make the random-projection fallback produce `"semantic": false` always
   (it is honest today only by accident).

## Tests you must add
- **Unit:** the self-test itself, run against (a) random projection → must
  fail, (b) MiniLM flat fallback → must pass with real weights, skipped if
  weights absent (assert the SKIP is visible).
- **Integration:** /health with random projection reports degraded/semantic
  false; with bge-base reports true. tests/http_surface.rs.
- **Regression:** a test asserting the OLD probe (dog/puppy) is no longer
  the sole criterion.

## Gate
```bash
cargo clippy --all-targets -- -D warnings
cargo test semantic
cargo test --test http_surface health
```

## Never
- Do not make the self-test flaky (no network, no model downloads at test time).
- Do not change the embedding space (that is PH-008's cousin, and it orphans nodes).
