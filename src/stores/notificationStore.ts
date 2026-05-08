/**
 * Notification Store - Zustand
 * 
 * Manages toast notifications using Zustand.
 * Provides auto-dismiss functionality for success and error notifications.
 */

import { create } from "zustand";

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

interface NotificationState {
  /** Current active notification */
  notice: DashboardNotice | null;
  /** Auto-dismiss timeout ID */
  dismissTimeout: number | null;
  
  // Actions
  /** Show a new notification */
  showNotice: (notice: DashboardNotice, autoDismissDelay?: number) => void;
  /** Dismiss the current notification */
  dismissNotice: () => void;
}

/**
 * Notification store for managing toast notifications
 */
export const useNotificationStore = create<NotificationState>((set, get) => ({
  notice: null,
  dismissTimeout: null,

  showNotice: (notice, autoDismissDelay = 4200) => {
    const { dismissTimeout } = get();
    
    // Clear existing timeout
    if (dismissTimeout !== null) {
      window.clearTimeout(dismissTimeout);
    }

    // Set new notice
    set({ notice });

    // Auto-dismiss for success and error
    if (notice.tone !== "info") {
      const timeout = window.setTimeout(() => {
        set({ notice: null, dismissTimeout: null });
      }, autoDismissDelay);
      set({ dismissTimeout: timeout });
    } else {
      set({ dismissTimeout: null });
    }
  },

  dismissNotice: () => {
    const { dismissTimeout } = get();
    
    // Clear timeout if exists
    if (dismissTimeout !== null) {
      window.clearTimeout(dismissTimeout);
    }

    set({ notice: null, dismissTimeout: null });
  },
}));
