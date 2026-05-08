import { invoke } from "@tauri-apps/api/core";
import { Activity, HardDrive } from "lucide-react";
import { useEffect, useState } from "react";
import { ServiceMap } from "../types/services";

interface StatusBarProps {
  services: Partial<ServiceMap>;
  appPaths?: { base_dir: string };
  [key: string]: unknown;
}

interface SystemMetrics {
  cpu_usage: number;
  memory_used_bytes: number;
  memory_total_bytes: number;
  network_rx_bps: number;
  network_tx_bps: number;
}

const METRICS_REFRESH_INTERVAL_MS = 1000;

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

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";

  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** exponent;
  const precision = exponent === 0 ? 0 : 1;

  return `${value.toFixed(precision)} ${units[exponent]}`;
}

export function StatusBar({ services, appPaths, ...props }: StatusBarProps) {
  const runningCount = Object.values(services).filter(
    (service) => service?.state === "running"
  ).length;
  const totalCount = Object.keys(services).length || 3;
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);

  useEffect(() => {
    let isActive = true;

    const refreshSystemMetrics = async () => {
      try {
        const response = await invoke<unknown>("get_system_metrics");
        if (isActive && isSystemMetrics(response)) {
          setMetrics(response);
        }
      } catch (error) {
        console.error("Failed to fetch system metrics:", error);
      }
    };

    refreshSystemMetrics();
    const interval = window.setInterval(refreshSystemMetrics, METRICS_REFRESH_INTERVAL_MS);

    return () => {
      isActive = false;
      window.clearInterval(interval);
    };
  }, []);

  const cpuText = metrics ? `${Math.max(metrics.cpu_usage, 0).toFixed(1)}%` : "--";
  const ramText = metrics
    ? `${formatBytes(metrics.memory_used_bytes)} / ${formatBytes(metrics.memory_total_bytes)}`
    : "--";
  const networkText = metrics
    ? `↓ ${formatBytes(metrics.network_rx_bps)}/s ↑ ${formatBytes(metrics.network_tx_bps)}/s`
    : "--";

  return (
    <footer className="status-bar" {...props}>
      <span>
        <Activity size={14} /> {runningCount}/{totalCount} running
      </span>
      {appPaths?.base_dir && (
        <span className="status-path">
          <HardDrive size={14} /> {appPaths.base_dir}
        </span>
      )}
      <span className="status-metrics">
        <span className="status-metric">
          <strong>CPU</strong> {cpuText}
        </span>
        <span className="status-metric">
          <strong>RAM</strong> {ramText}
        </span>
        <span className="status-metric">
          <strong>NET</strong> {networkText}
        </span>
      </span>
      <span>CHAMP By Thirawat27</span>
    </footer>
  );
}
