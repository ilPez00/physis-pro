# P10 criticality controls

Date: 2026-09-24 · Branch `agent/P-037-substrate`
Instrument: `research/papers/fit_empedocles.py::fit_sample` — **imported, not
reimplemented**, so the controls run byte-identical code to the Empedocles
verdict.
Artifacts: `research/p10_controls.json`, `research/p10_fss.json`
Reproduce: `python3 research/papers/p10_controls.py --seeds 12`

---

## Why this experiment exists

Paper 10 reports that the Empedoclean node field is not power-law distributed.
That conclusion is only informative if the same pipeline **can** detect a power
law when one is actually there. A test that cannot fire is not a falsification,
it is silence. So the instrument was run on systems where the answer is known
in advance.

Negative controls are not optional here either: an instrument that says
"power law" to everything would have produced exactly the same Empedocles
answer for uninteresting reasons.

---

## 4A/4B — positive and negative controls

12 seeds per control, 6000 clusters per sample, 160×160 lattice for
percolation. Sensitivity is the **fraction of seeds** on which the pipeline
returns "POWER LAW beats lognormal" — not one seed's verdict, because this
pipeline is already known (from Paper 10's own critical-band result) to be
seed-unstable.

### Positive controls (a power law is the truth, exponent known analytically)

| control | power law wins | median α | expected α |
|---|---|---|---|
| critical branching process (Poisson(1)) | **12/12** | **1.497** | 1.5 |
| 2D site percolation at p_c ≈ 0.5927 | 10/12 | 1.807 | 2.055 |

**The instrument is sensitive, and that part is a clean validation.** It
recovers the mean-field branching exponent to three decimals (1.497 vs 1.5) on
every seed. Percolation's exponent is biased low (1.807 vs 2.055) — the
expected direction for a finite lattice, where the asymptotic exponent is not
yet reached.

### Negative controls (no power law)

| control | power law wins | median α | truth |
|---|---|---|---|
| subcritical branching (mean 0.7) | **9/12** | 3.758 | exponential tail |
| supercritical branching (mean 1.3) | 0/12 | — | giant cluster |
| lognormal, σ = 0.6 | **0/12** | 2.130 | lognormal |
| lognormal, σ = 1.2 | **12/12** | 3.300 | lognormal |
| lognormal, σ = 2.0 | **12/12** | 2.157 | lognormal |

**The instrument is not specific, and the failure mode is systematic.** It
declares a power law on data that is *literally drawn from a lognormal* in
12/12 seeds once the lognormal is wide (σ ≥ 1.2), and on subcritical branching
— whose tail is exponential — in 9/12 seeds. It correctly stays quiet for the
narrow lognormal and for the supercritical case.

The width dependence is the tell. A wide lognormal is visually
power-law-like over a limited range, which is the classic Clauset–Shalizi–Newman
caveat. A second contributing cause is specific to this implementation: the
lognormal competitor is fitted with `scipy.stats.lognorm.fit` (continuous) and
compared against **integer** cluster sizes by the Vuong test, which biases the
comparison toward the power law.

**Verdict: instrument_sensitive = true, instrument_specific = false.**

**Process failure, recorded (same class as P1's).** Two runs of this script
raced one output path. The post-fix run (300 clusters per supercritical sample)
finished first and wrote its artifact; the pre-fix run (6000 clusters, slow)
finished later and overwrote it. The surviving artifact is therefore the
better-powered 6000-cluster run, and it is the one quoted above — but that was
luck of ordering, not design. The supercritical control also differs between
the two configurations: **0/12** at 6000 clusters versus **1/12** at 300. Both
count as "quiet" under the pre-registered rule (< 0.5), so the verdict is
unchanged, but the number is configuration-dependent and should be reported
with its sample size.

Two further fixes in the same class: the per-control header printed the global
default cluster count rather than the spec's actual value (so a 300-cluster
sample was logged as "6000 clusters each"), and the FSS sweep wrote its
artifact only at the very end, so every kill discarded the whole run. Both are
fixed; the FSS now writes partial output after each node count.

---

## What this does to Paper 10

**The negative result survives, and is strengthened.**

The Empedocles verdicts across the nine settings were: "no difference
(indistinguishable)" at five, and **"lognormal beats power law"** at 0.006,
0.04 and 0.08. It never once returned "POWER LAW beats lognormal".

Given that this instrument **over-fires in favour of the power law** — on wide
lognormals and on subcritical branching — its failure to fire on the
Empedoclean field is stronger evidence than the paper currently claims. A
biased-toward-criticality instrument reporting lognormal as the better fit in
three settings is a more decisive refutation than an unbiased one doing so.

Concretely, Paper 10 should:

1. Keep the refutation as written, and **add the sensitivity validation**
   (α recovered to 1.497 against a known 1.5) — this is a positive result for
   the instrument that the paper currently lacks.
2. State the **specificity failure** as a limitation of the *method*, and note
   that it makes the refutation conservative rather than fragile.
3. Not claim, anywhere, that "the pipeline prefers the power law" as evidence;
   the pipeline's positive verdicts in general are unreliable and should not
   be exported to other work. Within this portfolio the Empedocles paper is
   the only consumer of this pipeline, so the exposure is contained.

---

## 4C — finite-size scaling

Node counts swept 100 / 300 / 1000 / 3000 with a denser nemesis grid across the
region where the published nine points change character. Node count, grid and
seed count are read from the environment by the Rust example, whose defaults
and output format are unchanged (verified: default run still emits
`"nodes": 300` in the same schema that `fit_empedocles.py` parses).

Order parameters, and a correction worth recording:

- `largest_mean` — the spanning-cluster fraction, the order parameter itself.
- `chi_excl_mean` — susceptibility `Σ_{s≠largest} s²/N`, **excluding** the
  spanning cluster. The first version of this analysis included it, which makes
  χ trivially maximal (`χ = N`) as soon as the field fuses into one cluster:
  that locates the fragmented end point, not the transition. The standard
  percolation susceptibility drops the spanning cluster precisely for this
  reason.

**Result: all three node counts completed** (N=100 3.1 s, N=300 25.6 s,
N=1000 252.1 s; 8 seeds each; artifact `research/p10_fss.json`, diagnostics
recomputed without re-running the simulation). Two features appear at every
size:

| | N=100 | N=300 | N=1000 |
|---|---|---|---|
| largest-cluster fraction at nemesis 0 | 0.521 | 0.498 | 0.577 |
| clusters at nemesis 0 | 3.4 | 3.2 | 2.9 |
| full fusion (largest = 1.000) at | 0.002–0.004 | 0.002–0.004 | 0.002–0.004 |
| transition width | 0.008 | 0.010 | 0.080 |
| chi_max / N | 0.1077 | 0.1264 | 0.1036 |

1. **A sharp fusion step** between nemesis 0 and 0.002, identical at all three
   sizes: the field goes from ~3 medium clusters to one cluster holding
   everything.
2. **A fragmentation transition** between ~0.006 and ~0.02, where the largest
   cluster collapses.

**The verdict is: no critical scaling, and no transition even located.**

- **chi_max/N is constant** across a tenfold range in N (0.108, 0.126, 0.104).
  chi growing in proportion to N is the *trivial* signature of a fixed small
  number of O(N) clusters — with k clusters of size ~N/k,
  `chi_excl ≈ N(k-1)/k²`, so chi/N is constant. It is not critical scaling, and
  reporting the bare log-log slope (0.98) without chi/N would have invited
  exactly the wrong reading.
- **chi peaks at the edge of the nemesis grid** (nemesis = 0, the lowest value
  tested) at *every* node count. No critical point was found inside the sweep;
  chi merely rises monotonically to the boundary.
- **The transition WIDENS with N**: width 0.008 → 0.010 → 0.080, a log-log slope
  of **+1.01**, i.e. width ∝ N. A critical transition sharpens as N grows.

**An error of mine, and the most instructive one in this run.** The script's
first automated classifier reported the *opposite*: "chi peak grows with N and
the transition narrows: consistent with finite-size critical scaling". It was
wrong twice over. It read the chi ∝ N slope as a scaling signature with no
check that the peak lay inside the sweep, and its message said "narrows" as
fixed text while its own width slope was **positive** — the number and the
words disagreed, and the words were the flattering ones. A classifier that
prints a conclusion the data contradicts is worse than no classifier, because
it is quotable. Both defects are fixed: the sweep-boundary check and the
chi/N check now run before any scaling language is emitted, and the
narrowing/broadening branch is chosen by the sign of the width slope rather
than asserted. The corrected artifact carries `NO TRANSITION LOCATED`.

**A confound that would have to be resolved before any exponent could be
fitted.** `link_dist` is fixed at 0.15 while N varies in a fixed 2-D region, so
*density* grows with N and the effective coupling changes with system size. A
finite-size scaling analysis requires the interaction range or the density to be
held invariant; these runs do not do that. So even had a transition been
located, no exponent from this sweep would have been interpretable.

**LIMITATION.** **LIMITATION.** The 3000-node runs are ~10.4 s per (seed, nemesis) pair, so that
end of the sweep uses 8 seeds; the smallest N gets 16. Seed counts are
therefore not equal across sizes, which weakens the error bars at the largest N
exactly where they matter for a scaling collapse. A production run would
equalise them.

**BLOCKER (environmental).** Memory on this host is the binding constraint for
this whole program — swap is exhausted and long runs are OOM-killed (P2's
embedding arm is blocked by the same cause). P10 is the one experiment here
that survives it, because it needs no network, no embedder and negligible disk.
