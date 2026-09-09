# E29 — the corpus has no mode structure to discover

**The machine cannot propose the rewrite.** That is the result, and it is more
useful than a proposal would have been.

E22 localised the mode axis's failure to WORK/CREATE/MAINTAIN/SENSE — 397
entries, 68% of the corpus, ample population, no separation. E27 closed off
merging, E28 closed off head-only representation and factorisation. What was left
was definitional, so `run_e29.py` does the part a machine can do: cluster those
397 with nothing imposed, and show what they group into.

## They group by subject matter, not by activity

k-means in the shipping α = 0.5 space, k = 5. Every discovered group cuts almost
evenly across all four current modes:

| group | n | how it cuts across the current four | what it actually is |
|---|---:|---|---|
| 1 | 61 | WORK 23, SENSE 14, CREATE 14, MAINTAIN 10 | manufacturing and tooling — *Manufacturing Processes, Semiconductor Equipment, Machine Design, Tool Creation* |
| 2 | 29 | CREATE 12, WORK 11, SENSE 4, MAINTAIN 2 | engineering disciplines — head word **Engineering ×15**; *Structural, Ocean, Automotive, Transportation* |
| 3 | 135 | WORK 39, CREATE 39, SENSE 37, MAINTAIN 20 | agents, monitoring, detection — *Monitoring & Observability, Error Detection, Risk & Issue Tracking* |
| 4 | 116 | WORK 38, MAINTAIN 36, CREATE 27, SENSE 15 | management, organisation, personal practice — head word **Management ×12** |

**These are domains, not modes.** Group 2 is "things called Engineering". Group 1
is "factory floor". Unsupervised structure in this representation recovers
*subject matter*, which is exactly what E22 predicted when it found hints to be
domain-loaded — and it holds even at α = 0.5, where hints are already halved.

## And the discovered carves are worse objects than the current four

Held-out compression (E26's criterion; nearer 0 is more coherent):

| partition | compression |
|---|---:|
| **the current four** | **−0.014** |
| discovered k = 4 | −0.049 |
| discovered k = 5 | −0.082 |
| discovered k = 6 | −0.073 |

The existing four labels, whatever their faults, are **more coherent than
anything the geometry proposes**. There is no better carve hiding in this space
waiting to be found.

## What this settles

Every computational route to the mode axis is now closed, and each closed for a
different reason:

| route | result |
|---|---|
| coarser granularity (E27) | merging makes the axis *less* of an object; −0.187 at k = 5 vs domain's −0.007 |
| better representation (E28) | head-only is tidier and −0.091 worse at predicting |
| factorised decoding (E28) | −0.028; the axes share signal and are not independent |
| stronger embedder (E21) | mode moves +0.065 and does not close |
| **unsupervised discovery (E29)** | **finds domains; the current four are more coherent** |

**Mode is not underdetermined by its labels. It is absent from the
representation.** The four definitions cannot be repaired *from this corpus*,
because the corpus does not encode the distinction they are trying to draw. An
entry here is a name and some subject-matter hints — a description of what
something *is about*. Mode asks what is being *done*, and nothing in the record
observed anything being done.

**Which returns the question to where it started.** E23 found that machine
supervision is real where a convention or an exact log carries the label. Mode
needs entries observed *in use*, not described — an activity trace, not a
richer description. That was the original intuition behind bringing compilers and
command errors in, and it is now the only route left standing for this axis.

## Limits

1. k-means with cosine assignment, one seed, k ∈ {4, 5, 6}. A different clustering
   family could find a different structure; the compression column is the guard
   against over-reading any particular carve, and every carve tried was worse
   than the status quo.
2. Restricted to the four blurred modes. The other ten were not clustered, and a
   corpus-wide re-carve was not attempted.
3. Absence of evidence: this shows mode is not recoverable *by these methods in
   this space*, not that it is unrecoverable in principle.
