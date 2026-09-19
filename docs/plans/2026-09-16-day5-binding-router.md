# Day 5 — the binding route fails; keep the rules and review the residual (2026-09-16)

Day 5 of `docs/plans/2026-09-15-core-pro-development.md`. The research question:
*can explicit participant/time binding restrict the NLI residual enough to
preserve the rule baseline's precision while finding more implication and
dictionary-gap cases?*

**Answer: no, not on this evidence.** The route is worse than the bound rules on
both precision and overall recall. The plan's own fallback applies — ship the
rules plus explicit review for the residual — and the experiment identified two
specific, fixable blind spots rather than a dead end.

Deliverables:

- `benchmarks/conatus/fixtures/day5-holdout.json` — the fresh blind holdout, 40
  pairs, 13 contradictions, 8 classes × 5. Written before the router first ran.
- `benchmarks/conatus/e90_binding_router.py` — the plan's `compare(a, b)` with
  four arms scored in one pass.
- `benchmarks/conatus/e90-results.json` — the full record.

## 1. The holdout

The plan names seven classes, and the fixture carries all of them plus the
implication class E84 measured the rules failing on (0/3): `different-owner`,
`different-object`, `nonoverlapping-time`, `unit-conversion`,
`quantifier-scope`, `negated-predicate`, `paraphrase`, `implication`.

The three published fixtures are kept as **regression sets** and are not part of
the holdout, which is exactly the distinction the plan draws.

## 2. Four arms, same pairs

An abstention is never a correct negative. `NOT_COMPARABLE`, `REVIEW_REQUIRED`
and `UNKNOWN` leave both precision and recall denominators and are reported as
coverage instead; the classifier arms always answer, so their "no" is a decision
(`NO_CONTRADICTION`), not an abstention.

| arm | precision | recall (decided) | recall (overall) | coverage | tp | false+ | abstained |
|---|---|---|---|---|---|---|---|
| `rules-unbound` (E84) | 0.500 | 0.692 | 0.692 | 1.00 | 9 | 9 | 0 |
| `rules-bound` (E86) | **0.643** | 0.692 | 0.692 | 1.00 | 9 | 5 | 0 |
| `nli` | 0.448 | 1.000 | 1.000 | 1.00 | 13 | 16 | 0 |
| `union` (naive) | 0.419 | 1.000 | 1.000 | 1.00 | 13 | 18 | 0 |
| `routed` | 0.542 | 1.000 | 0.619 | 0.80 | 13 | 11 | 8 |

`recall (overall)` counts an abstention the same as a miss — the user got no
answer either way. `recall (decided)` is the other reading, and reporting both
is the point: the routed arm's 1.000 there is an artefact of abstaining on
everything hard.

**Verdict `ROUTE_HURTS_PRECISION_KEEP_RULES`.** The gate:

| condition | result |
|---|---|
| no new false positives on the locked fixtures | true (all three OK) |
| recall gain over `rules-bound` | **false** (0.619 vs 0.692) |
| precision at least `rules-bound` | **false** (0.542 vs 0.643) |

## 3. Five findings

1. **A fresh holdout costs the bound rules half their precision.** They scored
   1.00 on the E86 validation fixture and 0.643 here. The published fixtures are
   regression sets, which is what the plan said they were; this is the number a
   blind set produces.
2. **The binder fires on 8 of 40 pairs, and its blind spots are structural.**
   The participant guard is head-noun based, so it cannot separate *the operator
   checked the gauge* from *the operator checked the valve* — same head, different
   object. Five of the router's eleven false positives are exactly that. The same
   defect hides *Operator A* from *Operator B*. What it does catch: differing
   months, differing clock times, differing dimensions, differing equipment tags
   (`P-101` vs `P-102`), and differently-headed subjects.
3. **NLI's failures are specific, not diffuse.** 5/5 on `different-object`, 4/4 on
   `paraphrase`, 3/3 on `unit-conversion`. The unit-conversion case is arithmetic:
   *18 C* against *291 K* is the same temperature, and the model has no arithmetic
   to see it. This is the clearest single failure mode in the run.
4. **The naive union is worse than either arm alone** (0.419 against 0.643 and
   0.448). "Add the model to the rules" is not a route; it is a recall purchase
   at a precision the plan explicitly forbids.
5. **The gate was wrong before the route was.** The first version compared recall
   only and returned `ROUTE_PROMOTABLE` for a router carrying 19 false positives
   against the rules' 5. A gate that rewards catching everything by calling
   everything a contradiction is not a gate. It now names a precision floor, and
   that is recorded here because the bug was in the instrument, not in the arm.

## 4. What the plan's fallback ships

Rules plus explicit review for the residual. Concretely, on this evidence:

- `CONTRADICTION_CANDIDATE` from the bound rules stays the only thing that can
  assert a contradiction. It is the best single arm measured (0.643) and it is
  deterministic.
- A below-threshold residual arm is `NO_CONTRADICTION`, not `REVIEW_CANDIDATE`.
  Emitting a review candidate for both outcomes scores a below-threshold model
  score as a claim of contradiction, which is how a model score silently becomes
  a belief.
- `REVIEW_REQUIRED` and `UNKNOWN` are surfaced as abstentions with the reason
  attached, so a reviewer sees "unresolved binding" rather than a silent "no".

## 5. Next candidate, from the evidence

1. **Prove quantity *equality*, not just difference.** The binder currently
   proves only that two dimensions differ. Extending it to convert within a
   dimension and prove equality would turn *18 C* / *291 K*, *6 bar* / *87 psi*
   and *25.4 mm* / *1 in* into `NOT_COMPARABLE` — removing 3 of the 11 routed
   false positives without touching recall, and fixing the one failure mode with
   a deterministic answer.
2. **Give the guard an object slot.** The `different-object` class needs the
   verb's object, not the clause head. Five false positives sit behind that.

Both are testable on the same fixture, which is now frozen. Any change to the
binder after this point is scored on a set that has been seen, so a promoted
binder needs a **fresh holdout** — the same rule the retrieval track follows.

## 6. Evidence

| check | command | result |
|---|---|---|
| full run | `.venv-nli/bin/python benchmarks/conatus/e90_binding_router.py --out benchmarks/conatus/e90-results.json` | `ROUTE_HURTS_PRECISION_KEEP_RULES` |
| no-model run | `python3 benchmarks/conatus/e90_binding_router.py --no-nli` | router P 0.818 at coverage 0.275; NLI arms `NOT_MEASURED` |
| regression | in-run, three published fixtures | 0 new false positives on all three |

**Actor**: continuation session (OpenCode) · **Date**: 2026-09-16
**Revisions**: Pro `96a98c4` + working tree, Core `d65858b` + working tree
