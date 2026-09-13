#!/usr/bin/env bash
# Run the physis checklist instead of reciting it.
#
# The skill's four rules are mechanical, so an agent that "read the skill" and
# an agent that ran the checks are distinguishable only if the checks exist as
# commands. These are those commands.
#
#   physis-check calls <Symbol> [dir ...]      rule 1 — is it ever called?
#   physis-check sweep [dir ...]               rule 1 — whole tree
#   physis-check discriminate --a CMD --b CMD  rule 2 — does the number move?
#   physis-check map [dir ...]                 rule 3/4 — structural summary
#   physis-check flow [transcript.jsonl|session-id]  rule 6/7 — judge the session
#   physis-check recall "<task>"               rule 4 — was this already tried?
#   physis-check claim "<text>" [conf]         rule 5 — register a refutable claim
#   physis-check verdict "<text>" success|inert|failure
#                                              rule 5 — record the outcome
#   physis-check all [dir ...]                 the checklist, end to end
#
# Engine-backed steps degrade to `NOT MEASURED` when physis-core is absent.
# That is the point: a check that silently skips is worse than no check.
set -Eeuo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FAILED=0
say()  { printf '\n\033[1m── %s\033[0m\n' "$*"; }
pass() { printf '  \033[32mPASS\033[0m %s\n' "$*"; }
fail() { printf '  \033[31mFAIL\033[0m %s\n' "$*"; FAILED=1; }
skip() { printf '  \033[33mNOT MEASURED\033[0m %s\n' "$*"; }

# ── the engine, if this host has one ────────────────────────────────────────
# Searched in order: $PHYSIS_CORE, PATH, a physis-pro checkout next door. A
# dev build also needs a licence signed for *that* build — the gate verifies
# signatures even in dev builds, so a stale key fails closed.
find_engine() {
  local c
  for c in "${PHYSIS_CORE:-}" "$(command -v physis-core || true)" \
           "$HOME/dev/physis-pro/target/release/physis-core" \
           "$HOME/.cargo/bin/physis-core"; do
    [ -n "$c" ] && [ -x "$c" ] && { echo "$c"; return 0; }
  done
  return 1
}
ENGINE="$(find_engine || true)"
# physis-pro ships `note`, which CREATES a labelled node and asserts its verdict
# in one step. physis-core's `assert` only moves a node that already exists —
# recording a fresh outcome through it fails with "no node with label ...".
find_pro() {
  local c
  for c in "${PHYSIS_PRO:-}" "$(command -v physis-pro || true)" \
           "$HOME/dev/physis-pro/target/release/physis-pro" \
           "$HOME/.cargo/bin/physis-pro"; do
    [ -n "$c" ] && [ -x "$c" ] && { echo "$c"; return 0; }
  done
  return 1
}
PRO="$(find_pro || true)"
# The embedder falls back to a lexical hash when the ONNX model is not on disk,
# and it resolves `./models/...` relative to the CURRENT DIRECTORY — so running
# the same command from two directories gives semantic geometry in one and a
# hash in the other, with no error either way. Pin it to the engine's own tree.
for b in "$ENGINE" "${PRO:-}"; do
  [ -n "$b" ] || continue
  for root in "$(dirname "$b")/../.." "$(dirname "$b")"; do
    if [ -d "$root/models/bge-base-en-v1.5" ]; then
      root="$(cd "$root" && pwd)"
      [ -z "${PHYSIS_MODEL_DIR:-}" ] && export PHYSIS_MODEL_DIR="$root/models/bge-base-en-v1.5"
      [ -z "${PHYSIS_MODELS:-}" ]    && export PHYSIS_MODELS="$root/models"
      break 2
    fi
  done
done
export PHYSIS_ALLOW_DEV_LICENCE="${PHYSIS_ALLOW_DEV_LICENCE:-1}"
export PHYSIS_DATA_DIR="${PHYSIS_DATA_DIR:-$HOME/.physis}"

engine() {  # engine <args...> — stderr kept, licence banner dropped
  [ -n "$ENGINE" ] || return 127
  "$ENGINE" "$@" 2>&1 | grep -v '^physis: ' || true
}

