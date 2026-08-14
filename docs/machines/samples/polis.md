# Sample output — polis (social-graph machine)

`cargo run --example polis_demo`

## Output

```
polis — social-graph machine
perceived 8 interactions

TIES (strength · reciprocity):
  ana –you   strength 17.0  recip 0.89
  ben –you   strength  8.0  recip 0.14
  ana –ben   strength  8.0  recip 1.00
  cleo–you   strength  4.0  recip 1.00

STRENGTHENING (strong + mutual):
  ✓ ana–you
  ✓ ana–ben

FADING (one-sided — tend before it lapses):
  ⚠ ben–you  (recip 0.14)
```

## What it demonstrates

- **Primitive = tie** with a *reciprocity* (min/max of the two directions) — a
  relationship is coherent when affinity flows both ways.
- **Composition** — directed interactions aggregate into an undirected weighted graph.
- **Elaboration** — strengthening (strong + mutual) vs **fading** (`ben–you`,
  recip 0.14 — you carry it) surfaces the tie to tend *before* it lapses.
- `register_ties` bridges people as nodes with coherence-weighted edges
  (strength × reciprocity).
