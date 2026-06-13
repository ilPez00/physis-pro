# The Empedoclean Dyad — Eros, Nemesis, and the Metabolism of Physis

> Design note, 2026-06-13. A conceptual model for making the Physis engine
> behave less like a classifier and more like a living thing — and a
> **falsifiable experiment** to test whether the model does real work or is
> merely elegant.
>
> **Scope discipline (PREMORTEM P2):** this is a *research subsystem*. It is
> isolated in `src/empedocles.rs` + `examples/empedocles_criticality.rs` and
> touches **nothing** on the production classify / shift-report / web path. It
> does not improve the shippable product; it explores whether the engine's
> coherence/dream/memory core can self-organize. Keep it quarantined until a
> measurement justifies promoting any part of it.

---

## 1. The two forces

Empedocles (already a reducer in this repo) built his cosmos from two forces,
not four elements: **Φιλότης / Philotes (Love)** combines, **Νεῖκος / Neikos
(Strife)** separates. All Love → the Sphere, one undifferentiated blob: maximum
unity, **zero information**. All Strife → total atomization: maximum distinction,
**zero structure**. Reality is the *oscillation* between them. Life sits at the
metastable edge.

The user's intuition was Eros≈affinity, Nemesis≈coherence. One half is right,
one is a category error:

| Force | User's map | Verdict |
|---|---|---|
| **Eros** (Philotes) | affinity between semiotics | **Right, but sterile.** Cosine attraction groups like-with-like. Nature's Eros *generates* — it unites unlike things to birth a novel third. The engine has the gravitational half and no offspring. |
| **Nemesis** (Neikos) | coherence (+1/0/−1) | **Mis-map.** Coherence is a *judgment* (closer to *aletheia*, disclosure of truth). Nemesis is *retribution against excess* — she cuts down what grows too high, restores balance. That force is **absent**: it would be active **repulsion + decay**, the thing that stops one cell from dominating. The historical "BOND-hub bias" bug was a system with no Nemesis. |

So the corrected dyad:

- **Eros = attraction + generative synthesis.** Pull coherent nodes together;
  when two *distant-but-coherent* nodes meet, birth an offspring (blended
  vector + mutation). Synthesis is how the field invents new ontology from
  within — the same mechanism `discovery` runs from outside.
- **Nemesis = repulsion + decay.** Over-clustered nodes push apart (maintain
  spread, prevent collapse). Unreinforced nodes fade and die. Mortality is
  load-bearing: forgetting bounds memory, apoptosis sculpts form, niche
  pressure forces differentiation.

These are not two scalars. They are **one coupled dynamical system**.

---

## 2. Dream and compression are the two phases of one metabolism

The framing's real payoff: it dissolves "we have a `compress()` function and a
`dream()` function" into a single governed cycle.

- **Waking / compression = Neikos (Strife):** separate signal from noise,
  discriminate, gate by consistency. *Catabolism* — break the input flood down.
- **Rest / dream = Philotes (Love):** replay, recombine, abstract, synthesize,
  prune. *Anabolism* — build structure from what was kept.

The system *breathes* between separation and integration. That is metabolism,
the sleep/wake cycle, and Empedocles' cosmic oscillation at the scale of one
mind. The ratio of the two phases is the **same criticality tunable** as the
attraction/repulsion balance: too much waking → hoard noise, never consolidate;
too much dreaming → confabulate without grounding.

### What is wrong with each today

**Compression** is two operations under one name. `filter_context` (embed +
consistency gate) is sound — keep it. `compress_logs` (drop words under three
characters into "rules") is string-munging, "compression" in name only. The
deeper flaw: it is **lossy-by-deletion** (throw tokens away). Nature's is
**lossy-by-abstraction** — the hippocampus extracts the reusable *schema* and
lets episodic particulars decay (systems consolidation). Compression should
converge repeated episodes onto an invariant centroid and let specifics fade —
which is **the same mechanism as Nemesis-decay**. Compression and forgetting are
one operation seen twice.

**Dream** is the strongest piece conceptually (offline replay + counterfactual
recombination is real sleep neuroscience) but narrow:

1. It only dreams *failures*; real replay also consolidates *successes*.
2. It recombines *near-neighbour*; real dreams graft *distant* traces — that is
   where novelty comes from (Eros-generativity).
3. Selection is *exogenous* (a human grades); endogenous selection — variants
   that raise *global* coherence survive — closes the loop.

### The honest gap

The vision exceeds the code by a wide margin. Today `compress_logs` is word
filtering and `dream` is near-neighbour mutate/graft/prune with human grading.
This document describes an aspiration and a test of it, **not** a shipped system.

---

## 3. The criticality thesis (why this is measurable, not just poetry)

"Mimic nature" reduces to a concrete target. A node field under pure attraction
collapses to one giant cluster (supercritical, zero information). Under pure
repulsion it scatters to singletons (subcritical, zero structure). Between them
lies a critical band where mutual information / representational capacity is
**maximised** — self-organised criticality, the edge of chaos.

The fingerprint of criticality is a **power-law cluster-size distribution**
(scale-free), versus an exponential tail (subcritical) or one dominant cluster
(supercritical). So the dyad makes a falsifiable prediction:

> Sweep the attraction/repulsion(/decay) ratio. The extremes give one-giant-
> cluster and all-singletons. **If and only if** a middle band yields a
> power-law cluster-size distribution does the Empedoclean dyad do real
> organising work. If no such band exists, the framing is poetry and we drop it.

---

## 4. The model — `NodeField`

A minimal, pure, deterministic-seeded field (`src/empedocles.rs`). Nodes carry a
position vector and a charge. One tick applies four forces:

- **Attraction (Eros):** each node drifts toward neighbours within `eros_radius`,
  scaled by proximity and the neighbour's charge.
- **Repulsion (Nemesis):** each node is pushed off neighbours inside the closer
  `nemesis_radius`, inverse-square — prevents collapse.
- **Decay (Nemesis):** `charge *= 1 − decay`; nodes under `death_charge` die.
- **Synthesis (Eros):** two high-charge nodes in the attraction band (but not
  collapsed) may birth an offspring at their midpoint + mutation, inheriting
  blended charge; bounded by `max_nodes`.

Measurement: connected-component clustering at a distance threshold →
cluster-size histogram → power-law check (the experiment lives in
`examples/empedocles_criticality.rs`, which sweeps the ratio and prints the
distribution per setting).

---

## 5. What "better" means here

Not "higher classification accuracy" — this does not touch that. *Better* means:

- **Conceptual coherence:** dream and compression stop being ad-hoc utilities
  and become two necessary phases of one principled cycle.
- **Bounded, self-sculpting memory:** decay solves the unbounded-growth problem
  deferred in the Spanner sink (eviction becomes apoptosis, not a cron job).
- **Endogenous novelty:** synthesis births candidate ontology entries from
  within, complementing `discovery`'s from-corpus proposals.

Promotion criterion: only wire a mechanism (decay, repulsion, synthesis) into
the live coherence core **after** the criticality experiment shows it does
organising work *and* a separate test shows it does not regress classification.
Until then it stays in the research module.
