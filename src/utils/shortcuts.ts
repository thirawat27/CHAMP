/** Keyboard shortcuts: one table for the handler, Help, and button tooltips. */

const isMac = typeof navigator !== "undefined" && /mac/i.test(navigator.userAgent);

export const MOD_KEY = isMac ? "⌘" : "Ctrl";

/** Physical key positions, so shortcuts work in any keyboard layout. */
export const SHORTCUTS = {
  start: { code: "KeyS", key: "S" },
  restart: { code: "KeyR", key: "R" },
  stop: { code: "KeyX", key: "X" },
  website: { code: "KeyW", key: "W" },
  database: { code: "KeyD", key: "D" },
  projects: { code: "KeyO", key: "O" },
  logs: { code: "KeyL", key: "L" },
  terminal: { code: "KeyT", key: "T" },
  settings: { code: "Comma", key: "," },
} as const;

export type ShortcutId = keyof typeof SHORTCUTS;

/** "Ctrl S": split on the space to render one `<kbd>` per key. */
export const shortcutKeys = (id: ShortcutId) => `${MOD_KEY} ${SHORTCUTS[id].key}`;

/** "Ctrl+S": for tooltips. */
export const shortcutHint = (id: ShortcutId) => `${MOD_KEY}+${SHORTCUTS[id].key}`;

/** Ctrl/Cmd alone. Ctrl+Shift+D (debug menu) and browser chords stay separate. */
export const hasPrimaryModifier = (event: KeyboardEvent) =>
  (event.ctrlKey || event.metaKey) && !event.shiftKey && !event.altKey;

/** `?` in any layout: on Thai layouts Shift+/ does not produce "?". */
export const isHelpKey = (event: KeyboardEvent) =>
  event.key === "?" || (event.code === "Slash" && event.shiftKey);

/** Typing targets keep their own Ctrl+X, `?`, and similar keys. */
export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement
  );
}
