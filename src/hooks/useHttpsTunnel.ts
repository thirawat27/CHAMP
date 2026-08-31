import { invoke } from "@tauri-apps/api/core";
import { useCallback, useState } from "react";
import { Translations } from "../i18n/translations";
import { AudioManager } from "../utils/audioManager";
import {
  DashboardNotice,
  DEFAULT_TUNNEL_STATUS,
  normalizeTunnelStatus,
} from "../components/dashboard/types";
import { HttpsTunnelStatus } from "../types/services";

interface UseHttpsTunnelOptions {
  t: Translations;
  notify: (notice: DashboardNotice) => void;
  /** Refreshes service statuses after a tunnel start (matches original flow). */
  refreshStatuses: () => Promise<void>;
  /** Refreshes app metadata after a tunnel start (matches original flow). */
  refreshMetadata: () => Promise<void>;
}

export interface HttpsTunnelController {
  tunnelStatus: HttpsTunnelStatus;
  tunnelBusy: "start" | "stop" | null;
  tunnelReady: boolean;
  tunnelHasPendingUrl: boolean;
  refreshTunnelStatus: () => Promise<void>;
  startHttpsTunnel: () => Promise<void>;
  stopHttpsTunnel: () => Promise<void>;
  copyHttpsTunnelUrl: () => Promise<void>;
  resetTunnel: () => void;
}

/**
 * Owns the Cloudflare Quick Tunnel (HTTPS Preview) state and lifecycle.
 *
 * Notices are pushed through the shared `notify` callback so only one toast is
 * visible at a time. `resetTunnel` is passed to the services hook so a
 * successful stop-all clears the tunnel panel back to its not-running state.
 */
export function useHttpsTunnel({
  t,
  notify,
  refreshStatuses,
  refreshMetadata,
}: UseHttpsTunnelOptions): HttpsTunnelController {
  const [tunnelStatus, setTunnelStatus] = useState<HttpsTunnelStatus>(DEFAULT_TUNNEL_STATUS);
  const [tunnelBusy, setTunnelBusy] = useState<"start" | "stop" | null>(null);

  const refreshTunnelStatus = useCallback(async () => {
    try {
      const status = await invoke<HttpsTunnelStatus>("get_https_tunnel_status");
      setTunnelStatus(normalizeTunnelStatus(status));
    } catch (error) {
      console.error("Failed to get HTTPS tunnel status:", error);
    }
  }, []);

  const resetTunnel = useCallback(() => {
    setTunnelStatus(DEFAULT_TUNNEL_STATUS);
  }, []);

  const startHttpsTunnel = useCallback(async () => {
    setTunnelBusy("start");
    notify({
      tone: "info",
      action: "start",
      title: t.httpsTunnelStarting,
      message: t.httpsTunnelDescription,
    });

    try {
      const status = normalizeTunnelStatus(await invoke<HttpsTunnelStatus>("start_https_tunnel"));
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
  }, [notify, refreshMetadata, refreshStatuses, refreshTunnelStatus, t]);

  const stopHttpsTunnel = useCallback(async () => {
    setTunnelBusy("stop");
    try {
      const status = normalizeTunnelStatus(await invoke<HttpsTunnelStatus>("stop_https_tunnel"));
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
  }, [notify, t]);

  const copyHttpsTunnelUrl = useCallback(async () => {
    if (!tunnelStatus.url) return;
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
  }, [notify, t, tunnelStatus.url]);

  const tunnelReady = Boolean(tunnelStatus.running && tunnelStatus.ready && tunnelStatus.url);
  const tunnelHasPendingUrl = Boolean(
    tunnelStatus.running && tunnelStatus.url && !tunnelReady
  );

  return {
    tunnelStatus,
    tunnelBusy,
    tunnelReady,
    tunnelHasPendingUrl,
    refreshTunnelStatus,
    startHttpsTunnel,
    stopHttpsTunnel,
    copyHttpsTunnelUrl,
    resetTunnel,
  };
}
