# Sample output — harmony (music machine)

`cargo run --example harmony_demo`

## Output

```
harmony — music machine
perceived 9 notes

INTERVALS (key-invariant shape): [2, 2, -1, 2, 2, 2, -1, -1]

MOTIFS (recurring interval runs, len 3):
  [2, 2, -1]  ×2

VARIATIONS of the opening motif [60,62,64]:
  transpose +5: [65, 67, 69]
  invert:       [60, 58, 56]
```

## What it demonstrates

- **Primitive = interval, not note** — the melody reduces to a key-invariant
  interval shape (the elaborable unit; raw notes are exhausted leaves, like
  euclid's banned circles).
- **Composition** — recurring interval runs surface as a motif (`[2,2,-1] ×2`),
  found regardless of key.
- **Elaboration = variation** — transpose / invert generate the same shape
  transformed; memory of motifs deepens a personal style.
- `register_motifs` bridges motifs into the shared graph.
