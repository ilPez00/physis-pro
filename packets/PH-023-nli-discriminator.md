---
id: PH-023
track: physis-core
phase: 1
status: todo
rung: 3
depends: [PH-021]
gate: "cd ~/dev/computer-remake-research/experiments/nli && NLI_URL=<local> NLI_MODEL=<cross-encoder> python3 gate.py  # ships only at precision >= 0.90 AND recall > 0.50 on the 86-pair candidate set"
commit: "branch agent/PH-023; conventional commits; per PROTOCOL §6"
claimed_by:
claimed_at:
---

# Replace the contradiction discriminator with a local NLI cross-encoder

## Why (the failure this prevents)

Contradiction recall has been 0.50 since it was first measured, and the missed
pair is not a near miss — it is structurally invisible to the current rule:

```
extra-window-a  ...window is mandatory from 06:00 and night access is forbidden.
extra-window-b  ...window is forbidden from 06:00 and night access is required.
```

A **swap**. Same words, same numbers, cosine 0.984, `na == nb == 0`. Counting
negation markers cannot see it at any threshold, and adding "forbidden" to
`NEGATION_MARKERS` makes both sides score 1 — still equal, still missed.

Measured 2026-09-13 (`docs/plans/2026-09-13-nli-vs-cosine-contradiction.md`),
over the 86-pair candidate set the rule actually considers:

| arm | precision | recall |
|---|---|---|
| incumbent `map.rs` | 1.000 | 0.50 |
| null: cosine ≥ 0.95 | 0.054 | 1.00 |
| NLI `qwen2.5:7b` (local) | **0.054** | 1.00 |
| NLI `nemotron-3-super-120b` | **1.000** | **1.00** |

At 120B NLI strictly dominates the incumbent. At 7B it *is* the null. The whole
packet is about closing that gap without a hosted model.

## Context (measured 2026-09-13, verify before relying on it)

- `src/map.rs` — candidate gate `lexical_overlap >= 0.5`, then
  `cos < 0.55 || na != nb || number_clash`.
- The candidate set is 86 pairs of 435; both gold pairs are inside it. The
  discriminator is what is under test, not the candidate generator.
- `PH-022` fixed `number_clash`, which had been dead. It changed nothing here:
  `spec-A/B` was already caught by negation asymmetry.
- The experiment harness already exists and takes `NLI_URL`, `NLI_MODEL`,
  `NLI_TAG`, `NLI_KEY`:
  `~/dev/computer-remake-research/experiments/nli/gate.py`.

## Steps

1. **Try a dedicated cross-encoder before anything else.** DeBERTa-v3-MNLI
   class, ~400 MB, purpose-built for entailment/neutral/contradiction. The 7B
   result is evidence about a general chat model, not about local NLI. Run the
   existing harness against it — no new experiment code is needed.
2. If it clears the gate, wire it as the **discriminator only**. Cosine keeps
   doing candidacy; the cross-encoder runs on within-cluster pairs (86, not
   435), so cost is bounded by clustering that already exists.
3. Emit a `machines::Observation` (PH-021) with `machine: "nli"` and
   `ProofStatus::Numerical` — **not a boolean**. An NLI model is an interpreter,
   and a contradiction certified by an unrecorded model is graphiti's failure
   mode with a different noun.
4. Keep the symbolic legs. They are precision 1.000 and free; NLI adds recall.
   The rule becomes `symbolic OR nli`, and the report says which fired.
5. If no local model clears the gate, **set `status: blocked` and say so**. That
   is a real result about the offline claim, and it belongs in RESCOPE §6, not
   in a retry.

## Tests you must add

- **Unit:** a swap pair (`mandatory/forbidden` exchanged across two arguments)
  is flagged, and an instance-id pair (`Release 17` vs `Release 18`) is not.
  The second is the one that fails if the discriminator becomes a yes-machine.
- **Unit:** the NLI leg emits an `Observation` whose `status` is `Numerical` and
  whose `machine` names the model. A contradiction with no provenance must not
  be representable.
- **Harness:** extend the gold set beyond two pairs. Rule-generated contrast
  pairs — swap, negate, quantifier, double negation, conditional — so the
  correct label is known by construction and nobody authors the answer. Same
  shape as `benchmarks/direction/run.py`.

## Gate

```bash
cd ~/dev/computer-remake-research/experiments/nli
NLI_URL=<local endpoint> NLI_MODEL=<cross-encoder> NLI_TAG=local python3 gate.py
```

Ships only at **precision ≥ 0.90 AND recall > 0.50** on the 86-pair candidate
set. Precision is the binding constraint: the null already has recall 1.00, so
recall alone is worth nothing. A run with any errored pair reports no rate.

## Never

- Never ship the hosted model as the answer. Physis is offline-first; a hosted
  discriminator moves the trust boundary off the machine and makes every
  contradiction depend on an endpoint that returned five 503s during a 86-pair
  run.
- Never let NLI replace the symbolic legs. They are exact, free, and precision
  1.000. NLI is additive.
- Never report recall without precision. At cosine ≥ 0.80 the null scores
  recall 1.00 and precision 0.023.
