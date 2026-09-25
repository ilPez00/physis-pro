# P10 related-work audit

## Searches run
- "false positive power law detection heavy tail lognormal model comparison
  Clauset pitfalls" — **returned no usable results** (backend failure, empty set).

## Current citation basis
P10 cites Clauset, Shalizi & Newman (2009), which is the correct primary
reference for the fitting procedure and for the lognormal-alternative test it
uses. The paper's own control battery (critical branching, 2D percolation,
subcritical/supercritical branching, lognormal sigma 0.6/1.2/2.0) is original
to this work.

## Gap this pass could not close
The known literature on *misidentification* of power laws — competing heavy-tail
families, x_min selection sensitivity, likelihood-ratio caveats, and the
practical rate of false power-law calls — is not retrieved or cited. P10 makes a
specific empirical claim that its pipeline over-calls power laws on wide
lognormals (12/12 seeds at sigma 1.2 and 2.0) and that this is bias *in favour*
of the refuted claim. That claim is stronger with the prior literature attached.

## Required before submission
Re-run the search with a working backend and cite:
- competing heavy-tailed families (lognormal vs power law vs stretched
  exponential) and likelihood-ratio-vs-goodness-of-fit distinctions
- the sensitivity of x_min by KS minimisation (the paper already observes the
  failure mode where x_min lands at 2–3; this is documented in the literature)
- any published estimate of false-positive rates for power-law fits

## Not claimed
P10 does not claim the true distribution is lognormal. Its controls show the
pipeline cannot separate the two families at this cluster count, and the paper's
language was checked against that.
