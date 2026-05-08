import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  AlertTriangle,
  CheckCircle2,
  CircleHelp,
  Database,
  Folder,
  Globe,
  HardDrive,
  LoaderCircle,
  Play,
  RefreshCw,
  Settings,
  Square,
  TerminalSquare,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import champLogo from "../assets/CHAMP.png";
import {
  AppSettings,
  SERVICE_DISPLAY_NAMES,
  ServiceMap,
  ServiceState,
  ServiceType,
} from "../types/services";
import { HelpModal } from "./HelpModal";
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

type NoticeTone = "info" | "success" | "error";
type NoticeAction = "start" | "restart" | "stop";

interface DashboardNotice {
  tone: NoticeTone;
  action?: NoticeAction;
  title: string;
  message: string;
}

const STACK_COMMAND_COPY = {
  start_all_services: {
    pendingTitle: "Starting stack",
    pendingMessage: "Caddy, PHP-FPM, and MySQL are starting. This can take a few seconds.",
    successTitle: "Stack started",
    successMessage: "All stack commands finished. Statuses are refreshing now.",
    buttonLabel: "Starting...",
    action: "start",
  },
  restart_all_services: {
    pendingTitle: "Restarting stack",
    pendingMessage: "Services are stopping and starting again. The dashboard will update automatically.",
    successTitle: "Stack restarted",
    successMessage: "Restart command finished. Statuses are refreshing now.",
    buttonLabel: "Restarting...",
    action: "restart",
  },
  stop_all_services: {
    pendingTitle: "Stopping stack",
    pendingMessage: "Services are shutting down. This can take a moment.",
    successTitle: "Stack stopped",
    successMessage: "All services have received the stop command.",
    buttonLabel: "Stopping...",
    action: "stop",
  },
} as const;

