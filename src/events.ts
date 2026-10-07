/**
 * Ties an asynchronous Tauri event registration to an effect lifetime. If the effect is
 * cleaned up before `listen()` resolves (e.g. React StrictMode), the listener is disposed
 * as soon as it exists instead of leaking and delivering every event twice.
 */
export function disposeWhenUnmounted(registration: Promise<() => void>): () => void {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  void registration.then((dispose) => { if (disposed) dispose(); else unlisten = dispose; }).catch(() => undefined);
  return () => { disposed = true; unlisten?.(); };
}
