import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useCallback } from "react";
import { getTranslations } from "../stores/preferencesStore";
import { DashboardNotice } from "../types/dashboard";

/** Open links, folders, and terminals outside the app; failures become toasts. */
export function useDesktopActions(notify: (notice: DashboardNotice) => void) {
  const openExternal = useCallback(
    async (url: string) => {
      try {
        await openUrl(url);
      } catch (error) {
        notify({ tone: "error", title: getTranslations().commandFailed, message: String(error) });
      }
    },
    [notify]
  );

  const openFolder = useCallback(
    async (path?: string) => {
      if (!path) return;
      try {
        await invoke("open_folder", { path });
      } catch (error) {
        notify({
          tone: "error",
          title: getTranslations().openFolderFailed,
          message: String(error),
        });
      }
    },
    [notify]
  );

  const openTerminal = useCallback(
    async (path?: string) => {
      try {
        await invoke("open_project_terminal", { path: path || null });
      } catch (error) {
        notify({
          tone: "error",
          title: getTranslations().openTerminalFailed,
          message: String(error),
        });
      }
    },
    [notify]
  );

  return { openExternal, openFolder, openTerminal };
}
