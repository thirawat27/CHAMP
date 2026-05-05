import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Database,
  Folder,
  GitBranch,
  Globe,
  HardDrive,
  Play,
  RefreshCw,
  Settings,
  Square,
  TerminalSquare,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import champLogo from "../assets/CHAMP.png";
import { ServiceMap, ServiceState, ServiceType } from "../types/services";
import { ServiceCard } from "./ServiceCard";
import { SettingsPanel } from "./SettingsPanel";
import { StatusBar } from "./StatusBar";

interface AppPaths {
  base_dir: string;
  runtime_dir: string;
  config_dir: string;
  mysql_data_dir: string;
  logs_dir: string;
  projects_dir: string;
}

const SOURCE_REPO_URL = "https://github.com/thirawat27/CHAMP";

export function Dashboard() {
  const [services, setServices] = useState<Partial<ServiceMap>>({});
  const [showSettings, setShowSettings] = useState(false);
  const [appPaths, setAppPaths] = useState<AppPaths | null>(null);
  const [installedVersions, setInstalledVersions] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);

  const caddyPort = services[ServiceType.Caddy]?.port || 8080;
  const webServerUrl = `http://localhost:${caddyPort}`;
  const adminerUrl = `${webServerUrl}/adminer`;
  const runningCount = Object.values(services).filter(
    (service) => service?.state === ServiceState.Running
  ).length;
  const totalCount = Object.keys(services).length || 3;
  const isCaddyRunning = services[ServiceType.Caddy]?.state === ServiceState.Running;
  const allRunning = runningCount === totalCount;

  const refreshStatuses = useCallback(async () => {
    try {
      const statuses = await invoke<ServiceMap>("get_all_statuses");
      setServices(statuses);
    } catch (error) {
      console.error("Failed to get service statuses:", error);
    }
  }, []);

  const refreshMetadata = useCallback(async () => {
    try {
      const [paths, versions] = await Promise.all([
        invoke<AppPaths>("get_app_paths"),
        invoke<Record<string, string>>("get_installed_versions"),
      ]);
      setAppPaths(paths);
      setInstalledVersions(versions);
    } catch (error) {
      console.error("Failed to load app metadata:", error);
    }
  }, []);

  useEffect(() => {
    refreshStatuses();
    refreshMetadata();
    const interval = window.setInterval(refreshStatuses, 2000);
    return () => window.clearInterval(interval);
  }, [refreshMetadata, refreshStatuses]);

  const runStackCommand = async (
    command: "start_all_services" | "stop_all_services" | "restart_all_services"
  ) => {
    setBusy(command);
    try {
      const statuses = await invoke<ServiceMap>(command);
      setServices(statuses);
    } catch (error) {
      alert(`${command.replace(/_/g, " ")} failed:\n${error}`);
      await refreshStatuses();
    } finally {
      setBusy(null);
    }
  };

  const runServiceCommand = async (
    command: "start_service" | "stop_service" | "restart_service",
    service: ServiceType
  ) => {
    setBusy(`${command}:${service}`);
    try {
      const statuses = await invoke<ServiceMap>(command, { service });
      setServices(statuses);
    } catch (error) {
      alert(`Failed to ${command.split("_")[0]} ${service}:\n${error}`);
      await refreshStatuses();
    } finally {
      setBusy(null);
    }
  };

  const openFolder = async (path?: string) => {
    if (!path) return;
    try {
      await invoke("open_folder", { path });
    } catch (error) {
      alert(`Failed to open folder:\n${error}`);
    }
  };

  const versionBadges = useMemo(() => {
    const entries: Array<[string, unknown]> = [
      ["Caddy", installedVersions.caddy],
      ["PHP", installedVersions.php],
      ["MySQL", installedVersions.mysql],
      ["Adminer", installedVersions.adminer],
    ];
    return entries.filter(
      (entry): entry is [string, string] => typeof entry[1] === "string" && entry[1].length > 0
    );
  }, [installedVersions]);

  return (
    <div className="app-shell" data-testid="dashboard">
      <header className="titlebar">
        <div className="brand-mark" aria-hidden="true">
          <img className="brand-logo" src={champLogo} alt="" />
        </div>
        <div className="titlebar-copy">
          <h1>
            CHAMP <span>v1.0.0</span>
          </h1>
          <p>CHAMP By Thirawat27 · Caddy + HTTP(S) + Adminer + MySQL + PHP</p>
        </div>
        <div className="titlebar-actions">
          <button
            className="btn-command primary"
            onClick={() => runStackCommand("start_all_services")}
            disabled={Boolean(busy) || allRunning}
          >
            <Play size={16} /> Start Stack
          </button>
          <button
            className="btn-command"
            onClick={() => runStackCommand("restart_all_services")}
            disabled={Boolean(busy)}
          >
            <RefreshCw size={16} /> Restart
          </button>
          <button
            className="btn-command danger"
            onClick={() => runStackCommand("stop_all_services")}
            disabled={Boolean(busy) || runningCount === 0}
          >
            <Square size={15} /> Stop
          </button>
          <button
            className="icon-button"
            onClick={() => openUrl(SOURCE_REPO_URL)}
            title="Source repository"
            aria-label="Source repository"
          >
            <GitBranch size={18} />
          </button>
          <button
            className="icon-button"
            onClick={() => setShowSettings(true)}
            title="Settings"
            aria-label="Settings"
          >
            <Settings size={18} />
          </button>
        </div>
      </header>

      <main className="workspace">
        <section className="overview-band">
          <div>
            <span
              className={`stack-state ${allRunning ? "running" : runningCount > 0 ? "partial" : ""}`}
            >
              {allRunning ? "Running" : runningCount > 0 ? "Partial" : "Stopped"}
            </span>
            <h2>
              {runningCount}/{totalCount} services active
            </h2>
          </div>
          <div className="quick-actions">
            <button
              className="btn-quick-action"
              onClick={() => openUrl(webServerUrl)}
              disabled={!isCaddyRunning}
            >
              <Globe size={16} /> Site
            </button>
            <button
              className="btn-quick-action"
              onClick={() => openUrl(adminerUrl)}
              disabled={!isCaddyRunning}
            >
              <Database size={16} /> Adminer
            </button>
            <button className="btn-quick-action" onClick={() => openFolder(appPaths?.projects_dir)}>
              <Folder size={16} /> Projects
            </button>
            <button className="btn-quick-action" onClick={() => openFolder(appPaths?.logs_dir)}>
              <TerminalSquare size={16} /> Logs
            </button>
            <button className="btn-quick-action" onClick={() => openFolder(appPaths?.config_dir)}>
              <HardDrive size={16} /> Config
            </button>
            <button className="btn-quick-action accent" onClick={() => openUrl(SOURCE_REPO_URL)}>
              <GitBranch size={16} /> GitHub
            </button>
          </div>
        </section>

        {versionBadges.length > 0 && (
          <section className="version-strip" aria-label="Installed versions">
            {versionBadges.map(([name, version]) => (
              <span key={name}>
                {name} {version}
              </span>
            ))}
          </section>
        )}

        <section className="service-grid-responsive">
          {[ServiceType.Caddy, ServiceType.PhpFpm, ServiceType.MySQL].map((serviceType) => {
            const service = services[serviceType];
            if (!service) return null;
            return (
              <ServiceCard
                key={serviceType}
                serviceType={serviceType}
                state={service.state}
                port={service.port}
                error={service.error_message}
                busy={busy?.endsWith(serviceType)}
                onStart={() => runServiceCommand("start_service", serviceType)}
                onStop={() => runServiceCommand("stop_service", serviceType)}
                onRestart={() => runServiceCommand("restart_service", serviceType)}
              />
            );
          })}
        </section>
      </main>

      <StatusBar services={services} appPaths={appPaths || undefined} data-testid="status-bar" />

      {showSettings && (
        <SettingsPanel
          onClose={() => setShowSettings(false)}
          onSettingsChanged={() => {
            refreshStatuses();
            refreshMetadata();
          }}
        />
      )}
    </div>
  );
}
