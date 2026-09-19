---
id: PH-012
track: physis
phase: 2
status: done
rung: 3
depends: [PH-010]
gate: "cargo test --test api_stability && cargo public-api --diff 2>/dev/null || cargo test api"
commit: "branch agent/PH-012; API inventory commit, then snapshot test commit"
---

# physis-core public API: inventory, snapshot, and stability gate

## Why
physis-core is the piece a customer embeds. Today nothing prevents a refactor
from silently breaking the public surface. If the core's API is not pinned, it
cannot be a dependency — and licensing it Apache-2.0 (PH-003) promises
exactly that.

## Steps
1. RUNG 3 design: inventory the intended public API of physis-core
   (cargo public-api if installable on pansa, else doc-grep + mod.rs audit).
   The report proposes the frozen surface; human blesses it.
2. Write tests/api_stability.rs: a snapshot test enumerating public items
   (names + signatures). Any diff fails with a printed diff and instructions
   to bless the change deliberately.
3. Add doc(no) hidden-items audit: anything public that should not be, gets
   #[doc(hidden)] + an issue entry, not silent acceptance.
4. Semver rule goes into CONTRIBUTING or AGENTS.md: breaking the frozen
   surface requires a minor+1 (0.x) bump and a CHANGELOG 'BREAKING' entry.

## Tests you must add
- The snapshot test IS the deliverable. Prove it works: add a dummy pub fn,
  run, see it fail, remove, see it pass (record both outputs in the report).

## Gate
```bash
cargo test --test api_stability
cargo clippy --all-targets -- -D warnings
```

## Never
- Bless your own API diff. Deliberate changes go back to a human.
