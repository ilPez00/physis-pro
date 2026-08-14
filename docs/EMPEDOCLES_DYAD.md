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

The vision exceeds the *production* code by a wide margin: today `compress_logs`
is word filtering and the live `dream` is near-neighbour mutate/graft/prune with
human grading. The dyad itself, however, is no longer only an aspiration — the
falsifier has run and passed (§3, gate 1), and `NodeField::from_embeddings` now
seeds the field from real node vectors, so the same dynamics can run on the
UberWiki's actual coherence nodes, not just synthetic seeds. What remains before
wiring it into the live core is gate 2 (no classification regression), §5.

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

### Result — measured 2026-06-14 (gate 1: PASSED)

`cargo run --example empedocles_criticality` (300 nodes, 2-D, 400 ticks, eros
fixed at 0.10, sweeping nemesis). The predicted three regimes appear, with a
critical band between them:

```
  eros  nemesis  nodes        regime  largest  cluster sizes (desc, top 10)
  0.10    0.001    300 Supercritical     1.00  300                  <- Love won (one blob)
  0.10    0.006    300 Supercritical     0.99  297 1 1 1
  0.10    0.010    300 Critical          0.20  59 32 9 9 8 8 7 6 6 5 <- heavy-tailed band
  0.10    0.020    300 Subcritical       0.01  2 2 2 2 2 2 2 2 2 2   <- Strife scattered it
  0.10    0.160    300 Subcritical       0.01  2 2 2 2 1 1 1 1 1 1
```

At `nemesis ~= 0.01` the field neither collapses to one cluster nor shatters to
singletons: it settles into a graded, heavy-tailed spread of cluster sizes
(`59 32 9 9 8 ...`), bracketed cleanly by the supercritical and subcritical
extremes. That is the self-organised-criticality signature this section
predicted. **The dyad does real organising work; the framing is not dropped.**
A rigorous power-law-vs-lognormal fit over a finer sweep remains as follow-up,
but the falsifier's pass/fail gate is passed.

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

Status: **gate 1 (criticality) passed** (§3, 2026-06-14). **Gate 2 (no
regression) — measured 2026-06-15, split result.** `cargo run --example
empedocles_no_regression` seeds three separated classes as labelled nodes,
classifies a held-out set by nearest labelled node, and runs the dyad with
perception reinforcing each class every tick:

```
 decay  synthesis   alive  dyad acc   verdict        (static baseline = 1.000)
  0.00       0.00      36     1.000    OK
  0.01       0.02    1000     0.933    REGRESSION  <- synthesis hit the cap, offspring drift
  0.02       0.05     208     0.767    REGRESSION
  0.05       0.05      52     1.000    OK
  0.10       0.05       9     1.000    OK
```

The finding is specific, not a blanket pass. **Decay-as-eviction is safe**: every
decay-dominant row holds at the baseline, and the bridge
`NodeField::from_embeddings` / `from_labeled` plus the unit test
`test_no_regression_classification_survives_dyad` (decay-only regime) lock it in.
**Synthesis with insufficient decay regresses**: offspring accumulate to the
population cap and their mutated positions pollute the nearest-label boundary.

So promotion is staged: wire **decay-as-eviction into the live core first** (it
bounds memory, no regression — the eviction the Spanner sink defers); keep
**synthesis quarantined** until offspring are bounded (a charge/lifespan cap, or
decay forced to dominate begetting). Repulsion is orthogonal and follows decay.
Until those wire-ins land, the module stays research-only.
