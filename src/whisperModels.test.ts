import { describe, expect, it } from "vitest";
import { whisperActionPresentation, type WhisperAction } from "./whisperModels";

const name = "Präzise — Whisper Medium";

describe("whisperActionPresentation", () => {
  it("meldet einen Download sofort nach dem Klick sichtbar zurück", () => {
    const action: WhisperAction = { modelId: "whisper-medium", kind: "download" };

    expect(whisperActionPresentation("whisper-medium", name, false, false, action)).toEqual({
      acting: true,
      downloading: true,
      buttonLabel: "Abbrechen",
      statusMessage: `${name}: Download wird vorbereitet …`,
    });
  });

  it("zeigt die laufende Prüfung und Aktivierung eines installierten Modells", () => {
    const action: WhisperAction = { modelId: "whisper-medium", kind: "activate" };

    expect(whisperActionPresentation("whisper-medium", name, true, false, action)).toEqual({
      acting: true,
      downloading: false,
      buttonLabel: "Wird aktiviert …",
      statusMessage: `${name} wird geprüft und aktiviert …`,
    });
  });

  it("zeigt ein erfolgreich aktiviertes Modell als aktiv", () => {
    expect(whisperActionPresentation("whisper-medium", name, true, true, null)).toEqual({
      acting: false,
      downloading: false,
      buttonLabel: "Aktiv",
      statusMessage: null,
    });
  });

  it("lässt andere Modelle während einer Aktion unverändert beschriftet", () => {
    const action: WhisperAction = { modelId: "whisper-medium", kind: "activate" };

    expect(whisperActionPresentation("whisper-base", "Whisper Base", false, false, action)).toEqual({
      acting: false,
      downloading: false,
      buttonLabel: "Herunterladen",
      statusMessage: null,
    });
  });
});
