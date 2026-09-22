#!/bin/bash
set -euo pipefail

APP="src-tauri/target/release/bundle/macos/Voicely Local.app"
INSTALLER="scripts/install-to-user-applications.command"
README="scripts/README-INSTALL.txt"

[[ -d "$APP" ]] || { echo "Missing release bundle: $APP" >&2; exit 1; }
APP_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
DMG="src-tauri/target/release/bundle/dmg/Voicely Local_${APP_VERSION}_aarch64.dmg"
chmod +x "$APP/Contents/Resources/binaries/whisper-worker-aarch64-apple-darwin" "$APP/Contents/Resources/binaries/llm-worker-aarch64-apple-darwin"

LOCAL_SIGNING_IDENTITY="Voicely Local Development"
SIGNING_IDENTITY="${VOICELY_SIGNING_IDENTITY:-${APPLE_SIGNING_IDENTITY:-}}"
if [[ -z "$SIGNING_IDENTITY" ]] && security find-identity -v -p codesigning | grep -Fq "\"$LOCAL_SIGNING_IDENTITY\""; then
  SIGNING_IDENTITY="$LOCAL_SIGNING_IDENTITY"
fi
if [[ -z "$SIGNING_IDENTITY" ]]; then
  SIGNING_IDENTITY="-"
  echo "No persistent signing identity found; using an ad-hoc signature." >&2
fi

echo "Signing app with identity: $SIGNING_IDENTITY"
codesign --force --deep --sign "$SIGNING_IDENTITY" "$APP"
codesign --verify --deep --strict "$APP"
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

ditto "$APP" "$STAGING/Voicely Local.app"
cp "$INSTALLER" "$STAGING/Install Voicely Local.command"
cp "$README" "$STAGING/README — Install to User Applications.txt"
chmod +x "$STAGING/Install Voicely Local.command"
rm -f "$DMG"
hdiutil create -volname "Voicely Local" -srcfolder "$STAGING" -ov -format UDZO "$DMG"
echo "Created user-install DMG: $DMG"
