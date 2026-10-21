import { describe, expect, it, vi } from "vitest";
import { acceleratorFromKeyboardEvent, canonicalShortcut, replaceGlobalShortcut, ShortcutRegistration, type GlobalShortcutApi, type ShortcutHandler } from "./shortcuts";

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

  it("serialisiert StrictMode-Setup, Cleanup und erneutes Setup", async () => {
    const shortcut = "CommandOrControl+Shift+Space";
    const { active, api } = createApi();
    const registration = new ShortcutRegistration(api);
    const staleHandler = vi.fn();
    const currentHandler = vi.fn();

    const firstSetup = registration.set(shortcut, staleHandler);
    const staleCleanup = registration.clear();
    const currentSetup = registration.set(shortcut, currentHandler);
    await Promise.all([firstSetup, staleCleanup, currentSetup]);

    expect(active.get(shortcut)).toBe(currentHandler);
    expect(registration.active).toBe(shortcut);
    expect(api.register).toHaveBeenCalledTimes(2);
    expect(api.unregister).toHaveBeenCalledTimes(1);
  });

  it("unterscheidet Control und Command bei der Tastenerfassung", () => {
    const key = { key: "d", code: "KeyD", altKey: false, shiftKey: true };
    expect(acceleratorFromKeyboardEvent({ ...key, metaKey: false, ctrlKey: true })).toBe("Control+Shift+D");
    expect(acceleratorFromKeyboardEvent({ ...key, metaKey: true, ctrlKey: false })).toBe("Command+Shift+D");
    expect(acceleratorFromKeyboardEvent({ ...key, metaKey: true, ctrlKey: true })).toBe("Command+Control+Shift+D");
  });

  it("gibt null zurück bei Modifikatoren und ungültigen Edge-Case-Eingaben", () => {
    expect(acceleratorFromKeyboardEvent({ key: "Shift", code: "ShiftLeft", metaKey: false, ctrlKey: false, altKey: false, shiftKey: true })).toBeNull();
    expect(acceleratorFromKeyboardEvent({ key: "ControlLeft", code: "ControlLeft", metaKey: false, ctrlKey: true, altKey: false, shiftKey: false })).toBeNull();
    expect(acceleratorFromKeyboardEvent({ key: "Unidentified", code: "", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false })).toBeNull();
    expect(acceleratorFromKeyboardEvent({ key: "", code: "", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false })).toBeNull();
    expect(acceleratorFromKeyboardEvent({ key: "Meta", code: "MetaLeft", metaKey: true, ctrlKey: false, altKey: false, shiftKey: false })).toBeNull();
  });

  it("ordnet native macOS-Shortcut-Namen den gespeicherten Acceleratoren zu", () => {
    expect(canonicalShortcut("CommandOrControl+Shift+D")).toBe(canonicalShortcut("shift+super+KeyD"));
    expect(canonicalShortcut("CommandOrControl+Alt+S")).toBe(canonicalShortcut("alt+super+KeyS"));
    expect(canonicalShortcut("CommandOrControl+Shift+D")).not.toBe(canonicalShortcut("alt+super+KeyS"));
  });
});
