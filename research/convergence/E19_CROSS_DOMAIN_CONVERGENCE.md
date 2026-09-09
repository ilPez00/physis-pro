# E19 — cross-domain structural convergence

**Verdict: NOT SUPPORTED, and the mechanism is identified rather than merely
observed.** Five structurally congruent domains discriminate no better than one
(kappa +0.467 vs +0.400, Welch *t* = +0.89) and no better than one at
the same token count (A5, +0.450, *t* = +0.24), which is the control that rules
out text quantity.

> **CORRECTION 2026-09-09 (E19b).** This paragraph originally called `S5` "the
> decisive test" and read *"no better than the same five domains with their
> congruence destroyed."* **Withdrawn.** S5 is C5's five bodies cyclically
> shifted against five identical headings; the bodies are byte-identical and
> their mutual congruence is intact, while the headings are the one thing the
> generation prompt tells the model to discard. S5 is therefore not a
> congruence-destroying control, and `C5 ≈ S5` is near-tautological. The verdict
> is unchanged and now rests on `C5 vs A1` and `C5 vs A5`, which are valid. The
> real control is built in `E19b_CONTROL_REBUILD.md`.

What it does do is make the abstraction **more permissive**: positive acceptance
0.77 → 0.97, negative acceptance 0.37 → 0.50. The brief's §17 named this in
advance — "trivial universal principles such as everything interacts."

Scope: `qwen3:4b`, CPU-only. See *Limits*.

---

## The claim, made testable

The brief (§16): *providing an LLM with multiple independently grounded examples
of the same underlying relational pattern can improve its ability to discover,
formalize, and generalize the underlying structure.*

The target structure **T** was fixed before any run (`target_structure.md`):
coupling (C1), modification (C2), invariance (C3), differentiation (C4),
ensemble order (C5). Each clause has a natural negation that *looks* like T —
synchronisation violates C4, merging violates C3, common cause violates C1 —
which is what makes the claim narrow enough to lose.

**An abstraction is scored by what it does, never by how it reads.** Generation
and application are separate calls, and the applying call is blind to the arm
and to the stimulus: it sees only a schema and a probe. Labels were fixed
beforehand.

---

## Result

| arm | n | kappa | pairs solved | POS accepted | NEG accepted |
|---|---|---|---|---|---|
| NULL (vacuous schema) | 1 | +0.333 | 0.33 | 1.00 | 0.67 |
| A1 one domain, 92 words | 5 | +0.400 ± 0.109 | 0.47 | 0.77 | 0.37 |
| A5 one domain, 396 words | 5 | +0.450 ± 0.095 | 0.47 | 0.82 | 0.37 |
| B5 five unrelated domains | 5 | +0.050 ± 0.151 | 0.15 | 0.22 | 0.17 |
| S5 five domains, headings permuted (**not** a congruence control — see E19b) | 4 | +0.438 ± 0.185 | 0.44 | 0.96 | 0.52 |
| **C5 five congruent domains** | 5 | **+0.467 ± 0.126** | 0.50 | **0.97** | **0.50** |
| ORACLE hand-authored T | 1 | +0.583 | 0.58 | 0.92 | 0.33 |

**Pre-registered tests.** H1 required C5 to beat A1 *and* A5 *and* B5 *and* S5.

| test | Welch *t* | outcome |
|---|---|---|
| C5 vs A1 | +0.89 | no difference |
| C5 vs A5 | +0.24 | no difference |
| C5 vs S5 | +0.27 | no difference — but S5 is **not** a construction-matched control (E19b) |
| C5 vs B5 | +4.72 | C5 higher — but see *B5 does not test what it claims* |

H1 fails on three of four. The two that carry the verdict are A1 and A5;
the S5 row is retained for the record but carries none of it (E19b).

### Equivalence, not merely a failed test

A non-significant *t* says only that equality could not be rejected. E7 in this
track recorded twelve rejections that turned out to be **underpowered rather
than refuted**, so the null here is held to a bound instead: is the true
difference smaller than the smallest difference this instrument can resolve?
That threshold is the NULL→ORACLE dynamic range, **0.250 kappa**.

95% bootstrap CI on the difference in mean kappa (20 000 resamples over seeds):

| comparison | difference | 95% CI | |
|---|---|---|---|
| C5 − A1 | +0.067 | [−0.067, +0.183] | entirely inside the band |
| C5 − A5 | +0.017 | [−0.117, +0.133] | entirely inside the band |
| C5 − S5 | +0.029 | [−0.142, +0.217] | entirely inside the band |

All three intervals lie inside ±0.250. The claim is therefore **not** "no effect
was detected" but "any true effect of the multi-domain condition is smaller than
the smallest effect this instrument could have resolved." **C5 − S5 should not
be read as bounding a congruence effect (E19b);** C5 − A5 is the interval that
bounds the text-quantity account, and C5 − A1 the one-versus-five account.

