export type WhisperAction = {
  modelId: string;
  kind: "download" | "activate";
} | null;

export type WhisperActionPresentation = {
  acting: boolean;
  downloading: boolean;
  buttonLabel: string;
  statusMessage: string | null;
};

export function whisperActionPresentation(
  modelId: string,
  modelName: string,
  installed: boolean,
  active: boolean,
  action: WhisperAction,
): WhisperActionPresentation {
  const acting = action?.modelId === modelId;
  const downloading = acting && action.kind === "download";

  if (downloading) {
    return {
      acting,
      downloading,
      buttonLabel: "Abbrechen",
      statusMessage: `${modelName}: Download wird vorbereitet …`,
    };
  }

  if (acting) {
    return {
      acting,
      downloading,
      buttonLabel: "Wird aktiviert …",
      statusMessage: `${modelName} wird geprüft und aktiviert …`,
    };
  }

  return {
    acting,
    downloading,
    buttonLabel: active ? "Aktiv" : installed ? "Aktivieren" : "Herunterladen",
    statusMessage: null,
  };
}
