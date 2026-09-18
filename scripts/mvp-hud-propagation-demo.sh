#!/usr/bin/env bash
# mvp-hud-propagation-demo.sh — the 90-second Physis HUD demo, reproducible.
#
# One edit, honest propagation, no model, no network:
#   search → scope → open → inline-edit → save → rescan/Δ → markers → history.
#
# The committed fixture (fixtures/mvp_propagation) is NEVER edited: it is
# copied to a temp dir first, so every run starts from budget 500 and the
# `mvp_propagation` test keeps passing.
#
# Usage: scripts/mvp-hud-propagation-demo.sh
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$HERE/fixtures/mvp_propagation"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/physis-hud-demo-XXXXXX")"
cp -a "$FIXTURE/." "$WORK/"

resolve_bin() {
  if [ -n "${PHYSIS_HUD_BIN:-}" ]; then printf '%s' "$PHYSIS_HUD_BIN"; return; fi
  for candidate in "$HERE/target/release/physis-hud" "$HERE/target/debug/physis-hud"; do
    [ -x "$candidate" ] && { printf '%s' "$candidate"; return; }
  done
  command -v physis-hud 2>/dev/null || printf '%s' "$HERE/target/debug/physis-hud"
}
BIN="$(resolve_bin)"
if [ ! -x "$BIN" ]; then
  echo "error: no physis-hud binary (looked at \$PHYSIS_HUD_BIN, ./target/release, ./target/debug, PATH)" >&2
  echo "build one: cargo build --features tui --bin physis-hud" >&2
  exit 2
fi

# No override on purpose: unset PHYSIS_EMBEDDER uses the semantic cascade
# (local ollama nomic-embed-text, self-test gated — still offline, no cloud).
# PHYSIS_EMBEDDER=random-projection pins the deterministic lexical fallback;
# the header always prints whichever backend actually answered.
export PHYSIS_MODEL="${PHYSIS_MODEL:-none}"
export PHYSIS_DATA_DIR="$WORK/.physis"
mkdir -p "$PHYSIS_DATA_DIR"

echo "demo workspace: $WORK  (fixture copy; the repo is untouched)"
echo "--- prescan (cold; later starts are incremental) ---"
time "$BIN" "$WORK" --prescan-only

cat <<'KEYS'

90-second script — every line is one command-box entry (Enter submits):

  retrieval budget          search: world scopes around the budget
  :context                  bounded world slice, measured tokens
  open src/retrieval.rs     exact source, no summary
  edit src/retrieval.rs     inline editor opens (▌ is the cursor)
  <change 500 to 750>       arrows move, type, Enter newlines
  Ctrl+S                    save → atomic write → rescan → Δ
  3                         Δ pane: CHANGED / AFFECTED / STRUCTURE / VALIDATION / MAP
  g                         graph: ● retrieval · ◐ agent · ◐ context
  D                         dependents: code-shaped references, labelled as such
  4                         history tab: git past plus the recorded transition
  :trail                    recorded navigation steps · :pins lists pins
  $ git status              same box, workspace root, exit code shown
  ?                         full command vocabulary, no model involved

Revert path (optional, still honest): edit 750 back to 500, Ctrl+S,
watch a second Δ with the same shape.  `q` quits.  Workspace left at:
KEYS
echo "  $WORK"

exec "$BIN" "$WORK" --no-prescan
