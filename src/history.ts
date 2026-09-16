export type DictationEntry = { id: string; createdAt: string; text: string; raw: string; language: string; durationSeconds?: number };
const KEY = "voicely-dictation-history";
const MAX_ENTRIES = 100;

export function loadHistory(): DictationEntry[] {
  try { const data = JSON.parse(localStorage.getItem(KEY) || "[]"); return Array.isArray(data) ? data.filter(isEntry).slice(0, MAX_ENTRIES) : []; } catch { return []; }
}
export function saveHistory(entries: DictationEntry[]) { localStorage.setItem(KEY, JSON.stringify(entries.slice(0, MAX_ENTRIES))); }
export function addHistoryEntry(entries: DictationEntry[], entry: DictationEntry) { const next = [entry, ...entries].slice(0, MAX_ENTRIES); saveHistory(next); return next; }
export function removeHistoryEntry(entries: DictationEntry[], id: string) { const next = entries.filter((entry) => entry.id !== id); saveHistory(next); return next; }
export function clearHistory() { localStorage.removeItem(KEY); }
function isEntry(value: unknown): value is DictationEntry { return Boolean(value && typeof value === "object" && "id" in value && "text" in value && "createdAt" in value); }
