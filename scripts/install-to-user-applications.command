#!/bin/bash
set -euo pipefail

# This script intentionally installs only for the current macOS user.
SOURCE_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_SOURCE="$SOURCE_DIR/Voicely Local.app"
TARGET_DIR="$HOME/Applications"
TARGET_APP="$TARGET_DIR/Voicely Local.app"

if [[ ! -d "$APP_SOURCE" ]]; then
  osascript -e 'display alert "Voicely konnte nicht installiert werden" message "Das App-Bundle wurde im geöffneten DMG nicht gefunden." as critical'
  exit 1
fi

mkdir -p "$TARGET_DIR"
if [[ -e "$TARGET_APP" ]]; then
  BACKUP="$TARGET_DIR/Voicely Local (vorherige Version).app"
  rm -rf "$BACKUP"
  mv "$TARGET_APP" "$BACKUP"
fi

ditto "$APP_SOURCE" "$TARGET_APP"
xattr -dr com.apple.quarantine "$TARGET_APP" 2>/dev/null || true
open "$TARGET_DIR"
osascript -e 'display notification "Voicely wurde nach ~/Applications installiert." with title "Voicely Local"'
