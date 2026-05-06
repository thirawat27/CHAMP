import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Database,
  Folder,
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
import { AppSettings, ServiceMap, ServiceState, ServiceType } from "../types/services";
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
const DEFAULT_DATABASE_TOOL_ID = "phpmyadmin-5.2";

function GitHubIcon({ size = 16 }: { size?: number }) {
  return (
    <svg
      aria-hidden="true"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="currentColor"
      focusable="false"
    >
      <path d="M12 .5C5.65.5.5 5.65.5 12c0 5.08 3.29 9.39 7.86 10.91.58.1.79-.25.79-.56v-2.14c-3.2.7-3.87-1.36-3.87-1.36-.52-1.33-1.28-1.68-1.28-1.68-1.05-.72.08-.7.08-.7 1.16.08 1.77 1.19 1.77 1.19 1.03 1.76 2.7 1.25 3.36.96.1-.75.4-1.25.73-1.54-2.55-.29-5.23-1.28-5.23-5.68 0-1.25.45-2.28 1.19-3.08-.12-.29-.52-1.46.11-3.04 0 0 .97-.31 3.17 1.18A11.1 11.1 0 0 1 12 6.07c.98 0 1.95.13 2.87.39 2.2-1.49 3.17-1.18 3.17-1.18.63 1.58.23 2.75.11 3.04.74.8 1.19 1.83 1.19 3.08 0 4.41-2.69 5.38-5.25 5.67.42.36.78 1.06.78 2.14v3.14c0 .31.21.67.79.56A11.51 11.51 0 0 0 23.5 12C23.5 5.65 18.35.5 12 .5Z" />
    </svg>
  );
}

export function Dashboard() {
  const [services, setServices] = useState<Partial<ServiceMap>>({});
  const [showSettings, setShowSettings] = useState(false);
  const [appPaths, setAppPaths] = useState<AppPaths | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [installedVersions, setInstalledVersions] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);

  const caddyPort = services[ServiceType.Caddy]?.port || 8080;
  const webServerUrl = `http://localhost:${caddyPort}`;
  const databaseToolId = settings?.package_selection?.phpmyadmin ?? DEFAULT_DATABASE_TOOL_ID;
  const isAdminerSelected = databaseToolId.startsWith("adminer");
  const databaseToolName = isAdminerSelected ? "Adminer" : "phpMyAdmin";
  const databaseToolUrl = `${webServerUrl}/${isAdminerSelected ? "adminer" : "phpmyadmin"}`;
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
      const [paths, versions, loadedSettings] = await Promise.all([
        invoke<AppPaths>("get_app_paths"),
        invoke<Record<string, string>>("get_installed_versions"),
        invoke<AppSettings>("get_settings"),
      ]);
      setAppPaths(paths);
      setInstalledVersions(versions);
      setSettings(loadedSettings);
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
      [databaseToolName, installedVersions.phpmyadmin || installedVersions.adminer],
    ];
    return entries.filter(
      (entry): entry is [string, string] => typeof entry[1] === "string" && entry[1].length > 0
    );
  }, [databaseToolName, installedVersions]);

  return (
    <div className="app-shell" data-testid="dashboard">
      <header className="titlebar">
        <div className="brand-mark" aria-hidden="true">
          <img className="brand-logo" src={champLogo} alt="" />
        </div>
        <div className="titlebar-copy">
          <h1>
            CHAMP <span>v1.1.0</span>
          </h1>
          <p>CHAMP By Thirawat27 · Caddy + HTTP(S) + phpMyAdmin/Adminer + MySQL + PHP</p>
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
            className="icon-button github"
            onClick={() => openUrl(SOURCE_REPO_URL)}
            title="Source repository"
            aria-label="Source repository"
          >
            <GitHubIcon size={18} />
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
              className="btn-quick-action action-site"
              onClick={() => openUrl(webServerUrl)}
              disabled={!isCaddyRunning}
            >
              <Globe size={16} /> Site
            </button>
            <button
              className="btn-quick-action action-database"
              onClick={() => openUrl(databaseToolUrl)}
              disabled={!isCaddyRunning}
            >
              <Database size={16} /> {databaseToolName}
            </button>
            <button className="btn-quick-action action-projects" onClick={() => openFolder(appPaths?.projects_dir)}>
              <Folder size={16} /> Projects
            </button>
            <button className="btn-quick-action action-logs" onClick={() => openFolder(appPaths?.logs_dir)}>
              <TerminalSquare size={16} /> Logs
            </button>
            <button className="btn-quick-action action-config" onClick={() => openFolder(appPaths?.config_dir)}>
              <HardDrive size={16} /> Config
            </button>
            <button className="btn-quick-action github" onClick={() => openUrl(SOURCE_REPO_URL)}>
              <GitHubIcon size={16} /> GitHub
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
