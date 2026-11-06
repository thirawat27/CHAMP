import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { Translations } from "../i18n/translations";
import { getTranslations } from "../stores/preferencesStore";
import { DashboardNotice } from "../types/dashboard";
import {
  DATABASE_ENGINE_NAMES,
  DATABASE_TOOL_NAMES,
  DatabaseEngine,
  DatabaseTool,
  DownloadProgress,
} from "../types/services";
import { listenIfTauri } from "../utils/tauri";

type TitleKey = {
  [K in keyof Translations]: Translations[K] extends string ? K : never;
}[keyof Translations];

interface SwitchCopy {
  pending: TitleKey;
  done: TitleKey;
  failed: TitleKey;
}

const ENGINE_COPY: SwitchCopy = {
  pending: "switchingDatabase",
  done: "databaseSwitched",
  failed: "failedToSwitchDatabase",
};

const TOOL_COPY: SwitchCopy = {
  pending: "switchingDatabaseTool",
  done: "databaseToolSwitched",
  failed: "failedToSwitchDatabaseTool",
};

/**
 * Switch the database engine or web manager. MySQL and PostgreSQL fill one
 * database role, and the manager is chosen separately; either installs on first
 * use. The backend refuses phpMyAdmin for PostgreSQL.
 */
export function useDatabaseSwitch(
  notify: (notice: DashboardNotice) => void,
  onSettled: () => void
) {
  const [switchingEngine, setSwitchingEngine] = useState<DatabaseEngine | null>(null);
  const [switchingTool, setSwitchingTool] = useState<DatabaseTool | null>(null);
  const [switchPercent, setSwitchPercent] = useState(0);

  const runSwitch = async (name: string, copy: SwitchCopy, run: () => Promise<unknown>) => {
    const t = getTranslations();
    setSwitchPercent(0);
    const unlisten = listenIfTauri<DownloadProgress>("download-progress", (event) => {
      setSwitchPercent(event.payload.percent);
    });
    notify({ tone: "info", title: t[copy.pending].replace("{name}", name), message: "" });
    try {
      await run();
      notify({ tone: "success", title: t[copy.done].replace("{name}", name), message: "" });
    } catch (error) {
      notify({
        tone: "error",
        title: t[copy.failed].replace("{name}", name),
        message: String(error),
      });
    } finally {
      unlisten.then((dispose) => dispose());
      onSettled();
    }
  };

  const switchEngine = async (engine: DatabaseEngine) => {
    setSwitchingEngine(engine);
    await runSwitch(DATABASE_ENGINE_NAMES[engine], ENGINE_COPY, () =>
      invoke("switch_database_engine", { engine })
    );
    setSwitchingEngine(null);
  };

  const switchTool = async (tool: DatabaseTool) => {
    setSwitchingTool(tool);
    await runSwitch(DATABASE_TOOL_NAMES[tool], TOOL_COPY, () =>
      invoke("switch_database_tool", { tool })
    );
    setSwitchingTool(null);
  };

  return { switchingEngine, switchingTool, switchPercent, switchEngine, switchTool };
}