# ── rule 1 ──────────────────────────────────────────────────────────────────
cmd_calls() {
  local sym="${1:?usage: physis-check calls <Symbol> [dir ...]}"; shift
  local dirs=("${@:-.}")
  # -w so `Foo` does not match `FooBar`; the defining file is excluded by the
  # caller reading the list, not by the grep — a symbol can be used twice in
  # its own file and still be dead outside it.
  local hits defs decl_files uses
  hits=$(grep -rnw --exclude-dir={.git,node_modules,target,dist,build,vendor,__pycache__,.venv} \
         -- "$sym" "${dirs[@]}" 2>/dev/null || true)
  defs=$(printf '%s\n' "$hits" | grep -E ":[0-9]+:[[:space:]]*(pub |export |func |def |class |type |const |impl )" || true)
  # A symbol used only inside the file that declares it — its own unit tests,
  # typically — is not wired into anything. `OnnxEmbedder` had seven such
  # "uses" and every benchmark number still came from a lexical hash. Drop the
  # declaring files before counting.
  decl_files=$(printf '%s\n' "$defs" | cut -d: -f1 | sort -u | grep -v '^$' || true)
  uses="$hits"
  while read -r df; do
    [ -n "$df" ] && uses=$(printf '%s\n' "$uses" | grep -vF "$df:" || true)
  done <<< "$decl_files"
  uses=$(printf '%s\n' "$uses" | grep -v '^$' || true)

  printf '%s\n' "$uses" | sed 's/^/  /' | head -30
  # Tests are evidence the code compiles, not evidence it is reached in
  # production. Counted separately, never as a call site.
  local n t
  n=$(printf '%s\n' "$uses" | grep -c . || true)
  # Path-named test files, plus Rust's inline `#[cfg(test)]` modules: a use
  # below that attribute in its own file is a test use, and nothing on the path
  # says so. OnnxEmbedder's seven "call sites" were all of this kind.
  t=$(printf '%s\n' "$uses" | awk -F: '
    $0 ~ /(^|\/)(tests?|spec|__tests__)\/|_test\.|test_|\.test\.|\.spec\./ { c++; next }
    # The cfg(test) cutoff is a Rust rule. Applied to every language it made
    # this script fail on itself: physis-check.sh contains the literal string
    # "#[cfg(test)]" in a comment, so every use below that comment counted as a
    # test and a live dispatch table read as dead code. Rust files only, and the
    # attribute must start its line so a mention in prose does not arm it.
    $1 !~ /\.rs$/ { next }
    {
      if (!(($1) in cut)) {
        cut[$1] = 0
        while ((getline l < $1) > 0) { ln++; if (l ~ /^[[:space:]]*#\[cfg\(test\)\]/ && !cut[$1]) cut[$1] = ln }
        close($1); ln = 0
      }
      if (cut[$1] > 0 && $2+0 > cut[$1]) c++
    }
    END { print c+0 }')
  if [ "${n:-0}" = "0" ]; then
    fail "$sym: $(printf '%s\n' "$defs" | grep -c . || true) declaration(s) in $(printf '%s\n' "$decl_files" | grep -c . || true) file(s), 0 use sites elsewhere. It does not run — say so."
  elif [ "$n" = "$t" ]; then
    fail "$sym: all $n use site(s) are tests. It compiles and is exercised; nothing in the product calls it."
  else
    pass "$sym: $((n - t)) non-test use site(s) outside its declaring file(s) (+$t in tests)"
  fi
}

# ── rule 2 ──────────────────────────────────────────────────────────────────
# A benchmark whose score does not move when the thing it measures moves is not
# a benchmark. Name the arm it should lose to, run both, subtract.
cmd_discriminate() {
  local a="" b="" metric='[0-9]+\.[0-9]+'
  while (($#)); do case "$1" in
    --a) a="$2"; shift 2 ;; --b) b="$2"; shift 2 ;;
    --metric) metric="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; return 2 ;;
  esac; done
  [ -n "$a" ] && [ -n "$b" ] || { echo "usage: physis-check discriminate --a CMD --b CMD [--metric REGEX]" >&2; return 2; }
  local oa ob na nb
  oa=$(eval "$a" 2>&1 || true); ob=$(eval "$b" 2>&1 || true)
  na=$(printf '%s' "$oa" | grep -oE "$metric" | head -1 || true)
  nb=$(printf '%s' "$ob" | grep -oE "$metric" | head -1 || true)
  printf '  arm A (%s) → %s\n  arm B (%s) → %s\n' "$a" "${na:-?}" "$b" "${nb:-?}"
  if [ -z "$na" ] || [ -z "$nb" ]; then
    skip "no number matched /$metric/ in one of the arms — the comparison did not happen"
    return
  fi
  local d; d=$(awk -v x="$na" -v y="$nb" 'BEGIN{printf "%.6f", x-y}')
  printf '  DISCRIMINATION: A - B = %s\n' "$d"
  if awk -v d="$d" 'BEGIN{exit !(d<0.001 && d>-0.001)}'; then
    fail "Δ ≈ 0 — this measurement cannot fail, so it is not evidence. Do not quote it."
  else
    pass "the score moves with the thing it measures (Δ=$d)"
  fi
}

# ── rule 5 ──────────────────────────────────────────────────────────────────
cmd_claim() {
  local text="${1:?usage: physis-check claim \"<text>\" [confidence]}"; local conf="${2:-0.6}"
  [ -n "$ENGINE" ] || { skip "no physis-core on this host — claim not registered, and therefore not refutable later"; return; }
  engine hypothesis create "$text" --confidence "$conf" | sed 's/^/  /'
}
cmd_verdict() {
  local text="${1:?usage: physis-check verdict \"<text>\" success|inert|failure}"
  local v="${2:?verdict required: success | inert | failure}"
  local n
  case "$v" in
    success|1)  v=success; n=1 ;;
    inert|0)    v=inert;   n=0 ;;
    failure|-1) v=failure; n=-1 ;;
    *) echo "verdict must be success|inert|failure (or 1|0|-1)" >&2; return 2 ;;
  esac
  # A failure recorded is the only thing that stops the next session retrying
  # it. `physis-check recall` is the read half — run it before starting.
  if [ -n "$PRO" ]; then
    # Rule 6, found by this check failing to record its own outcome: without
    # PHYSIS_ALLOW_DEV_LICENCE a dev build exits 2 on the licence gate, `|| true`
    # swallowed it, and `verdict` printed nothing and exited 0. A verdict that
    # silently does not record is worse than none — the next session repeats the
    # work believing it was logged.
    # `out=$(cmd)` under `set -e` aborts the script the moment cmd fails, so
    # `rc=$?` on the next line never runs — which is how this fix silently did
    # nothing on its first attempt. Capture the status in the same expression.
    local out rc=0
    out=$("$PRO" note "$text" "--verdict=$n" 2>&1) || rc=$?
    # `|| true`: when every line is a `physis:` banner, grep -v filters them all
    # and returns 1, which under `set -e -o pipefail` aborts before the failure
    # below can be reported. Third variant of the same trap in this one function.
    printf '%s\n' "$out" | grep -v '^physis: ' | sed 's/^/  /' || true
    if [ "$rc" -ne 0 ]; then
      # Built as a plain variable: a `$( ... grep -q ... && ... )` inside the
      # message returns non-zero when the pattern misses, and `set -e` kills the
      # script before `fail` ever prints — which is how the first version of
      # this very fix stayed silent.
      # Tested 2026-09-13 against a real dev build. There are three distinct
      # licence failures here and they need different advice; the first version
      # of this hint offered only one, and it was the one that does not apply.
      # PHYSIS_ALLOW_DEV_LICENCE=1 disables the *gate*, not the *check*, so it
      # never fixes a missing or mismatched licence on its own. A hint that does
      # not resolve the failure is a silent failure one level up.
      local hint=""
      case "$out" in
        *"signature is not valid"*)
          hint=" The licence in PHYSIS_DATA_DIR (default ~/.physis) is signed for a different vendor key. Point PHYSIS_LICENSE_FILE at one built for this binary." ;;
        *"no licence found"*|*"no license found"*)
          hint=" No licence: set PHYSIS_LICENSE_FILE=/path/to/license.key, or place one at .physis/license.key." ;;
        *licence*|*license*)
          hint=" Licence check failed; PHYSIS_ALLOW_DEV_LICENCE=1 disables the gate but does not supply a licence." ;;
      esac
      fail "verdict NOT recorded — '$PRO note' exited $rc.$hint"
      return 1
    fi
  elif [ -n "$ENGINE" ]; then
    # No physis-pro: `assert` needs the node to exist already, so say which
    # label is missing rather than reporting a silent success.
    local out; out=$(engine assert "$text" "$v")
    printf '%s\n' "$out" | sed 's/^/  /'
    printf '%s' "$out" | grep -q 'no node with label' && \
      skip "physis-core assert cannot create a node; install physis-pro for \`note\`, or register the claim first with: physis-check claim \"$text\""
  else
    skip "no engine on this host — outcome not recorded, so the next session repeats this"
  fi
}
cmd_recall() {
  local q="${1:?usage: physis-check recall \"<task>\"}"
  # Must read the store `physis-check verdict` writes. physis-pro and
  # physis-core keep SEPARATE graphs, so mixing the halves silently recalls
  # nothing you ever wrote — the loop looks alive and remembers nothing.
  if [ -n "$PRO" ]; then
    "$PRO" node-search "$q" 2>&1 | grep -v '^physis: ' | sed 's/^/  /' || true
  elif [ -n "$ENGINE" ]; then
    engine node-search "$q" | sed 's/^/  /'
  else
    skip "no engine on this host — cannot check whether this was already tried and already failed"
  fi
}

