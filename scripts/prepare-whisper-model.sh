#!/bin/bash
set -euo pipefail

MODEL_DIR="src-tauri/resources/models"
MODEL_PATH="$MODEL_DIR/ggml-small.bin"
PART_PATH="$MODEL_PATH.part"
MODEL_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
EXPECTED_SHA256="1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b"

verify() {
  [[ -f "$1" ]] || return 1
  [[ "$(shasum -a 256 "$1" | awk '{print $1}')" == "$EXPECTED_SHA256" ]]
}

mkdir -p "$MODEL_DIR"
if verify "$MODEL_PATH"; then
  echo "Whisper Small is already present and SHA-256 verified."
  exit 0
fi

rm -f "$MODEL_PATH"
echo "Downloading bundled Whisper Small model (~465 MB)…"
# curl resumes only a previous .part download. The final model is never exposed
# until its complete SHA-256 value matches the pinned upstream artifact.
curl --fail --location --retry 3 --continue-at - --output "$PART_PATH" "$MODEL_URL"

if ! verify "$PART_PATH"; then
  echo "Whisper Small SHA-256 verification failed; removing incomplete download." >&2
  rm -f "$PART_PATH"
  exit 1
fi

mv "$PART_PATH" "$MODEL_PATH"
echo "Whisper Small downloaded and SHA-256 verified."
