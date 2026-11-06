import { useEffect } from "react";
import { AppPaths } from "../types/services";
import {
  SHORTCUTS,
  ShortcutId,
  hasPrimaryModifier,
  isEditableTarget,
  isHelpKey,
} from "../utils/shortcuts";
import { StackCommand } from "./commandCopy";

interface DashboardShortcutOptions {
  /** A command, update, or engine switch is running */
  operationBusy: boolean;
  anyModalOpen: boolean;
  allRunning: boolean;
  runningCount: number;
  isCaddyRunning: boolean;
  appPaths: AppPaths | null;
  webServerUrl: string;
  databaseToolUrl: string;
  runStackCommand: (command: StackCommand) => void;
  openExternal: (url: string) => void;
  openFolder: (path?: string) => void;
  openTerminal: (path?: string) => void;
  openGuide: () => void;
  openHelp: () => void;
  toggleSettings: () => void;
}

const SHORTCUT_IDS = Object.keys(SHORTCUTS) as ShortcutId[];

/** The documented keyboard shortcuts of the dashboard. */
export function useDashboardShortcuts({
  operationBusy,
  anyModalOpen,
  allRunning,
  runningCount,
  isCaddyRunning,
  appPaths,
  webServerUrl,
  databaseToolUrl,
  runStackCommand,
  openExternal,
  openFolder,
  openTerminal,
  openGuide,
  openHelp,
  toggleSettings,
}: DashboardShortcutOptions) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const editing = isEditableTarget(e.target);

      // F1 and Ctrl/Cmd + Comma work anywhere, including while typing.
      if (e.key === "F1") {
        e.preventDefault();
        openGuide();
        return;
      }
      if (hasPrimaryModifier(e) && e.code === SHORTCUTS.settings.code) {
        e.preventDefault();
        toggleSettings();
        return;
      }

      // ? to show help; a text field needs the character itself.
      if (isHelpKey(e)) {
        if (!editing && !anyModalOpen) {
          e.preventDefault();
          openHelp();
        }
        return;
      }

      if (!hasPrimaryModifier(e)) return;

      // Ctrl/Cmd + X in a text field is Cut.
      if (editing && e.code === SHORTCUTS.stop.code) return;

      const id = SHORTCUT_IDS.find((shortcut) => SHORTCUTS[shortcut].code === e.code);
      if (!id) return;

      // Claim the key before checking app state, so a disabled action never
      // falls through to the webview's own Reload or Save.
      e.preventDefault();
      if (editing || anyModalOpen || e.repeat) return;

      switch (id) {
        case "restart":
          if (!operationBusy) runStackCommand("restart_all_services");
          break;
        case "start":
          if (!operationBusy && !allRunning) runStackCommand("start_all_services");
          break;
        case "stop":
          if (!operationBusy && runningCount > 0) runStackCommand("stop_all_services");
          break;
        case "projects":
          openFolder(appPaths?.projects_dir);
          break;
        case "terminal":
          openTerminal(appPaths?.projects_dir);
          break;
        case "logs":
          openFolder(appPaths?.logs_dir);
          break;
        case "website":
          if (isCaddyRunning) openExternal(webServerUrl);
          break;
        case "database":
          if (isCaddyRunning) openExternal(databaseToolUrl);
          break;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    operationBusy,
    anyModalOpen,
    allRunning,
    runningCount,
    isCaddyRunning,
    appPaths,
    webServerUrl,
    databaseToolUrl,
    runStackCommand,
    openExternal,
    openFolder,
    openTerminal,
    openGuide,
    openHelp,
    toggleSettings,
  ]);
}
