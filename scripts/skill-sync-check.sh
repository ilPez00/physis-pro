#!/usr/bin/env bash
# Is this tree's copy of the physis skill still the skill?
#
# The skill lives in its own repo (ilPez00/physis-skill) and is duplicated here
# as scripts/physis-check.sh and docs/PHYSIS_SKILL.md. Those copies drifted apart
# once already, and the broken half was the distributed one — a `verdict` that
# recorded nothing and exited 0 lived in the shipped copy while the fix sat here.
# Divergence has to be visible rather than discovered.
#
# Usage: skill-sync-check.sh [path-to-physis-skill-clone]
#        PHYSIS_SKILL_REPO=... skill-sync-check.sh
#
# Exit 0 in sync or not measurable, 1 diverged.
set -Eeuo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

R="${1:-${PHYSIS_SKILL_REPO:-$HOME/dev/physis-skill}}"
if [ ! -d "$R" ]; then
  # Rule 6: a check that cannot run must not read as a check that passed.
  echo "NOT MEASURED: no physis-skill clone at $R (pass a path or set PHYSIS_SKILL_REPO)"
  exit 0
fi

rc=0
compared=0
for pair in "scripts/physis-check.sh:scripts/physis-check.sh" "SKILL.md:docs/PHYSIS_SKILL.md"; do
  there="$R/${pair%%:*}"; here="${pair##*:}"
  [ -f "$there" ] && [ -f "$here" ] || { echo "NOT MEASURED: missing $there or $here"; continue; }
  compared=$((compared + 1))
  diff -u "$there" "$here" || rc=1
done

if [ "$compared" = "0" ]; then
  echo "NOT MEASURED: nothing compared"
  exit 0
fi
if [ "$rc" = "0" ]; then
  echo "in sync with $R ($compared file(s) compared)"
else
  echo "DIVERGED from $R ($compared file(s) compared)." >&2
  echo "Reconcile BOTH directions before copying either way: each copy has held" >&2
  echo "content the other lacked, so a one-way sync destroys real work." >&2
fi
exit "$rc"