### The mechanism: multiplicity, not text volume

*(Originally titled "multiplicity, not congruence, and not text volume". The
congruence half of that claim rested on S5 and is withdrawn — E19b.)*

The four arms separate the three candidate causes cleanly, because the design
holds each constant in turn:

| arm | items | words | POS acc | NEG acc |
|---|---|---|---|---|
| A1 | 1 | 92 | 0.77 | 0.37 |
| A5 | 1 | 396 | 0.82 | 0.37 |
| S5 | 5 | 419 | 0.96 | 0.52 |
| C5 | 5 | 419 | 0.97 | 0.50 |

- **Text volume is not it.** A5 gives one domain 4× the words of A1 and moves
  acceptance by 0.05/0.00.
- **Congruence is untested here.** ~~C5 and S5 are token-identical and differ
  only in whether the relational bodies belong to their settings.~~ They are
  indistinguishable on every measure because S5 *contains the same five
  congruent bodies*; only the headings moved. This bullet claimed more than the
  design delivers. See `E19b_CONTROL_REBUILD.md`.
- **Item count is it.** Going from one item to five raises positive acceptance
  ~0.19 and negative acceptance ~0.14. ~~whether or not the five share
  structure~~ — every five-item arm measured here (C5, S5) is congruent, so
  "whether or not" is unsupported; `B5b` in E19b is the arm that tests it.

Presenting five separate cases makes the model generalise *wider*, not *deeper*.
That is a real and reproducible effect; it is simply not the effect the brief
predicted, and it costs specificity.

### The failure is upstream of the judge: the clause is never abstracted

The verdicts above could mean either that the model writes good schemas and
cannot apply them, or that it never writes the clause at all. Reading the
generated schemas separates the two. Fraction of schemas whose text encodes
each clause (keyword scan over name, roles, conditions and excludes):

| arm | n | states C4 (difference increases) | excludes convergence | states C3 (identity kept) |
|---|---|---|---|---|
| A1 | 5 | 0.20 | 0.00 | **1.00** |
| A5 | 5 | 0.20 | 0.20 | **1.00** |
| B5 | 5 | 0.00 | 0.00 | 0.60 |
| S5 | 4 | 0.25 | 0.25 | 0.75 |
| **C5** | 5 | **0.20** | **0.00** | **1.00** |
| ORACLE | 1 | 1.00 | 1.00 | – |

**Every single-domain and congruent-domain schema states C3.** Identity
preservation is recovered universally, from one example or five.

**C4 is stated by one schema in five, and C5 is no better than A1 — or than
S5.** Not one of the five congruent-domain schemas excludes convergence, though
all five stimuli are cases of difference *increasing* through interaction:
crowns growing apart, phonemes dispersing, firms moving into each other's weak
spots. The model read five demonstrations of the one clause that distinguishes
the pattern and encoded it once.

So the mechanism fails at **both** stages, independently:

1. **Generation.** 80% of abstractions never contain the differentiating
   clause, and congruent domains do not raise that rate.
2. **Application.** ORACLE *does* contain it, stated explicitly with the
   exclusion "a change that makes them more alike fails this" — and still
   accepts every convergence decoy.

Cross-domain congruence cannot repair either. This is the strongest evidence in
the experiment, and it does not depend on kappa, on the instrument band, or on
any between-arm comparison.

### Which null survives

**H0b, the trivial universal** — with the refinement that the driver is item
multiplicity. Not H0a (text quantity), which A5 rules out directly. Kappa is
flat because rising positive and negative acceptance cancel, which is why
acceptance is reported apart from kappa: on kappa alone this looks like "no
effect", and it is actually "a measurable effect in the wrong direction".

### The load-bearing clause, and nothing recovers it

False-accept rate per violated clause (1.00 = every decoy of that kind accepted):

| arm | C1 coupling | C2 modification | C3 invariance | **C4 differentiation** | C5 ensemble |
|---|---|---|---|---|---|
| NULL | 0.00 | 0.00 | 1.00 | 1.00 | 1.00 |
| A1 | 0.27 | 0.40 | 0.20 | 0.67 | 0.40 |
| A5 | 0.07 | 0.00 | 0.45 | 0.73 | 0.20 |
| B5 | 0.00 | 0.00 | 0.05 | 0.47 | 0.40 |
| S5 | 1.00 | 1.00 | 0.25 | 0.67 | 1.00 |
| C5 | 0.58 | 0.00 | 0.31 | **0.83** | 0.75 |
| ORACLE | 0.33 | 0.00 | **0.00** | **1.00** | 0.00 |

