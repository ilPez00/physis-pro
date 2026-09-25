# P1 related-work audit

## Searches run
- "tool call failure prediction coding agent reliability telemetry" — **returned
  no usable results** (backend failure, empty result set).
- "operational failure prediction / cross-agent transfer / agent telemetry" —
  no dedicated hit obtained.

## What this means for the paper
P1 currently positions itself against its own prior experiments and against the
E5/E74 line, and cites no external cross-agent transfer work. That is a **gap,
not a strength**: the literature may well contain agent-reliability and
tool-use-failure work P1 should be measured against, and this pass could not
retrieve it.

## Required before submission
A second literature pass with a working search backend, covering:
- tool-use failure prediction / reliability of LLM agents
- cross-task and cross-model transfer of behavioural priors
- bootstrap inference under session-level clustering (the unit-choice argument
  P1 makes is standard in the clustered-data literature and should cite it)

## Do not claim
Novelty for "failure priors transfer between agents" as a *question*. The
defensible contribution is the measured answer on a four-harness corpus plus the
multiplicity/partition distinction, not the framing.
