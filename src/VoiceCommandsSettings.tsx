type CustomAppCommand = { phrase: string; appPath: string };
type RecordingMode = "toggle" | "pushToTalk";

interface VoiceCommandsSettingsProps {
  commandShortcutDraft: string;
  commandShortcutMessage: string;
  capturingCommandShortcut: boolean;
  commandMode: RecordingMode;
  customAppCommands: CustomAppCommand[];
  commandEditDraft: CustomAppCommand;
  commandEditIndex: number | null;
  onCaptureCommandShortcut: (event: React.KeyboardEvent<HTMLInputElement>) => void;
  onSaveCommandShortcut: () => Promise<void>;
  onSetCapturingCommandShortcut: (state: boolean) => void;
  onSetCommandShortcutDraft: (shortcut: string) => void;
  onUpdateCommandMode: (mode: RecordingMode) => void;
  onAddCustomCommand: () => void;
  onDeleteCustomCommand: (index: number) => void;
  onEditCustomCommand: (index: number, command: CustomAppCommand) => void;
  onCancelEditCustomCommand: () => void;
  onSetCommandEditDraft: (command: CustomAppCommand) => void;
}

export default function VoiceCommandsSettings(props: VoiceCommandsSettingsProps) {
  return (
    <details>
      <summary>Sprachbefehle <span>⌄</span></summary>
      <p>Steuere Apps oder benutzerdefinierte Anwendungen mit Sprachbefehlen. Integrierte Apps: Mail, Kalender, iTerm, Notizen (kein Slack oder Shell-Befehle).</p>
      <label className="shortcut-setting">
        Sprachbefehl-Shortcut{" "}
        <span>
          {props.capturingCommandShortcut
            ? "Jetzt gewünschte Tastenkombination drücken · Escape zum Abbrechen"
            : "Feld anklicken, dann gewünschte Tastenkombination drücken"}
        </span>
        <input
          className={props.capturingCommandShortcut ? "shortcut-recorder capturing" : "shortcut-recorder"}
          value={props.capturingCommandShortcut ? "Tastenkombination drücken …" : props.commandShortcutDraft}
          readOnly
          onFocus={() => props.onSetCapturingCommandShortcut(true)}
          onClick={() => props.onSetCapturingCommandShortcut(true)}
          onKeyDown={props.onCaptureCommandShortcut}
          onBlur={() => props.onSetCapturingCommandShortcut(false)}
          aria-label="Sprachbefehl-Shortcut aufnehmen"
        />
      </label>
      <button className="save-shortcut" onClick={() => void props.onSaveCommandShortcut()}>
        Shortcut speichern und aktivieren
      </button>
      {props.commandShortcutMessage && <p className="shortcut-message">{props.commandShortcutMessage}</p>}
      <label className="shortcut-setting">
        Sprachbefehl-Modus
        <select value={props.commandMode} onChange={(event) => props.onUpdateCommandMode(event.target.value as RecordingMode)}>
          <option value="toggle">Toggle — einmal drücken zum Starten/Stoppen</option>
          <option value="pushToTalk">Push-to-talk — gedrückt halten zum Aufnehmen</option>
        </select>
      </label>
      <fieldset>
        <legend>Benutzerdefinierte App-Befehle</legend>
        <div className="custom-commands-list">
          {props.customAppCommands.map((command, index) => (
            <div key={index} className="custom-command-item">
              <div>
                <strong>{command.phrase}</strong>
                <span>{command.appPath}</span>
              </div>
              <span>
                <button onClick={() => props.onEditCustomCommand(index, command)}>Bearbeiten</button>
                <button onClick={() => props.onDeleteCustomCommand(index)}>Löschen</button>
              </span>
            </div>
          ))}
        </div>
        <div className="custom-command-form">
          <input
            type="text"
            placeholder='Sprachbefehl (z.B. "öffne notion")'
            value={props.commandEditDraft.phrase}
            onChange={(e) => props.onSetCommandEditDraft({ ...props.commandEditDraft, phrase: e.target.value })}
          />
          <input
            type="text"
            placeholder="App-Pfad (z.B. /Applications/Notion.app)"
            value={props.commandEditDraft.appPath}
            onChange={(e) => props.onSetCommandEditDraft({ ...props.commandEditDraft, appPath: e.target.value })}
          />
          <button onClick={props.onAddCustomCommand}>
            {props.commandEditIndex !== null ? "Aktualisieren" : "Hinzufügen"}
          </button>
          {props.commandEditIndex !== null && <button onClick={props.onCancelEditCustomCommand}>Abbrechen</button>}
        </div>
      </fieldset>
    </details>
  );
}
