# E72 — does the state of *this* workspace add anything to the prior?

Run 2026-09-14. E71 ended with a named gap: the cross-session prior over action
kinds carries the predictive signal, and what a prior cannot hold is the state of
the workspace right now — an edit that has not compiled yet, a service that is
down. E72 measures exactly that increment on the same 9466 recorded actions,
**before** any of it is shipped into `physis system predict`.

## Arms and the gate

Fitted leave-one-session-out over 27 sessions, scored by AUC and Brier, paired
over sessions:

| arm | features |
|---|---|
| `kind-prior` | the failure rate of this action kind elsewhere — what ships today |
| `fitted` | + session state: last failure, current streak, this target failed before |
| `fitted+edits` | + edits since the last success of this kind, and edits in the last five actions |
| `edits-only` | the new features alone, without the prior |

Pre-registered: the state features are shipped **only if** `fitted+edits` beats
`kind-prior` on AUC with a bootstrap CI over sessions excluding zero.

## Result — the gate fails, on both labels

`harness` label (3.1% positive):

| arm | AUC | Brier |
|---|---:|---:|
| kind-prior | 0.620 | 0.0305 |
| fitted | 0.639 | 0.0307 |
| **fitted+edits** | **0.661** | 0.0307 |
| edits-only | 0.568 | 0.0327 |

```
fitted        vs kind-prior   AUC +0.0194 [-0.0233, +0.0655]  TIES
fitted+edits  vs kind-prior   AUC +0.0406 [-0.0175, +0.1055]  TIES
fitted+edits  vs fitted       AUC +0.0213 [-0.0138, +0.0535]  TIES
```

`markers` label (6.0% positive):

```
kind-prior 0.666 · fitted 0.665 · fitted+edits 0.666 · edits-only 0.542

fitted+edits  vs kind-prior   AUC -0.0003 [-0.0324, +0.0328]  TIES
edits-only    vs kind-prior   AUC -0.1240 [-0.1845, -0.0663]  loses
```

**Nothing is shipped.** The +0.041 on the clean label is inside its own
confidence interval, and on the broader label the same comparison is −0.0003 —
flat to four decimals. A difference that appears under one labelling and
vanishes under another, neither significant, is not a finding.

## What it does and does not license

- `edits-only` reaches AUC 0.568 on the harness label, so the edit signal is not
  *nothing*. It is just not anything **the prior does not already carry**: an
  action kind that fails after edits (a build, a test) has a high base rate
  precisely because it is the kind that runs after edits.
- The honest reading is **underpowered, not refuted**. The CI half-width here is
  ±0.06 AUC on 27 sessions; resolving a real +0.04 would need roughly four times
  the sessions. That number is stated so the next attempt has a size to hit
  rather than an intuition to follow.
- The proxies are weak by construction. "Edits since this kind last succeeded"
  is read off the action stream, not off the workspace: it cannot tell an edit
  that changed a header from one that fixed a typo in a comment, and it knows
  nothing about a service being down. A real state signal — compile status,
  test status, whether a port answers — is not in this data at all.

## Consequence for the shipped predictor

`physis system predict` stays exactly as measured in E71: the per-kind prior,
its counts, and `NOT MEASURED` on an empty log. The one design change E72 does
justify is negative and worth recording — **do not** add session-state features
to it on the strength of the harness-label number alone, which is what a reading
that stopped at the first table would have done.

## Limits

Same corpus as E67/E70/E71: one machine, 27 sessions, mostly four projects, 294
harness failures. The two labels are not independent — `markers` is a superset
of `harness` — so agreement between them is weaker evidence than two separate
corpora would be.
