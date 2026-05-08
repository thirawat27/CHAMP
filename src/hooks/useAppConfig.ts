/**
 * Custom hook for managing application configuration and metadata
 * 
 * This hook encapsulates all app-level configuration including:
 * - Application paths (runtime, config, logs, projects, etc.)
 * - User settings (ports, project root, auto-start, etc.)
 * - Installed component versions
 * 
 * @example
 * ```tsx
 * const { settings, appPaths, installedVersions, refreshMetadata } = useAppConfig();
 * ```
 */

import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { AppSettings } from "../types/services";

export interface AppPaths {
  base_dir: string;
  runtime_dir: string;
  config_dir: string;
  mysql_data_dir: string;
  logs_dir: string;
  projects_dir: string;
}

export interface UseAppConfigReturn {
  /** Application directory paths */
  appPaths: AppPaths | null;
  /** User settings (ports, project root, etc.) */
  settings: AppSettings | null;
  /** Installed component versions (caddy, php, mysql, phpmyadmin/adminer) */
  installedVersions: Record<string, string>;
  /** Refresh all metadata from backend */
  refreshMetadata: () => Promise<void>;
  /** Update settings */
  updateSettings: (newSettings: AppSettings) => void;
}

/**
 * Hook for managing application configuration and metadata
 * 
 * Loads app paths, settings, and installed versions on mount.
 * Provides methods for refreshing and updating configuration.
 * 
 * @param autoLoad - Whether to automatically load metadata on mount (default: true)
 * @returns Application configuration interface
 */
export function useAppConfig(autoLoad = true): UseAppConfigReturn {
  const [appPaths, setAppPaths] = useState<AppPaths | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [installedVersions, setInstalledVersions] = useState<Record<string, string>>({});

  /**
   * Fetch all application metadata from the backend
   * 
   * Loads:
   * - Application paths (runtime, config, logs, projects)
   * - Installed component versions
   * - User settings
   */
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

  /**
   * Update settings in local state
   * 
   * Note: This only updates the local state. To persist changes,
   * you need to call the backend save_settings command separately.
   * 
   * @param newSettings - The new settings to apply
   */
  const updateSettings = useCallback((newSettings: AppSettings) => {
    setSettings(newSettings);
  }, []);

  // Auto-load metadata on mount
  useEffect(() => {
    if (autoLoad) {
      refreshMetadata();
    }
  }, [autoLoad, refreshMetadata]);

  return {
    appPaths,
    settings,
    installedVersions,
    refreshMetadata,
    updateSettings,
  };
}
