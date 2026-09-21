import { describe, expect, it, vi } from "vitest";
import { replaceGlobalShortcut, type GlobalShortcutApi, type ShortcutHandler } from "./shortcuts";

function createApi(initial: string[] = [], rejected: string[] = []) {
  const active = new Map<string, ShortcutHandler>();
  for (const shortcut of initial) active.set(shortcut, () => undefined);

  const api: GlobalShortcutApi = {
    isRegistered: vi.fn(async (shortcut) => active.has(shortcut)),
    register: vi.fn(async (shortcut, handler) => {
      if (rejected.includes(shortcut) || active.has(shortcut)) throw new Error("nicht verfügbar");
      active.set(shortcut, handler);
    }),
    unregister: vi.fn(async (shortcut) => { active.delete(shortcut); }),
  };

  return { active, api };
}

describe("replaceGlobalShortcut", () => {
  it("aktiviert den neuen Shortcut unmittelbar und entfernt den alten", async () => {
    const { active, api } = createApi(["CommandOrControl+Shift+Space"]);
    const handler = vi.fn();

    await replaceGlobalShortcut(
      "CommandOrControl+Shift+Space",
      "CommandOrControl+Alt+Space",
      handler,
      api,
    );

    expect(active.has("CommandOrControl+Shift+Space")).toBe(false);
    expect(active.get("CommandOrControl+Alt+Space")).toBe(handler);
  });

  it("stellt den alten Shortcut wieder her, wenn der neue nicht registriert werden kann", async () => {
    const previous = "CommandOrControl+Shift+Space";
    const next = "CommandOrControl+Alt+Space";
    const { active, api } = createApi([previous], [next]);
    const handler = vi.fn();

    await expect(replaceGlobalShortcut(previous, next, handler, api)).rejects.toThrow("nicht verfügbar");

    expect(active.get(previous)).toBe(handler);
    expect(active.has(next)).toBe(false);
  });
});
