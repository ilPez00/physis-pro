---
id: PH-021
track: physis-core
phase: 1
status: review
rung: 2
depends: []
gate: "cd ~/dev/physis-pro/physis-core && cargo test --lib machines:: && cargo build --example impossible_machine_experiment --features embed-onnx && cargo build --example experiment48_exponent_necessity && cargo clippy --lib --tests -- -D warnings"
commit: "branch agent/PH-021; conventional commits; per PROTOCOL §6"
claimed_by: claude-pansa-2026-09-13
claimed_at: 2026-09-13
---

# The structural-machine primitives are trapped in an example

## Why (the failure this prevents)

`physis-core/examples/impossible_machine_experiment.rs` is 1457 lines and
contains the only implementation in this codebase of three things the engine
does not otherwise have:

- **a numeric claim that carries its own certification level** —
  `ProofStatus { Established, Derived, EstablishedFinite, Numerical, Heuristic,
  Contradicted }`, where `Heuristic` is documented as "dream-generated
  compatibility only — never certification";
- **a scaling-exponent estimator** — KAIROS fits `λ = d log D / d log x` by
  least squares in log-log space over the top half of the range, on the stated
  principle that "the EXPONENT is the invariant that survives the scale
  change";
- **refusal as an observation** — EUCLID records "the EXACT layer contains no
  witness for or against β ≠ 1/2" as a first-class finding rather than as
  silence.

Nothing in `src/` can call any of it. By this repo's own rule — *declared ≠
called*, `physis-skill` §1 — a symbol reachable only from one example is a
symbol the library does not have. The engine's claim to be a meta-tool is
weaker than its own example demonstrates, and every future domain (markets is
the live one, see `docs/plans/2026-09-13-note-markets-as-a-domain.md`) would
have to re-implement these or copy the example.

Copying is the specific trap: PH-012 already owns one instance of it, where
`TokenFixedRetriever` exists twice and a fix to one copy silently did not reach
the other. Promoting without rewiring the example would create the second
instance on purpose.

## Context (measured 2026-09-13, verify before relying on it)

- `examples/impossible_machine_experiment.rs:33` — `enum ProofStatus`.
- `:66` — `struct Observation { machine, target, claim, quantity, status }`.
- `:431` — `trait StructuralMachine { fn name(&self) -> &'static str;
  fn inspect(&self, w: &RiemannWorld) -> Vec<Observation>; }`
- `:408` — `struct RiemannWorld` — **domain-specific**: `zeros`,
  `psi_exact_at`, `mertens_at`, `euler_product_coherence`.
- Seven machines: euclid, logos, pythagoras, nous, empedocles, physis, kairos.
- `examples/experiment48_exponent_necessity.rs` is a second consumer.

## Steps

1. Add `physis-core/src/machines.rs` containing **only what is domain-
   independent**:
   - `ProofStatus` and `Observation`, unchanged in meaning.
   - `StructuralMachine<W>` — the trait generalised over the world it
     inspects, so a machine is a *stance* toward any world rather than toward
     `RiemannWorld`.
   - `scaling_exponent(points: &[(f64, f64)]) -> f64` — KAIROS's operation,
     lifted verbatim including the top-half-of-range rule and the reason for
     it (a truncation floor at small x dominates the fit otherwise).
   - `Observation::refusal(...)` — EUCLID's stance as a constructor, so
     "this layer cannot see it" is as easy to record as a measurement.
2. **Rewire both examples to use the library types.** This is not optional and
   is the point of the packet: a promotion that leaves the definitions behind
   creates the PH-012 duplication deliberately.
3. Do **not** move the seven machines' bodies. Their logic is Riemann-specific
   — `sieve`, `verify_von_mangoldt`, `psi_recon` — and belongs with its domain.
   The trait is the portable part; the arithmetic is not.
4. Do **not** invent a world abstraction beyond the type parameter. There is
   exactly one world today; a second one (a price series) is the thing that
   would tell us what the abstraction should be, and guessing now means
   guessing wrong.

## Tests you must add

- **Unit (`machines.rs`):** `scaling_exponent` recovers a known slope — build
  points from `y = a + λx` with λ = 0.5 and assert recovery to within
  tolerance; assert it recovers a different λ (say 0.8) too, so the test
  cannot pass by returning a constant.
- **Unit:** `scaling_exponent` on fewer than two usable points returns
  something defined rather than panicking or producing NaN.
- **Unit:** a `Refused` observation round-trips with its status intact — a
  refusal must not be silently gradeable as a measurement.
- **Compile-level:** building the two examples *against the library types* is
  itself the test that the rewiring in step 2 is real. If they still compile
  with their own local definitions, step 2 was not done.

## Gate

```bash
cd ~/dev/physis-pro/physis-core
cargo test --lib machines::
cargo build --example impossible_machine_experiment --features embed-onnx
cargo build --example experiment48_exponent_necessity
cargo clippy --lib --tests -- -D warnings
```

**Gate correction (2026-09-13).** The first draft said `cargo build
--examples`, which can never pass: `fn main` in the impossible-machine example
sits behind `#[cfg(feature = "embed-onnx")]`, so a featureless build reports
`error[E0601]: main function not found` and has always done so. That is a
feature gate, not a defect — verified by reproducing it on an unmodified
checkout before changing anything. The example must be built **with its
feature** to be a test of anything.

Note: `cargo clippy --all-targets` is red on 31 pre-existing errors in
`examples/`, so the clippy leg is scoped to `--lib --tests` until that is
cleared. The two named example builds still have to pass, which is what this
packet can be held to.

## Never

- Never copy the types into `src/` and leave the example's own definitions in
  place. That is PH-012 by choice.
- Never promote a machine's Riemann arithmetic into the library to make the
  trait look better used. A library symbol with one caller in an example is the
  problem this packet exists to fix, not the shape to reproduce.

## Relation to other work

`docs/plans/2026-09-13-note-markets-as-a-domain.md` names this as the first
piece of work for any numeric domain. It is a prerequisite, not a commitment to
that domain: the primitives are worth having in the library whether or not
markets is ever pursued.
