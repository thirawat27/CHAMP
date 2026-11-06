import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { getTranslations } from "../stores/preferencesStore";
import { DashboardNotice } from "../types/dashboard";
import { InstalledPhpVersion } from "../types/services";

/** Offers and installs the newest patch release of the active PHP. */
export function usePhpUpdate(notify: (notice: DashboardNotice) => void, onInstalled: () => void) {
  const [phpUpdate, setPhpUpdate] = useState<InstalledPhpVersion | null>(null);
  const [phpUpdating, setPhpUpdating] = useState(false);

  const checkPhpUpdate = useCallback(async () => {
    try {
      const versions = await invoke<InstalledPhpVersion[]>("get_installed_php_versions");
      setPhpUpdate(versions.find((php) => php.active && php.update_available) ?? null);
    } catch (error) {
      console.error("Failed to check PHP updates:", error);
    }
  }, []);

  // The catalog is refreshed once per launch; the offer is re-checked whenever
  // Settings changes PHP.
  useEffect(() => {
    invoke("refresh_php_catalog")
      .catch(() => undefined)
      .then(checkPhpUpdate);
  }, [checkPhpUpdate]);

  const updatePhp = async () => {
    if (!phpUpdate) return;
    const t = getTranslations();
    setPhpUpdating(true);
    notify({ tone: "info", title: t.phpUpdating, message: phpUpdate.display_name });
    try {
      await invoke("download_php_version", { phpId: phpUpdate.id });
      notify({
        tone: "success",
        title: t.phpUpdated.replace("{version}", phpUpdate.latest_version),
        message: phpUpdate.display_name,
      });
      setPhpUpdate(null);
      onInstalled();
    } catch (error) {
      notify({ tone: "error", title: t.failedToInstallPhpVersion, message: String(error) });
    } finally {
      setPhpUpdating(false);
    }
  };

  return { phpUpdate, phpUpdating, updatePhp, checkPhpUpdate };
}
