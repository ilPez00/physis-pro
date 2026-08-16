#!/usr/bin/env bash
# Build a signed, licence-gated customer package.
#
# Produces dist/physis-pro-<version>-<target>.tar.gz plus a detached Ed25519
# signature, so an air-gapped customer can verify the tarball offline before
# unpacking it.
#
# Usage:
#   PHYSIS_LICENSE_PUBKEY=<hex> PHYSIS_RELEASE_KEY=~/.physis-vendor/release.key \
#     scripts/package.sh
#
# Two distinct keys are used on purpose:
#   PHYSIS_LICENSE_PUBKEY  public key compiled into the binary; validates licences
#   PHYSIS_RELEASE_KEY     private key that signs the tarball; proves provenance
# Compromising one must not compromise the other.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
TARGET="${PHYSIS_TARGET:-$(rustc -vV | awk '/^host:/{print $2}')}"
STAGE="$(mktemp -d)"
OUT="$ROOT/dist"
NAME="physis-pro-${VERSION}-${TARGET}"
trap 'rm -rf "$STAGE"' EXIT

# ── 1. Refuse to build an ungated package ────────────────────────────────────
# Without a stamped production key the binary falls back to the well-known
# development key, and ANY licence anyone mints would validate. Shipping that
# would hand the product away, so fail loudly rather than produce it.
if [ -z "${PHYSIS_LICENSE_PUBKEY:-}" ]; then
  echo "ERROR: PHYSIS_LICENSE_PUBKEY is not set." >&2
  echo "       Without it the build embeds the DEVELOPMENT key and every" >&2
  echo "       forged licence validates. Generate a production key with:" >&2
  echo "         cargo run --features vendor --bin physis-license -- keygen" >&2
  exit 1
fi

if [ "${#PHYSIS_LICENSE_PUBKEY}" -ne 64 ]; then
  echo "ERROR: PHYSIS_LICENSE_PUBKEY must be 64 hex chars (32 bytes)." >&2
  exit 1
fi

# ── 2. Build ─────────────────────────────────────────────────────────────────
# `--features full` minus `vendor`: the licence-minting tool must never ship.
echo "==> building $NAME"
PHYSIS_LICENSE_PUBKEY="$PHYSIS_LICENSE_PUBKEY" \
  cargo build --release --features full --target "$TARGET"

BIN_DIR="target/${TARGET}/release"

# Guard against the mistake that gives the product away: if the licence tool
# ended up in the release directory, someone built with --features vendor.
if [ -f "${BIN_DIR}/physis-license" ]; then
  echo "ERROR: physis-license present in release build — the vendor feature" >&2
  echo "       was enabled. Rebuild without it; that binary mints licences." >&2
  exit 1
fi

# ── 3. Stage ─────────────────────────────────────────────────────────────────
mkdir -p "$STAGE/$NAME/bin"
for b in physis-pro physis-pro-web physis-pro-tui physis-pro-wiki; do
  if [ -f "${BIN_DIR}/${b}" ]; then
    cp "${BIN_DIR}/${b}" "$STAGE/$NAME/bin/"
    strip "$STAGE/$NAME/bin/${b}" 2>/dev/null || true
  fi
done

cp LICENSE.txt "$STAGE/$NAME/"
[ -f THIRD-PARTY-NOTICES ] && cp THIRD-PARTY-NOTICES "$STAGE/$NAME/"
[ -f INSTALL.md ] && cp INSTALL.md "$STAGE/$NAME/"
mkdir -p "$STAGE/$NAME/config"
cp config/*.json "$STAGE/$NAME/config/"

cat > "$STAGE/$NAME/install.sh" <<'INSTALLER'
#!/usr/bin/env bash
# Install Physis Pro into ~/.local/bin and ~/.physis.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

mkdir -p "$PREFIX/bin" "$HOME/.physis/config"
cp "$HERE"/bin/* "$PREFIX/bin/"
cp "$HERE"/config/*.json "$HOME/.physis/config/"

echo "Installed to $PREFIX/bin"
if [ ! -f "$HOME/.physis/license.key" ]; then
  echo
  echo "No licence found. Physis will not run without one."
  echo "Place the licence file supplied by your vendor at:"
  echo "  $HOME/.physis/license.key"
fi
INSTALLER
chmod +x "$STAGE/$NAME/install.sh"

# ── 4. Third-party notices ───────────────────────────────────────────────────
# Required by section 5 of LICENSE. Generated rather than hand-maintained so it
# cannot drift from Cargo.lock.
if command -v cargo-license >/dev/null 2>&1; then
  echo "==> generating THIRD-PARTY-NOTICES"
  cargo license --features full > "$STAGE/$NAME/THIRD-PARTY-NOTICES"
else
  echo "WARNING: cargo-license not installed; THIRD-PARTY-NOTICES not regenerated." >&2
  echo "         Install with: cargo install cargo-license" >&2
fi

# ── 5. Archive ───────────────────────────────────────────────────────────────
mkdir -p "$OUT"
tar -czf "$OUT/${NAME}.tar.gz" -C "$STAGE" "$NAME"
( cd "$OUT" && sha256sum "${NAME}.tar.gz" > "${NAME}.tar.gz.sha256" )

# ── 6. Sign ──────────────────────────────────────────────────────────────────
# The signature is what an air-gapped customer checks; the sha256 alone proves
# nothing, since anyone substituting the tarball can substitute the checksum.
if [ -n "${PHYSIS_RELEASE_KEY:-}" ] && [ -f "${PHYSIS_RELEASE_KEY}" ]; then
  echo "==> signing ${NAME}.tar.gz"
  cargo run --quiet --features vendor --bin physis-license -- \
    sign-artifact --key "$PHYSIS_RELEASE_KEY" \
    --file "$OUT/${NAME}.tar.gz" > "$OUT/${NAME}.tar.gz.sig"
else
  echo "WARNING: PHYSIS_RELEASE_KEY unset — package is UNSIGNED." >&2
  echo "         Air-gapped customers cannot verify provenance." >&2
fi

echo
echo "==> $OUT/${NAME}.tar.gz"
ls -la "$OUT/${NAME}".tar.gz*
