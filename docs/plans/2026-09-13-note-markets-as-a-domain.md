# Note: could physis find invariants in markets?

Raised 2026-09-13. **A note, not a plan.** Nothing here has been measured, and
the point of writing it down is to record what would have to be true, and what
the cheapest falsifying test is, before anyone spends a quarter on it.

## Why the question is a good one

Physis is a meta-tool: its subject is not files, it is *claims and whether they
survived*. Markets are the domain where that discipline is most valuable and
least practised, for a specific reason — **in market research, a positive
result is almost always an artifact, and the field knows it and mostly cannot
stop.** Backtest overfitting is the default outcome of searching a large
hypothesis space against a short, noisy, non-stationary sample.

The standing rule of this repository, earned seven times over on ontology work,
is:

> treat any positive as an artifact until a construction-matched control says
> otherwise

That is, almost word for word, the discipline a quantitative research process
needs and rarely enforces. This is the strongest argument for the domain fit,
and it is an argument about *process*, not about prediction.

## What physis already has that transfers

Three things, and they are not generic:

**1. `direction.rs` — a transformation scored against a null of the same
magnitude.** The survey in `computer-remake-research/` found no other system,
in twenty-one, that performs this operation. Its market analogue is exact and
well known: a strategy must be scored against a null that achieves the same
turnover, the same exposure, the same number of trades — a random strategy
matched on construction. The module's own shape (`destop` +0.2078 HOLD against
`truncate` +0.0000, the null, at 5/5) is the same experiment with different
nouns.

**2. Bi-temporality, now writable.** `TemporalValidity` carries
`valid_from`/`valid_until` (when the claim held) beside `expired_at` (when we
learned it), and since PH-019 a boundary can be *narrowed by later evidence*
with the correction recorded as a `Revision`.

This is not a nice-to-have in finance; it is the whole problem. Point-in-time
correctness — knowing what was knowable *when*, not what the database says now
after restatements, index reconstitutions and survivorship — is the single
largest source of silently-wrong backtests. A system whose native question is
*"what did we believe at T, on what evidence, and what changed it"* is
describing a point-in-time research database without meaning to.

**3. The ledger's failure semantics.** A hypothesis that predicted something,
and whether the prediction held, is `hypothesis.rs` today. Signal decay —
the fact that a market invariant has a *lifetime* and dies — is the thing the
bi-temporal legs are for. "This worked from 2019-03 to 2021-06" is a
`TemporalValidity`, not a footnote.

## Correction: the numeric half already exists

**An earlier draft of this note said physis has "no numeric or time-series
primitives" and that "everything in the engine is text, embeddings and a
semiotic grid". That is false, and the error mattered — it understated the
domain fit by the widest possible margin.** Recorded here rather than quietly
deleted, because the corrected version is the argument.

**The Impossible Machine** (`physis-core/examples/impossible_machine_experiment.rs`,
`research/impossible_machine/`) runs seven `StructuralMachine`s — euclid, logos,
pythagoras, nous, empedocles, physis, kairos — over a numeric world, each
returning `Observation { machine, target, claim, quantity, status }`. The
status is an epistemic grade on a *numeric* finding:

```
Established | Derived | EstablishedFinite | Numerical | Heuristic
```

with `Heuristic` documented as "dream-generated compatibility only — never
certification". A numeric claim that carries its own certification level is
exactly what a market claim needs and almost never has.

**KAIROS is a scaling-exponent estimator.** It measures a deviation across
scales and fits `λ = d log D / d log x` by least squares in log-log space,
over the top half of the range to avoid the truncation floor — and its stated
principle is "the EXPONENT is the invariant that survives the scale change".
That is the Hurst / structure-function family, the same tool used on returns
series.

**E51 and E52 already run the full quant-research protocol on numeric series.**
This is the part that should change anyone's estimate of the domain fit:

