# E87 — MaxSim doubles the pooled arm and still lands inside the null

> Follows the E80 report: file-level nearest neighbour sorts a directory
> (0.643, decisive); pooling each project's files into one mean vector
> destroys the signal (0.400, inside the null). The retrieval literature's
> answer to pooling is late interaction (ColBERT's MaxSim): score two
> aggregates by the best-matching pair of their parts. E87 ports that exact
> move to the project level — score(P, Q) = max cosine over file pairs —
> reusing E80's sampling, embedding and ground truth unchanged. BGE-base, 140
> files, 10 projects, 4 kinds. Script and raw results: the e87 files in
> benchmarks/conatus.

## Result

| arm | accuracy | label-permuted null |
|---|---:|---|
| pooled (E80's arm, rerun) | 0.200 | [0.000, 0.500], mean 0.215 |
| MaxSim (E87) | 0.400 | [0.000, 0.600], mean 0.222 |

MaxSim doubles pooled in the same run, and both sit inside their null bands.
E80's bottom-up vote (0.600) still beats both.

Two cautions, both honest. First, the pooled baseline reads 0.200 this run
against 0.400 in the E80 report — the tree moved under the experiment (repos
grow; sampling is deterministic but the directories are not), so the only fair
baseline is the rerun arm in this same run, not the report number. Second,
n=10 projects is why the band spans 0.000–0.600: nothing short of the vote arm
can leave a band that wide.

## What the misses say — the part that earns its keep

Every MaxSim miss is a near-identical file (scores 0.886–0.941):

- praxis-android / praxis_webapp and agora → praxis_webapp: sibling projects
  sharing vendored/config files the skip-list does not cover.
- llm-wiki-rag / trustgraph ↔ repowise: same phenomenon across the webapp line.
- physis-core ↔ physis-global and aion ↔ multiagent score correctly — the rust
  and python cores have no shared near-duplicates.

MaxSim is a best-case statistic: one duplicated file between two projects
decides their pairing. Mean-pooling buries that file; the majority vote
(median-like) outvotes it. The three arms form a strictness hierarchy — vote
beats MaxSim beats pooled — and the hierarchy is exactly what the content says
should happen: the right statistic for shared-duplicate corpora is the robust
one, not the best-case one.

## What this changes

* The E80 pooling verdict survives its own retried baseline plus one serious
  challenger. The literature-motivated fix moves accuracy 0.200 → 0.400 but
  cannot leave the null — so R7-style late interaction is not the design the
  recursion wants; the existing majority vote is, and E87 is why the vote arm
  should be the one the bottom-up design keeps.
* Pooled baselines must be rerun in-run, never cited from the report — the
  0.400 → 0.200 drift is a warning against every frozen baseline in this
  track.
* The shared-duplicate misses name the next confound to control: dedupe or
  near-dedupe sampled files before scoring, and re-check whether MaxSim's
  misses or the vote's remain.

## Limits

n=10 projects, 4 kinds — no accuracy between 0.000 and 0.600 is evidence for
anything except against its own null. Permutation null at 200 rounds (not 500,
time-bound) — replay with 500 before citing the band.
