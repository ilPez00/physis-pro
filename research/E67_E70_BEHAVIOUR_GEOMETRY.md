# E67 / E70 — does an agent's behaviour carry a recoverable geometry?

Run 2026-09-14 on 165 Claude Code transcripts already on disk: 25 sessions with
≥ 10 file touches, 1938 touches, 1913 scored steps (1022 of them *switches*,
where the next file differs from the current one), four projects, candidate sets
of 57–214 files. Harnesses: `benchmarks/behaviour/{extract,e67,e70}.py`. No
model, no licence, no new subsystem.

Both experiments were pre-registered in
`docs/plans/2026-09-14-world-map-machine.md` §5 and §11, including the
thresholds and what each outcome would mean.

## E67 — geometry held still

Leave-one-session-out. At each step an arm ranks the project's files; the score
is where the file actually touched lands. Two populations, because the easy one
flatters everything: **all** steps (which are ~47% "touch the same file again")
and **switch** steps, where the current file is removed from the candidate set —
without that removal `lexical` holds rank 1 by scoring a file against its own
text, a prediction known to be wrong on exactly these steps.

Switch steps, mean over 25 held-out sessions:

| arm | top1 | top5 | MRR | Δtop1 vs recency+frequency [95% CI] |
|---|---:|---:|---:|---|
| recency+frequency (the arm to beat) | 0.046 | 0.121 | 0.095 | — |
| recency | 0.064 | 0.169 | 0.124 | +0.019 [−0.003, +0.039] ties |
| frequency | 0.000 | 0.066 | 0.033 | −0.046 [−0.088, −0.015] loses |
| transition | 0.032 | 0.054 | 0.059 | −0.013 [−0.058, +0.025] ties |
| co-access | 0.040 | 0.110 | 0.086 | −0.005 [−0.054, +0.041] ties |
| lexical (BM25) | 0.092 | 0.300 | 0.197 | +0.046 [−0.012, +0.097] ties |
| **tree (directory distance)** | 0.107 | 0.304 | 0.214 | +0.062 [−0.002, +0.119] ties |
| **structure = co-access + transition + tree** | **0.115** | **0.344** | **0.236** | **+0.070 [+0.010, +0.122] beats** |

The pre-registered gate was: beat recency+frequency by ≥ 0.05 top-1 with a CI
excluding zero, **and** beat the arm's own order-shuffled tables. The first half
passes. The second does not:

```
structure vs its own shuffled tables   top1 +0.011 [-0.014, +0.036]   TIES
transition vs its own shuffled tables  top1 +0.007 [-0.017, +0.032]   TIES
co-access vs its own shuffled tables   top1 +0.024 [+0.006, +0.048]   beats
structure vs tree alone                top1 +0.008 [-0.002, +0.025]   TIES
```

**Verdict: the gate fails, and the reason is specific.** Something in the
workspace does predict the next file — but it is the **directory tree**, which
is a static property of the repository and identical under any permutation of
the trace. Order-dependent structure (`transition`) sits at its shuffled null.
Only `co-access` clears its own shuffle, by +0.024 top-1, from a base so low
(0.040) that it changes no decision.

So the honest reading of E67 is not "behaviour carries geometry". It is:
**behaviour is predicted by where things sit in the workspace, not by the order
in which they were touched.** That is a fact about this data, at this scale, on
this choice space — 1938 touches on one machine over one week — and the plan
said in advance it would be underpowered for small effects.

## E70 — geometry allowed to move

Same data, same leave-one-session-out. State is a weight vector over five
feature arms; each step fuses them, measures the rank of the file actually
touched, and moves the weights toward whichever features ranked it best (Hedge,
η = 0.5). Five arms, each a control that could fire.

