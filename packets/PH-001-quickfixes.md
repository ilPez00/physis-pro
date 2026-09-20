---
id: PH-001
track: physis
phase: 0
status: review
rung: 1
depends: [PH-000]
gate: "cargo clippy --all-targets -- -D warnings && cargo test && ! grep -rn 'PHICS_LOG_LEVEL' .env* && ! grep -rn '\"0\.1\.1[0-9]\"' src/ --include=*.rs | grep -i version"
commit: "branch agent/PH-001; three separate commits, one per fix"
claimed_by: cline-feather-2026-09-11
claimed_at: 2026-09-11T11:55:00Z
---

# Three one-line drift fixes: version, env typo, feature set

## Why
Three independent facts drifted from their source of truth. Each one silently
misleads a user, a customer, or the operator.

## Context
1. `/health` reported `0.1.17` while `Cargo.toml` said `0.1.19+`. Read the
   version from `env!("CARGO_PKG_VERSION")` so it can never drift again.
2. `.env` sets `PHICS_LOG_LEVEL` — a typo nothing reads; the documented
   `PHYSIS_LOG_LEVEL` is therefore silently unconfigurable. Fix `.env` (and
   grep the whole repo incl. docs for both spellings; keep only the correct one).
3. The `default` feature set excludes `mcp` and `embed-onnx` — the two features
   carrying the value proposition. Changing this is a DECISION (rung 5), so:
   DO NOT change Cargo.toml. Instead write a consequence report (see Steps).

## Steps
1. Version: replace the hardcoded string with `env!("CARGO_PKG_VERSION")`.
2. Env typo: fix `.env` + any doc referencing `PHICS_LOG_LEVEL`.
3. Features: produce packets/reports/PH-001.md section "default features":
   what compiles today without mcp/embed-onnx, what breaks for a customer who
   enables them, binary size, build time. End with a recommendation. A human
   decides; that decision becomes PH-006 input.

## Tests you must add
- **Unit:** test that `/health`'s version field equals `env!("CARGO_PKG_VERSION")`
  (not a literal). In tests/, assert the struct or JSON field derives from the env.
- **Integration:** extend tests/http_surface.rs: /health returns 200, body has
  `version` == CARGO_PKG_VERSION, and a test that env `PHYSIS_LOG_LEVEL=debug`
  changes log behaviour (or at least is parsed without error).

## Gate
```bash
cargo clippy --all-targets -- -D warnings
cargo test
! grep -rn 'PHICS_LOG_LEVEL' . --include='*.env*' --include='*.md' 2>/dev/null
grep -rn 'CARGO_PKG_VERSION' src/ | grep -i health
```

## Commit strategy
Three commits: `fix(health): read version from CARGO_PKG_VERSION (PH-001)`,
`fix(env): PHICS_LOG_LEVEL typo → PHYSIS_LOG_LEVEL (PH-001)`,
`docs(packets): default-features consequence report (PH-001)`.

## Never
- Bump Cargo.toml version yourself (releases are packet PH-010's job).