# ── rule 6/7 — judge the session, not only the artifact ─────────────────────
# Every claim an agent makes in a transcript is either adjacent to evidence it
# produced, or it is not. That is checkable without a model. What needs the
# engine is the geometry: do the claims drift away from what was asked, do they
# cohere, where do they land on the grid — and does any of that beat its null.
cmd_flow() {
  local tx="${1:-}"
  # A bare session id works as well as a path. With neither, the newest
  # transcript on disk is used — which is NOT necessarily this session, since
  # any other running agent's file may be newer. The path is printed for that
  # reason; check it before believing the result is about your own work.
  if [ -n "$tx" ] && [ ! -f "$tx" ]; then
    tx=$(find "$HOME/.claude/projects" -name "${tx}.jsonl" 2>/dev/null | head -1 || true)
  fi
  if [ -z "$tx" ]; then
    tx=$(find "$HOME/.claude/projects" -name '*.jsonl' -printf '%T@ %p\n' 2>/dev/null \
         | sort -rn | head -1 | cut -d' ' -f2- || true)
    [ -n "$tx" ] && printf '  (no transcript given — using the newest on disk, which may be another session)\n'
  fi
  [ -n "$tx" ] && [ -f "$tx" ] || { fail "no transcript found (pass one: physis-check flow <file.jsonl>)"; return; }
  local work; work="$(mktemp -d)"; FLOW_TMP="$work"
  trap 'rm -rf "${FLOW_TMP:-}"' EXIT
  python3 "$HERE/flow.py" "$tx" "$work" || { fail "transcript parse failed"; return; }

  local total unsup ask
  total=$(cat "$work/total" 2>/dev/null || echo 0)
  unsup=$(cat "$work/unsupported" 2>/dev/null || echo 0)
  ask=$(cat "$work/ask" 2>/dev/null || true)
  printf '  transcript: %s\n  %s event(s) · %s claim(s) · first ask: %s\n' \
    "$tx" "$(cat "$work/events" 2>/dev/null || echo 0)" "$total" "${ask:0:70}"

  if [ "$total" = "0" ]; then
    skip "0 claims extracted from $(cat "$work/events" 2>/dev/null || echo 0) events — an empty result, not a clean one (rule 6). Check the cue list before believing it."
  elif [ "$unsup" = "0" ]; then
    pass "every claim has a tool result between it and the previous claim"
  else
    fail "$unsup/$total claim(s) with no tool call behind them:"
    sed 's/^/      · /' "$work/unsupported.txt" 2>/dev/null | head -10
  fi

  if [ -n "$ENGINE" ] && [ "$total" != "0" ]; then
    # The corpus is one file per claim; `chain` gives structure (repeats,
    # differences, contradictions), coverage against the grid, drift, AND the
    # label-permuted control in the same pass. Read the CONTROL line first: if
    # it says NOT ABOVE THE NULL, everything above it is geometry, not knowledge.
    say "geometry of the session (physis-core chain)"
    local out; out=$(engine chain --corpus "$work/claims" --query "$ask")
    printf '%s\n' "$out" | sed 's/^/  /'
    # Rule 1 turned on this tool: `chain` runs happily on the lexical-hash
    # fallback embedder, and every number it then prints describes a hash. It
    # names the embedder it used — read that line before the results.
    if printf '%s' "$out" | grep -q 'embedder random-projection'; then
      fail "chain ran on the random-projection fallback: that geometry is a lexical
       hash, not meaning. Set PHYSIS_MODEL_DIR to a pulled model and re-run."
    elif printf '%s' "$out" | grep -q 'NOT ABOVE THE NULL'; then
      # A Δ≈0 is a statement about THIS corpus, not about the method: the same
       # pass returns Δ +0.0655 on a corpus whose topics share no vocabulary.
       # Session claims are prose about one repository — the regime where the
       # arms converge — so expect this line and read the ungated halves above.
      fail "the session's geometry does not beat its own label-permuted null —
       the cells above are shape, not knowledge. Report nothing from them.
       (This says the claims share too much vocabulary to group, not that the
       method is broken. Structure, compression and unplaced counts still hold.)"
    elif printf '%s' "$out" | grep -q 'CONTROL     not run'; then
      # No control ran, so there is no null to have beaten. Calling that a pass
      # is the exact move rule 2 exists to stop.
      skip "no control ran on this session (too few claims to rank) — nothing here is evidence yet"
    else
      pass "session geometry computed on a semantic embedder, above its null"
    fi
  else
    skip "session geometry (drift / coherence / classification) — needs physis-core"
  fi
}

