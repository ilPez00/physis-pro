#!/usr/bin/env bash
# Find public items that are declared, exported, and called by nothing.
#
# This defect class bit four times in one session, every time the same shape:
#   - OnnxEmbedder          exported from lib.rs, called by nothing.
#                           Every benchmark number came from a lexical hash.
#   - FitnessShifted        declared in the event enum, emitted by no path.
#     OutcomeObserved       replay disagreed with `hypothesis list` (PH-017).
#   - structural classifier built its own RandomProjectionEmbedder::new(64),
#                           so DOMAIN×MODE sequences were noise with a schema.
#   - build_structural      never joined to ledger.rs — both halves, no join.
#
# A module that compiles and exports is not a module that runs. Run this before
# claiming a capability exists.
#
# ponytail: textual, not a compiler pass. It counts `name` occurrences outside
# the defining file, so trait-method dispatch and macro use read as calls it
# cannot see. Deliberately: it must never be trusted as proof a thing IS used —
# only as a list of things to go look at. Zero hits is a question, not a verdict.
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DIRS=(${@:-physis-core/src src})
TMP="$(mktemp)"; trap 'rm -f "$TMP"' EXIT

# Names too generic to be evidence of anything, or conventionally called by the
# language rather than by our code.
SKIP='^(new|default|from|fmt|len|is_empty|clone|get|set|run|id|next|into|drop)$'

for dir in "${DIRS[@]}"; do
  [ -d "$dir" ] || continue
  echo "-- $dir"
  find "$dir" -name '*.rs' | sort | while read -r f; do
    # Every stage needs `|| true`: grep exits 1 on no match, and under
    # `set -e` + `pipefail` the first header-less or symbol-less file ends the
    # whole sweep silently. It did — the first run stopped at `batch_csv.rs`
    # and looked like a complete result.
    { grep -hoE '^[[:space:]]*pub (fn|struct|enum) [A-Za-z_][A-Za-z0-9_]*' "$f" 2>/dev/null \
      | awk '{print $NF}' | sort -u | grep -Ev "$SKIP" || true; } | while read -r sym; do
      [ -z "$sym" ] && continue
      # `--include` MUST precede `--`, or `--` ends option parsing and the
      # flag is taken as a filename — which silently makes every symbol look
      # uncalled. This check reported 15 false positives that way.
      hits=$(grep -rlF --include='*.rs' -- "$sym" physis-core/src src 2>/dev/null \
             | grep -vxF -- "$f" | wc -l | tr -d ' ') || hits=0
      if [ "${hits:-0}" = "0" ]; then
        printf '  %-34s %s\n' "$sym" "${f}"
        echo x >> "$TMP"
      fi
    done
  done
done

echo
if [ -s "$TMP" ]; then
  echo "$(wc -l < "$TMP" | tr -d ' ') item(s). Each line is a QUESTION, not a defect:"
  echo "confirm by reading before acting. Trait-method dispatch and macro use are"
  echo "invisible to this check, so a listed item may still be reached."
else
  echo "no declared-never-called public items found"
fi
