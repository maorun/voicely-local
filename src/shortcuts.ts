export type ShortcutState = "Pressed" | "Released";
export type ShortcutHandler = (event: { state: ShortcutState }) => void;
export type KeyboardShortcutEvent = { key: string; code: string; metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean };

export function acceleratorFromKeyboardEvent(event: KeyboardShortcutEvent): string | null {
  if (["Meta", "Control", "Alt", "Shift"].includes(event.key)) return null;
  const key = event.code === "Space" ? "Space" : event.code.startsWith("Key") ? event.code.slice(3) : event.code.startsWith("Digit") ? event.code.slice(5) : event.key.length === 1 ? event.key.toUpperCase() : event.key;
  const modifiers = [event.metaKey ? "Command" : "", event.ctrlKey ? "Control" : "", event.altKey ? "Alt" : "", event.shiftKey ? "Shift" : ""].filter(Boolean);
  return [...modifiers, key].join("+");
}

export function canonicalShortcut(shortcut: string): string {
  const modifiers = new Set<string>();
  let key = "";
  for (const token of shortcut.split("+").map((part) => part.trim().toLowerCase()).filter(Boolean)) {
    if (["commandorcontrol", "cmdorctrl", "command", "cmd", "super", "meta"].includes(token)) modifiers.add("super");
    else if (["control", "ctrl"].includes(token)) modifiers.add("control");
    else if (["option", "alt"].includes(token)) modifiers.add("alt");
    else if (token === "shift") modifiers.add("shift");
    else if (/^[a-z]$/.test(token)) key = `key${token}`;
    else if (/^[0-9]$/.test(token)) key = `digit${token}`;
    else key = token;
  }
  return ["shift", "control", "alt", "super"].filter((modifier) => modifiers.has(modifier)).concat(key).join("+");
}

export type GlobalShortcutApi = {
  isRegistered: (shortcut: string) => Promise<boolean>;
  register: (shortcut: string, handler: ShortcutHandler) => Promise<void>;
  unregister: (shortcut: string) => Promise<void>;
};

export async function ensureGlobalShortcut(
  shortcut: string,
  handler: ShortcutHandler,
  api: GlobalShortcutApi,
): Promise<void> {
  if (!await api.isRegistered(shortcut)) await api.register(shortcut, handler);
}

export class ShortcutRegistration {
  private activeShortcut: string | null = null;
  private queue: Promise<void> = Promise.resolve();

  constructor(private readonly api: GlobalShortcutApi) {}

  get active(): string | null { return this.activeShortcut; }

  set(shortcut: string, handler: ShortcutHandler): Promise<void> {
    return this.enqueue(async () => {
      await replaceGlobalShortcut(this.activeShortcut, shortcut, handler, this.api);
      this.activeShortcut = shortcut;
    });
  }

  clear(): Promise<void> {
    return this.enqueue(async () => {
      const active = this.activeShortcut;
      if (active && await this.api.isRegistered(active)) await this.api.unregister(active);
      this.activeShortcut = null;
    });
  }

  private enqueue(operation: () => Promise<void>): Promise<void> {
    const result = this.queue.then(operation);
    this.queue = result.catch(() => undefined);
    return result;
  }
}

export async function replaceGlobalShortcut(
  previous: string | null,
  next: string,
  handler: ShortcutHandler,
  api: GlobalShortcutApi,
): Promise<void> {
  if (previous === next) {
    await ensureGlobalShortcut(next, handler, api);
    return;
  }

  const previousWasRegistered = previous ? await api.isRegistered(previous) : false;
  if (previousWasRegistered && previous) await api.unregister(previous);

  try {
    if (await api.isRegistered(next)) throw new Error(`Shortcut ${next} ist bereits registriert.`);
    await api.register(next, handler);
  } catch (error) {
    if (previousWasRegistered && previous && !await api.isRegistered(previous)) {
      await api.register(previous, handler);
    }
    throw error;
  }
}
