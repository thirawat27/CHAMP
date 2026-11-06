import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { WIZARD_COMPONENT_COUNT } from "../constants";
import {
  DependencyCheckResult,
  DownloadProgress,
  PackageSelection,
  getDatabaseEngine,
  isAdminerSelected,
} from "../types/services";
import { listenIfTauri } from "../utils/tauri";

export type WizardStep =
  "welcome" | "packages" | "dependencies" | "confirm" | "download" | "complete";

export interface ExistingComponent {
  name: string;
  version: string;
  displayName: string;
  isExisting: boolean;
}

/** The four parts of the selected stack, with what is already on disk. */
function selectedComponents(
  selection: PackageSelection,
  existing: Record<string, string>
): ExistingComponent[] {
  const component = (name: string, displayName: string): ExistingComponent => ({
    name,
    displayName,
    version: existing[name] || "",
    isExisting: !!existing[name],
  });

  return [
    component("caddy", "Caddy"),
    component("php", "PHP"),
    getDatabaseEngine(selection) === "postgresql"
      ? component("postgresql", "PostgreSQL")
      : component("mysql", "MySQL"),
    isAdminerSelected(selection)
      ? component("adminer", "Adminer")
      : component("phpmyadmin", "phpMyAdmin"),
  ];
}

/** Dependency check, existing-component review, and the runtime download. */
export function useRuntimeInstall(
  packageSelection: PackageSelection,
  setStep: (step: WizardStep | ((current: WizardStep) => WizardStep)) => void
) {
  const [progress, setProgress] = useState<DownloadProgress>({
    step: "downloading",
    percent: 0,
    currentComponent: "",
    componentDisplay: "",
    version: "",
    totalComponents: WIZARD_COMPONENT_COUNT,
    downloadedBytes: 0,
    totalBytes: 0,
  });
  const [error, setError] = useState<string | null>(null);
  const [existingComponents, setExistingComponents] = useState<ExistingComponent[]>([]);
  const [dependencyCheckResult, setDependencyCheckResult] = useState<DependencyCheckResult | null>(
    null
  );

  useEffect(() => {
    const unlisten = listenIfTauri<DownloadProgress>("download-progress", (event) => {
      setProgress(event.payload);
      // Only this wizard's own install may finish the wizard.
      if (event.payload.step === "complete") {
        setStep((current) => (current === "download" ? "complete" : current));
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setStep]);

  const proceedWithDownload = async (skipList: string[]) => {
    setError(null);
    setStep("download");

    try {
      // Save the package selection to settings
      await invoke("update_package_selection", { packageSelection });

      if (skipList.length > 0) {
        await invoke<string>("download_runtime_with_skip", { packageSelection, skipList });
      } else {
        await invoke<string>("download_runtime_with_packages", { packageSelection });
      }
      // The command succeeding is the source of truth; the event may have been missed.
      setStep("complete");
    } catch (err) {
      console.error("Download error:", err);
      setError(String(err));
      setStep("confirm");
    }
  };

  const startDownload = async () => {
    setError(null);

    // First, check system dependencies
    try {
      const depsResult = await invoke<DependencyCheckResult>("check_system_dependencies");
      setDependencyCheckResult(depsResult);

      if (!depsResult.all_satisfied) {
        setStep("dependencies");
        return;
      }
    } catch (err) {
      console.error("Failed to check dependencies:", err);
      // Continue anyway if dependency check fails
    }

    // Then, check for existing components
    try {
      const existing = await invoke<Record<string, string>>("check_existing_components");
      setExistingComponents(selectedComponents(packageSelection, existing));
      setStep("confirm");
      return;
    } catch (err) {
      console.error("Failed to check existing components:", err);
    }

    // No existing components, proceed with download
    proceedWithDownload([]);
  };

  const installAll = () => proceedWithDownload([]);

  const keepExistingAndInstall = () =>
    proceedWithDownload(existingComponents.filter((c) => c.isExisting).map((c) => c.name));

  const cancelReview = () => {
    setStep("packages");
    setExistingComponents([]);
  };

  return {
    progress,
    error,
    clearError: () => setError(null),
    existingComponents,
    dependencyCheckResult,
    startDownload,
    installAll,
    keepExistingAndInstall,
    cancelReview,
  };
}
