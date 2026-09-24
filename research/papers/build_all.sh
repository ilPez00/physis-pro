#!/usr/bin/env bash
# Build all papers and report unresolved refs/citations. Fails loudly.
set -u
cd "$(dirname "$0")"
PAPERS="paper1_agent_priors paper2_trajectories paper3_ontology_instability paper4_propose_certify paper9_dataset paper10_empedocles paperM_methodology"
fail=0
for f in $PAPERS; do
  if [ ! -f "$f.tex" ]; then
    printf '%-32s %s\n' "$f" "SKIP (no .tex yet)"
    continue
  fi
  for _ in 1 2 3; do pdflatex -interaction=nonstopmode "$f.tex" >/dev/null 2>&1; done
  undef=$(grep -c 'Warning: Citation.*undefined\|Warning: Reference.*undefined\|There were undefined references' "$f.log" 2>/dev/null || true)
  qq=$(grep -c 'LaTeX Warning: Reference .* on page .* undefined' "$f.log" 2>/dev/null || true)
  navail=$(grep -c 'File .* not found' "$f.log" 2>/dev/null || true)
  pages=$(pdfinfo "$f.pdf" 2>/dev/null | awk '/^Pages/{print $2}')
  status=OK
  if [ "${undef:-0}" != "0" ] || [ "${navail:-0}" != "0" ]; then status=FAIL; fail=1; fi
  printf '%-32s pages=%-3s undefined=%-3s missingfile=%-3s %s\n' "$f" "${pages:-?}" "${undef:-?}" "${navail:-?}" "$status"
done
[ "$fail" = "0" ] && echo "ALL PAPERS CLEAN" || echo "BUILD HAS UNRESOLVED REFERENCES"
exit $fail
