import { Activity, HardDrive } from "lucide-react";
import pkg from "../../package.json";
import { ServiceMap } from "../types/services";

interface StatusBarProps {
  services: Partial<ServiceMap>;
  appPaths?: { base_dir: string };
  [key: string]: unknown;
}

export function StatusBar({ services, appPaths, ...props }: StatusBarProps) {
  const runningCount = Object.values(services).filter(
    (service) => service?.state === "running"
  ).length;
  const totalCount = Object.keys(services).length || 3;

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
      <span>CHAMP By Thirawat27</span>
      <span>CHAMP v{pkg.version}</span>
    </footer>
  );
}
