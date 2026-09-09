# Handoff — cross-domain structural convergence (E19/E20)

**For an agent picking this up cold.** Written 2026-09-08. Branch
`research/perspective-discovery-v2`. Everything below lives in
`research/convergence/` unless stated otherwise.

---

## 0. Read these first, in this order

1. `research/RESEARCH_STATUS.md` — the E1–E18 track. **Non-negotiable context.**
2. `PLAN.md` § "Research track" — the five standing rules, each earned by a
   failure. Rule 1 (treat any positive as artifact until a construction-matched
   control says otherwise) has killed seven hypotheses. Rule 4 (run the
   pre-flight and honour it) was overridden once, in E10, and that is recorded
   as a mistake rather than quietly fixed.
3. `research/convergence/target_structure.md` — the formalization under test.

---

## 1. The brief, and what was actually made testable

The prompt proposed that presenting an LLM with several materially different
but *structurally congruent* phenomena helps it discover higher-order
relational invariants. It asked for this to be made computational, formal,
falsifiable, and prototyped, and explicitly asked that failures be preserved
rather than the idea protected.

Most of the brief is unfalsifiable as written. One section is not: **§16**, the
central empirical claim, and **§9**, which names the conditions. That is what
E19 tests. The rest of the brief (§5 recursive composability, §12 the Lean/
Physis validation stack, §13 typed symbolic composition, §14 Lean as substrate)
is **not built and should not be built yet** — see §7 below for why.

### The target structure T, fixed before any run

A system of two or more entities under a relation instantiates T iff all five
clauses hold. Full statement in `target_structure.md`:

| clause | holds iff |
|---|---|
| C1 coupling | each entity's course depends on the other's; a shared external cause acting separately on both does **not** count |
| C2 modification | the coupling changes what at least one entity becomes vs. its uncoupled counterfactual |
| C3 invariance | each remains individuable afterwards as the same kind of thing |
| C4 differentiation | the change makes them **less** alike, or each less like its uncoupled self |
| C5 emergent order | an ensemble regularity holds that no single entity carries or decides |

The clauses matter because each has a natural negation that *looks* like T:
`¬C4` is synchronisation (coupled clocks, fireflies) — the textbook emergence
story, and T is precisely not it. `¬C3` is merging. `¬C1` is common cause.
This is what makes the brief's claim narrow enough to lose.

---

## 2. What is built and working

| file | what it is |
|---|---|
| `target_structure.md` | T, its five clauses, and what "a good abstraction" is defined to mean |
| `probes.json` | 24 probes as **12 matched pairs**; ground truth fixed before any model ran |
| `stimuli.json` | the five conditions (A1, A5, B5, C5, S5), token-matched |
| `preflight.py` | probe-set integrity gate — **run this before trusting anything** |
| `run_e19.py` | the experiment; writes `results_e19.json`, resumable |
| `results_e19_batch8_discarded.json` | superseded smoke results, kept deliberately (see §5) |

### The measurement, and why it is not "does the abstraction read well"

Three phases, and the separation between them is the whole point:

1. **GENERATE** — an arm's stimulus goes in, a machine-readable schema
   (`roles`, `conditions`, `excludes`) comes out.
2. **APPLY** — a *second* call, **blind to the arm and to the stimulus**, sees
   only that schema and a probe, and rules the probe in or out.
3. **SCORE** — against pre-fixed labels.

So an abstraction is scored by what it *does* to unseen cases. An abstraction
that generalises to "two things interact and something emerges" accepts every
probe and earns kappa 0 — §17's "trivial universal" is the measured null here,
not a worry someone has to remember to check.

### The arms

| arm | what it is |
|---|---|
| `NULL` | deliberately vacuous schema, no generation. **The floor.** |
| `A1` | one congruent domain (brief §9A) |
| `A5` | the same one domain at C5's token count — **token control** |
| `B5` | five surface-matched, structurally *unrelated* domains (brief §9B) |
| `C5` | five structurally congruent domains (brief §9C/D) — **the claim** |
| `S5` | C5's exact tokens with bodies permuted across headings — **NOT a control; withdrawn, see E19b** |
| `B5b` | rebuilt heterogeneous arm — **falsifier fired, not cited (E19b)** |
| `B5c` | five coupled domains each failing a different clause — **valid at 9B, falsifier fires at 4B (E19b)** |
| `ORACLE` | T hand-authored. **The ceiling.** |

