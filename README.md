# Voicely Local

A privacy-first, local desktop voice-writing assistant for macOS. Speak naturally, let Voicely transcribe locally, clean up dictated text, apply local writing transformations, and paste the result into the app you are already using.

## Highlights

- **Local transcription:** bundled Whisper Small model; dictated audio is processed on-device.
- **Local writing assistant:** optional Qwen GGUF models run through a local Metal-accelerated worker.
- **System-wide dictation:** configurable global shortcut with toggle and push-to-talk modes.
- **Smart dictation cleanup:** punctuation, filler words, repetitions, spoken self-corrections, and a personal dictionary.
- **Automatic paste:** optionally paste the completed dictation into the active macOS app.
- **Multi-monitor overlay:** the floating dictation bar follows the focused window, with a safe screen-center fallback.
- **Configurable audio:** input device, noise suppression, echo cancellation, gain control, and a long silence timeout.
- **Local history and stats:** text-only history, word counts, and dictation WPM. Audio is never stored.
- **Safe model management:** optional text models are explicitly downloaded, resumable, cancellable, and SHA-256 validated.

## Privacy

Voicely is designed to keep sensitive content on the device:

- No dictated audio is uploaded.
- No transcripts or writing-model prompts are sent to a cloud API.
- Audio is not stored in history.
- Optional text models are downloaded only after a user action.
- Diagnostics are local, capped in size, and can be cleared from the app.

## Requirements

- macOS on Apple Silicon (the current release target is `aarch64-apple-darwin`)
- Node.js 24+
- Rust/Cargo
- Xcode Command Line Tools and CMake

```bash
xcode-select --install
brew install cmake
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Development

Install JavaScript dependencies:

```bash
npm ci
```

Prepare the bundled Whisper Small model. The 465 MB model is deliberately not committed to Git; the script downloads it only if necessary and verifies its pinned SHA-256 checksum.

```bash
npm run prepare:models
```

Start the app in development mode:

```bash
npm run tauri dev
```

Useful checks:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml --features whisper-worker --bin whisper-worker
cargo check --manifest-path src-tauri/Cargo.toml --features llm-worker --bin llm-worker
git diff --check
```

## Build and install

Build the full desktop app. This validates Whisper Small, builds both worker binaries, then creates the Tauri macOS bundle.

```bash
npm run build:desktop
```

Create the user-scoped DMG:

```bash
npm run dmg:user
```

The DMG is written to:

```text
src-tauri/target/release/bundle/dmg/Voicely Local_0.1.0_aarch64.dmg
```

Open the DMG and run **Install Voicely Local**. It installs the app for the current user at:

```text
~/Applications/Voicely Local.app
```

No administrator access is required.

## macOS permissions

Voicely requests the following permissions only when their related feature is used:

| Permission | Used for |
| --- | --- |
| Microphone | Recording local dictation |
| Accessibility / Bedienungshilfen | Automatic paste into the active app and focused-window overlay placement |

Accessibility can be enabled in **System Settings → Privacy & Security → Accessibility**.

## Architecture

```text
React + TypeScript + Vite
        │
        ├── Web Audio API: capture PCM, WAV encoding, level, silence detection
        │
Tauri 2 / Rust main process
        │
        ├── whisper-worker: isolated local Whisper inference
        ├── llm-worker: persistent local llama.cpp / Metal GGUF inference
        └── macOS integrations: global shortcuts, overlay, accessibility paste
```

Whisper and llama.cpp run in isolated workers to avoid duplicate `ggml` symbol conflicts and to isolate native inference failures from the UI process.

## CI

GitHub Actions runs on macOS for pushes to `master` and pull requests. It caches npm, Cargo build artifacts, and the verified Whisper Small model, then runs the full desktop build plus native worker checks.

## License and model notices

Before public distribution, include the licenses and notices for Tauri, whisper.cpp / whisper-rs, llama.cpp / llama-cpp-2, Qwen models, and bundled Whisper model assets. Public macOS distribution additionally requires Developer ID signing and Apple notarization.
