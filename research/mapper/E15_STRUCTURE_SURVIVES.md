# E15 — Stage 12's refutations were measured on the wrong task

`research/mapper/run_e15.py` — five scorers × four injection policies, one pass,
everything read from artifacts already on disk.

E14 showed the geometric scorer collapses 0.880 → 0.506 once the injected error
becomes plausible. Stage 12 refuted every structural scorer *against that 0.880*.
So the refutations were decided on a task nobody faces. This measures the
structural scorers under the same policies.

## Result — the ranking inverts

| scorer | random | same-domain | same-mode | **nearest** | drop |
|---|---:|---:|---:|---:|---:|
| **COSINE** | **0.882** | 0.824 | 0.777 | **0.506** | **−0.376** |
| NAME-ONLY | 0.866 | 0.822 | 0.753 | **0.597** | −0.268 |
| **CONSTRAINT** | 0.675 | 0.525 | 0.697 | **0.612** | **−0.063** |
| ANCHORVOC | 0.654 | 0.646 | 0.631 | 0.584 | −0.070 |
| ANCESTRY | 0.627 | 0.615 | 0.609 | 0.566 | −0.061 |

*Stage 9, the corpus's own real misfilings: 0.410*

**Under random injection cosine leads by 0.207. Under realistic injection it
trails CONSTRAINT by 0.106.** The structural scorers barely move (−0.06 to
−0.07) while geometry loses 0.376.

Bootstrap over entries, `nearest` injection, 2000 resamples:

| comparison | diff | 95% CI | |
|---|---:|---:|---|
| CONSTRAINT − COSINE | **+0.106** | [+0.029, +0.183] | **significant** |
| NAME-ONLY − COSINE | **+0.091** | [+0.034, +0.153] | **significant** |
| ANCHORVOC − COSINE | **+0.078** | [+0.007, +0.151] | **significant** |
| ANCESTRY − COSINE | +0.035 | [−0.055, +0.127] | not significant |

## What this does and does not establish

**It does NOT establish that structural scorers work.** The best is 0.612, which
is not a usable certifier, and on Stage 9's *real* misfilings nothing reached
above 0.410. A plausible synthetic error is a proxy for a real one, not the
thing itself.

**It does establish that Stage 12's comparison was invalid.** CONSTRAINT was
refuted for losing to cosine 0.685 vs 0.733 — a ranking that reverses on the
task the system actually has. *"Plain cosine beat every structural alternative
tried against it"* has been this track's summary sentence for the whole
certification effort. It is true only of the injection policy, not of the
problem.

**Why structure is robust and geometry is not** is visible in E14's displacement
figures: random injection moves an entry 0.125 below its home similarity, a
plausible error 0.029. Geometry scores *distance moved*, so it degrades with the
distance. CONSTRAINT asks whether a discrete field is attested elsewhere in the
domain — a question whose answer does not soften as the error gets subtler.

**NAME-ONLY beating COSINE (+0.091) is a second finding.** Dropping the hints
makes the geometric scorer more robust to plausible errors. That connects to
Stage 13, where hints carried authorial format rather than meaning: the hint
text helps on easy errors and hurts on hard ones.

## Limitations, stated

1. These are **Python re-implementations**, not experiment38's exact scorers.
   CONSTRAINT reproduces at 0.675 against Stage 12's 0.685 under the same policy,
   which validates the port but does not make it identical.
2. `nearest` injection is a **proxy** for a careful author's error, chosen
   because it is the most plausible error available by construction. Real
   misfilings may be different again — Stage 9's 0.410 sits below every number
   here.
3. One corpus, one split, and absolute levels remain low throughout.

## The re-test that now matters most

**Score these five on the corpus's real misfilings**, not on injections. The
Stage 11 regeneration produced adjudication artifacts
(`cell_adjudication.tsv`, `refiling_gate0.tsv`) that carry human judgements about
actual filings. That is the only ground truth in this project that is not
synthetic, and E15 makes it worth spending: if CONSTRAINT holds its margin there,
the track has a certification signal it spent a year concluding it did not have.

If it does not, then the honest position is that plausible-synthetic and real
errors differ again, and the human-in-the-loop conclusion stands — but for a
better-understood reason than before.
