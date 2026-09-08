# E18 — ANCHORVOC closed, and the port checked

`research/mapper/run_e18.py` — three questions in one pass, no embedding.

E16 found ANCHORVOC the only scorer beating chance on real misfilings (0.664).
E17 found the confound tests disagreed. This settles it, and checks two things
E15/E16 had flagged as unverified.

## 1. Port validation — and one number needed restating

E15/E16 used Python re-implementations of experiment38's Rust scorers.
`corpus_fit.tsv` holds the Rust values, so the port is checkable.

| scorer | Pearson r vs Rust | |
|---|---:|---|
| CONSTRAINT | **+0.943** | faithful |
| ANCESTRY | **+0.976** | faithful |
| **COSINE** | **+0.197** | **divergent** |

**The divergence is expected and is not a port bug: experiment38 embeds with
MiniLM (384-d), and this corpus is embedded with BGE (768-d).** The structural
scorers are embedder-independent by construction, so they port exactly;
geometry is not.

The consequence is worth stating precisely: **E15/E16's COSINE is the *stronger*
BGE baseline**, not the MiniLM one Stage 12 refuted against (E1: BGE beats
MiniLM by +0.132 on injected errors). The structural scorers were therefore
compared against a harder baseline than Stage 12 used, not an easier one.

## 2. ANCHORVOC controls — most of it is entry length

| arm | AUC | vs COSINE (95% CI) | |
|---|---:|---:|---|
| COSINE (reference) | 0.519 | — | |
| **ANCHORVOC (E16's result)** | **0.664** | +0.145 [+0.072, +0.219] | sig |
| normalised by entry length | 0.667 | +0.148 [+0.074, +0.223] | sig |
| CONTROL — a **random** cell's anchor | 0.565 | +0.046 [−0.022, +0.111] | ns |
| **CONTROL — entry length only** | **0.619** | **+0.100 [+0.027, +0.179]** | **sig** |
| CONTROL — anchor size only | 0.495 | −0.024 [−0.109, +0.060] | ns |

**Entry length alone reaches 0.619 of ANCHORVOC's 0.664.** Longer entries were
more likely to be moved, and ANCHORVOC's overlap count grows with entry length,
so most of the "signal" is a text-length effect.

The decisive comparison — does any vocabulary effect survive once length is
controlled?

```
normalised ANCHORVOC  vs  length-only  :  +0.048  [-0.019, +0.116]   NOT significant
```

**No.** The random-anchor control is also not significant, which is consistent:
what little is left is not specific to the cell's own vocabulary either.

## 3. Second ground truth — it does not replicate

`cell_adjudication.tsv` audits the **original shipped filing**, which the rater
did *not* produce — more independent than `refiling_gate0.tsv`.

| positives | ANCHORVOC | COSINE |
|---|---:|---:|
| NO only (26 of 60) | 0.544 | **0.691** |
| NO + MARGINAL (41 of 60) | 0.662 | **0.739** |

**Cosine beats ANCHORVOC on both definitions**, reversing E16's ordering. n = 60
with sparse cells, so this is a consistency check rather than a second
measurement — but it fails in the direction that matters.

## Verdict

```
ANCHORVOC IS NOT A CERTIFICATION SIGNAL
```

Three independent reasons, any one of which would be enough:

1. **Most of it is entry length** (0.619 of 0.664), and no vocabulary effect
   survives controlling for length (+0.048, ns).
2. **It does not replicate** on the more independent ground truth, where cosine
   beats it.
3. **E17's Test 3 already flagged** hint-dependence a filing-independent signal
   should not have.

## Where E14–E18 leave the track

| claim | status |
|---|---|
| Geometry certifies filings | **NO** — 0.519 on real misfilings |
| Stage 12's refutations were validly *reasoned* | **NO** — wrong injection policy (E14/E15) |
| ...but were they wrong? | **NO** — structure fails on real errors too (E16) |
| Lexical anchor overlap certifies filings | **NO** — length artifact, fails replication (E18) |

**The track's central negative stands, and now stands for the right reason.**
Before E14 it rested on a comparison decided by an injection policy that
inverted the ranking. It now rests on three scorer families measured against two
real ground truths, with the length and vocabulary confounds separated.

*No tested mechanism detects a real misfiling.* The human-in-the-loop conclusion
is not a fallback from a failed search — it is the finding.

## The arc, kept because the process is the point

E15 reopened the question on plausible-synthetic errors. E16 narrowed the
reopening to one scorer. E17 found its confound tests disagreed. E18 closed it.
**Four experiments, each one correcting the last, and the final position is the
one the track held before E14 — for entirely different and much better reasons.**