| | E51 (`PH-051`) | E52 (`PH-052`) |
|---|---|---|
| result | **clean failure**, recorded | partial-positive |
| split | discovery vs held-out, **frozen in advance** | discovery `(20,100k]`, held-out `(100k,200k]` |
| nulls | representation-agnostic observables | Cramér random primes, iid geometric gaps |
| gates | pre-registered | `specific = D > 1.5·max_null`; `heldout = |Δ| ≤ 0.5·D` |
| observables | **excess kurtosis, lag-1 autocorrelation**, … | `R(k) = N(k)/E(k)`, deviation `D = RMS(R(k)−1)` |

Out-of-sample validation, explicit null models, acceptance criteria fixed
before the run, and a negative result published as a result. Those are the four
things that separate quantitative research from backtest mining, and they are
implemented and exercised here, on numeric series, today. The observables named
in E51 — excess kurtosis, lag-1 autocorrelation — are time-series statistics
already.

## What is genuinely missing, stated plainly

Narrower than the earlier draft claimed, and worth being precise about:

- **It is example-scoped, not library-scoped.** `StructuralMachine`,
  `Observation`, `ProofStatus` and the exponent fit live in
  `physis-core/examples/`, not in `src/`. Nothing else in the crate can call
  them, and `physis-check calls` would report them as declared-not-called from
  the library's point of view. Promoting them is a real piece of work, and it
  is the first piece.
- **No price/returns data model.** There is no instrument, no bar, no
  corporate-action handling, no bid-ask. The observables are general; the
  ingestion is not.
- **The worlds are deterministic and stationary.** Prime-density fields hold
  still while being measured. That is the one structural difference that does
  not go away, and it is discussed below.
- **The ontology results do not transfer, and neither do their numbers.**
  E1–E18 are about whether embedding geometry yields stable ontology structure.
  Nobody should quote `precision@25 = 0.480` in a market context; it is about
  discovery candidates in a text corpus.
- **Retrieval is unearned.** The 2×2 run the same day scored three arms at 0/7
  and one at 1/7 against a whole-file ceiling of 5/7
  (`docs/plans/2026-09-13-2x2-first-run.md`). A substrate whose retrieval has
  not yet beaten BM25+RRF should not be sold as a research engine for anything.
- **Markets are adversarial and non-stationary; a filing cabinet is not.**
  Every invariant physis has been tested against so far holds still while being
  measured. A market invariant is arbitraged away *because* it was found. No
  part of the current engine models that, and the epistemic machinery does not
  make it go away — it only makes the decay legible after the fact.

## The honest framing

Physis would not be a predictor. It would be **the record that makes a
quantitative research process auditable**: what was hypothesised, on what
evidence, what it predicted, whether the prediction held, when it stopped
holding, and — the part nothing else does — whether it ever beat a
construction-matched null.

That is a real product shape and a defensible one. It is also a different
product from "find invariants in markets", and the difference should not be
blurred: the first is a discipline the engine already implements, the second
requires numeric primitives it does not have.

## The cheapest falsifying test, if anyone picks this up

Do **not** start by ingesting market data. Start by asking whether the control
mechanism detects the failure the domain is famous for:

> Generate N random strategies over one real price series. Select the best by
> in-sample Sharpe — the standard way to manufacture an overfit result. Then
> ask `direction`-style scoring whether the winner beats a construction-matched
> null (same turnover, same exposure, same trade count) out of sample.
>
> **Pass:** the machinery reports no separation, i.e. it correctly refuses to
> certify a backtest that is pure selection bias.
> **Fail:** it certifies the overfit winner, in which case the control does not
> transfer and the domain claim collapses immediately.

This is cheap, it needs no ingestion pipeline, the correct answer is known by
construction, and it is decisive in one direction. It is the same shape as
`benchmarks/direction/run.py`: rule-generated arms, so nobody authors the
answer.

Only if that passes is there a reason to build numeric primitives.

## Relation to existing work

- The substrate argument is in `computer-remake-research/notes.md` §5–§7; the
  three-layer separation (observe / believe / act, model outside) is what makes
  the audit claim above possible.
- `gaps.md` Gap 9 is the null-control gap, and Gap 13 is "nobody measures
  whether the structure paid for itself" — both apply here with more force than
  in the desktop domain, not less.