const SERVICE_COMMAND_COPY = {
  start_service: {
    pendingTitle: "Starting service",
    pendingMessage: "The service is starting. Status will update automatically.",
    successTitle: "Service started",
    buttonLabel: "Starting...",
    action: "start",
  },
  restart_service: {
    pendingTitle: "Restarting service",
    pendingMessage: "The service is restarting. Status will update automatically.",
    successTitle: "Service restarted",
    buttonLabel: "Restarting...",
    action: "restart",
  },
  stop_service: {
    pendingTitle: "Stopping service",
    pendingMessage: "The service is stopping. Status will update automatically.",
    successTitle: "Service stopped",
    buttonLabel: "Stopping...",
    action: "stop",
  },
} as const;

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
  const [showHelp, setShowHelp] = useState(false);
  const [appPaths, setAppPaths] = useState<AppPaths | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [installedVersions, setInstalledVersions] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<DashboardNotice | null>(null);

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
  const busyStackCommand = busy?.startsWith("stack:")
    ? (busy.slice("stack:".length) as keyof typeof STACK_COMMAND_COPY)
    : null;
  const expectedPorts = useMemo(
    () => ({
      [ServiceType.Caddy]: settings?.web_port ?? 8080,
      [ServiceType.PhpFpm]: settings?.php_port ?? 9000,
      [ServiceType.MySQL]: settings?.mysql_port ?? 3306,
    }),
    [settings]
  );

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

  // Helper functions
  const markStackTransition = (
    command: "start_all_services" | "stop_all_services" | "restart_all_services"
  ) => {
    const transitionState =
      command === "stop_all_services" ? ServiceState.Stopping : ServiceState.Starting;

    setServices((current) => {
      const next = { ...current };
      for (const serviceType of [ServiceType.Caddy, ServiceType.PhpFpm, ServiceType.MySQL]) {
        const service = next[serviceType];
        if (!service) continue;
        next[serviceType] = {
          ...service,
          state: transitionState,
          error_message: undefined,
        };
      }
      return next;
    });
  };

  const markServiceTransition = (
    command: "start_service" | "stop_service" | "restart_service",
    service: ServiceType
  ) => {
    const transitionState =
      command === "stop_service" ? ServiceState.Stopping : ServiceState.Starting;

    setServices((current) => {
      const selected = current[service];
      if (!selected) return current;
      return {
        ...current,
        [service]: {
          ...selected,
          state: transitionState,
          error_message: undefined,
        },
      };
    });
  };

  const fallbackPortMessage = useMemo(() => {
    return (statuses: ServiceMap, fallbackMessage: string) => {
      const changedPorts = [ServiceType.Caddy, ServiceType.PhpFpm, ServiceType.MySQL]
        .map((serviceType) => {
          const service = statuses[serviceType];
          const expectedPort = expectedPorts[serviceType];
          if (!service || service.port === expectedPort) return null;
          return `${SERVICE_DISPLAY_NAMES[serviceType]} ${service.port}`;
        })
        .filter((value): value is string => Boolean(value));

      if (changedPorts.length === 0) {
        return fallbackMessage;
      }

      return `Using fallback ports: ${changedPorts.join(", ")}.`;
    };
  }, [expectedPorts]);

  const runStackCommand = useCallback(async (
    command: "start_all_services" | "stop_all_services" | "restart_all_services"
  ) => {
    const copy = STACK_COMMAND_COPY[command];
    setBusy(`stack:${command}`);
    setNotice({
      tone: "info",
      action: copy.action,
      title: copy.pendingTitle,
      message: copy.pendingMessage,
    });
    markStackTransition(command);
    try {
      const statuses = await invoke<ServiceMap>(command);
      setServices(statuses);
      setNotice({
        tone: "success",
        action: copy.action,
        title: copy.successTitle,
        message: fallbackPortMessage(statuses, copy.successMessage),
      });
    } catch (error) {
      setNotice({
        tone: "error",
        title: `${command.replace(/_/g, " ")} failed`,
        message: String(error),
      });
      await refreshStatuses();
    } finally {
      setBusy(null);
    }
  }, [refreshStatuses, fallbackPortMessage]);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Esc to dismiss toast (works in any keyboard layout)
      if (e.code === "Escape" && notice) {
        setNotice(null);
      }
      // ? to show help (Shift + Slash)
      if (e.key === "?" && !showSettings && !showHelp) {
        e.preventDefault();
        setShowHelp(true);
      }
      // Ctrl/Cmd + Comma to open settings (physical key position)
      if ((e.ctrlKey || e.metaKey) && e.code === "Comma") {
        e.preventDefault();
        setShowSettings((prev) => !prev);
      }
      // Ctrl/Cmd + R to restart stack (physical key position)
      if ((e.ctrlKey || e.metaKey) && e.code === "KeyR" && !busy) {
        e.preventDefault();
        runStackCommand("restart_all_services");
      }
      // Ctrl/Cmd + S to start stack
      if ((e.ctrlKey || e.metaKey) && e.code === "KeyS" && !busy && !allRunning) {
        e.preventDefault();
        runStackCommand("start_all_services");
      }
      // Ctrl/Cmd + X to stop stack
      if ((e.ctrlKey || e.metaKey) && e.code === "KeyX" && !busy && runningCount > 0) {
        e.preventDefault();
        runStackCommand("stop_all_services");
      }
      // Ctrl/Cmd + O to open projects folder
      if ((e.ctrlKey || e.metaKey) && e.code === "KeyO") {
        e.preventDefault();
        openFolder(appPaths?.projects_dir);
      }
      // Ctrl/Cmd + L to open logs folder
      if ((e.ctrlKey || e.metaKey) && e.code === "KeyL") {
        e.preventDefault();
        openFolder(appPaths?.logs_dir);
      }
      // Ctrl/Cmd + W to open website
      if ((e.ctrlKey || e.metaKey) && e.code === "KeyW" && isCaddyRunning) {
        e.preventDefault();
        openUrl(webServerUrl);
      }
      // Ctrl/Cmd + D to open database tool
      if ((e.ctrlKey || e.metaKey) && e.code === "KeyD" && isCaddyRunning) {
        e.preventDefault();
        openUrl(databaseToolUrl);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [notice, busy, runStackCommand, allRunning, runningCount, appPaths, isCaddyRunning, webServerUrl, databaseToolUrl, showSettings, showHelp]);

  useEffect(() => {
    if (!notice || notice.tone === "info") return undefined;

    const timeout = window.setTimeout(() => {
      setNotice(null);
    }, 4200);

    return () => window.clearTimeout(timeout);
  }, [notice]);

  const runServiceCommand = async (
    command: "start_service" | "stop_service" | "restart_service",
    service: ServiceType
  ) => {
    const copy = SERVICE_COMMAND_COPY[command];
    const displayName = SERVICE_DISPLAY_NAMES[service];
    setBusy(`${command}:${service}`);
    setNotice({
      tone: "info",
      action: copy.action,
      title: `${copy.pendingTitle}: ${displayName}`,
      message: copy.pendingMessage,
    });
    markServiceTransition(command, service);
    try {
      const statuses = await invoke<ServiceMap>(command, { service });
      setServices(statuses);
      setNotice({
        tone: "success",
        action: copy.action,
        title: `${copy.successTitle}: ${displayName}`,
        message: fallbackPortMessage(statuses, "The dashboard is refreshing service status."),
      });
    } catch (error) {
      setNotice({
        tone: "error",
        title: `Failed to ${command.split("_")[0]} ${displayName}`,
        message: String(error),
      });
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
      setNotice({
        tone: "error",
        title: "Failed to open folder",
        message: String(error),
      });
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
            CHAMP <span>v1.2.0</span>
          </h1>
          <p>CHAMP By Thirawat27</p>
        </div>
        <div className="titlebar-actions">
          <button
            className="btn-command primary"
            onClick={() => runStackCommand("start_all_services")}
            disabled={Boolean(busy) || allRunning}
            title="Start all services (Ctrl+S)"
          >
            {busyStackCommand === "start_all_services" ? (
              <LoaderCircle size={16} className="spin-icon" />
            ) : (
              <Play size={16} />
            )}
            {busyStackCommand === "start_all_services"
              ? STACK_COMMAND_COPY.start_all_services.buttonLabel
              : "Start Stack"}
          </button>
          <button
            className="btn-command"
            onClick={() => runStackCommand("restart_all_services")}
            disabled={Boolean(busy)}
            title="Restart all services (Ctrl+R)"
          >
            {busyStackCommand === "restart_all_services" ? (
              <LoaderCircle size={16} className="spin-icon" />
            ) : (
              <RefreshCw size={16} />
            )}
            {busyStackCommand === "restart_all_services"
              ? STACK_COMMAND_COPY.restart_all_services.buttonLabel
              : "Restart"}
          </button>
          <button
            className="btn-command danger"
            onClick={() => runStackCommand("stop_all_services")}
            disabled={Boolean(busy) || runningCount === 0}
            title="Stop all services (Ctrl+X)"
          >
            {busyStackCommand === "stop_all_services" ? (
              <LoaderCircle size={15} className="spin-icon" />
            ) : (
              <Square size={15} />
            )}
            {busyStackCommand === "stop_all_services"
              ? STACK_COMMAND_COPY.stop_all_services.buttonLabel
              : "Stop"}
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
            onClick={() => setShowHelp(true)}
            title="Keyboard shortcuts (?)"
            aria-label="Help"
          >
            <CircleHelp size={18} />
          </button>
          <button
            className="icon-button"
            onClick={() => setShowSettings(true)}
            title="Settings (Ctrl+,)"
            aria-label="Settings"
          >
            <Settings size={18} />
          </button>
        </div>
      </header>

      {notice && (
        <div
          className={`stack-notice ${notice.tone} ${notice.action ?? ""}`}
          role={notice.tone === "error" ? "alert" : "status"}
        >
          <span className="stack-notice-icon" aria-hidden="true">
            {notice.tone === "info" && <LoaderCircle size={18} className="spin-icon" />}
            {notice.tone === "success" && <CheckCircle2 size={18} />}
            {notice.tone === "error" && <AlertTriangle size={18} />}
          </span>
          <span>
            <strong>{notice.title}</strong>
            <small>{notice.message}</small>
          </span>
          <button className="notice-close" onClick={() => setNotice(null)} aria-label="Dismiss notification">
            ×
          </button>
        </div>
      )}

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
              title={`Open ${webServerUrl} (Ctrl+W)`}
            >
              <Globe size={16} /> Site
            </button>
            <button
              className="btn-quick-action action-database"
              onClick={() => openUrl(databaseToolUrl)}
              disabled={!isCaddyRunning}
              title={`Open ${databaseToolName} (Ctrl+D)`}
            >
              <Database size={16} /> {databaseToolName}
            </button>
            <button 
              className="btn-quick-action action-projects" 
              onClick={() => openFolder(appPaths?.projects_dir)}
              title="Open projects folder (Ctrl+O)"
            >
              <Folder size={16} /> Projects
            </button>
            <button 
              className="btn-quick-action action-logs" 
              onClick={() => openFolder(appPaths?.logs_dir)}
              title="Open logs folder (Ctrl+L)"
            >
              <TerminalSquare size={16} /> Logs
            </button>
            <button 
              className="btn-quick-action action-config" 
              onClick={() => openFolder(appPaths?.config_dir)}
              title="Open config folder"
            >
              <HardDrive size={16} /> Config
            </button>
            <button 
              className="btn-quick-action github" 
              onClick={() => openUrl(SOURCE_REPO_URL)}
              title="View source code on GitHub"
            >
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
            const busyServiceCommand = busy?.endsWith(serviceType)
              ? (busy.split(":")[0] as keyof typeof SERVICE_COMMAND_COPY)
              : null;
            return (
              <ServiceCard
                key={serviceType}
                serviceType={serviceType}
                state={service.state}
                port={service.port}
                error={service.error_message}
                busy={Boolean(busyServiceCommand)}
                busyLabel={busyServiceCommand ? SERVICE_COMMAND_COPY[busyServiceCommand].buttonLabel : undefined}
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

      {showHelp && <HelpModal onClose={() => setShowHelp(false)} />}
    </div>
  );
}
