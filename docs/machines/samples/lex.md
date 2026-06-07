# Sample output — lex (contract machine)

`cargo run --example lex_demo`

## Output

```
lex — contract machine
perceived 5 clauses

ALWAYS DUE (unconditional obligations): ["deliver"]

BREACH PATH if `delivered` fails: ["pay"]
UNHANDLED CONTINGENCIES (required but undefined): ["registered", "returned"]
```

## What it demonstrates

- **Primitives** — obligation / right / condition, with `requires` dependencies.
- **Composition** — clauses form a dependency graph.
- **Elaboration = breach simulation** — fail `delivered` and `pay` becomes
  unsatisfiable (the breach path).
- **The "dream" beat** — `registered` and `returned` are *required* but never
  *defined* → unhandled contingencies surfaced (the gap that bites later).
- Pairs with `ingest` (doc → clauses); `register_clauses` bridges them + edges.
