import { useCallback, useEffect, useState } from "react";
import { TOAST_DISMISS_MS } from "../constants";
import { DashboardNotice } from "../types/dashboard";

interface ToastController {
  notice: DashboardNotice | null;
  notify: (notice: DashboardNotice) => void;
  dismiss: () => void;
}

/**
 * Owns the single toast notice state shared across the dashboard.
 *
 * Only one notice is visible at a time. Non-`info` notices auto-dismiss;
 * `info` (pending) notices persist until replaced or dismissed.
 */
export function useToast(): ToastController {
  const [notice, setNotice] = useState<DashboardNotice | null>(null);

  const notify = useCallback((next: DashboardNotice) => {
    setNotice(next);
  }, []);

  const dismiss = useCallback(() => {
    setNotice(null);
  }, []);

  useEffect(() => {
    if (!notice || notice.tone === "info") return undefined;

    const timeout = window.setTimeout(() => {
      setNotice(null);
    }, TOAST_DISMISS_MS);

    return () => window.clearTimeout(timeout);
  }, [notice]);

  return { notice, notify, dismiss };
}
