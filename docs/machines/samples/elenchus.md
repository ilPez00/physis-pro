# Sample output — elenchus (dialogue machine)

`cargo run --example elenchus_demo`. (Socratic cross-examination — not "agora".)

## Output

```
elenchus — dialogue machine
perceived 6 statements

CLAIM STANDING:
  c1  Supported    Remote work raises output
  c2  Contested    Open offices boost collaboration
  c3  Unsupported  Four-day weeks are universally better

PRESS (unsupported — ask for a warrant): ["c3"]
RECALIBRATE (contested by a rebuttal):   ["c2"]
```

## What it demonstrates

- **Toulmin primitives** — statements reduce to claim / warrant / rebuttal.
- **Standing = coherence** — warranted+unrefuted `c1` → Supported (+1); rebutted
  `c2` → Contested (−1, a ConstructiveRefutation); bare `c3` → Unsupported (0).
- **Elaboration = the next move** — surfaces what to *press* (unsupported claims)
  and what to *recalibrate* (contested claims) — a debate partner that finds the
  weak joint.
- `register_argument` bridges statements as nodes with support/refute edges.
