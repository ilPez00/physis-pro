# Sample output — soma (body machine)

`cargo run --example soma_demo`

## Output

```
soma — body machine
perceived 7 sessions

PER-SESSION (status → coherence):
  day 1  load 0.5 rec 0.9  →  Recovery (+0)
  day 2  load 0.7 rec 0.8  →  Effort (+1)
  day 3  load 0.3 rec 0.9  →  Recovery (+0)
  day 4  load 0.8 rec 0.6  →  Effort (+1)
  day 5  load 0.8 rec 0.5  →  Effort (+1)
  day 6  load 0.9 rec 0.4  →  Overreaching (-1)
  day 7  load 0.9 rec 0.3  →  Overreaching (-1)

DRIFT over last 6 sessions: +0.62
  ⚠ drifting toward overreaching — load up while recovery falls
```

## What it demonstrates

- **Primitives = episodes** (Effort / Recovery / Overreaching), each mapped to the
  engine's coherence rating (+1 / 0 / −1) — a protocol is Success only if it
  produced effect *without* overreaching.
- **Elaboration = drift** — load rising while recovery falls scores +0.62, surfacing
  the slide toward burnout *before* it lands (the "dream" beat for the body).
- Pairs with `register_behavioural_vector`; `register_sessions` bridges episodes
  into the shared graph.
