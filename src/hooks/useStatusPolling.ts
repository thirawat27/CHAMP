import { useEffect, useRef } from "react";
import { STATUS_POLL_INTERVAL_MS } from "../constants";

interface PollingOptions {
  /** Poll only while true (default true). */
  enabled?: boolean;
  /** Refresh as soon as polling starts, not only after the first interval (default true). */
  immediate?: boolean;
}

/**
 * Refresh on an interval while the window is visible, never overlapping: a
 * backend call that outlasts the interval (the process manager can be busy for
 * minutes initializing a database) must not queue up more of the same call.
 */
export function useStatusPolling(
  refresh: () => Promise<unknown>,
  { enabled = true, immediate = true }: PollingOptions = {}
) {
  const inFlight = useRef(false);

  useEffect(() => {
    if (!enabled) return undefined;

    const tick = () => {
      if (inFlight.current) return;
      inFlight.current = true;
      refresh().finally(() => {
        inFlight.current = false;
      });
    };

    if (immediate) tick();
    const interval = window.setInterval(() => {
      if (document.visibilityState === "visible") tick();
    }, STATUS_POLL_INTERVAL_MS);
    return () => window.clearInterval(interval);
  }, [refresh, enabled, immediate]);
}
