import { useEffect, useRef } from "react";

type EscapeHandler = { current: () => void };

/** Open layers, oldest first. Only the newest one reacts to Esc. */
const stack: EscapeHandler[] = [];

function handleKeyDown(event: KeyboardEvent) {
  if (event.code !== "Escape") return;
  const top = stack[stack.length - 1];
  if (!top) return;
  event.preventDefault();
  top.current();
}

/**
 * Close the active layer (modal, drawer, toast) on Esc.
 *
 * All layers share one window listener and a stack, so one key press closes
 * exactly one layer. Registration follows `enabled` rather than the handler's
 * identity, so an inline handler keeps its place in the stack across renders.
 */
export function useEscape(handler: () => void, enabled = true) {
  const latest = useRef(handler);
  useEffect(() => {
    latest.current = handler;
  });

  useEffect(() => {
    if (!enabled) return undefined;
    stack.push(latest);
    if (stack.length === 1) window.addEventListener("keydown", handleKeyDown);
    return () => {
      stack.splice(stack.indexOf(latest), 1);
      if (stack.length === 0) window.removeEventListener("keydown", handleKeyDown);
    };
  }, [enabled]);
}
