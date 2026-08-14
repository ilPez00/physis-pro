#!/bin/bash
set -e

cd "$(dirname "$0")"

cargo build --release

cp target/release/llmwiki llmwiki
cp target/release/llmwiki wiki/llmwiki

echo "Done — ./llmwiki is ready"
