---
id: PH-CORE-A4
title: hash-chained provenance with first-break reporting
status: review
phase: 2
depends: [PH-CORE-RETIREMENT]
claimed_by: opencode
claimed_at: 2026-09-20
---

# PH-CORE-A4

PLANNING.md §7.1 A4.

## Gate

```sh
cd physis-core
cargo test --test epistemic_p2     # ledger_tamper_breaks_chain_at_sequence
cargo test --lib
cargo clippy --lib --tests -- -D warnings
cargo check --all-targets
cd .. && cargo test --test api_stability   # snapshot blessed (additive)
```

## Execution

Branch `agent/PH-CORE-A4` @ `54b8a8d`. Report:
`packets/reports/PH-CORE-A4.md`.

## Follow-up

Expose `ChainIntegrity` on `explanation::ExplanationReport` so a report says
whether its own provenance chain verifies.
