#!/bin/bash
set -euo pipefail

APP="src-tauri/target/release/bundle/macos/Voicely Local.app"
DMG="src-tauri/target/release/bundle/dmg/Voicely Local_0.1.0_aarch64.dmg"
INSTALLER="scripts/install-to-user-applications.command"
README="scripts/README-INSTALL.txt"

[[ -d "$APP" ]] || { echo "Missing release bundle: $APP" >&2; exit 1; }
chmod +x "$APP/Contents/Resources/binaries/whisper-worker-aarch64-apple-darwin" "$APP/Contents/Resources/binaries/llm-worker-aarch64-apple-darwin"
codesign --force --deep --sign - "$APP"
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
