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
*(empty until the run completes)*
