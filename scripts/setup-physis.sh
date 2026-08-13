#!/usr/bin/env bash
# setup-physis.sh — probe + install every external dependency Physis needs.
#
# Idempotent, interactive. Run on a fresh machine:
#   ./scripts/setup-physis.sh [--full] [--lean] [--voice] [--skip-build]
#
# Modes:
#   --full        install every optional dep (lean + voice + ollama floor)
#   --lean        install Lean 4 via elan (apodeixis machine)
#   --voice       install ffmpeg + arecord (voice capture + whisper pipeline)
#   --skip-build  stop after deps; don't cargo build
#
# What it touches:
#   - ~/.cargo/bin  (rustup)
#   - ~/.elan       (lean toolchain manager)
#   - ~/.ollama     (LLM floor models, optional)
#   - system packages via apt (Debian/Ubuntu) or brew (macOS)
#   - ~/.local/share/physis/models (ONNX weights, optional)
#
# No root required for Rust/elan/ollama. apt/brew need sudo for system packages.

set -euo pipefail

FEATURES="cli,web,tui,embed-onnx"
INSTALL_LEAN=0
INSTALL_VOICE=0
INSTALL_OLLAMA=0
SKIP_BUILD=0
SUDO=""
MODELS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/physis/models"

# ── args ─────────────────────────────────────────────────────────────────────
for arg in "$@"; do
  case "$arg" in
    --full)    INSTALL_LEAN=1; INSTALL_VOICE=1; INSTALL_OLLAMA=1 ;;
    --lean)    INSTALL_LEAN=1 ;;
    --voice)   INSTALL_VOICE=1 ;;
    --ollama)  INSTALL_OLLAMA=1 ;;
    --skip-build) SKIP_BUILD=1 ;;
    *) echo "unknown flag: $arg"; exit 1 ;;
  esac
done

# ── helpers ──────────────────────────────────────────────────────────────────
info()  { echo -e "\033[1;34m[setup]\033[0m $*"; }
ok()    { echo -e "\033[1;32m[ok]\033[0m $*"; }
warn()  { echo -e "\033[1;33m[warn]\033[0m $*"; }
fail()  { echo -e "\033[1;31m[fail]\033[0m $*"; exit 1; }

have() { command -v "$1" >/dev/null 2>&1; }

detect_os() {
  case "$(uname -s)" in
    Linux)  OS=linux ;;
    Darwin) OS=macos ;;
    *)      fail "unsupported OS: $(uname -s) (need Linux or macOS)" ;;
  esac
}

ensure_sudo() {
  if [ -z "$SUDO" ]; then
    if ! have sudo; then
      warn "no sudo found — skipping system packages"
      return 1
    fi
    if ! sudo -n true 2>/dev/null; then
      info "sudo requires a password (needed for apt/brew)"
      sudo -v || return 1
    fi
    SUDO="sudo"
  fi
  return 0
}

# ── OS detection ─────────────────────────────────────────────────────────────
detect_os
info "OS: $OS"

# ── 1. Rust toolchain ────────────────────────────────────────────────────────
info "Checking Rust toolchain..."
if have rustc && have cargo; then
  ok "rustc $(rustc --version) + cargo $(cargo --version)"
else
  info "Installing Rust via rustup..."
  ensure_sudo || true
  if [ "$OS" = "linux" ]; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  fi
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env" 2>/dev/null || true
  ok "rustup installed. Restart your shell or source ~/.cargo/env"
fi

# ── 2. System packages ───────────────────────────────────────────────────────
info "Checking system packages..."
if [ "$OS" = "linux" ]; then
  ensure_sudo || true
  NEED_PKGS=""
  for pkg in curl ca-certificates openssl pkg-config; do
    dpkg -s "$pkg" >/dev/null 2>&1 || NEED_PKGS="$NEED_PKGS $pkg"
  done
  if [ -n "$NEED_PKGS" ]; then
    info "Installing: $NEED_PKGS"
    $SUDO apt-get update -qq
    $SUDO apt-get install -y -qq $NEED_PKGS
    ok "system packages installed"
  else
    ok "system packages present"
  fi
elif [ "$OS" = "macos" ]; then
  if ! have brew; then
    info "Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    # Add brew to PATH for this session
    eval "$(/opt/homebrew/bin/brew shellenv 2>/dev/null || /usr/local/bin/brew shellenv 2>/dev/null || true)"
  fi
  for pkg in curl openssl pkg-config; do
    brew list "$pkg" >/dev/null 2>&1 || brew install "$pkg"
  done
  ok "Homebrew packages ready"
fi

