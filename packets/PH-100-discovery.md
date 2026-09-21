---
id: PH-100
track: physis
phase: D
status: claimed
claimed_by: kiro
claimed_at: "2026-09-21T14:50:00+02:00"
rung: 3
depends: [PH-000]
gate: "test -f packets/PH-1xx-proposals.md && test $(grep -c '^## PH-' packets/PH-1xx-proposals.md) -ge 3"
commit: "branch agent/PH-100; proposals commit (docs only)"
---

# DISCOVERY: write the next quarter's physis packets

## Why
This skeleton cannot see the future. Whoever runs it learns things the author
did not know. Discovery packets convert that learning into new packets so the
skeleton grows instead of rotting.

## Steps
1. Sources: packets/reports/* unknowns sections; `mesh mem traps` (physis
   scope); PLAN.md hardening backlog items NOT yet covered here (H-numbers);
   the last 20 git log subjects; customer/ops packets (~/ops) promising
   things physis must deliver.
2. Write packets/PH-1xx-proposals.md with ≥3 proposals, each in full packet
   format (frontmatter + Why/Context/Steps/Tests/Gate/Never), numbered
   PH-101.. onward, phase 1 or 2.
3. Mark each rung honestly. Anything with a human decision gets rung 3/5 and
   says WHAT must be decided.
4. A human (or strong reviewer) copies accepted proposals into real packet
   files. The agent does NOT number-collide: check existing ids first.

## Tests you must add
- Each proposal must contain a gate that could actually run (commands that
  exist). A proposal whose gate references a nonexistent script names the
  packet that must create it as a dependency.

## Gate
```bash
test -f packets/PH-1xx-proposals.md
grep -c '^## PH-' packets/PH-1xx-proposals.md   # >= 3
```

## Never
- Propose new surface (routes/tools) while the H11 constraint stands. Propose
  removal, hardening, measurement, or enablement of what exists.
