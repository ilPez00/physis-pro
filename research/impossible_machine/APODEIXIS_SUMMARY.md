# Apodeixis: Formal Target Status

## The compiled chain (Chain.lean)

Every link is a classical theorem recorded as an axiom where Lean's core does
not yet contain the analytic definitions. Status of each:

| Link | Status |
|---|---|
| von Mangoldt identity | axiom (classical) |
| Möbius convolution inverse | axiom (classical) |
| 1/zeta = Dirichlet series of mu | axiom (classical) |
| Functional equation | axiom (classical, Riemann 1859) |
| **Fixed locus s = 1 - s_bar iff Re(s) = 1/2** | **PROVEN** |
| Explicit formula | axiom (classical) |
| I(rho) = max(beta,1-beta) - 1/2 | defined |
| I(rho) = 0 for all rho iff RH | axiom (von Koch 1901 / Ingham 1932) |

## The genuinely new formal target (Target.lean)

> psi(x) - x = O(x^(1/2) log^2 x)  ->  no zero with beta > 1/2

This is the contrapositive of Ingham's Omega theorem (1932).

**Status: OPEN.** Not provable with current Lean + classical results alone.
The obstruction: Ingham's Omega direction is not formalized, and the
explicit formula itself is only an axiom here. The proof would require:
  1. formalizing the Guinand-Weil explicit formula
  2. formalizing Ingham's Omega theorem
  3. the asymptotic analysis to extract the contradiction

## The standing open question

> WHY 1/2?

All machine answers (self-ratio, scale stability, fixed locus, Love/Strife
balance) are either conditional or heuristic. The critical line's NECESSITY
for the actual zeros remains unproven.

## Build


cd research/impossible_machine/apodeixis && lake build


Requires: Lean 4 (elan), no Mathlib dependency.
