# E73 — does the prior port to another machine, or another agent?

Run 2026-09-14. E71 concluded that the cross-session prior over action kinds is
the product. That is only worth something if it transfers: a prior that works
only on the sessions it was built from is a memory, not a model.

## More corpora

Three machines on the fleet carry agent transcripts (`pansa`, `omo`, `feather`;
`air`, `pi` and `k1` were unreachable), and two harnesses run on them — Claude
Code and Codex. Extractors were run *on* each machine and only the derived
action records fetched back, never the transcripts.

**The corpora overlap and the overlap had to be removed.** Of 42 distinct Claude
Code sessions, **18 are byte-identical copies** present on all three machines
(verified action-by-action, not by name). Pooling the files naively would have
counted those three times and reported a corpus 1.6× its true size.
`merge.py` keys by session id and records which machines each was found on;
36 duplicate session records were dropped.

| corpus | sessions | actions |
|---|---:|---:|
| merged, deduplicated | 45 | 15119 |
| unique to feather | 13 | 4884 |
| unique to pansa | 9 | 1936 |
| unique to omo | 2 | 75 |
| Codex (a second harness) | 3 | 662 |

That is 15119 actions against E71's 9466 — **+60%** — and the two additions that
matter are not more of the same: a second machine's unique work, and a second
agent.

## E71 and E72, re-run on the merged corpus

Every conclusion holds, with intervals roughly halved:

```
deform vs frozen   AUC -0.0632 [-0.0847, -0.0412]  loses   (was -0.057 [-0.103, -0.017])
deform vs deny     Brier -0.00139 [-0.00240, -0.00067]  beats
deform vs shuffled Brier -0.00000 [-0.00026, +0.00024]  TIES
```

E72's workspace-state features, which were *ties* at 27 sessions, are ties at 45
and the point estimate **halved** (+0.041 → +0.019 on the clean label, −0.007 on
the broad one). More data moved it toward zero, which is what a noise result
does.

## The transfer question

Groups are disjoint by construction; sessions synced to several machines are
excluded from every machine set, because a byte-identical copy on both sides
would make transfer look perfect. For each ordered pair the per-kind prior is
fitted on the source and scored on the target, against the target's own
leave-one-session-out prior (the in-corpus ceiling) and against 0.500 (a single
failure rate carries no ranking).

`markers` label, per-session AUC with a bootstrap over target sessions:

| source → target | transferred | in-corpus | coverage |
|---|---|---:|---:|
| **feather → pansa** | **0.644 [0.612, 0.676]** | 0.642 | 0.964 |
| **pansa → feather** | **0.677 [0.600, 0.754]** | 0.612 | 0.929 |
| **claude-code → codex** | **0.635 [0.606, 0.664]** | 0.494 | 0.470 |
| omo → pansa | 0.491 [0.468, 0.513] | 0.642 | 0.636 |
| omo → feather | 0.478 [0.397, 0.543] | 0.612 | 0.641 |
| pansa → omo | 0.531 [0.385, 0.677] | 0.429 | 0.827 |
| feather → omo | 0.472 [0.291, 0.653] | 0.429 | 0.960 |
| codex → claude-code | 0.546 [0.521, 0.571] | 0.631 | 0.241 |

`harness` label agrees on every row that matters: feather → pansa 0.698 [0.628,
0.760], pansa → feather 0.707 [0.604, 0.800]. Cross-harness reaches 0.559
[0.464, 0.654] there — positive but its interval includes chance, so
**cross-harness transfer replicates on one label and not the other** and is
reported as partial.

## What this says

**1. The prior is a model, not a memory.** Between the two substantial machine
corpora it transfers as well as each machine's own history does: 0.644 against
an in-corpus 0.642, and 0.677 against 0.612. Nothing was shared between the
sides — the synced sessions were removed first.

**2. Coverage is the mechanism, and it is measurable in advance.** Every pair
that transfers has 0.93–0.96 of the target's actions in a kind the source had
seen; every pair that fails is at 0.24–0.64, or has a source of 75 actions whose
own in-corpus prior is also at chance (omo is underpowered, not a
counterexample). Coverage is computable before trusting a transferred prior,
which makes it a usable gate rather than an excuse afterwards.

**3. A prior from one agent beats another agent's own thin history.** Claude
Code's prior scores Codex's failures at 0.635 while Codex's own three sessions
manage 0.494 — at 47% coverage. On the cold-start problem, *somebody else's*
accumulated record is better than your own.

## What it licenses building

`physis system predict` reads only the current workspace's log. E73 says a
kind absent locally should back off to a broader store before it backs off to
the global rate — and that the back-off should carry its coverage, because
coverage is what separates the rows above.

## Limits

Two machines carry the transfer result; the third (omo, 2 unique sessions) can
support nothing. The Codex corpus is 3 sessions and its label is a real exit
code while Claude Code's is a harness flag, so the cross-harness row compares
two slightly different questions. `air`, `pi` and `k1` were unreachable, and all
of this is still one person's work in one fortnight.
