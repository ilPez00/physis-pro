# E19b — rebuilding the controls E19 leaned on

**Status: PRE-REGISTERED 2026-09-09, before any B5b call.** Everything above the
`RESULTS` divider was written and committed first. House rule 1 applies to the
*instrument* before it applies to the result.

E19's verdict (cross-domain congruence NOT SUPPORTED) is not under revision.
**Its stated reason is.** E19 called `S5` "the decisive test" and "the
construction-matched control". S5 is neither, and this experiment builds the
control E19 believed it already had.

---

## 1. What is wrong with S5

S5 was described as *"C5's exact tokens, congruence destroyed by permuting
bodies across settings."* The permutation is a cyclic shift of the five
paragraph bodies against five identical headings:

```
S5 body order, as indices into C5:  [1, 2, 3, 4, 0]
C5 body set == S5 body set          True   (byte-identical, all five)
headings C5 == headings S5          True
tf-cosine(S5, C5)                   1.000
```

**The five relational bodies are unchanged, so the structural congruence among
them is unchanged.** What the shift destroys is the pairing between a heading
and the story under it — and the generation prompt instructs the model to *"read
the material as evidence about relational STRUCTURE, not about biology, markets
or language"* and to *"not name trees, fungi, markets, cells or languages."* The
headings are the one part of the stimulus the task tells the model to discard.

So `C5 ≈ S5` is close to a tautology. It measures the instrument's
seed-to-seed noise between two near-identical stimuli — which is worth knowing,
and is in fact large (S5 and C5 differ by 0.42 in the C1 false-accept column on
identical content) — but it is **not** evidence that congruence does no work.
E19's headline sentence *"no better than the same five domains with their
congruence destroyed"* is withdrawn.

**What survives untouched.** The valid tests of the brief already in hand are
`C5 vs A1` (*t* = +0.89) and `C5 vs A5` (*t* = +0.24, one domain at C5's token
count). Both are null, both are inside the instrument band, and A5 is a genuine
control for the text-quantity confound. **E19's verdict rests on these, and they
are unaffected.** The differentiation-clause finding — C4 is the worst column
for every arm including ORACLE, and 4 of 5 C5 schemas never state it — is a
within-arm reading of the generated schemas and is likewise unaffected.

## 2. What is wrong with B5

Recorded in E19 already: B5's five domains all turned out to be asymmetric
one-way relations, and the model abstracted exactly that (`actor / resource /
consequence`). B5 measures *a different shared structure*, not *no shared
structure*. Its 0.22 positive acceptance is a successful abstraction of the
wrong thing. Second defect, not previously recorded: B5 is **479 words against
C5's 419**, so it is not token-matched either.

## 3. B5b — one arm that fixes both

`B5b` reuses **C5's own five settings** — tree and fungus, two neighbouring
trees, lymphocytes, firms, phonemes — and writes each as a *different*
relational structure. Surface matched, structure heterogeneous.

| domain | structure instantiated | why it is not T, and not the others |
|---|---|---|
| 1 fungus / dying tree | asymmetric consumption | no reciprocity (¬C1); the tree ceases to be individuable (¬C3) |
| 2 two ridge trees | common cause | no coupling at all (¬C1); the wind makes them **alike** (¬C4) |
| 3 lymphocyte counts / index | part-whole aggregation | no interaction between parts; a whole that *represents* them (¬C5 clause) |
| 4 firm / imitating firm | diachronic transmission | one-way copying, the source unaltered, no standing relation |
| 5 phoneme / spelling | fixed complementarity | pre-stipulated fit, neither party modified (¬C2) |

No two share a higher-order invariant: 1 is destructive and asymmetric, 2 has no
coupling, 3 is arithmetic rather than relational, 4 is a lineage rather than a
relation, 5 involves no change in either party.