# ── 3. Lean 4 (apodeixis) ────────────────────────────────────────────────────
if [ "$INSTALL_LEAN" -eq 1 ]; then
  info "Checking Lean 4 (apodeixis machine)..."
  if have lean; then
    ok "lean $(lean --version 2>&1 | head -1)"
  else
    info "Installing elan (Lean version manager)..."
    ensure_sudo || true
    curl https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh -sSf | sh -s -- -y --default-toolchain none
    # shellcheck disable=SC1090
    source "$HOME/.elan/env" 2>/dev/null || true
    info "Installing Lean 4 stable toolchain (this takes ~30s)..."
    elan toolchain install stable
    elan default stable
    ok "lean installed: $(lean --version 2>&1 | head -1)"
    info "NOTE: Lean is ~500 MB. Add ~/.elan/bin to your PATH permanently."
  fi
else
  info "Skipping Lean (pass --lean or --full to install apodeixis deps)"
fi

# ── 4. Voice deps ────────────────────────────────────────────────────────────
if [ "$INSTALL_VOICE" -eq 1 ]; then
  info "Checking voice dependencies..."
  VOICE_FEAT="voice"
  if [ "$OS" = "linux" ]; then
    ensure_sudo || true
    NEED_VOICE=""
    for pkg in ffmpeg alsa-utils; do
      dpkg -s "$pkg" >/dev/null 2>&1 || NEED_VOICE="$NEED_VOICE $pkg"
    done
    if [ -n "$NEED_VOICE" ]; then
      info "Installing voice deps: $NEED_VOICE"
      $SUDO apt-get update -qq
      $SUDO apt-get install -y -qq $NEED_VOICE
      ok "voice deps installed"
    else
      ok "ffmpeg + arecord present"
    fi
    FEATURES="$FEATURES,voice"
  elif [ "$OS" = "macos" ]; then
    if ! have ffmpeg; then brew install ffmpeg; fi
    ok "ffmpeg installed (macOS uses native audio, no arecord needed)"
    FEATURES="$FEATURES,voice"
  fi
else
  info "Skipping voice deps (pass --voice or --full to install)"
fi

# ── 5. Ollama (optional LLM floor) ──────────────────────────────────────────
if [ "$INSTALL_OLLAMA" -eq 1 ]; then
  info "Checking Ollama (LLM floor)..."
  if have ollama; then
    ok "ollama $(ollama --version 2>/dev/null || echo 'present')"
  else
    info "Installing Ollama..."
    curl -fsSL https://ollama.com/install.sh | sh || warn "ollama install failed — check https://ollama.com/install"
    if have ollama; then
      # Pull a small floor model in background
      ( ollama pull gemma2:2b > /tmp/ollama-pull.log 2>&1 || echo "ollama pull failed (run manually later)" ) &
      ok "ollama installed (pulling gemma2:2b in background)"
    fi
  fi
else
  info "Skipping Ollama (pass --ollama or --full to install)"
fi

# ── 6. ONNX embedding models ─────────────────────────────────────────────────
info "Checking ONNX embedding models..."
MODEL_DIR="$MODELS_DIR/bge-base-en-v1.5"
if [ -d "$MODEL_DIR/onnx" ] && [ -f "$MODEL_DIR/onnx/model.onnx" ]; then
  ok "embedding model present at $MODEL_DIR/onnx/"
else
  info "Downloading BGE-base-en-v1.5 ONNX model (~400 MB)..."
  mkdir -p "$MODEL_DIR/onnx"
  HF_URL="https://huggingface.co/BAAI/bge-base-en-v1.5/resolve/main/onnx/model.onnx"
  curl -fL "$HF_URL" -o "$MODEL_DIR/onnx/model.onnx" || warn "HF download failed; set MODEL_BASE_URL or download manually"
  if [ -f "$MODEL_DIR/onnx/model.onnx" ]; then
    ok "embedding model downloaded"
  fi
fi

# ── 7. Build ─────────────────────────────────────────────────────────────────
if [ "$SKIP_BUILD" -eq 0 ]; then
  info "Building physis-pro (features: $FEATURES)..."
  cargo build --release --features "$FEATURES" --bin physis-pro --bin physis-pro-web
  ok "build complete"
  info "Binaries:"
  ls -lh target/release/physis-pro target/release/physis-pro-web 2>/dev/null || true
else
  info "Skipping build (--skip-build)"
fi

echo
echo "== Setup complete =="
echo "Next:"
echo "  ./scripts/install-physis.sh --port 8080    # deploy web server"
echo "  # or run directly:"
echo "  ./target/release/physis-pro-web"
echo
echo "Features enabled: $FEATURES"
echo "Data dir: $MODELS_DIR"
echo "Lean:      $(lean --version 2>&1 | head -1 || echo 'not installed')"
echo "Ollama:   $(ollama --version 2>/dev/null || echo 'not installed')"
