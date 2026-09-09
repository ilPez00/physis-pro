# E25 — is a compound an operator applied to a head, or a sum?

**Testing `dog-modulus|tooth|`.** The claim: "dog tooth" is not `dog + tooth`.
It is the dog-transform applied to tooth — a specialised instance of tooth that
is still its own entity, with dog acting as an operator. Fractal, because the
move recurs at every scale.

That is a claim about representation with sharp consequences, so it was tested
as one. `run_e25.py`, 537 compound names, 1256 strings embedded with
`nomic-embed-text`.

| | ADDITIVE predicts | TRANSFORM predicts |
|---|---|---|
| offsets `v(MH) − v(H)` for one modifier | cluster tightly — a constant offset | no more coherent than random — the contribution depends on the operand |
| compound's position | no head/modifier asymmetry | closer to its **head**, which keeps its identity |

## The geometry is additive. Flatly.

| test | result | |
|---|---:|---|
| offset coherence within a modifier (8 modifiers, n ≥ 4) | **+0.697** | |
| same-size random (modifier, head) groups — **the null** | +0.036 | |
| **excess over null** | **+0.661** | additive |
| compound nearer its head | 0.451 of cases | coin flip — **no asymmetry** |
| cos(compound, head) / cos(compound, modifier) | 0.787 / **0.801** | modifier is *nearer*, if anything |
| reconstruction by `norm(v(M)+v(H))` | **0.923** | addition nearly recovers the compound |

Both signature predictions of the transform model fail. **In this embedding
space a modifier contributes a near-constant offset regardless of what it
attaches to**, which is what addition means.

## But addition is measurably lossy, and the roles are asymmetric

Two results complicate the flat verdict, and both point back toward the original
intuition.

**1. The true compound beats its own additive reconstruction.** Despite 0.923
cosine agreement, on the actual filing task (498 compound entries with a blind
re-filing):

| representation | domain top-1 | mode top-1 |
|---|---:|---:|
| **full compound vector** | **0.620** | **0.526** |
| additive `norm(v(M)+v(H))` | 0.562 | 0.494 |

Addition loses **0.058 domain and 0.032 mode**. So the compound is *not* the sum:
there is task-relevant residual that `v(M)+v(H)` destroys, even though the two
vectors are nearly parallel. **Geometric near-identity is not informational
identity** — a distinction this track has now been bitten by twice.

**2. The head carries the mode; the modifier carries almost none of it.**

| representation | domain (floor 0.231) | mode (floor 0.209) |
|---|---:|---:|
| modifier only `v(M)` | 0.467 → **+0.236** | 0.263 → **+0.054** |
| head only `v(H)` | 0.507 → **+0.276** | 0.460 → **+0.251** |

**Domain sits in both constituents roughly equally. Mode sits almost entirely in
the head** — the modifier carries about a fifth as much mode signal, barely
clearing the majority floor.

That is the asymmetry the fractal model asserts, arriving informationally rather
than geometrically. "Incident Response": RESPONSE is what is being done, INCIDENT
is the context it is done in. **The modifier is not a co-equal addend — it
supplies domain context and essentially no mode.** The head is the entity and
keeps its own nature, which is exactly *"still its own separate entity"*.

## The honest synthesis

**The vector arithmetic is additive; the semantic roles are not.** `T_M · v(H)`
is not how this embedder composes, and the offsets prove it. But the modifier
and head are not interchangeable addends: one carries context and the other
carries the entity's own nature, and the true compound holds structure that
addition loses.

**The consequence for ontology design is the useful part.** If the fractal model
is right about the domain, **the current representation is structurally unable to
express it** — an additive space cannot distinguish `dog + tooth` from
`T_dog · tooth`, so it collapses them. That is an argument for building the
transform *explicitly*, as structure the system carries, rather than hoping
geometry encodes it. Which lands next to E9, where the discrete signifier address
scored 0.244 held-out against geometry's 0.447: **the symbolic route
underperformed, and the geometric route cannot represent the thing.** That
tension is now measured from both sides and is the sharpest open question here.

## Limits, stated plainly

1. **This measures English noun compounds as one embedder sees them**, not
   ontological part-whole structure. "Dog tooth" being additive in
   `nomic-embed-text` is a fact about its lexicon as much as about teeth.
2. **8 modifiers with n ≥ 4.** The offset-coherence result rests on a small
   group count, though the excess over null (+0.661) is far too large to be noise.
3. **Offset coherence has a lexical floor.** Every compound sharing modifier M
   literally contains the string M, and embedders weight token overlap heavily.
   So "additive" here may be shallow lexical additivity rather than semantic
   composition — which does not change the conclusion (the space composes
   additively) but does limit what it says about meaning.
4. **Not a prescription to drop the modifier.** Head-only mode (0.460) is still
   worse than the full compound (0.526). The finding is about where information
   concentrates, not about what to discard.
