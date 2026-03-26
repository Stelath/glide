# Glide — unified build system
# Install: brew install just  (or: cargo install just)
# Usage:   just              (list all recipes)
#          just dev           (debug build + run for current OS)
#          just dev ios       (debug build + run on iOS Simulator)
#          just build         (release build + package for current OS)
#          just build ios     (release build for iOS device)

ROOT := justfile_directory()
current_os := if os() == "macos" { "mac" } else { os() }

# List available recipes
default:
    @just --list

# ─── Primary Commands ─────────────────────────────────────────────────────────

# Debug build + auto-run (platform: mac, ios, linux, windows)
dev platform=current_os:
    @just _dev-{{platform}}

# Release build + package artifacts (platform: mac, ios, linux, windows)
build platform=current_os:
    @just _build-{{platform}}

# ─── macOS ────────────────────────────────────────────────────────────────────

[private]
_dev-mac:
    cargo run

[private]
_build-mac:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Building Glide (release)..."
    cargo build --release
    echo "Packaging Glide.app..."
    APP="{{ROOT}}/target/release/Glide.app"
    rm -rf "$APP"
    mkdir -p "$APP/Contents/MacOS"
    mkdir -p "$APP/Contents/Resources/assets/icons"
    cp "{{ROOT}}/target/release/Glide" "$APP/Contents/MacOS/Glide"
    cp "{{ROOT}}/Info.plist" "$APP/Contents/Info.plist"
    cp "{{ROOT}}/assets/icons/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"
    cp "{{ROOT}}/assets/icons/"*.png "$APP/Contents/Resources/assets/icons/" 2>/dev/null || true
    cp "{{ROOT}}/assets/icons/"*.svg "$APP/Contents/Resources/assets/icons/" 2>/dev/null || true
    echo "Done: $APP"

# ─── iOS ──────────────────────────────────────────────────────────────────────

[private]
_dev-ios:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Building glide-core for iOS Simulator..."
    cargo build --package glide-core --target aarch64-apple-ios-sim
    echo "Generating Xcode project..."
    cd "{{ROOT}}/ios" && xcodegen generate --quiet
    SIM_NAME="iPhone 17 Pro"
    echo "Building iOS app (debug, simulator: $SIM_NAME)..."
    cd "{{ROOT}}/ios" && xcodebuild build \
        -project Glide.xcodeproj \
        -scheme GlideApp \
        -destination "platform=iOS Simulator,name=$SIM_NAME" \
        -configuration Debug \
        -derivedDataPath "{{ROOT}}/ios/build" \
        -quiet
    echo "Launching iOS Simulator..."
    APP_PATH=$(find "{{ROOT}}/ios/build" -name "GlideApp.app" -path "*/Debug-iphonesimulator/*" | head -1)
    if [ -z "$APP_PATH" ]; then
        echo "Error: Could not find built GlideApp.app"
        exit 1
    fi
    xcrun simctl boot "$SIM_NAME" 2>/dev/null || true
    open -a Simulator
    xcrun simctl install "$SIM_NAME" "$APP_PATH"
    xcrun simctl launch "$SIM_NAME" com.glide.app

[private]
_build-ios:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Building glide-core for iOS device..."
    cargo build --package glide-core --release --target aarch64-apple-ios
    echo "Generating Xcode project..."
    cd "{{ROOT}}/ios" && xcodegen generate --quiet
    echo "Building iOS app (release, device)..."
    cd "{{ROOT}}/ios" && xcodebuild build \
        -project Glide.xcodeproj \
        -scheme GlideApp \
        -destination 'generic/platform=iOS' \
        -configuration Release \
        -quiet
    echo "Done: iOS release build complete"

# ─── Linux ────────────────────────────────────────────────────────────────────

[private]
_dev-linux:
    cargo run

[private]
_build-linux:
    cargo build --release

# ─── Windows ──────────────────────────────────────────────────────────────────

[private]
_dev-windows:
    cargo run

[private]
_build-windows:
    cargo build --release

# ─── Xcode ────────────────────────────────────────────────────────────────────

# Generate Xcode project and open it
xcode:
    cd {{ROOT}}/ios && xcodegen generate
    open {{ROOT}}/ios/Glide.xcodeproj

# Generate Xcode project only
xcode-gen:
    cd {{ROOT}}/ios && xcodegen generate

# ─── Setup ────────────────────────────────────────────────────────────────────

# Check and install required tools
setup:
    {{ROOT}}/setup.sh

# Install iOS Rust compilation targets
setup-ios:
    rustup target add aarch64-apple-ios aarch64-apple-ios-sim

# ─── Quality ──────────────────────────────────────────────────────────────────

# Run all tests
test:
    cargo test --workspace

# Run clippy lints
lint:
    cargo clippy --workspace -- -D warnings

# Format all code
fmt:
    cargo fmt --all

# Check compilation without building
check:
    cargo check --workspace

# ─── Cleanup ──────────────────────────────────────────────────────────────────

# Remove all build artifacts
clean:
    cargo clean
    rm -rf {{ROOT}}/ios/Glide.xcodeproj
    rm -rf {{ROOT}}/ios/build
    rm -rf {{ROOT}}/ios/DerivedData
