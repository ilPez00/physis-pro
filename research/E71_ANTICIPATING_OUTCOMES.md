# E71 — can the world model anticipate what an action does to the world?

Run 2026-09-14. E67/E70 scored "which file next" and found the **directory
tree** explains it — a static property of the workspace, not anything learned.
E71 asks the question a tree cannot answer: *will this action fail?* The label
is the tool's own outcome, which is the one place in this data where reality,
not the model, has the last word.

**Data.** The same 165 transcripts, read as actions rather than touches:
`benchmarks/behaviour/extract_actions.py` pairs every `tool_use` with its
`tool_result`. 27 sessions with ≥ 20 actions, **9466 actions**, 218 action kinds
(`bash:cd`, `Edit`, `bash:grep`, `bash:timeout`, …), 10 results unmatched. Five
times the data E67 had, and outcome-bearing.

**Two labels, both reported.** `harness` is the transcript's own `is_error` flag
(positive rate 0.031); `markers` adds a failure marker at the start of a line of
output — Traceback, `error[E0433]`, FAILED, fatal:, panicked at, command not
found (0.060). The first is clean and narrow, the second broad and noisy (a
command that merely *prints* "error" counts). Every finding below holds under
both.

**Task.** Leave-one-session-out. Before each action's result is seen, the arm
emits P(failure) from the prefix alone: the kind's failure rate in other
sessions, its rate so far here, whether the previous action failed, the current
failure streak, whether this exact target failed earlier, whether the kind is
new, and position in the session. Scored by Brier (lower better) and AUC, paired
over sessions.

## Results — `harness` label, lr 0.30

| arm | Brier | AUC | 2nd − 1st half Brier |
|---|---:|---:|---:|
| base-rate (the null) | 0.0326 | 0.500 | +0.0017 |
| kind-prior | 0.0305 | 0.620 | +0.0021 |
| frozen (fitted on other sessions, never updated) | 0.0306 | **0.645** | +0.0008 |
| **deform** (same, updated online on every outcome) | **0.0299** | 0.588 | −0.0007 |
| cold-deform (online from nothing) | 0.0449 | 0.471 | −0.0205 |
| deny (a surprise failure is discarded, not learned from) | 0.0317 | 0.635 | +0.0019 |
| shuffled (deform on a permuted order) | 0.0300 | 0.549 | −0.0059 |

```
deform vs base-rate    Brier -0.00267 [-0.00488, -0.00099]  beats
deform vs kind-prior   Brier -0.00059 [-0.00272, +0.00086]  TIES
deform vs frozen       Brier -0.00069 [-0.00196, +0.00017]  TIES
deform vs deny         Brier -0.00180 [-0.00352, -0.00048]  beats
deform vs shuffled     Brier -0.00003 [-0.00054, +0.00053]  TIES
deform vs cold-deform  Brier -0.01497 [-0.02105, -0.00955]  beats
deform vs frozen       AUC   -0.0572 [-0.1028, -0.0173]     LOSES
```

The `markers` label reproduces every line of that shape: deform beats base-rate
(−0.0030) and deny (−0.0031), ties kind-prior, frozen and shuffled on Brier, and
loses to frozen on AUC (−0.0539 [−0.0844, −0.0237]).

**Step size is not the explanation.** Swept, and reported rather than chosen:

| lr | deform vs frozen, Brier | deform vs frozen, AUC |
|---:|---|---|
| 0.05 | −0.00023 [−0.00050, −0.00004] beats | −0.0294 [−0.0411, −0.0163] loses |
| 0.30 | −0.00069 [−0.00196, +0.00017] ties | −0.0572 [−0.1028, −0.0173] loses |
| 0.60 | −0.00060 [−0.00194, +0.00045] ties | −0.0704 [−0.1230, −0.0239] loses |

## What this says

**1. Anticipation is possible, and it is a prior, not a lesson.** Failure is
predictable above chance (frozen AUC 0.645, kind-prior 0.620 against 0.500), and
nearly all of that comes from *which kind of action this is*, learned from other
sessions. Within-session adaptation adds nothing to it — and `cold-deform`,
which has only within-session experience, is worse than the base rate.

**2. E70's I5 result has to be re-read, and narrowed.** In E70 the `frozen` arm
was *uniform* weights: deformation beat having no model at all. Here `frozen` is
a model already fitted on other sessions, and deformation does **not** beat it —
it ties on Brier and is reliably worse at ranking. The honest statement is:

> Deforming on mismatch beats starting from nothing. It does not beat starting
> from a model fitted on other runs of the same kind of work.

That is a weaker claim than "mistakes are the growth function", and it is the
one the measurements support. The growth function currently does the job a prior
would have done better.

**3. I2 survives its rival a second time.** `deny` — discard the failure the
model did not expect — is worse than `deform` on both labels
(−0.0018 and −0.0031, CIs excluding zero) and worse than `frozen` on Brier.
Refusing to update on the observations that embarrass the model costs
measurably, on 9466 actions. Nothing in this series has yet favoured denial.

**4. Order carries nothing, for the third time.** `shuffled` ties `deform` on
Brier under both labels, exactly as transitions tied their shuffled tables in
E67 and as E70's loop drove the order-dependent features to near zero. Three
independent routes, same answer: **on this data, at this horizon, sequence is
not where the information is.**

## The primitives this names

- **The prior is the product.** If cross-session priors over action kinds carry
  the signal and within-session updates do not, the thing worth building is the
  store that *accumulates* those priors across sessions, machines and actors —
  which is what the observation log and `clients/opencode-physis-observer` are
  already for, and what nothing currently reads back at prediction time.
- **Adaptation needs a signal the prior cannot express.** Failure of a *kind* is
  priored. What a prior cannot know is the state of *this* workspace right now —
  an edit that has not compiled yet, a service that is down. Those are
  observations the log holds and these features do not use.
- **Stop testing order on this horizon.** Three negatives is enough. If
  transitions matter, they will show up between entities inside a task, not
  between tool calls across a week.

## Limits

One machine, 27 sessions, one week, mostly four projects; 294 harness failures
in 9466 actions, so every AUC rests on a few hundred positives. Four sessions
contain no failure at all and are excluded from AUC (not from Brier). The label
is what the harness reported, not what the user judged: a command that "failed"
usefully and one that failed uselessly are the same row.
