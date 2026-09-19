---
id: PH-006
track: physis
phase: 1
status: blocked
rung: 3
depends: [PH-001, PH-004]
gate: "cargo test pro && cargo test --test http_surface licence"
commit: "branch agent/PH-006; feature-flag commit, then enforcement, then tests"
---

# Make the Pro tier real: licence enforcement behind a feature flag

## Why
physis_pro currently has no code-level distinction from core. Until the Pro
tier gates something, it cannot be sold, and the licence story (PH-003) stays
theoretical.

## Context
- Depends on the PH-001 features decision and PH-003 licence decision. If
  either report lacks a human decision, this packet is blocked — write that
  in the report, do not guess.
- Constraint from PLAN.md H11: NO NEW SURFACE (routes/CLI/MCP) while the
  backlog is open. Gating existing surface is allowed; adding is not.

## Steps
1. Design doc first: what does Pro add? Candidates from existing code: pack
   (PH-007), multi-tenant admin routes, higher embedder registry slots, MCP
   surface. Pick a MINIMUM of one real, already-built capability to gate.
2. Implement a licence check module (reads LICENSE acceptance file/env at
   startup; test mode bypass). Unit-test every branch: missing, expired,
   malformed, valid.
3. Gate the chosen capability behind `#[cfg(feature = "pro")]` + runtime
   licence check. Default build behaves exactly as today.
4. Document the sales story in the report: what a buyer gets, what they run.

## Tests you must add
- **Unit:** licence module: valid/missing/expired/malformed/wrong-tenant cases.
- **Integration:** without feature → capability absent from surface enumeration
  (http_surface test asserting route count/shape unchanged); with feature but
  no licence → capability returns the licence error, not a crash.

## Gate
```bash
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --features pro
```

## Never
- Do not add new routes/tools for Pro. Gate what exists. H11 discipline.
- No licence-server phone-home (offline EULA acceptance is enough for now;
  anything else is a design packet for a human).
