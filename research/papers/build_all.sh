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

# --- gates: a paper build is not "done" if a claim or a release artifact is dirty
REPO="$(cd ../.. && pwd)"
echo
echo "--- gates ---"
if python3 "$REPO/research/results_registry/validate.py" >/tmp/reg_gate.txt 2>&1; then
  tail -1 /tmp/reg_gate.txt | sed 's/^/registry: /'
else
  echo "registry: FAIL"; tail -5 /tmp/reg_gate.txt; fail=1
fi
if python3 "$REPO/research/release_audit.py" >/tmp/audit_gate.txt 2>&1; then
  grep -m1 VERDICT /tmp/audit_gate.txt | sed 's/^/release: /'
else
  echo "release: FAIL — credential-class material in a release artifact"
  grep -A3 'credential / secret' /tmp/audit_gate.txt | head -6
  fail=1
fi

exit $fail