Token match: **B5b 427 words / 2352 chars against C5's 419 / 2375** (B5 was 479 /
2833). Lexical proximity to C5 rises from B5's tf-cos 0.293 to **0.479**, against
A5's 0.515 — i.e. B5b is about as surface-close to C5 as C5's own domain is.

Probes, judge, prompts, seeds and scoring are unchanged. Only `stimuli.json`
gains an arm.

## 4. Pre-registered, before the first B5b call

**The design's own falsifier, checked first.** Read all five B5b schemas. If
they converge on one shared invariant the way B5 converged on
`actor / resource / consequence`, **B5b is broken in the same way as B5 and must
not be cited** — the arm fails, whatever its kappa. This is checked before any
between-arm comparison, exactly as H0c is in E19.

Then, given the arm survives:

- **P1 — the brief is right.** C5 beats B5b by more than the instrument's
  NULL→ORACLE band (0.250 kappa). This is the comparison E19 thought S5 gave it
  and did not.
- **P0 — E19's multiplicity account.** B5b shows the same permissiveness jump as
  C5 relative to A1 (POS acceptance ≈ 0.95+, NEG acceptance ≈ 0.50), and kappa
  is flat between them. Five items generalise the schema wider whether or not
  the items share structure.
- **P0′ — B5b lands near NULL, not near B5.** Genuinely heterogeneous evidence
  should push the model toward a vacuous schema (high acceptance in *both*
  classes, kappa → 0), not toward B5's over-narrow one (acceptance 0.22). If
  B5b instead reproduces B5's low acceptance, the model has again found a shared
  invariant and the falsifier above should have caught it.

**P0 is the prediction of record.** E19 already showed item count, not text
volume, drives acceptance; B5b holds item count at five and varies only
congruence, which is the manipulation S5 was supposed to perform and did not.

**Stated in advance so it cannot be relitigated afterwards:** if P1 holds, E19's
verdict is overturned and the brief's central claim is supported at 4B. That
outcome is live. S5's failure as a control means the congruence manipulation has
never actually been run in this track.

---
## RESULTS

## 5. B5b's falsifier fired. The arm is not cited.

The pre-registered check in §4 was *"read all five B5b schemas; if they converge
on one shared invariant the way B5 converged on `actor / resource / consequence`,
B5b is broken in the same way and must not be cited."*

**It converged, at both model scales, on ¬C1 — the absence of mutual influence.**

`qwen2.5:7b-instruct` (omo, 5 seeds):

| seed | schema name | the shared invariant, in the schema's own words |
|---|---|---|
| 0 | Passive Influence Structure | "the relationship … is one-way, with no mutual interaction" |
| 1 | Interdependent Components | "interact without direct reciprocal action" |
| 2 | Structural Absence and Indirect Influence | excludes "systems where interactions are reciprocal or bidirectional" |
| 3 | Candidate System for Relational Structure | "neither actively responds to changes initiated by the other" |
| 4 | Unilateral Non-interaction Pattern | "no direct feedback loop between entities" |

`qwen3:4b` (feather): all four schemas are named **"Relational Structure
Pattern"** and every one excludes mutual influence — seed 0 "systems where
entities directly influence each other's properties", seed 2 "systems where the
abstraction results from direct interaction between relational entities", seed 3
"the Target does not respond to or influence the Agent".

**B5b is therefore not cited, and no C5-vs-B5b comparison is reported.** P1 is
not adjudicated. The verdict of E19 is unchanged and continues to rest on A1
and A5.

### Why this is a result and not a second careless design

B5 and B5b were designed independently, months apart in reasoning, to have "no
shared invariant". Both collapsed onto the *same* one: **the five domains all
lacked mutual coupling.**

That is structural, not accidental. T requires coupling (C1). The cheapest way
to write a domain that is *not* T is to write one where the two entities do not
act on each other — consumption, common cause, aggregation, transmission,
stipulated fit are all such cases, and they were five of five in B5b. **The
negation space of T is dominated by absence of coupling**, so a set assembled to
be "structurally unrelated" drifts into sharing ¬C1 unless coupling is held
fixed on purpose.

