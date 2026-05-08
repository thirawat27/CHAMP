/**
 * Custom hook for managing toast notifications
 * 
 * This hook provides a simple interface for showing and dismissing
 * toast notifications with auto-dismiss functionality.
 * 
 * @example
 * ```tsx
 * const { notice, showNotice, dismissNotice } = useNotifications();
 * 
 * showNotice({
 *   tone: "success",
 *   title: "Success!",
 *   message: "Operation completed successfully"
 * });
 * ```
 */

import { useCallback, useEffect, useState } from "react";

export type NoticeTone = "info" | "success" | "error";
export type NoticeAction = "start" | "restart" | "stop";

export interface DashboardNotice {
  /** Visual tone of the notification */
  tone: NoticeTone;
  /** Optional action type for styling */
  action?: NoticeAction;
  /** Notification title */
  title: string;
  /** Notification message */
  message: string;
}

export interface UseNotificationsReturn {
  /** Current active notification (null if none) */
  notice: DashboardNotice | null;
  /** Show a new notification */
  showNotice: (notice: DashboardNotice) => void;
  /** Dismiss the current notification */
  dismissNotice: () => void;
}

/**
 * Hook for managing toast notifications
 * 
 * Automatically dismisses success and error notifications after a delay.
 * Info notifications stay visible until manually dismissed.
 * 
 * @param autoDismissDelay - Delay in milliseconds before auto-dismissing (default: 4200)
 * @returns Notification management interface
 */
export function useNotifications(autoDismissDelay = 4200): UseNotificationsReturn {
  const [notice, setNotice] = useState<DashboardNotice | null>(null);

  /**
   * Show a new notification
   * 
   * Replaces any existing notification.
   * 
   * @param newNotice - The notification to display
   */
  const showNotice = useCallback((newNotice: DashboardNotice) => {
    setNotice(newNotice);
  }, []);

  /**
   * Dismiss the current notification
   */
  const dismissNotice = useCallback(() => {
    setNotice(null);
  }, []);

  // Auto-dismiss success and error notifications
  useEffect(() => {
    if (!notice || notice.tone === "info") return undefined;

    const timeout = window.setTimeout(() => {
      setNotice(null);
    }, autoDismissDelay);

    return () => window.clearTimeout(timeout);
  }, [notice, autoDismissDelay]);

  return {
    notice,
    showNotice,
    dismissNotice,
  };
}
