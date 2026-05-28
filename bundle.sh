#!/usr/bin/env bash
set -euo pipefail

# Build a macOS .app bundle for Glide
# Usage: ./bundle.sh [--debug] [--sign|--no-sign]

PROFILE="release"
CARGO_FLAG="--release"
SIGN_MODE="auto"
TEAM_ID="${GLIDE_APPLE_TEAM_ID:-${APPLE_TEAM_ID:-3C99V29U26}}"
IDENTITY="${GLIDE_APPLE_CODESIGN_IDENTITY:-${APPLE_CODESIGN_IDENTITY:-}}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug)
            PROFILE="debug"
            CARGO_FLAG=""
            ;;
        --sign)
            SIGN_MODE="required"
            ;;
        --no-sign)
            SIGN_MODE="never"
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 2
            ;;
    esac
    shift
done

ROOT="$(cd "$(dirname "$0")" && pwd)"
APP="$ROOT/target/$PROFILE/Glide.app"
HELPER_APP="$APP/Contents/Helpers/GlideAppleHelper.app"
ENTITLEMENTS="$ROOT/macos/Glide.entitlements"
APP_BUNDLE_ID="${GLIDE_APP_BUNDLE_ID:-com.ghenti.glide.mac}"
APP_DISPLAY_NAME="${GLIDE_APP_DISPLAY_NAME:-Glide}"
if [[ "$PROFILE" == "debug" ]]; then
    APP_BUNDLE_ID="${GLIDE_APP_BUNDLE_ID:-com.ghenti.glide.mac.dev}"
    APP_DISPLAY_NAME="${GLIDE_APP_DISPLAY_NAME:-Glide Dev}"
fi
HELPER_BUNDLE_ID="$APP_BUNDLE_ID.apple-helper"

find_codesign_identity() {
    if [[ -n "$IDENTITY" ]]; then
        printf '%s\n' "$IDENTITY"
        return 0
    fi

    if [[ -z "$TEAM_ID" ]]; then
        return 1
    fi

    security find-identity -v -p codesigning 2>/dev/null \
        | awk -v team="$TEAM_ID" '
            index($0, "(" team ")") || index($0, team) {
                print $2
                exit
            }
        '
}

if [[ "$SIGN_MODE" != "never" ]]; then
    IDENTITY="$(find_codesign_identity || true)"
    if [[ "$SIGN_MODE" == "required" && -z "$IDENTITY" ]]; then
        echo "No valid code-signing identity found for Apple team $TEAM_ID." >&2
        echo "Install an Apple Development or Developer ID Application certificate, or set GLIDE_APPLE_CODESIGN_IDENTITY." >&2
        exit 1
    fi
fi

if [[ -n "$IDENTITY" ]]; then
    export GLIDE_APPLE_CODESIGN_IDENTITY="$IDENTITY"
    export GLIDE_APPLE_TEAM_ID="$TEAM_ID"
fi

echo "Building Glide ($PROFILE)..."
cargo build $CARGO_FLAG

echo "Creating app bundle..."
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
mkdir -p "$APP/Contents/Resources/assets/icons"
mkdir -p "$HELPER_APP/Contents/MacOS"

# Binary
cp "$ROOT/target/$PROFILE/Glide" "$APP/Contents/MacOS/Glide"
HELPER="$(find "$ROOT/target/$PROFILE/build" -path '*/out/GlideAppleHelper' -type f 2>/dev/null | head -1 || true)"
if [[ -n "$HELPER" ]]; then
    cp "$HELPER" "$HELPER_APP/Contents/MacOS/GlideAppleHelper"
    HELPER_INFO="$(dirname "$HELPER")/GlideAppleHelper-Info.plist"
    if [[ -f "$HELPER_INFO" ]]; then
        cp "$HELPER_INFO" "$HELPER_APP/Contents/Info.plist"
    else
        cat > "$HELPER_APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>GlideAppleHelper</string>
  <key>CFBundleIdentifier</key>
  <string>com.ghenti.glide.mac.apple-helper</string>
  <key>CFBundleName</key>
  <string>Glide Apple Helper</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>NSSpeechRecognitionUsageDescription</key>
  <string>Glide uses Apple Speech for local on-device dictation when selected.</string>
</dict>
</plist>
PLIST
    fi
    /usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $HELPER_BUNDLE_ID" "$HELPER_APP/Contents/Info.plist"
fi

# Info.plist
cp "$ROOT/Info.plist" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $APP_BUNDLE_ID" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName $APP_DISPLAY_NAME" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleName $APP_DISPLAY_NAME" "$APP/Contents/Info.plist"

# Icon
cp "$ROOT/assets/icons/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"

# Assets (logos, provider icons, accent icons)
cp "$ROOT/assets/icons/"*.icns "$APP/Contents/Resources/assets/icons/" 2>/dev/null || true
cp "$ROOT/assets/icons/"*.png "$APP/Contents/Resources/assets/icons/" 2>/dev/null || true
cp "$ROOT/assets/icons/"*.svg "$APP/Contents/Resources/assets/icons/" 2>/dev/null || true

if [[ "$SIGN_MODE" != "never" ]]; then
    if [[ -n "$IDENTITY" ]]; then
        echo "Signing Glide.app with: $IDENTITY"
        if [[ -f "$HELPER_APP/Contents/MacOS/GlideAppleHelper" ]]; then
            codesign --force --timestamp --options runtime --sign "$IDENTITY" \
                --identifier "$HELPER_BUNDLE_ID" \
                "$HELPER_APP"
        fi
        codesign --force --timestamp --options runtime --sign "$IDENTITY" \
            --entitlements "$ENTITLEMENTS" \
            --identifier "$APP_BUNDLE_ID" \
            "$APP/Contents/MacOS/Glide"
        codesign --force --timestamp --options runtime --sign "$IDENTITY" \
            --entitlements "$ENTITLEMENTS" \
            "$APP"
    elif [[ -n "$TEAM_ID" ]]; then
        echo "Signing skipped: no valid code-signing identity found for Apple team $TEAM_ID." >&2
        echo "Apple Speech locale assets require a signed helper." >&2
    fi
fi

echo "Done: $APP"