So the finding is about the control, and it is worth more than the arm would
have been: *a "no shared structure" arm may not be constructible by free
choice.* Any five relational descriptions share something; the design question
is only whether what they share is more specific than the trivial universal.

## 6. B5c — pre-registered 2026-09-09, before any B5c call

Written and committed before the first call, as §4 was.

**The fix: hold C1 true in all five, and vary which of the remaining clauses
fails.** Then the set cannot collapse onto shared non-coupling, because every
domain has coupling.

| domain | coupling? | clause that fails | how |
|---|---|---|---|
| 1 tree root / fungus | yes | **¬C3** invariance | tissues interleave into one organ; neither can be lifted out again |
| 2 two trees | yes | **¬C4** differentiation | root-grafted, buffered, forms *converge* |
| 3 lymphocyte / antigen | yes | **¬C2** modification | bind and release; both exactly as they were, nothing transferred |
| 4 two firms | yes | **¬C5** ensemble | a closed duopoly: the arrangement is between the two and nothing holds above it |
| 5 two phonemes | yes | **¬C5** ensemble, differently | the spacing is published and revised at will by one academy |

Token match: **B5c 431 words / 2333 chars against C5's 419 / 2375.**

### The falsifier, restated for what is now achievable

B5c's five domains **will** share mutual coupling — that is the point of the
design, and it is what T's own C1 asserts. So the check cannot be "share
nothing":

> **B5c fails if the five schemas converge on an invariant MORE SPECIFIC than
> mutual coupling** — that is, if they agree on any of modification, invariance,
> differentiation or ensemble order. Converging on "two things that mutually
> affect each other" and nothing further is the *intended* outcome: it is §17's
> trivial universal, which is exactly what a congruence control should leave the
> model holding.

**Pre-registered predictions.**

- **P1c — the brief is right.** C5 beats B5c by more than the instrument band.
  Congruence around the *full* structure T beats congruence around the *trivial*
  part of it.
- **P0c — E19's account.** C5 ≈ B5c: five items produce a permissive schema
  whether the shared structure is T or only coupling, and kappa is flat between
  them.

**What B5c can and cannot settle.** It compares specific congruence against
trivial congruence — not against *no* congruence, which §5 argues may be
unconstructible. That is a narrower question than the brief poses, and it is
stated here so it is not later reported as the broader one.

**Third attempt, and every attempt is on the record.** B5 was confounded, B5b's
falsifier fired, B5c is the third design of the same control. Recorded in full
rather than presented as if the third had been the first, because iterating
controls until one yields a publishable comparison is precisely how a garden of
forking paths is built. If B5c's falsifier also fires, the honest conclusion is
that this control cannot be constructed at this corpus scale, and the brief's
central claim is **untestable by this design** rather than refuted by it.

---

## 7. B5c's result: the falsifier is model-dependent

**This was not among the anticipated outcomes.** §6 named two: the schemas
converge on something more specific than coupling (arm fails), or they converge
on coupling alone (arm works). What happened is that **which one occurs depends
on the model**.

### `qwen3:4b` — FIRES

All five schemas converge on *coupling without transfer or permanent change* —
that is ¬C2, one of the four clauses §6 named:

| seed | in the schema's own words |
|---|---|
| 0 | excludes "systems where components transfer resources, information, or state between each other"; excludes "permanent change in state due to interaction" |
| 1 | "without transfer of material, information, or identity between the entities"; "without any permanent change to either entity" |
| 2 | "reversible state where both units maintain identical properties before and after interaction cycles, with no permanent change or record" |
| 3 | excludes "systems with information transfer", "systems with irreversible interactions", "permanent state change" |
| 4 | "does not transfer physical or information resources between the entities" |

Five of five. **B5c is not cited at 4B.**

