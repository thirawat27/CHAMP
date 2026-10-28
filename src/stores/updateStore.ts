/**
 * App update state shared by the dashboard banner and the Settings panel.
 *
 * Updates come from GitHub Releases through the Tauri updater plugin, which
 * only installs packages signed with CHAMP's release key.
 */

import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, Update } from "@tauri-apps/plugin-updater";
import { create } from "zustand";
import { isTauriRuntime } from "../utils/tauri";

export type UpdatePhase = "idle" | "checking" | "available" | "current" | "installing" | "error";

const LAST_CHECK_KEY = "champ:last-update-check";
const AUTO_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;

interface UpdateState {
  phase: UpdatePhase;
  update: Update | null;
  /** Download percent while installing, when the size is known. */
  progress: number | null;
  error: string | null;
  checkedAt: number | null;
  checkNow: (options?: { silent?: boolean }) => Promise<void>;
  /** Check once a day at most; failures stay silent. */
  autoCheck: () => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

const readLastCheck = (): number | null => {
  try {
    const value = Number(localStorage.getItem(LAST_CHECK_KEY));
    return Number.isFinite(value) && value > 0 ? value : null;
  } catch {
    return null;
  }
};

export const useUpdateStore = create<UpdateState>((set, get) => ({
  phase: "idle",
  update: null,
  progress: null,
  error: null,
  checkedAt: readLastCheck(),

  checkNow: async ({ silent = false } = {}) => {
    if (!isTauriRuntime() || get().phase === "checking" || get().phase === "installing") return;
    set({ phase: "checking", error: null });
    try {
      const update = await check();
      const now = Date.now();
      try {
        localStorage.setItem(LAST_CHECK_KEY, String(now));
      } catch {
        // Storage can be unavailable; the next start simply checks again.
      }
      set({ update, phase: update ? "available" : "current", checkedAt: now });
    } catch (error) {
      set(silent ? { phase: "idle" } : { phase: "error", error: String(error) });
    }
  },

  autoCheck: async () => {
    const last = get().checkedAt;
    if (last && Date.now() - last < AUTO_CHECK_INTERVAL_MS) return;
    await get().checkNow({ silent: true });
  },

  install: async () => {
    const update = get().update;
    if (!update) return;
    set({ phase: "installing", progress: null, error: null });
    try {
      // The installer replaces the app, so stop every service first.
      await invoke("prepare_for_update");
      let total = 0;
      let downloaded = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (total > 0) set({ progress: Math.min(100, Math.round((downloaded / total) * 100)) });
        }
      });
      await relaunch();
    } catch (error) {
      set({ phase: "error", error: String(error), progress: null });
    }
  },

  dismiss: () => set({ phase: "idle" }),
}));
