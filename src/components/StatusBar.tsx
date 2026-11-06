import { invoke } from "@tauri-apps/api/core";
import { Activity, ArrowDown, ArrowUp, Cpu, HardDrive, MemoryStick, Network } from "lucide-react";
import { HTMLAttributes, useEffect, useRef, useState } from "react";
import { METRICS_REFRESH_INTERVAL_MS } from "../constants";
import { useTranslation } from "../stores/preferencesStore";
import { formatBytes } from "../utils/format";

interface StatusBarProps extends HTMLAttributes<HTMLElement> {
  runningCount: number;
  totalCount: number;
  appPaths?: { base_dir: string; portable?: boolean };
}

interface SystemMetrics {
  cpu_usage: number;
  memory_used_bytes: number;
  memory_total_bytes: number;
  network_rx_bps: number;
  network_tx_bps: number;
}

function isSystemMetrics(value: unknown): value is SystemMetrics {
  if (!value || typeof value !== "object") return false;

  const metrics = value as Record<string, unknown>;
  return (
    typeof metrics.cpu_usage === "number" &&
    typeof metrics.memory_used_bytes === "number" &&
    typeof metrics.memory_total_bytes === "number" &&
    typeof metrics.network_rx_bps === "number" &&
    typeof metrics.network_tx_bps === "number"
  );
}

export function StatusBar({ runningCount, totalCount, appPaths, ...props }: StatusBarProps) {
  const { t } = useTranslation();
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);
  const metricsInFlight = useRef(false);

  useEffect(() => {
    let isActive = true;

    const refreshSystemMetrics = async () => {
      if (metricsInFlight.current || document.visibilityState !== "visible") return;
      metricsInFlight.current = true;
      try {
        const response = await invoke<unknown>("get_system_metrics");
        if (isActive && isSystemMetrics(response)) {
          setMetrics(response);
        }
      } catch (error) {
        console.error("Failed to fetch system metrics:", error);
      } finally {
        metricsInFlight.current = false;
      }
    };

    refreshSystemMetrics();
    const interval = window.setInterval(refreshSystemMetrics, METRICS_REFRESH_INTERVAL_MS);

    return () => {
      isActive = false;
      window.clearInterval(interval);
    };
  }, []);

  return (
    <footer className="status-bar" {...props}>
      <span>
        <Activity size={13} /> {runningCount}/{totalCount} {t.running}
      </span>
      {appPaths?.base_dir && (
        <span className="status-path" title={appPaths.base_dir}>
          <HardDrive size={13} /> {appPaths.portable ? t.dataLocationPortable : t.dataLocationLocal}
          : {appPaths.base_dir}
        </span>
      )}
      <span className="status-metrics">
        <span className="status-metric" title="CPU">
          <Cpu size={13} /> {metrics ? `${Math.max(metrics.cpu_usage, 0).toFixed(1)}%` : "--"}
        </span>
        <span className="status-metric" title="RAM">
          <MemoryStick size={13} />
          {metrics
            ? `${formatBytes(metrics.memory_used_bytes)} / ${formatBytes(metrics.memory_total_bytes)}`
            : "--"}
        </span>
        <span className="status-metric" title={t.network}>
          <Network size={13} />
          {metrics ? (
            <>
              <ArrowDown size={11} /> {formatBytes(metrics.network_rx_bps)}/s
              <ArrowUp size={11} /> {formatBytes(metrics.network_tx_bps)}/s
            </>
          ) : (
            "--"
          )}
        </span>
      </span>
      <span>CHAMP By Thirawat27</span>
    </footer>
  );
}