**Why, and it is the same mechanism as B5b at a different address.** Domain 3
(lymphocyte/antigen, reversible binding) is the one domain whose positive
description is carried by negations — *"both are exactly as they were: no bond
is broken in either, nothing is transferred, and neither carries any record."*
The 4B model generalised those negations across the whole set. So the model does
not extract what five heterogeneous cases *share*; it builds their intersection,
and an intersection inherits whichever exclusions are most saliently written.
**Designing for heterogeneity produced over-specification, not vagueness.**

### `ornith-1.5-9b` — DOES NOT FIRE

The same five stimuli at 9.2B scatter, and *contradict each other*:

| seed | schema | what it commits to beyond coupling |
|---|---|---|
| 3 | "Reciprocal Dependency" | nothing — two parts, each depends on the other. **The intended trivial universal.** |
| 4 | "coupling" | coupling, not from an external mechanism, entities "stay recognisably different" |
| 0 | "binary coupling with mutual dependence and no separability…" | requires **no separable boundary**, no larger collection, no third-party governance |
| 1 | "Coupled-Continuant Relation" | irreversibility, emergent capability |
| 2 | "Nested Boundary" | containment — and **explicitly excludes** "root and fungus fused … that cannot be split apart" |

Seeds 0 and 2 assert opposite things about separability, so there is no shared
commitment beyond mutual coupling. By §6's criterion **the arm is valid at 9B**.

**And this is not the 9B model simply failing to converge on anything.** Run
`B5b` through the same model and it converges as hard as ever — seed 0
"Rule-based structural mimicry **without causal coupling**", seed 2 "template
reproduction … the carrier's own state … remains unchanged", seed 3 "E does not
act back on A or B", seed 4 "each role produces a stable feature … independent
of other roles", every one excluding mutual causation. **Same model, same
prompt, same seeds: B5b converges and B5c does not.** The difference is in the
stimulus, which is what makes B5c's validity at 9B a fact about the design and
not about the model's laxity.

### What the valid comparison says

At 9B, with the control admissible:

| | kappa |
|---|---|
| C5 | +0.600 ± 0.137 |
| B5c | +0.233 ± 0.207 |
| difference | **+0.367** |
| instrument band NULL→ORACLE | **0.556** |

**+0.367 is inside the band, so P1c is not supported.** C5 does not beat
congruence-around-the-trivial-part by more than the smallest difference this
instrument is held to.

### A methodological objection to the band, recorded rather than used

The band rule ("no difference smaller than NULL→ORACLE is interpretable") was
inherited from E19 and applied here unchanged, because changing a criterion
after seeing the number it governs is exactly what pre-registration is for.
But it behaves perversely across models: the 9B has the *widest* band (0.556)
precisely because it is the *best* instrument — its ORACLE is highest and its
NULL lowest. So a better instrument makes every effect harder to call
significant. As an effect-size normalisation +0.367/0.556 = **66% of the full
vacuous-to-hand-authored range**, which is not nothing.

**The honest statement is therefore narrower than "refuted":** at the one scale
where a valid control exists, C5's advantage over it is large as a fraction of
range but does not clear the pre-registered bar, on n = 5 seeds. This is the
comparison to repeat first if the track ever returns to the brief.

### Where three attempts leave the control

| attempt | design | outcome |
|---|---|---|
| `B5` | five domains chosen to share nothing | confounded — all five were asymmetric one-way relations |
| `B5b` | five surface-matched, structurally heterogeneous | falsifier fired at **4B, 7B and 9B** — converged on ¬C1 at every scale |
| `B5c` | five coupled, each failing a different clause | fires at 4B (¬C2), **valid at 9B** |

The control is constructible, but only at a scale where the model stops
over-generalising from the most salient negation. That is a finding about
**what it takes to run this experiment at all**, and it is the most transferable
thing in E19b: *a control arm's validity is not a property of the stimulus
alone. It is a property of the stimulus and the model together, and it has to be
checked per model rather than assumed from the design.*
