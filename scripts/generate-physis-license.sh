#!/usr/bin/env bash
# Vendor-side wrapper around the Rust licence generator.
# Keep the signing key on an offline vendor machine. Do not run keygen on pansa.

set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if command -v cargo >/dev/null 2>&1; then
  RUN=(cargo run --quiet --locked --no-default-features --features vendor --bin physis-license --)
else
  echo "cargo is required; run this on the vendor/build machine, not pansa" >&2
  exit 1
fi

usage() {
  cat >&2 <<'EOF'
Usage:
  generate-physis-license.sh keygen [vendor-dir]
  generate-physis-license.sh issue --key KEY --org ORG --exp YYYY-MM-DD [options] --out FILE
  generate-physis-license.sh inspect FILE

Issue options are passed to physis-license sign: --seats, --features,
--maint-exp, and --id. The private signing key never leaves the vendor host.
EOF
}

case "${1:-}" in
  keygen)
    shift
    "${RUN[@]}" keygen --out "${1:-$HOME/.physis-vendor}"
    ;;
  issue)
    shift
    out=""
    args=()
    while (($#)); do
      if [[ "$1" == "--out" ]]; then
        [[ $# -ge 2 ]] || { echo "--out needs a path" >&2; exit 2; }
        out="$2"; shift 2
      else
        args+=("$1"); shift
      fi
    done
    [[ -n "$out" ]] || { echo "issue requires --out FILE" >&2; usage; exit 2; }
    mkdir -p "$(dirname "$out")"
    tmp="$(mktemp "${out}.XXXXXX")"
    trap 'rm -f "$tmp"' EXIT
    "${RUN[@]}" sign "${args[@]}" >"$tmp"
    chmod 600 "$tmp"
    mv -f "$tmp" "$out"
    trap - EXIT
    echo "wrote $out (install only the public licence blob on the customer host)"
    ;;
  inspect)
    [[ $# -eq 2 ]] || { usage; exit 2; }
    "${RUN[@]}" inspect "$2"
    ;;
  *)
    usage
    exit 2
    ;;
esac
