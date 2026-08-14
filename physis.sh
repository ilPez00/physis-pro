#!/usr/bin/env bash
# Physis standalone classify — map a .txt or image file onto the semiotic grid.
# Usage: ./physis.sh /path/to/file.txt
#        ./physis.sh /path/to/image.jpg
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
FILE="${1:-}"

if [ -z "$FILE" ]; then
    echo "Usage: $0 <file.txt>"
    exit 1
fi

if [ ! -f "$FILE" ]; then
    echo "ERROR: file not found: $FILE"
    exit 1
fi

# Build CLI if needed
if [ -f "$ROOT/target/release/physis" ]; then
    BIN="$ROOT/target/release/physis"
elif [ -f "$ROOT/target/debug/physis" ]; then
    BIN="$ROOT/target/debug/physis"
else
    echo "Building physis CLI..."
    cargo build --release --bin physis 2>&1
    BIN="$ROOT/target/release/physis"
fi

"$BIN" classify "$FILE"
