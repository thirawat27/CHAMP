import { useCallback, useEffect, useRef, useState } from "react";
import { COPIED_FEEDBACK_MS } from "../constants";

/**
 * A "Copied" flag that turns itself off after a moment. Copying again restarts
 * the moment instead of being cut short by the previous timer, and closing the
 * component cancels it.
 */
export function useCopiedFlag() {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const markCopied = useCallback(() => {
    window.clearTimeout(timer.current);
    setCopied(true);
    timer.current = window.setTimeout(() => setCopied(false), COPIED_FEEDBACK_MS);
  }, []);

  const resetCopied = useCallback(() => {
    window.clearTimeout(timer.current);
    setCopied(false);
  }, []);

  return { copied, markCopied, resetCopied };
}
