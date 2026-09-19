---
id: PH-005
track: physis
phase: 1
status: done
rung: 3
depends: [PH-004]
gate: "cargo test --test http_surface && cargo test degraded"
commit: "branch agent/PH-005; design commit, then code, then tests"
---

# Close the confidence/accuracy inversion end-to-end

## Why
PH-004 makes the self-test honest. This packet makes every surface that
reports confidence PAY for accuracy: an embedder that is degraded must not be
allowed to answer customer queries as if it were fine, anywhere.

## Context
- After PH-004: /health knows the truth. Audit every OTHER surface: CLI output,
  MCP tools, any x-API-version header, marketing pages quoting SECOM (10 test
  rows — a liability, see 20-OPERATING-YEAR quarterly item).

## Steps
1. Enumerate every surface reporting embedder/semantic state (grep degraded,
   semantic, self_test, confidence). Table in the report.
2. Define one shared `EmbedderHealth` struct; every surface reads IT, not its
   own logic. Unit-test the struct.
3. Degraded embedder + query → response carries a visible warning field; CLI
   prints to stderr; MCP tool result includes it. HTTP 200 with warning, not
   silent success.
4. Remove/replace the SECOM citation wherever customer-facing.

## Tests you must add
- **Unit:** EmbedderHealth construction from each real embedder kind (bge-base,
  MiniLM fallback, random projection); warning field presence logic.
- **Integration:** HTTP surface — degraded embedder returns warning in every
  query-bearing endpoint sampled; healthy returns none.

## Gate
```bash
cargo clippy --all-targets -- -D warnings
cargo test
grep -rn 'SECOM' --include='*.html' . | wc -l   # must be 0 unless decision says keep
```

## Never
- Do not hide degraded state to make demos look better. That is the inversion
  winning.
