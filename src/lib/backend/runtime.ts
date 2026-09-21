// Runtime detection shared by the composition root (`BackendContext`) and the
// transport adapter (`TauriSermonBackend`). This is the single source of truth
// for "are we inside the native Tauri webview?".
//
// SSR/build-safe: never touches `window` when there is no `window`.

type TauriWindow = typeof window & {
  __TAURI_INTERNALS__?: unknown;
  __TAURI__?: unknown;
};

/** True when running inside the Tauri webview (global injected by Tauri v2). */
export function isTauriRuntime(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }
  const w = window as TauriWindow;
  return w.__TAURI_INTERNALS__ != null || w.__TAURI__ != null;
}
