/**
 * Configuration Store - Zustand
 * 
 * Manages application configuration and metadata using Zustand.
 * Handles app paths, settings, and installed versions.
 */

import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { AppSettings } from "../types/services";

export interface AppPaths {
  base_dir: string;
  runtime_dir: string;
  config_dir: string;
  mysql_data_dir: string;
  logs_dir: string;
  projects_dir: string;
}

interface ConfigState {
  /** Application directory paths */
  appPaths: AppPaths | null;
  /** User settings */
  settings: AppSettings | null;
  /** Installed component versions */
  installedVersions: Record<string, string>;
  
  // Actions
  /** Set app paths */
  setAppPaths: (paths: AppPaths | null) => void;
  /** Set settings */
  setSettings: (settings: AppSettings | null) => void;
  /** Set installed versions */
  setInstalledVersions: (versions: Record<string, string>) => void;
  /** Refresh all metadata from backend */
  refreshMetadata: () => Promise<void>;
}

/**
 * Configuration store for managing app config and metadata
 */
export const useConfigStore = create<ConfigState>((set) => ({
  appPaths: null,
  settings: null,
  installedVersions: {},

  setAppPaths: (appPaths) => set({ appPaths }),

  setSettings: (settings) => set({ settings }),

  setInstalledVersions: (installedVersions) => set({ installedVersions }),

  refreshMetadata: async () => {
    try {
      const [paths, versions, loadedSettings] = await Promise.all([
        invoke<AppPaths>("get_app_paths"),
        invoke<Record<string, string>>("get_installed_versions"),
        invoke<AppSettings>("get_settings"),
      ]);
      set({
        appPaths: paths,
        installedVersions: versions,
        settings: loadedSettings,
      });
    } catch (error) {
      console.error("Failed to load app metadata:", error);
    }
  },
}));
