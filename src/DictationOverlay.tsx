import { emit, listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { disposeWhenUnmounted } from "./events";

type LevelEvent = { level: number };
const thresholds = [0.05, 0.12, 0.2, 0.3, 0.42, 0.55, 0.7, 0.84];

export default function DictationOverlay() {
  const [level, setLevel] = useState(0);
  useEffect(() => {
    document.body.classList.add("overlay-body");
    const unlisten = disposeWhenUnmounted(listen<LevelEvent>("dictation-level", (event) => setLevel(Math.max(0, Math.min(1, event.payload.level)))));
    return () => { document.body.classList.remove("overlay-body"); unlisten(); };
  }, []);

  return <div className="dictation-overlay" aria-live="polite">
    <button className="drag-handle" data-tauri-drag-region aria-label="Diktat-Leiste verschieben" title="Leiste verschieben">⠿</button>
    <div className="level-meter" aria-label={`Mikrofonpegel ${Math.round(level * 100)} Prozent`}>
      {thresholds.map((threshold, index) => <span key={threshold} className={level >= threshold ? "level-bar active" : "level-bar"} style={{ height: `${28 + (index % 4) * 12}%` }} />)}
    </div>
    <div><p>VOICELY DIKTAT</p><strong>Ich höre zu</strong></div>
    <div className="overlay-actions"><span className="overlay-stop">⌘/Ctrl + ⇧ + Leertaste<br/>zum Stoppen</span><button className="cancel-dictation" onClick={() => void emit("dictation-cancel")}>Abbrechen</button></div>
  </div>;
}
