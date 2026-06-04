#!/usr/bin/env bash
# Download the ONNX models Physis needs for semantic classification, plus an
# optional uncensored local LLM for the agent/chat path.
#
# Embedders have no "censored/uncensored" axis — they output vectors, not text —
# so for those we just pick the strongest open weights. The uncensored choice
# applies only to the generative LLM (--llm).
#
# Usage:
#   scripts/download_models.sh            # embedders (default) — unblocks classify
#   scripts/download_models.sh --llm      # also fetch the uncensored local LLM
#   scripts/download_models.sh --all
#   scripts/download_models.sh --llm-only
set -euo pipefail

cd "$(dirname "$0")/.."
MODELS_DIR="models"
mkdir -p "$MODELS_DIR"

have() { command -v "$1" >/dev/null 2>&1; }

# Resolve a downloader: prefer hf (handles HF xet + resume + parallel).
if have hf; then
  DL="hf"
elif have huggingface-cli; then
  DL="hfcli"
else
  DL="curl"
fi
echo "[models] downloader: $DL"

# hf_get <repo> <repo_path> <local_dir>
hf_get() {
  local repo="$1" path="$2" dir="$3"
  echo "[models] $repo :: $path -> $dir/"
  case "$DL" in
    hf)    hf download "$repo" "$path" --local-dir "$dir" ;;
    hfcli) huggingface-cli download "$repo" "$path" --local-dir "$dir" ;;
    curl)  mkdir -p "$dir/$(dirname "$path")"
           curl -fL -C - --retry 5 --retry-delay 5 \
             "https://huggingface.co/$repo/resolve/main/$path" \
             -o "$dir/$path" ;;
  esac
}

download_embedders() {
  echo "── Jina CLIP v2 (primary: 1024-d, multilingual, text+image) ──"
  # fp16 is the standalone 1.7GB graph that loads under ort rc.12. The q4f16
  # quant on disk does NOT load (graph-optimizer error) — fp16 replaces it; the
  # Rust loader prefers fp16 first (src/embed/jina.rs).
  local JD="$MODELS_DIR/jina-clip-v2"
  hf_get jinaai/jina-clip-v2 onnx/model_fp16.onnx        "$JD"
  hf_get jinaai/jina-clip-v2 tokenizer.json              "$JD"
  hf_get jinaai/jina-clip-v2 config.json                 "$JD"
  hf_get jinaai/jina-clip-v2 preprocessor_config.json    "$JD"

  echo "── BGE base en v1.5 (fallback: 768-d text, loads cleanly in ort) ──"
  local BD="$MODELS_DIR/bge-base-en-v1.5"
  hf_get Xenova/bge-base-en-v1.5 onnx/model.onnx         "$BD"
  hf_get Xenova/bge-base-en-v1.5 tokenizer.json          "$BD"
  hf_get Xenova/bge-base-en-v1.5 config.json             "$BD"
  hf_get Xenova/bge-base-en-v1.5 tokenizer_config.json   "$BD"
  echo "[models] embedders done."
}

download_llm() {
  echo "── Dolphin 3.0 Llama 3.1 8B (uncensored instruct, GGUF Q4_K_M ~4.9GB) ──"
  # Uncensored, llama.cpp-compatible. For the local agent/chat path (point the
  # AI provider cascade or a llama.cpp server at this file). Swap the repo/file
  # for a different uncensored model if preferred.
  local LD="$MODELS_DIR/llm"
  hf_get cognitivecomputations/Dolphin3.0-Llama3.1-8B-GGUF \
    Dolphin3.0-Llama3.1-8B-Q4_K_M.gguf "$LD"
  echo "[models] LLM done: $LD/Dolphin3.0-Llama3.1-8B-Q4_K_M.gguf"
}

DO_EMB=1; DO_LLM=0
case "${1:-}" in
  --llm)      DO_LLM=1 ;;
  --all)      DO_LLM=1 ;;
  --llm-only) DO_EMB=0; DO_LLM=1 ;;
  "" )        : ;;
  *) echo "unknown arg: $1"; exit 2 ;;
esac

[ "$DO_EMB" = 1 ] && download_embedders
[ "$DO_LLM" = 1 ] && download_llm
echo "[models] all requested downloads complete."
