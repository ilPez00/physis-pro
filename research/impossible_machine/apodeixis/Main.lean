import ImpossApodeixis

/-- The apodeixis executable: prints the formal target status. -/
def main : IO Unit := do
  IO.println "apodeixis: the compiled chain is formalized; the target remains open"
  IO.println "  proven: fixed_locus_critical_line (s = 1 - s.conj ↔ s.re = 1/2)"
  IO.println "  axioms: von_mangoldt, mobius_inverse, inv_zeta, functional_equation, explicit_formula"
  IO.println "  open: bound_implies_no_beta_gt_half (the genuinely new target)"
  IO.println "  open: why_one_half_unproven (the standing question)"
