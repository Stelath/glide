#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info()  { printf "${GREEN}[+]${NC} %s\n" "$*"; }
warn()  { printf "${YELLOW}[!]${NC} %s\n" "$*"; }
die()   { printf "${RED}[x]${NC} %s\n" "$*" >&2; exit 1; }

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"

# ── 1. Xcode ─────────────────────────────────────────────────────────────
info "Checking Xcode..."
if ! xcode-select -p &>/dev/null; then
    die "Xcode not found. Install Xcode from the App Store, then re-run this script."
fi
info "  Xcode: $(xcodebuild -version 2>/dev/null | head -1) at $(xcode-select -p)"

# ── 2. Homebrew ──────────────────────────────────────────────────────────
info "Checking Homebrew..."
if ! command -v brew &>/dev/null; then
    warn "Homebrew not found — installing..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    eval "$(/opt/homebrew/bin/brew shellenv)" 2>/dev/null || eval "$(/usr/local/bin/brew shellenv)" 2>/dev/null || true
fi
info "  Homebrew: $(brew --version | head -1)"

# ── 3. just ──────────────────────────────────────────────────────────────
info "Checking just..."
if ! command -v just &>/dev/null; then
    warn "just not found — installing via Homebrew..."
    brew install just
fi
info "  just: $(just --version)"

# ── 4. xcodegen ──────────────────────────────────────────────────────────
info "Checking xcodegen..."
if ! command -v xcodegen &>/dev/null; then
    warn "xcodegen not found — installing via Homebrew..."
    brew install xcodegen
fi
info "  xcodegen: $(xcodegen --version 2>/dev/null || echo 'installed')"

# ── 5. Rust / rustup ─────────────────────────────────────────────────────
info "Checking Rust..."
if ! command -v rustup &>/dev/null; then
    warn "rustup not found — installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    source "$HOME/.cargo/env"
fi
info "  Rust: $(rustc --version)"

# ── 6. iOS Rust targets ──────────────────────────────────────────────────
info "Checking iOS Rust targets..."
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
    if rustup target list --installed 2>/dev/null | grep -q "^${target}$"; then
        info "  Already installed: $target"
    else
        info "  Installing: $target"
        rustup target add "$target"
    fi
done

# ── 7. Smoke test ────────────────────────────────────────────────────────
cd "$REPO_DIR"
info "Running smoke test (cargo check --workspace)..."
cargo check --workspace

info ""
info "Setup complete!"
info ""
info "Commands:"
info "  just dev           # debug build + run (macOS)"
info "  just dev ios       # debug build + run (iOS Simulator)"
info "  just build         # release build + .app bundle (macOS)"
info "  just build ios     # release build (iOS device)"
info "  just test          # run all tests"
info "  just               # list all recipes"
