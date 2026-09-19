# Predeclared E86/E89 routed comparison protocol — frozen before running

Status: **SPEC ONLY. NOT RUN. NOT MEASURED.** This is written and frozen
before any arm has been scored on the new held-out fixture
(`fixtures/contradiction_holdout_disjoint_2026-09-15.json`). Predeclaring
before seeing results is the entire point (Day-5 plan: "Store the
specification before seeing held-out results"). This worker's ownership for
this task does not include `benchmarks/conatus/e86_participant_binding.py`
or `benchmarks/conatus/e89_nli_arm.py` (existing shared benchmark scripts,
out of scope) or the `.venv-nli` model environment, so **no arm was executed
under this task** — running this protocol is handed off via `REQUESTS.md`.

## Question

Does routing (structured rules first, NLI confined to the residual, per E86's
own recommendation) preserve the rules' adversarial precision while adding
E89's recall gain, on contradiction pairs the routing design has never seen —
different owners, different objects, nonoverlapping time intervals, unit
conversions, quantifier scope, negated predicates and paraphrases?

## Arms (identical for every case, same fixture, same order)

| arm | definition |
|---|---|
| RULES_ONLY | `e86_participant_binding.py`'s bound rule set, unmodified |
| NLI_ONLY | `e89_nli_arm.py`'s `cross-encoder/nli-deberta-v3-xsmall`, threshold 0.5 on P(contradiction), unmodified |
| NAIVE_UNION | contradiction if either RULES_ONLY or NLI_ONLY fires |
| ROUTED | RULES_ONLY first; if no rule fires, defer to NLI_ONLY only on the residual (E86/E89's own proposed design) |

No new rule or threshold may be written or tuned against
`contradiction_holdout_disjoint_2026-09-15.json`'s `heldout` split. The `dev`
split in the same file may be used to sanity-check that a harness correctly
invokes all four arms end-to-end before the frozen run.

## Primary metric and denominators

- Primary: **false-positive count on `heldout`** (precision-preserving is the
  gate; E86/E89 already show recall is easy to buy and precision is the risk).
- Secondary, reported but not gating: recall on genuine-contradiction cases,
  abstention/coverage (cases where an arm declines rather than answers), and
  latency per case class.
- Report every count with its denominator (e.g. "0/10 heldout negatives fire"
  not "0% false positive rate") and break out by `class`
  (participant/object/time/negation_scope/unit_conversion/paraphrase)
  separately — the fixture is deliberately small (30 pairs total, 20
  held-out) and a single pooled percentage would hide which class drives any
  failure.
- Unknown/abstained cases must not be scored as correct negatives (Day-5
  plan's explicit instruction) — report them in their own column.

## Stopping rule / promotion gate

- **Zero new false positives on `heldout`** for ROUTED, matching RULES_ONLY's
  and NAIVE_UNION's counts on the same fixture, is required before ROUTED can
  be called a candidate for product routing.
- If ROUTED introduces even one new false positive `heldout` gives NAIVE_UNION
  does not already have, the verdict is `KEEP_BASELINE` (ship RULES_ONLY, keep
  NLI for explicit human review only) — this mirrors the Day-5 plan's fallback
  exactly.
- Recall gains do not offset a precision loss under this gate; this fixture is
  small enough (n=20 held-out) that any interval on recall is provisional and
  must be reported as such, not promoted on its own.

## What would make this NOT_MEASURED instead of a verdict

- Any arm that cannot run in the target environment (e.g. `.venv-nli`
  unavailable) makes that arm's row `NOT MEASURED`, not zero or absent from
  the table.
- n=20 held-out pairs is a small, single-author sample; a clean pass licenses
  "no regression found on this fixture," not "routing is safe in general."

## Handoff

Whoever executes this (Core/Pro's Day-5 owner, or a future research pass with
the existing scripts in scope) should: run all four arms over
`fixtures/contradiction_holdout_disjoint_2026-09-15.json`'s `heldout` array
using the harness's existing pair-scoring convention (`{class, contradiction,
a, b}`, matching `adversarial-consistent-2.json`'s shape so no new I/O layer
is needed), fill in the tables above, and record the run through
`physis system remember` with actor identifying the executing role, per this
task's durable-record requirement. See `REQUESTS.md` (report directory) item
R3.