### Pre-registered hypotheses

- **H1** (brief is right): C5 > A1, *and* C5 > B5, *and* C5 > S5, *and* C5 > A5.
- **H0a** (text quantity): C5 ≈ A5 ≈ S5 > A1.
- **H0b** (§17's trivial universal): C5 accepts more positives **and** more
  negatives; acceptance rises, kappa does not.
- **H0c** (no instrument resolution): ORACLE itself at chance ⇒ the result is
  about a 4B model, not about the hypothesis. **Checked first.**
  *(2026-09-09: settled by replication at 7.6B and 9.2B — the ceiling does not
  climb with scale and C5 never separates from one domain. Not a 4B artifact.)*

---

## 3. Results — E19 is COMPLETE. Verdict: NOT SUPPORTED

Full write-up: `E19_CROSS_DOMAIN_CONVERGENCE.md`. Re-score any time with
`python3 analyze_e19.py` (reads JSON, runs no inference).

| arm | items | words | n | kappa | POS acc | NEG acc |
|---|---|---|---|---|---|---|
| NULL | – | – | 1 | +0.333 | 1.00 | 0.67 |
| A1 | 1 | 92 | 5 | +0.400 ± 0.109 | 0.77 | 0.37 |
| A5 | 1 | 396 | 5 | +0.450 ± 0.095 | 0.82 | 0.37 |
| B5 | 5 | 479 | 5 | +0.050 ± 0.151 | 0.22 | 0.17 |
| S5 | 5 | 419 | 4 | +0.438 ± 0.185 | 0.96 | 0.52 |
| **C5** | 5 | 419 | 5 | **+0.467 ± 0.126** | 0.97 | 0.50 |
| ORACLE | – | – | 1 | +0.583 | 0.92 | 0.33 |

**C5 vs A1 t=+0.89, vs A5 t=+0.24 — no difference on either.** A5 is the token
control: one domain at C5's word count. The verdict rests on these two.

> **CORRECTED 2026-09-09 (E19b).** This block originally added *"vs S5 t=+0.27"*
> and called S5 "the construction-matched control ... congruence destroyed",
> concluding *"the positive died to the construction-matched control."*
> **Withdrawn.** S5's five bodies are C5's, byte-identical, cyclically shifted
> against five identical headings (tf-cos 1.000); their mutual congruence is
> intact, and the generation prompt tells the model to discard exactly the part
> that moved. `C5 ≈ S5` is near-tautological. Two rebuilt controls followed:
> B5b's falsifier fired at both scales, and B5c's fires at 4B but not at 9B.
> **Read `E19b_CONTROL_REBUILD.md` before citing any control in this file.**

**The mechanism is item multiplicity, not text volume.** A5 holds items at 1
while quadrupling words: acceptance barely moves. Going 1 item → 5 items raises
POS acceptance ~0.19 and NEG acceptance ~0.14. Whether congruence adds anything
on top is what the control was meant to answer, and did not.

**C4 — differentiation, the brief's entire novel claim — is the worst clause for
every arm**, ORACLE included (1.00 false accepts: it takes every synchronising
clock, phase-locking firefly and levelling dialect, while rejecting all four
identity-destruction decoys). The model detects interaction that *destroys*
difference and fails on interaction that *reduces* it. Being handed the
exclusion in writing does not fix it.

## 4. What is promising

1. **The matched-pair probe design.** Reusable as ground truth for anything
   that claims to detect relational structure — including E20 below. It is
   length-controlled and pre-flighted, which took four rewrites.
2. **The generate/apply split.** It makes "abstraction quality" a measured
   quantity rather than a judgement. Portable to other questions in this track.
3. **The Qwen3.8-Flash-Next n-gram embedding table (E20, not started).**
   **A table read, not inference** — so it is affordable on modest hardware,
   unlike everything else here. See §6. *(2026-09-09: omo and pansa were both at
   100% disk, which blocked the download; space has since been freed — omo 157G,
   pansa 114G.)*

---

## 5. What is broken, and what was tried and abandoned

**Local compute is the binding constraint — on *feather*, and only there.**
Feather is a Ryzen 5 4500U, 6 cores, no GPU; `qwen3:4b` runs at ~4.4 tok/s and a
45s call is normal.

> **CORRECTED 2026-09-09.** This paragraph read "no GPU" full stop and shaped
> every design decision above it. **The fleet has one.** `omo`
> (192.168.1.7 / 100.116.39.57, `alias omo` in `~/.zshrc`) has an RX 6650 XT and
> its ollama serves on `0.0.0.0:11434` with `OLLAMA_VULKAN=1` and
> `HSA_OVERRIDE_GFX_VERSION=10.3.0`, reporting **100% GPU** and ~38 tok/s on a
> 9B — roughly 50x feather. A full 36-job E19 grid takes ~12 minutes there
> against ~3 hours locally. Drive it remotely with
> `E19_MODEL=... E19_HOST=http://100.116.39.57:11434 ./supervise_e19_remote.sh`;
> `run_e19.py` writes one results file per model and refuses to mix models.
>
> **omo has 8GB VRAM — never run two grids at once**, they evict each other on
> every call. `orchestrate_omo.sh` serialises them.
>
> **Do not use `pansa` for inference.** It has 15GB RAM and hosts Praxis
> production; loading the 17GB `qwen3.8-27b-q4` drove it to load 8+ with swap at
> 15.5/17.5GB and it had to be cleared by hand. Confirmed the hard way.

**Four probe-set drafts died to the pre-flight.** Recorded because the failures
are the useful part:

1. Draft 1 — **word count alone scored AUC 0.941.** Positives need three clauses
   spelled out; negatives are shorter stories. This is precisely the confound
   that killed ANCHORVOC in E18 (entry length alone scored 0.619).
2. Draft 2 — overcorrected to AUC 0.000, i.e. perfect *inverted* length
   separation. Equally fatal.
3. Draft 3 — length fixed, but **TF-IDF separated the classes at AUC 0.847**.
   The tell was the structural framing itself: positives said "each keeps its
   own… than either alone… between them", negatives did not.
4. Draft 4 — gave negatives the same phrasing; TF-IDF went **up** to 0.889.
   The remaining tell was **tense**: negatives had drifted into narrative past
   ("were", "met", "having").

**Fix that worked: matched pairs.** Each negative now shares subject matter and
vocabulary with a positive and differs only in which clause fails (finches
pushed apart vs. pushed together; forks diverging vs. merged back). Plus
uniform present tense.

**A statistic that was invalid and was replaced.** Under matched pairs, plain
leave-one-out nearest-centroid reported near-perfect *inverted* separation
(0.083) because each held-out probe's nearest neighbour is its own pair partner
in the opposite class. It was measuring the pairing, not a vocabulary tell.
Replaced with **leave-one-pair-out**. If you change the probe set, this
distinction will bite you again.

**Current declared floors — every arm must beat these:**
- length AUC 0.378 (words) / 0.420 (chars) — at chance, fine
- **TF-IDF leave-one-pair-out forced choice 0.750** — *not* at chance. Residual
  lexical signal remains. Reported as a floor rather than pretended away.

**Batch size 8 in the judge was wrong** and its results were discarded, not
patched: one-probe-per-call fixed 3 of 5 checked errors. Now `BATCH = 2`
(2.3 min/schema vs. 3.5 at batch 1, vs. errors at 8).

**B5 does not test what its label claims, and the schemas show it.** B5's arm
was designed as "five domains with no shared invariant". The model found one
anyway: all five B5 domains are asymmetric one-way relations (fungus consumes
log, vine uses tree, antibody binds antigen, index aggregates prices, language
borrows word), and B5 seed 0 abstracted exactly that — roles `actor / resource /
consequence`, condition "the actor interacts with the resource to produce a
consequence". Applied to a probe set that is entirely *mutual*, that schema
correctly rejects almost everything, which is why B5 sits at POS acceptance 0.22
and kappa +0.050.

So B5 measures "a different shared structure", not "no shared structure". The
low score is not a failure to abstract — it is a successful abstraction of the
wrong thing. Anyone rewriting this arm must pick five domains that are genuinely
structurally heterogeneous *with respect to each other*, which is harder than it
sounds, and should verify it by reading the generated schemas rather than
assuming it from the stimulus design.

**A1 already recovers most of T from one domain, except the clause that matters.**
A1 seed 0, given only the mycorrhiza case, produced: two distinct entities that
maintain a "mutual, non-merging interaction"; each "retains its own internal
autonomy"; a "stable, distributed pattern... without being physically
represented by either". That is C1, C3 and C5 from a single domain. What it does
not contain is **C4, differentiation** — and A1's C4 false-accept rate is 0.67.
The same hole appears in S5. If C5 also misses C4, the finding is that
cross-domain presentation conveys the easy clauses and not the load-bearing one.

**Dead end, do not repeat:** the Qwen **BPE merge DAG** as a structural signal.
E2 refuted it as a map, and the reason generalises — merges are orthographic, so
"differentiation"/"differential" share ancestors because they share letters. It
is a substring measure with extra steps. The n-gram *embedding* table (§6) is a
different object and is not covered by that refutation.

---

## 6. E20 — the n-gram sieve, designed but blocked

**Blocked on:** the download finishing on omo/pansa. Need machine + path.

**Idea (user's).** Qwen3.8-Flash-Next carries a 51B-parameter n-gram embedding
table: learned vectors keyed by local context, paged from host RAM/SSD. Use it
as an **archetype inventory / sieve** — admit a candidate concept or
correspondence only if it is *attested* in the table rather than a coinage.
This is closer to what E9 wanted and failed to find (a discrete signifier
address; E9's best scored 0.244 held-out vs. geometry's 0.447) because E9's
addresses had no content and these rows do.

**Test.** Reuse `probes.json` as ground truth. Build a table-derived
representation of each probe; ask whether it separates the matched pairs better
than **(a)** BGE embeddings, **(b)** plain corpus frequency, **(c)** length.

**Controls (b) and (c) are non-negotiable.** "Attestedness" in an n-gram table
is a frequency measure wearing a hat, and E17/E18 burned four experiments on
exactly that.

**Pre-registered prediction, so it can fail cleanly:** the sieve will **not**
separate the C1 (common-cause) pairs, because that distinction is causal rather
than lexical and a table of local-pattern representations is the wrong
instrument for it. Separating only the C3 (identity-destruction) pairs is the
expected honest positive, and would still be useful.

---

## 7. What was deliberately NOT built, and why

§5 (recursive composability), §12 (LLM → Lean → Physis validation stack), §13
(typed symbolic composition), §14 (Lean as formal substrate).

**Not built because the track already bears on it.** E9 tested whether a
discrete signifier address was worth formalising in Lean and returned **NOT
YET** (0.244 held-out against geometry's 0.447). Building a Lean substrate for
a structure whose *empirical* discriminability is currently 0.25 kappa of
dynamic range would be formalizing something not yet known to exist. §14 of the
brief itself demands formal validity and semantic grounding be kept separate;
the honest reading is that grounding has not been demonstrated yet.

**Revisit if and only if** E19 shows C5 beating B5 and S5 by more than the
NULL→ORACLE band, replicated on a larger model.

---

## 8. Immediate next actions

1. **The one experiment worth the compute: re-run on a frontier model.**
   Everything is on disk and `analyze_e19.py` runs no inference; only
   `E19_MODEL` changes. The 4B ceiling (ORACLE +0.583) is the main threat to
   these conclusions, and it is cheap to remove.
2. **Fix B5 before citing the one comparison C5 won.** See §5.
3. **Top up S5 to n=5** (currently 4, one row lost to the process race).
4. **E20 when the download lands.**
5. **Do not over-read a 4B result.** With a 0.250 kappa band this supports
   "no detectable effect", not a fine ranking of arms.

## 9. The rule that governs all of this

From `PLAN.md`, earned seven times: **treat any positive as artifact until a
construction-matched control says otherwise.** In E19 that control is `S5`
(same tokens, congruence destroyed) and `B5` (same substrates, no shared
invariant). If C5 beats A1 but not S5, the brief's hypothesis has not been
supported — it has been shown to be a text-quantity effect.

If the beautiful intuition collapses, preserve the failure. That instruction is
in the brief itself, and it is also this track's house style.
