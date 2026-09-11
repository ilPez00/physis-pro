import Lake
open Lake DSL

package "impossible_machine_apodeixis" where
  -- Formal target: the compiled chain from the Impossible Machine experiment
  -- Attempts the one genuinely new formal target produced by the run:
  --   ψ(x) − x = O(x^(1/2) log²x) ⟹ no zero with β > 1/2

@[default_target]
lean_lib «ImpossApodeixis» where

@[default_target]
lean_exe «apodeixis» where
  root := `Main
