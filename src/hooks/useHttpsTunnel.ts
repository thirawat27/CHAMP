import { invoke } from "@tauri-apps/api/core";
import { useCallback, useRef, useState } from "react";
import { getTranslations } from "../stores/preferencesStore";
import { AudioManager } from "../utils/audioManager";
import { DashboardNotice, DEFAULT_TUNNEL_STATUS, normalizeTunnelStatus } from "../types/dashboard";
import { HttpsTunnelStatus } from "../types/services";

interface UseHttpsTunnelOptions {
  notify: (notice: DashboardNotice) => void;
  /** Refreshes service statuses after a tunnel start. */
  refreshStatuses: () => Promise<void>;
  /** Refreshes app metadata after a tunnel start: ngrok may have been installed. */
  refreshMetadata: () => Promise<void>;
}

export interface HttpsTunnelController {
  tunnelStatus: HttpsTunnelStatus;
  /** The first status query finished, so `authtoken_configured` is trustworthy. */
  tunnelStatusLoaded: boolean;
  tunnelBusy: "start" | "stop" | null;
  tunnelReady: boolean;
  tunnelHasPendingUrl: boolean;
  refreshTunnelStatus: () => Promise<void>;
  startHttpsTunnel: () => Promise<void>;
  stopHttpsTunnel: () => Promise<void>;
  copyHttpsTunnelUrl: () => Promise<void>;
  /** Saves the ngrok authtoken (empty clears it); resolves true on success. */
  saveNgrokAuthtoken: (token: string) => Promise<boolean>;
  resetTunnel: () => void;
}

function sameTunnelStatus(a: HttpsTunnelStatus, b: HttpsTunnelStatus): boolean {
  return (
    a.running === b.running &&
    a.url === b.url &&
    a.ready === b.ready &&
    a.local_url === b.local_url &&
    a.error === b.error &&
    a.log_path === b.log_path &&
    a.pid === b.pid &&
    a.authtoken_configured === b.authtoken_configured
  );
}

/**
 * Owns the ngrok tunnel (HTTPS Preview) state and lifecycle.
 *
 * Notices are pushed through the shared `notify` callback so only one toast is
 * visible at a time. `resetTunnel` clears the panel after a stack stop.
 */
export function useHttpsTunnel({
  notify,
  refreshStatuses,
  refreshMetadata,
}: UseHttpsTunnelOptions): HttpsTunnelController {
  const [tunnelStatus, setTunnelStatus] = useState<HttpsTunnelStatus>(DEFAULT_TUNNEL_STATUS);
  const [tunnelStatusLoaded, setTunnelStatusLoaded] = useState(false);
  const [tunnelBusy, setTunnelBusy] = useState<"start" | "stop" | null>(null);
  // A poll that started before a stop or reset must not bring the old URL back.
  const statusGeneration = useRef(0);

  const refreshTunnelStatus = useCallback(async () => {
    const generation = statusGeneration.current;
    try {
      const status = normalizeTunnelStatus(
        await invoke<HttpsTunnelStatus>("get_https_tunnel_status")
      );
      if (generation === statusGeneration.current) {
        setTunnelStatus((current) => (sameTunnelStatus(current, status) ? current : status));
      }
    } catch (error) {
      console.error("Failed to get HTTPS tunnel status:", error);
    } finally {
      setTunnelStatusLoaded(true);
    }
  }, []);

  const resetTunnel = useCallback(() => {
    statusGeneration.current += 1;
    // Stopping the tunnel does not forget the saved authtoken.
    setTunnelStatus((current) => ({
      ...DEFAULT_TUNNEL_STATUS,
      authtoken_configured: current.authtoken_configured,
    }));
  }, []);

  const startHttpsTunnel = useCallback(async () => {
    statusGeneration.current += 1;
    const t = getTranslations();
    setTunnelBusy("start");
    notify({
      tone: "info",
      action: "start",
      title: t.httpsTunnelStarting,
      message: t.httpsTunnelDescription,
    });

    try {
      const status = normalizeTunnelStatus(await invoke<HttpsTunnelStatus>("start_https_tunnel"));
      statusGeneration.current += 1;
      setTunnelStatus(status);
      await Promise.all([refreshStatuses(), refreshMetadata()]);
      AudioManager.playNotification("success", "start");
      notify({
        tone: "success",
        action: "start",
        title: t.httpsTunnelStarted,
        message: status.ready && status.url ? status.url : t.httpsTunnelValidating,
      });
    } catch (error) {
      AudioManager.playNotification("error");
      notify({
        tone: "error",
        title: t.httpsTunnelError,
        message: String(error),
      });
      await refreshTunnelStatus();
    } finally {
      setTunnelBusy(null);
    }
  }, [notify, refreshMetadata, refreshStatuses, refreshTunnelStatus]);

  const stopHttpsTunnel = useCallback(async () => {
    statusGeneration.current += 1;
    const t = getTranslations();
    setTunnelBusy("stop");
    try {
      const status = normalizeTunnelStatus(await invoke<HttpsTunnelStatus>("stop_https_tunnel"));
      statusGeneration.current += 1;
      setTunnelStatus(status);
      AudioManager.playNotification("success", "stop");
      notify({
        tone: "success",
        action: "stop",
        title: t.httpsTunnelStopped,
        message: t.httpsTunnelDevOnly,
      });
    } catch (error) {
      AudioManager.playNotification("error");
      notify({
        tone: "error",
        title: t.httpsTunnelError,
        message: String(error),
      });
    } finally {
      setTunnelBusy(null);
    }
  }, [notify]);

  const copyHttpsTunnelUrl = useCallback(async () => {
    if (!tunnelStatus.url) return;
    const t = getTranslations();
    try {
      await navigator.clipboard.writeText(tunnelStatus.url);
      notify({
        tone: "success",
        title: t.copiedToClipboard,
        message: tunnelStatus.url,
      });
    } catch (error) {
      notify({
        tone: "error",
        title: t.copyFailed,
        message: String(error),
      });
    }
  }, [notify, tunnelStatus.url]);

  const saveNgrokAuthtoken = useCallback(
    async (token: string) => {
      const t = getTranslations();
      try {
        const status = normalizeTunnelStatus(
          await invoke<HttpsTunnelStatus>("set_ngrok_authtoken", { token })
        );
        setTunnelStatus(status);
        notify({
          tone: "success",
          title: t.httpsTunnelTokenSaved,
          message: t.httpsTunnelDevOnly,
        });
        return true;
      } catch (error) {
        notify({
          tone: "error",
          title: t.httpsTunnelError,
          message: String(error),
        });
        return false;
      }
    },
    [notify]
  );

  const tunnelReady = Boolean(tunnelStatus.running && tunnelStatus.ready && tunnelStatus.url);
  const tunnelHasPendingUrl = Boolean(tunnelStatus.running && tunnelStatus.url && !tunnelReady);

  return {
    tunnelStatus,
    tunnelStatusLoaded,
    tunnelBusy,
    tunnelReady,
    tunnelHasPendingUrl,
    refreshTunnelStatus,
    startHttpsTunnel,
    stopHttpsTunnel,
    copyHttpsTunnelUrl,
    saveNgrokAuthtoken,
    resetTunnel,
  };
}
