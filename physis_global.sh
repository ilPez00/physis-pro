#!/bin/bash

# PHYSIS GLOBAL - Unified Ontological Mapping & Speculation
# ==============================================================================

# Configuration
# ==============================================================================
PROJECT_DIR="."
# Use .env or existing environment variables
if [ -f .env ]; then
  source .env
fi
# ==============================================================================

function show_help() {
    echo "Usage: physis_global.sh [command] [args]"
    echo ""
    echo "Commands:"
    echo "  voice           Start real-time voice dashboard (http://localhost:3000)"
    echo "  tui             Start Terminal UI visualization"
    echo "  scan <dir>      Scan directory, build map, and save to wiki format"
    echo "  deep-scan <dir> AI-powered multimodal scan (text + images)"
    echo "  graph <dir>     Scan directory and output Mermaid graph"
    echo "  dream <count>   Generate speculative ontological paths (dreams)"
    echo "  translate <txt> <lang>  Translate text using the AI cascade"
    echo "  stats           Show engine statistics"
}

case "$1" in
    voice)
        echo "Starting Physis Voice Dashboard..."
        cd $PROJECT_DIR && cargo run --release --bin physis-voice
        ;;
    tui)
        echo "Starting Physis TUI..."
        cd $PROJECT_DIR && cargo run --release --bin physis-tui
        ;;
    scan)
        echo "Scanning $2..."
        cd $PROJECT_DIR && cargo run --release --bin physis -- scan "$2" --format wiki
        ;;
    deep-scan)
        echo "Performing Multimodal Deep Scan on $2..."
        cd $PROJECT_DIR && cargo run --release --bin physis -- deep-scan "$2"
        ;;
    graph)
        echo "Generating graph for $2..."
        cd $PROJECT_DIR && cargo run --release --bin physis -- scan "$2" --format mermaid
        ;;
    dream)
        echo "Generating $2 dreams..."
        cd $PROJECT_DIR && cargo run --release --bin physis -- scan . --format wiki > /dev/null
        cargo run --release --bin physis -- dream --count "$2"
        ;;
    translate)
        # We need a small rust snippet or an expansion to 'physis' cli for this
        echo "Translating '$2' to $3..."
        cd $PROJECT_DIR && cargo run --release --bin physis -- config | grep -q "ollama" # Just to check build
        echo "[Note: Use 'physis-voice' for AI-integrated features]"
        ;;
    stats)
        cd $PROJECT_DIR && cargo run --release --bin physis -- stats
        ;;
    *)
        show_help
        ;;
esac