# ── rule 3/4 delegates ──────────────────────────────────────────────────────
cmd_sweep() { bash "$HERE/declared-never-called.sh" "$@"; }
cmd_map()   { bash "$HERE/gen-wiki.sh" "$@"; }

cmd_all() {
  local dirs=("${@:-.}")
  say "rule 1 — declared ≠ called"
  cmd_sweep "${dirs[@]}" || fail "sweep exited nonzero (see above)"
  say "rule 2 — does the measurement discriminate?"
  skip "only you know which arm your number should lose to. Run:
       physis-check discriminate --a '<real arm>' --b '<stupid control>'"
  say "rule 3/4 — read before grepping"
  cmd_map "${dirs[@]}" || fail "module map not generated"
  say "rule 6/7 — judge the session"
  cmd_flow
  say "verdict"
  if [ "$FAILED" = "0" ]; then pass "checklist clean — and rule 2 is still on you"
  else fail "checklist has open items. Do not report the work done."; fi
  return "$FAILED"
}

case "${1:-}" in
  calls)         shift; cmd_calls "$@" ;;
  sweep)         shift; cmd_sweep "$@" ;;
  discriminate)  shift; cmd_discriminate "$@" ;;
  map)           shift; cmd_map "$@" ;;
  flow)          shift; cmd_flow "$@" ;;
  claim)         shift; cmd_claim "$@" ;;
  recall)        shift; cmd_recall "$@" ;;
  verdict)       shift; cmd_verdict "$@" ;;
  all)           shift; cmd_all "$@" ;;
  ""|-h|--help)  sed -n '2,20p' "$0" ;;
  *) echo "unknown subcommand: $1" >&2; exit 2 ;;
esac
exit "$FAILED"
