import { useEffect, useRef } from "react";
import { STATUS_POLL_INTERVAL_MS } from "../constants";

/** Refresh on mount and on an interval while the window is visible, never overlapping. */
export function useStatusPolling(refresh: () => Promise<unknown>) {
  const inFlight = useRef(false);

  useEffect(() => {
    const tick = () => {
      if (inFlight.current) return;
      inFlight.current = true;
      refresh().finally(() => {
        inFlight.current = false;
      });
    };

    tick();
    const interval = window.setInterval(() => {
      if (document.visibilityState === "visible") tick();
    }, STATUS_POLL_INTERVAL_MS);
    return () => window.clearInterval(interval);
  }, [refresh]);
}
