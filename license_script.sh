#!/usr/bin/env bash
# physis-pro License Management Script
set -euo pipefail

PHYSIS_LICENSE="physis-license"
PHYSIS_PRO="physis-pro-web"
VENDOR_DIR="${HOME}/.physis-vendor"

log() { echo -e "\e[1;32m[INFO]\e[0m $1"; }
warn() { echo -e "\e[1;33m[WARN]\e[0m $1"; }

check_bin() {
    if ! cargo run --bin "$1" --help >/dev/null 2>&1; then
        log "Building $1..."
        cargo build --bin "$1" 2>/dev/null | grep -E "Finished|error"
    fi
}

ensure_keyring() {
    if [ -d "$VENDOR_DIR" ] && [ -f "$VENDOR_DIR/signing.key" ]; then
        warn "Keyring already exists at $VENDOR_DIR"
    else
        log "Generating vendor keyring..."
        cargo run --bin "$PHYSIS_LICENSE" --features vendor -- keygen --out "$VENDOR_DIR"
        warn "Keep signing.key OFFLINE! Never commit to git/CI."
    fi
}

sign_license() {
    local exp=$(date -d "+365 days" +%Y-%m-%d)
    local maint=$(date -d "+730 days" +%Y-%m-%d)

    log "Signing license — expires $exp, maintenance $maint"
    physis-license sign \
        --key "$VENDOR_DIR/signing.key" \
        --org "$ORG" \
        --seats "$SEATS" \
        --features "$FEATURES" \
        --exp "$exp" \
        --maint-exp "$maint"
}

run_app() {
    if [ -n "$1" ]; then
        PHYSIS_LICENSE_FILE="$1" cargo run --bin "$PHYSIS_PRO" --features web 2>&1 | grep -E "^physis:"
    else
        echo "Usage: $0 run [--license-file <path>]"
        echo "  Or: PHYSIS_LICENSE='payload sig' cargo run --bin physis-pro-web --features web"
    fi
}

case "${1:-}" in
    init)
        check_bin "$PHYSIS_LICENSE"
        ensure_keyring
        ;;
    sign)
        check_bin "$PHYSIS_LICENSE"
        sign_license
        ;;
    run)
        run_app "${2:-}"
        ;;
    *)
        echo "physis-pro License Manager"
        echo "Usage: $0 {init|sign|run}"
        echo "  init     - Generate vendor keyring"
        echo "  sign     - Sign a new license"
        echo "  run      - Run physis-pro with license"
        ;;
esac