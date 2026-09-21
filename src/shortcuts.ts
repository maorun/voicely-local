export type ShortcutState = "Pressed" | "Released";
export type ShortcutHandler = (event: { state: ShortcutState }) => void;

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
