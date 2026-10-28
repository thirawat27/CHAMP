import { EventCallback, listen, UnlistenFn } from "@tauri-apps/api/event";

/** True inside the Tauri webview (false in tests and plain browsers). */
export const isTauriRuntime = () => "__TAURI_INTERNALS__" in window;

/** Subscribe to a Tauri event, or do nothing outside the Tauri runtime. */
export function listenIfTauri<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  return isTauriRuntime() ? listen<T>(event, handler) : Promise.resolve(() => {});
}