**C4 is the brief's entire novel claim** — that interaction can produce and
maintain difference rather than erase it. It is the worst column for every arm.
ORACLE, handed T in writing *including the explicit exclusion "a change that
makes them more alike fails this"*, accepts **every** convergence decoy:
synchronising pendulum clocks, phase-locking fireflies, dialects levelling under
broadcast media. The same ORACLE rejects **all four** identity-destruction
decoys.

So the model reliably detects when interaction **destroys** difference, and
reliably fails to detect when interaction **reduces** difference without
destroying it. Adding congruent domains makes this worse (C5: 0.83) rather than
better.

**A single domain already yields most of T.** A1, given only the mycorrhiza
case, produced "two distinct entities that maintain a mutual, non-merging
interaction", each retaining "its own internal autonomy", forming "a stable,
distributed pattern ... without being physically represented by either" — C1, C3
and C5 from one domain. What it lacks is C4. Cross-domain presentation conveys
the clauses that were never in doubt and not the one that was.

---

## What went wrong in the instrument, recorded rather than buried

**B5 does not test what it claims.** It was designed as "five domains with no
shared invariant". The model found one: all five are asymmetric one-way
relations, and B5 abstracted `actor / resource / consequence`. Applied to a probe
set that is entirely mutual, that schema correctly rejects nearly everything.
B5's +0.050 is a *successful abstraction of the wrong thing*, so the C5 vs B5
comparison is not evidence for congruence and is not counted as such.

**S5 is not the control it was labelled (E19b, 2026-09-09).** Its five bodies
are C5's, cyclically shifted against identical headings — congruence among them
intact. Recorded here rather than silently corrected, because the arm was
described as decisive in the first reading of this experiment. The replacement
arm B5b, and what does and does not survive, are in `E19b_CONTROL_REBUILD.md`.

**S5 rows were lost once and regenerated.** A race between four concurrently
running processes (three orphans from OOM restarts, all writing one results
file) clobbered them. No data was corrupted — every surviving row was unique,
zero duplicates — and the arm was re-run to n = 4. The C5-vs-S5 test reported
above is from the regenerated rows.

**Four probe-set drafts died to the pre-flight before any model ran.** Draft 1:
word count alone scored AUC 0.941 — E18's exact confound. Draft 2 overcorrected
to inverted separation. Draft 3 leaked TF-IDF at 0.847 on the structural framing
itself. Draft 4 rose to 0.889 on a tense artifact. Fixed by matched pairs, where
each negative shares subject and vocabulary with a positive and differs only in
which clause fails. Residual TF-IDF leave-one-pair-out forced choice **0.750** is
declared as a floor rather than pretended away.

Plain leave-one-out was **invalid** under matched pairs — each held-out probe's
nearest neighbour is its own partner in the opposite class, reporting 0.083
inverted separation. Replaced with leave-one-pair-out.

**Judging protocol changed once and the old results were discarded, not patched.**
Batch-8 judging caused 3 of 5 checked errors; `BATCH = 2`. An earlier reading of
this experiment blamed clause C1; that came from the discarded protocol and is
superseded by the table above.

---

## Limits

1. **One 4B model, CPU-only.** External validity to frontier models is untested,
   and is the single most valuable follow-up.
2. **The instrument's dynamic range is +0.250 kappa** (NULL → ORACLE). Any
   between-arm difference smaller than that is not interpretable. C5 − A1 =
   +0.038 is comfortably inside it. This experiment can support "no detectable
   effect"; it cannot support a fine ranking of arms.
3. **ORACLE at +0.583 is a low ceiling.** A hand-authored formalization of T
   buys only 0.250 over a deliberately empty schema, because it too fails C4.
4. Probes are near-transfer in places (crown shyness shown, plant roots probed).

---

## What this does and does not settle

It does **not** show cross-domain congruence is useless in general. It shows
that at 4B, on this task, congruence produced generality rather than
discrimination — and that the specific distinction the brief is built on
(difference increased vs. difference reduced) is not conveyed by examples, and
is not even reliably applied when stated outright.

**Consequence for the rest of the brief.** §5 (recursive composability), §12
(the Lean/Physis validation stack), §13 (typed composition) and §14 (Lean as
substrate) were not built. Formalizing T in Lean would be formalizing a
structure whose empirical discriminability is currently indistinguishable from
what one domain gives you. E9 reached the same stopping point for the discrete
signifier address (0.244 held-out vs geometry's 0.447) and returned NOT YET.
Same answer here, for the same reason.

Revisit if C5 beats A1 and **B5b** by more than the NULL→ORACLE band on a larger
model. (Originally written as "A1 and S5"; corrected by E19b, which shows S5
cannot carry that role.) That is one experiment, and it is the one worth
running.
