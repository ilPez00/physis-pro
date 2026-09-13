# justfile — Physis Pro common commands
# Requires: just (https://github.com/casey/just)

set shell := ["bash", "-c"]

# Default: show available recipes
default:
    @just --list

# Build the main binary (default features)
build:
    cargo build --bin physis-pro

# Build with all features
build-all:
    cargo build --all-features

# Run tests (default features)
test:
    cargo test

# Run tests with web feature
test-web:
    cargo test --features web

# Run tests with web + industrial features
test-industrial:
    cargo test --features "web industrial"

# Run tests with all features
test-all:
    cargo test --all-features

# Run the HTTP surface test suite
test-http:
    cargo test --test http_surface --features "web industrial"

# Run clippy (deny warnings)
lint:
    cargo clippy --all-features -- -D warnings

# Format code
fmt:
    cargo fmt

# Check formatting without changing files
fmt-check:
    cargo fmt -- --check

# Run the web server
web:
    cargo run --bin physis-pro-web --features web

# Run the TUI
tui:
    cargo run --bin physis-pro-tui --features tui

# Run the voice binary
voice:
    cargo run --bin physis-pro-voice --features "voice web"

# Seed demo data (requires web server running)
demo-seed:
    @echo "Seed demo data via: curl -X POST http://localhost:19876/api/v1/industrial/seed-demo"
    @echo "Or open http://localhost:19876/demo and click 'Seed demo data'"

# Doctor: validate config, data dir, embedder, license
doctor:
    cargo run --bin physis-pro -- doctor

# Show version
version:
    cargo run --bin physis-pro -- --version

# Clean build artifacts
clean:
    cargo clean

# Dogfood loop: recall outcome-memory for a task, run the fast gate, feed
# the result back as a verdict-stamped note. Offline, no server needed.
# Usage: just dev-loop "fix clippy E0599 on coherence edit"
dev-loop task:
    ./scripts/physis-dev-loop.sh "{{ task }}"

# Recall what the engine remembers about a task (read half of the loop).
recall q:
    cargo run --quiet --bin physis-pro -- node-search "{{ q }}"

# Remember one outcome with a verdict (-1 failed, 0 inert, +1 worked).
note text verdict="0.0":
    cargo run --quiet --bin physis-pro -- note "{{ text }}" --verdict {{ verdict }}

# Full CI check: fmt + clippy + tests
ci: fmt-check lint test
    @echo "CI passed"

# Install the repo's git hooks into this clone (hooks are per-clone, not
# inherited from the repo — run this once after cloning).
install-hooks:
    git config core.hooksPath scripts/hooks
    @echo "hooksPath set to scripts/hooks"

# Release physis-core to crates.io. Gated on check-release-integrity.sh so a
# publish can only happen from a clean, tagged tree at a version not already
# published — see docs/RELEASING.md. This is the only path to `cargo publish`.
release-core:
    ./scripts/check-release-integrity.sh --crate-dir physis-core
    cd physis-core && cargo publish


# Report whether this tree's copies of the physis skill still match the skill
# repo. Body lives in scripts/ so it is runnable without `just` — which is not
# installed on every machine that has this repo, including the one where this
# recipe was written.
skill-sync-check repo="":
    ./scripts/skill-sync-check.sh {{ repo }}
