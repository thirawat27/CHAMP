import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { STATUS_POLL_INTERVAL_MS } from "../constants";
import { AddonChange, AddonInstallProgress, AddonKind, AddonStatus } from "../types/addons";
import { ServiceState } from "../types/services";
import { listenIfTauri } from "../utils/tauri";

export interface AddonsController {
  addons: AddonStatus[];
  loaded: boolean;
  /** The add-on a command is running for, if any. */
  busyKind: AddonKind | null;
  progress: Partial<Record<AddonKind, number>>;
  error: string | null;
  clearError: () => void;
  mailCount: number | null;
  refresh: () => Promise<void>;
  install: (kind: AddonKind) => Promise<boolean>;
  start: (kind: AddonKind) => Promise<boolean>;
  stop: (kind: AddonKind) => Promise<boolean>;
  configure: (kind: AddonKind, change: AddonChange) => Promise<boolean>;
}

const isTransitional = (state: ServiceState) =>
  state === ServiceState.Starting || state === ServiceState.Stopping;

/**
 * Status and actions for the optional services.
 *
 * Polls only while something is enabled, running, or installing, so a stack
 * without add-ons costs nothing.
 */
export function useAddons(): AddonsController {
  const [addons, setAddons] = useState<AddonStatus[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [busyKind, setBusyKind] = useState<AddonKind | null>(null);
  const [progress, setProgress] = useState<Partial<Record<AddonKind, number>>>({});
  const [error, setError] = useState<string | null>(null);
  const [mailCount, setMailCount] = useState<number | null>(null);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<AddonStatus[]>("get_addons_status");
      if (!mounted.current) return;
      // An older backend (or a mock) can answer with something else.
      if (!Array.isArray(next)) return;
      setAddons(next);
      const mailpitRunning = next.some(
        (addon) => addon.kind === "mailpit" && addon.state === ServiceState.Running
      );
      const count = mailpitRunning
        ? await invoke<number | null>("get_mailpit_message_count")
        : null;
      if (mounted.current) setMailCount(typeof count === "number" ? count : null);
    } catch (refreshError) {
      // Keep the last known list; a failed poll is not worth a banner.
      console.warn("Failed to load add-on status:", refreshError);
    } finally {
      if (mounted.current) setLoaded(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const active = addons.some(
    (addon) =>
      addon.enabled ||
      addon.installing ||
      addon.state === ServiceState.Running ||
      isTransitional(addon.state)
  );
  useEffect(() => {
    if (!active) return undefined;
    const timer = window.setInterval(() => void refresh(), STATUS_POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [active, refresh]);

  useEffect(() => {
    const unlistenProgress = listenIfTauri<AddonInstallProgress>(
      "addon-install-progress",
      (event) => {
        setProgress((current) => ({ ...current, [event.payload.kind]: event.payload.percent }));
      }
    );
    // Errors from add-ons started with the stack (Start all, auto-start).
    const unlistenErrors = listenIfTauri<string[]>("addon-errors", (event) => {
      setError(event.payload.join("\n"));
      void refresh();
    });
    return () => {
      unlistenProgress.then((dispose) => dispose());
      unlistenErrors.then((dispose) => dispose());
    };
  }, [refresh]);

  const run = useCallback(
    async (kind: AddonKind, action: () => Promise<AddonStatus[]>): Promise<boolean> => {
      setBusyKind(kind);
      setError(null);
      try {
        const next = await action();
        if (mounted.current && Array.isArray(next)) setAddons(next);
        return true;
      } catch (actionError) {
        if (mounted.current) setError(String(actionError));
        // The command may have changed state before failing.
        await refresh();
        return false;
      } finally {
        if (mounted.current) {
          setBusyKind(null);
          setProgress((current) => {
            const rest = { ...current };
            delete rest[kind];
            return rest;
          });
        }
      }
    },
    [refresh]
  );

  const install = useCallback(
    (kind: AddonKind) => run(kind, () => invoke<AddonStatus[]>("install_addon", { kind })),
    [run]
  );
  const start = useCallback(
    (kind: AddonKind) => run(kind, () => invoke<AddonStatus[]>("start_addon", { kind })),
    [run]
  );
  const stop = useCallback(
    (kind: AddonKind) => run(kind, () => invoke<AddonStatus[]>("stop_addon", { kind })),
    [run]
  );
  const configure = useCallback(
    (kind: AddonKind, change: AddonChange) =>
      run(kind, () => invoke<AddonStatus[]>("configure_addon", { kind, change })),
    [run]
  );

  const clearError = useCallback(() => setError(null), []);

  return {
    addons,
    loaded,
    busyKind,
    progress,
    error,
    clearError,
    mailCount,
    refresh,
    install,
    start,
    stop,
    configure,
  };
}
