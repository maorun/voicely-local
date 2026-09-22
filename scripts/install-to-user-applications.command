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

# Replacing a running app bundle leaves the old executable alive from the backup
# location. Stop it first so macOS cannot keep using stale code and permissions.
if pgrep -x "voicely-local" >/dev/null; then
  osascript -e 'tell application id "com.voicely.local" to quit' 2>/dev/null || true
  for _ in {1..20}; do
    pgrep -x "voicely-local" >/dev/null || break
    sleep 0.25
  done
  if pgrep -x "voicely-local" >/dev/null; then
    pkill -TERM -x "voicely-local" 2>/dev/null || true
    sleep 1
  fi
  if pgrep -x "voicely-local" >/dev/null; then
    osascript -e 'display alert "Voicely konnte nicht aktualisiert werden" message "Bitte beende Voicely vollständig und starte die Installation erneut." as critical'
    exit 1
  fi
fi

if [[ -e "$TARGET_APP" ]]; then
  BACKUP="$TARGET_DIR/Voicely Local (vorherige Version).app"
  rm -rf "$BACKUP"
  mv "$TARGET_APP" "$BACKUP"
fi

ditto "$APP_SOURCE" "$TARGET_APP"
xattr -dr com.apple.quarantine "$TARGET_APP" 2>/dev/null || true
open "$TARGET_APP"
open "$TARGET_DIR"
osascript -e 'display notification "Voicely wurde nach ~/Applications installiert und gestartet." with title "Voicely Local"'
