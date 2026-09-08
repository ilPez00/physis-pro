# E10 — Form through selective error

`research/mapper/run_e10.py`

> Some organized systems maintain form not by eliminating every deviation, but by
> discriminating between deviations according to their consequences: those that
> prevent further viable composition are abandoned, while non-handicapping ones
> may persist and become part of the evolving form.

## What this can and cannot settle

It **cannot** establish that music and DNA are genuinely congruent. That is a
claim about two domains and no amount of code decides it. What it **can** test is
the abstraction the brief actually proposes — *selection by downstream
compositional viability versus selection by local correctness* — in a system
where the answer is known by construction. If the abstraction loses here, the
cross-domain framing does not rescue it.

## The system (§2, §7, §8 made computable)

Music is the brief's own test case and the one that formalises without argument,
because it separates identity from attributes cleanly: **a motif's identity is
its interval contour, not its pitches.**

```
O = (I, A, R, C)
  I  interval contour   — the invariant
  A  pitch realisation  — the attributes; register lives here
  R  adjacency constraint to neighbouring motifs
  C  position in the phrase
```

**§8's condition holds by construction.** A register transformation is a
transposition, and `I(Tr(O)) = I(O)` with `A(Tr(O)) ≠ A(O)` for 24/24 motifs.
That is a definition holding, not an empirical finding, and it is stated so it
cannot be mistaken for one. The formalism in §8 is coherent for this system.

Operational definitions kept distinct per §18 — deviation, error (breaks I),
neutral (preserves I and viability), handicap (preserves I, reduces viability),
correction, selection, noise — are in the source header.

## Results — 40 replicates × 240 generations

| arm | depth | identity | diversity | corrections | neutral used | downstream |
|---|---:|---:|---:|---:|---:|---:|
| A reject every deviation | **172** | n/a | **171** | 240 | 0 | 0.00 |
| B accept every deviation | 137 | 0.13 | 123 | 132 | 5 | 2.16 |
| C select on **local** correctness | 132 | 0.13 | 125 | 181 | 7 | 5.67 |
| D select on **downstream** viability | 126 | **0.15** | 119 | 187 | **8** | 5.99 |

*(Arm A's identity rate is **undefined**, not zero: it accepts no deviations, so
the denominator is empty. Printed as 0.00 by the harness; do not read it as a
score.)*

### Downstream vs local — the comparison the hypothesis rests on

| measure | local | downstream | Δ |
|---|---:|---:|---:|
| identity preservation | 0.13 | 0.15 | **+0.01** |
| neutral variation exploited | 7.00 | 7.70 | **+0.70** |
| compositional depth | 132.3 | 126.5 | **−5.80** |
| diversity | 124.9 | 118.7 | **−6.20** |
| corrections spent | 181.1 | 186.5 | **+5.40** |

**Selecting on downstream viability buys about one percentage point of identity
preservation and costs compositional depth, diversity, and extra corrections.**
On this instantiation the trade is bad.

## Falsification (§19)

Two of the brief's own falsification targets are met here:

- **"downstream viability is worse than immediate correctness"** — yes, on depth
  (−5.8) and diversity (−6.2), for +0.01 identity.
- **"allowing neutral variation causes uncontrolled combinatorial growth"** — no.
  Accept-everything reaches diversity 123 against downstream's 119; growth is not
  the failure mode. The hypothesis fails for a different reason than predicted.

And an unplanned result worth keeping: **arm A, which rejects every deviation,
achieves the greatest depth and diversity.** Its correction fallback is a small
transposition of the parent — which preserves I by construction and is always
legal. So "reject everything novel and shift register instead" outperforms every
selective policy on the composite measures.

**That is partly an artifact and is flagged as one:** the fallback is itself a
productive move, so arm A is not really conservative — it is "always apply the
safe register transformation." But the artifact carries a real reading. In this
system the deviation that reliably preserves identity while changing attributes
is *register*, exactly as §7 proposes, and the exotic deviations (ornament,
interval perturbation) mostly break the invariant. The register insight survives;
the selection mechanism does not add to it.

## The honest limit of this test

**The mechanism the hypothesis is about barely fires.** Neutral variation —
deviations that preserve I *and* stay viable — occurs 5 to 8 times in 240
generations, under 3%. The arm-level comparison had power (pre-flight MDE 0.068
against a claimed 0.10), but the *within-system* rate of the phenomenon is so low
that this design cannot strongly test it either way.

Two changes would make a real test: a deviation set where identity-preserving
moves are common rather than rare, and a viability horizon longer than six steps
so that "prevents further viable composition" can actually discriminate.

## Verdict

```
SELECTIVE-ERROR HYPOTHESIS: NOT SUPPORTED IN THIS INSTANTIATION
```

Specifically: **selection by downstream compositional viability did not beat
selection by local correctness** — +0.01 identity for −5.8 depth and −6.2
diversity. The broader principle ("systems maintain form by regulating which
changes persist") is **not** refuted by this; it was tested through one
mechanism, in one system, where the mechanism was rare.

**What did survive:** §8's register formalism is coherent and its condition holds
by construction; and in this system register transformation is empirically the
productive deviation, which is the part of the framework with support.

## A note on the process

The first run of this experiment produced an all-zeros identity column and two
byte-identical arms. Three faults, all mine:

1. `dev_transpose` drew a **fresh random offset per note**, scrambling the
   contour instead of transposing it. No deviation ever preserved the invariant,
   so the neutral-variation axis — the thing the hypothesis is about — was empty.
2. The downstream filter never bound: extension almost always succeeded (mean
   6/6), so arms C and D were identical.
3. **The pre-flight returned STOP on power and the first run proceeded anyway** —
   exactly the failure the checker was built to prevent. The replicate count is
   now raised until the check passes, and the check is re-run to confirm.

The third is the one worth recording. A checker that gets overridden by the
person who built it is not a control.
