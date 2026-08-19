#!/usr/bin/env bash
# predeploy.sh — local mirror of the GitHub CI gate (PREMORTEM P9).
#
# GitHub Actions is the gate Railway auto-deploy waits on. When Actions is down
# (billing) we deploy by hand with `railway up`, which BYPASSES that gate and
# ships the local working tree. This script puts the gate back in front of a
# manual deploy: run it, and only `railway up` if it prints PREDEPLOY: PASS.
#
#   ./scripts/predeploy.sh && railway up --service physis --detach
#
# It runs the exact CI steps (clippy -D warnings, cargo test --features full,
# the feature-set checks) plus two things CI can't: it refuses a dirty tree
# (prod must equal a pushed commit — provenance), and it runs the #[ignore]
# accuracy floor IF a real model is on disk (the silent-regression guard that
# otherwise runs nowhere). Set SKIP_ACCURACY=1 to skip the ~10-min model run.

set -uo pipefail
cd "$(dirname "$0")/.."

fail() { echo "PREDEPLOY: FAIL — $1" >&2; exit 1; }
step() { echo; echo "▶ $1"; }

# 1. Clean tree — prod must equal a pushed commit, not an unreviewed worktree.
if [ -n "$(git status --porcelain)" ]; then
  fail "working tree is dirty; commit (or stash) so prod == a git ref"
fi
LOCAL=$(git rev-parse HEAD)
REMOTE=$(git rev-parse '@{u}' 2>/dev/null || echo "")
if [ -n "$REMOTE" ] && [ "$LOCAL" != "$REMOTE" ]; then
  fail "HEAD ($LOCAL) is not pushed to upstream ($REMOTE); push first"
fi

# 2. The CI feature-set checks.
step "cargo check (default / full / industrial,cli / spanner / wiki-embedded / mcp)"
for f in "" "--features full" "--features industrial,cli" "--features spanner" \
         "--features wiki-embedded" "--features mcp"; do
  cargo check --quiet $f || fail "cargo check $f"
done

# 2b. Check the exact default feature set to catch silent ONNX fallback
step "cargo check default features"
cargo check --quiet --features "cli,voice,web,tui,embed-onnx,sql,postgres" || fail "cargo check default features: ONNX enabled but model check failed"

step "cargo build --examples"
cargo build --quiet --examples || fail "examples build"

# 3. Clippy as a hard gate (matches CI -D warnings).
step "cargo clippy --all-targets -- -D warnings"
cargo clippy --quiet --all-targets -- -D warnings || fail "clippy"

# 4. Full test suite.
step "cargo test --features full"
cargo test --quiet --features full || fail "tests"

# 5. Accuracy floor — runs nowhere automatically (needs a real model on disk).
#    This is the P9 silent-regression guard. Skippable for speed.
if [ "${SKIP_ACCURACY:-0}" = "1" ]; then
  echo "  (skipping accuracy floor: SKIP_ACCURACY=1)"
elif [ -f models/bge-base-en-v1.5/onnx/model.onnx ] || [ -f models/model.onnx ]; then
  step "accuracy floor (real model present; ~10 min)"
  cargo test --quiet --features full --test classification -- --ignored accuracy_floor \
    || fail "accuracy floor cannot run - no model on disk. Set SKIP_ACCURACY=1 to skip, or ensure a model is available."
else
  fail "accuracy floor cannot run - no model on disk. Set SKIP_ACCURACY=1 to skip, or ensure a model is available."
fi

echo
echo "PREDEPLOY: PASS — safe to \`railway up --service physis --detach\`"
