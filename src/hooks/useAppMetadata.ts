import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { AppPaths, AppSettings } from "../types/services";

/** Data folders, installed versions, and saved settings shown across the dashboard. */
export function useAppMetadata() {
  const [appPaths, setAppPaths] = useState<AppPaths | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [installedVersions, setInstalledVersions] = useState<Record<string, string>>({});

  const refreshMetadata = useCallback(async () => {
    try {
      const [paths, versions, loadedSettings] = await Promise.all([
        invoke<AppPaths>("get_app_paths"),
        invoke<Record<string, string>>("get_installed_versions"),
        invoke<AppSettings>("get_settings"),
      ]);
      setAppPaths(paths);
      setInstalledVersions(versions);
      setSettings(loadedSettings);
    } catch (error) {
      console.error("Failed to load app metadata:", error);
    }
  }, []);

  useEffect(() => {
    refreshMetadata();
  }, [refreshMetadata]);

  return { appPaths, settings, installedVersions, refreshMetadata };
}
