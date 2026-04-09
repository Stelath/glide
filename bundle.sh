#!/usr/bin/env bash
set -euo pipefail

# Build a macOS .app bundle for Glide
# Usage: ./bundle.sh [--debug]

PROFILE="release"
CARGO_FLAG="--release"
if [[ "${1:-}" == "--debug" ]]; then
    PROFILE="debug"
    CARGO_FLAG=""
fi

ROOT="$(cd "$(dirname "$0")" && pwd)"
APP="$ROOT/target/$PROFILE/Glide.app"

echo "Building Glide ($PROFILE)..."
cargo build $CARGO_FLAG

echo "Creating app bundle..."
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
mkdir -p "$APP/Contents/Resources/assets/icons"

# Binary
cp "$ROOT/target/$PROFILE/Glide" "$APP/Contents/MacOS/Glide"

# Info.plist
cp "$ROOT/Info.plist" "$APP/Contents/Info.plist"

# Icon
cp "$ROOT/assets/icons/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"

# Assets (logos, provider icons, accent icons)
cp "$ROOT/assets/icons/"*.icns "$APP/Contents/Resources/assets/icons/" 2>/dev/null || true
cp "$ROOT/assets/icons/"*.png "$APP/Contents/Resources/assets/icons/" 2>/dev/null || true
cp "$ROOT/assets/icons/"*.svg "$APP/Contents/Resources/assets/icons/" 2>/dev/null || true

echo "Done: $APP"
