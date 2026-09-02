---
id: PH-007
track: physis
phase: 1
status: todo
rung: 1
depends: [PH-000]
gate: "bash scripts/bench-pack.sh --assert-warm-under 30s --assert-cold-under 600s && cargo test pack"
commit: "branch agent/PH-007; harness commit, then thresholds commit"
---

# scripts/bench-pack.sh — make `pack` demonstrable with numbers

## Why
pack (cold export / warm reuse of a physis store) is a headline capability but
has no measured numbers. The script referenced in the backlog does not exist.
Writing it is the first work packet.

## Steps
1. Build scripts/bench-pack.sh: creates a synthetic store (N documents,
   configurable, default 1k), measures (a) cold pack (fresh process),
   (b) warm pack (store unchanged since last pack — cache path), (c) pack
   integrity (re-import into a fresh dir, node count + a hash of sorted node
   ids must match). Output: a machine-readable line `PACK_BENCH cold=Ws warm=Ws
   nodes=N` appended to bench/bench-pack.results (gitignored) and human table.
2. Flags: --assert-warm-under / --assert-cold-under exit non-zero if missed.
3. Record today's numbers in packets/reports/PH-007.md on this machine AND
   note the machine spec (mesh capability output snippet).

## Tests you must add
- **Integration:** cargo test pack — pack → fresh import → equality of node
  set (likely exists; strengthen if weak: assert embeddings preserved bit-exact
  for a fixed fixture).
- **Shell:** bench script itself is testable with N=10 and generous thresholds;
  that invocation is the test.

## Gate
```bash
bash scripts/bench-pack.sh --nodes 10 --assert-warm-under 5s --assert-cold-under 60s
cargo test pack
git check-ignore bench/bench-pack.results   # must be ignored
```

## Never
- Commit benchmark result files. Commit the HARNESS, not the numbers.