| arm | MRR | 1st half | 2nd half | within-session gain |
|---|---:|---:|---:|---:|
| **deform** | **0.479** | 0.501 | 0.457 | −0.044 |
| frozen | 0.417 | 0.438 | 0.396 | −0.042 |
| random-deform | 0.407 | 0.428 | 0.387 | −0.042 |
| deny | 0.416 | 0.441 | 0.392 | −0.049 |
| shuffled | 0.230 | 0.223 | 0.238 | +0.016 |

Paired over the 25 sessions, 95% bootstrap CI:

```
deform vs frozen          MRR +0.0626 [+0.0312, +0.0992]  beats
deform vs random-deform   MRR +0.0721 [+0.0361, +0.1126]  beats
deform vs deny            MRR +0.0631 [+0.0239, +0.1099]  beats
deform vs shuffled        MRR +0.2493 [+0.1724, +0.3179]  beats
```

**I5 (growth is measured) passes, barely and with a caveat.** The threshold was
a ≥ 10% reduction in mean reciprocal-rank error: frozen error 0.583, deformed
0.521, a 10.6% reduction. It clears the bar it was given, and it beats
`random-deform`, so it is *this* change rather than change in general.

The caveat is in the last column and it matters: **every arm gets worse within a
session**, deformed included (−0.044 from first half to second). The deformation
is not improving with experience; it is allocating weight quickly and then
holding on while the task drifts away from it. "Mistakes are the growth
function" is supported as *better than not deforming*, and not supported as
*improving over time* — which the within-session split makes visible and an
average over the session would have hidden.

**I2 (mismatch deforms, it does not deny) passes.** The `deny` policy — drop
the observation that embarrassed the model — is indistinguishable from `frozen`
(0.416 vs 0.417) and loses to `deform` by +0.063 [+0.024, +0.110]. Denial buys
nothing here; deformation does.

**I4 (aim at stable futures, not next states) fails.** From the first quarter of
a session, naming the files the last quarter will touch:

```
attractor (accumulated basin over the whole prefix)  precision 0.176
one_step  (the fused next-step ranking, rolled forward) 0.172
frequency (prior over the prefix)                       0.090

attractor vs one_step   +0.0043 [-0.0233, +0.0271]  TIES
attractor vs frequency  +0.0856 [+0.0339, +0.1466]  beats
```

Both beat the frequency prior. The attractor arm does **not** beat rolling the
next-step ranking forward. At this scale, on this task, aiming at stable futures
is a preference, not a property — exactly as §11 said it would have to be read
if the numbers came out this way.

## What the deformation learned

Mean final weight, uniform start 1.0:

```
lexical 2.472 · recency 1.333 · tree 0.966 · co-access 0.169 · transition 0.061
```

The loop independently drove the two order-dependent features to near zero. That
is E67's finding recovered by a different route, which is the kind of agreement
worth more than either number alone.

## The primitives this failure names

Per the plan: keep the failure, extract the next primitive.

1. **The choice space is too easy and too poor.** "Which file next" is dominated
   by where files sit. The next version scores *actions with outcomes* — the
   observer log's intent → argv → exit records — where a wrong prediction has a
   consequence a directory tree cannot explain.
2. **Order carries nothing here, so nothing built on order can be tested here.**
   Transitions (E56's measured strength on *text*) do not transfer to
   file-touch traces. Either the unit is wrong (files, not entities) or the
   horizon is: E56 worked over sentences within a document, not files within a
   week.
3. **Deformation needs a reason to keep moving.** It converges and then decays
   with the task. The primitive that is missing is *reproposal* — `dream.rs`
   exists and nothing calls it from this loop, which is the same
   write-with-no-read shape rule 1 exists to catch.

## Limits

One machine, one week, four projects, 1938 touches; every interval overlaps its
neighbour. Both experiments share the extractor, so a defect there is common to
both. `lexical` reads only a 16 KiB prefix per file and only files still present
on disk — moved or deleted files are silently absent from that arm and present
in every other. Nothing here is evidence about a different agent, a different
codebase, or a longer horizon.
