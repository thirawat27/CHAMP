import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  AlertTriangle,
  ArrowUpCircle,
  BookOpen,
  CheckCircle2,
  Copy,
  Database,
  ExternalLink,
  Folder,
  FileCode2,
  FolderCog,
  Globe,
  Globe2,
  HardDrive,
  Keyboard,
  LoaderCircle,
  Monitor,
  Package,
  Play,
  Route,
  RotateCw,
  ScrollText,
  Settings,
  ShieldCheck,
  Square,
  Stethoscope,
  TerminalSquare,
  X,
  Zap,
} from "lucide-react";
import { ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import packageInfo from "../../package.json";
import champLogo from "../assets/champ-logo.png";
import { useTranslation } from "../stores/languageStore";
import { useUpdateStore } from "../stores/updateStore";
import { AudioManager } from "../utils/audioManager";
import { listenIfTauri } from "../utils/tauri";
import {
  AgentActivity,
  AppSettings,
  DEFAULT_PORTS,
  DATABASE_ENGINE_NAMES,
  DATABASE_ENGINE_SERVICES,
  DATABASE_TOOL_NAMES,
  DatabaseEngine,
  DatabaseTool,
  DownloadProgress,
  InstalledPhpVersion,
  SERVICE_DISPLAY_NAMES,
  ServiceState,
  ServiceType,
  getDatabaseEngine,
  getDatabaseTool,
} from "../types/services";
import { useServices } from "../hooks/useServices";
import { useHttpsTunnel } from "../hooks/useHttpsTunnel";
import { useToast } from "../hooks/useToast";
import { AppPaths, SERVICE_COMMAND_COPY, SOURCE_REPO_URL, ServiceCommand } from "./dashboard/types";
import { GitHubIcon } from "./dashboard/GitHubIcon";
import { GuideModal } from "./GuideModal";
import { HelpModal } from "./HelpModal";
import { LanguageSelector } from "./LanguageSelector";
import { LogDoctorModal } from "./LogDoctorModal";
import { EngineSwitch, ServiceCard, ToolSwitch } from "./ServiceCard";
import { SettingsPanel } from "./SettingsPanel";
import { StatusBar } from "./StatusBar";

const DASHBOARD_SERVICE_TYPES = [
  ServiceType.Caddy,
  ServiceType.PhpFpm,
  ServiceType.MySQL,
  ServiceType.PostgreSQL,
] as const;

const isMac = typeof navigator !== "undefined" && /mac/i.test(navigator.userAgent);
const MOD_KEY = isMac ? "⌘" : "Ctrl";

function PathIcon({ serviceType }: { serviceType: ServiceType }) {
  if (serviceType === ServiceType.Caddy) return <Globe2 size={14} />;
  if (serviceType === ServiceType.PhpFpm) return <FileCode2 size={14} />;
  return <Database size={14} />;
}

interface RailAction {
  id: string;
  icon: ReactNode;
  label: string;
  shortcut?: string;
  onSelect: () => void;
}

export function Dashboard() {
  const { t } = useTranslation();
  const [showSettings, setShowSettings] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  const [showGuide, setShowGuide] = useState(false);
  const [appPaths, setAppPaths] = useState<AppPaths | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [installedVersions, setInstalledVersions] = useState<Record<string, string>>({});
  const [doctorService, setDoctorService] = useState<ServiceType | null>(null);
  const [phpUpdate, setPhpUpdate] = useState<InstalledPhpVersion | null>(null);
  const [phpUpdating, setPhpUpdating] = useState(false);
  const [switchingEngine, setSwitchingEngine] = useState<DatabaseEngine | null>(null);
  const [switchingTool, setSwitchingTool] = useState<DatabaseTool | null>(null);
  const [switchPercent, setSwitchPercent] = useState(0);

  const { notice, notify, dismiss } = useToast();
  const updatePhase = useUpdateStore((state) => state.phase);
  const availableUpdate = useUpdateStore((state) => state.update);
  const updateProgress = useUpdateStore((state) => state.progress);
  const installUpdate = useUpdateStore((state) => state.install);
  const dismissUpdate = useUpdateStore((state) => state.dismiss);
  const autoCheckUpdates = useUpdateStore((state) => state.autoCheck);

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

  // The services and tunnel hooks reference each other's refresh callbacks.
  // A ref breaks the initialization cycle: the tunnel hook reads the latest
  // services refresh without depending on the services hook being built first.
  const refreshStatusesRef = useRef<() => Promise<void>>(() => Promise.resolve());
  const refreshStatusesStable = useCallback(() => refreshStatusesRef.current(), []);

  const tunnel = useHttpsTunnel({
    t,
    notify,
    refreshStatuses: refreshStatusesStable,
    refreshMetadata,
  });

  const services = useServices({
    t,
    settings,
    notify,
    refreshTunnel: tunnel.refreshTunnelStatus,
    resetTunnel: tunnel.resetTunnel,
  });

  useEffect(() => {
    refreshStatusesRef.current = services.refreshStatuses;
  }, [services.refreshStatuses]);

  const {
    services: serviceMap,
    busy,
    busyStackCommand,
    runningCount,
    totalCount,
    allRunning,
    isCaddyRunning,
    stackServiceTypes,
    packageSelection,
    refreshStatuses,
    runStackCommand,
    runServiceCommand,
  } = services;

  const {
    tunnelStatus,
    tunnelBusy,
    tunnelReady,
    tunnelHasPendingUrl,
    startHttpsTunnel,
    stopHttpsTunnel,
    copyHttpsTunnelUrl,
    saveNgrokAuthtoken,
  } = tunnel;
  const [ngrokTokenDraft, setNgrokTokenDraft] = useState("");
  const [editingNgrokToken, setEditingNgrokToken] = useState(false);
  const showNgrokTokenForm =
    !tunnelStatus.running && (!tunnelStatus.authtoken_configured || editingNgrokToken);
  const submitNgrokToken = async () => {
    if (await saveNgrokAuthtoken(ngrokTokenDraft)) {
      setNgrokTokenDraft("");
      setEditingNgrokToken(false);
    }
  };

  // Initialize audio context on first user interaction
  useEffect(() => {
    const initAudio = () => {
      AudioManager.initialize();
    };
    window.addEventListener("click", initAudio, { once: true });
    return () => window.removeEventListener("click", initAudio);
  }, []);

  useEffect(() => {
    refreshMetadata();
  }, [refreshMetadata]);

  // Offer the newest patch release of the active PHP (checked once per launch).
  useEffect(() => {
    invoke("refresh_php_catalog")
      .catch(() => undefined)
      .then(() => invoke<InstalledPhpVersion[]>("get_installed_php_versions"))
      .then((versions) =>
        setPhpUpdate(versions.find((php) => php.active && php.update_available) ?? null)
      )
      .catch((error) => console.error("Failed to check PHP updates:", error));
  }, []);

  const updatePhp = async () => {
    if (!phpUpdate) return;
    setPhpUpdating(true);
    notify({ tone: "info", title: t.phpUpdating, message: phpUpdate.display_name });
    try {
      await invoke("download_php_version", { phpId: phpUpdate.id });
      notify({
        tone: "success",
        title: t.phpUpdated.replace("{version}", phpUpdate.latest_version),
        message: phpUpdate.display_name,
      });
      setPhpUpdate(null);
      refreshMetadata();
      refreshStatuses();
    } catch (error) {
      notify({ tone: "error", title: t.failedToInstallPhpVersion, message: String(error) });
    } finally {
      setPhpUpdating(false);
    }
  };

  // MySQL and PostgreSQL fill one database role; switching installs the chosen
  // engine on first use and moves the stack onto it.
  const switchDatabaseEngine = async (engine: DatabaseEngine) => {
    const name = DATABASE_ENGINE_NAMES[engine];
    setSwitchingEngine(engine);
    setSwitchPercent(0);
    const unlisten = listenIfTauri<DownloadProgress>("download-progress", (event) => {
      setSwitchPercent(event.payload.percent);
    });
    notify({ tone: "info", title: t.switchingDatabase.replace("{name}", name), message: "" });
    try {
      await invoke("switch_database_engine", { engine });
      notify({ tone: "success", title: t.databaseSwitched.replace("{name}", name), message: "" });
    } catch (error) {
      notify({
        tone: "error",
        title: t.failedToSwitchDatabase.replace("{name}", name),
        message: String(error),
      });
    } finally {
      unlisten.then((dispose) => dispose());
      setSwitchingEngine(null);
      refreshMetadata();
      refreshStatuses();
    }
  };

  // The web manager is chosen separately from the engine. Switching installs it
  // on first use; the backend refuses phpMyAdmin for PostgreSQL.
  const switchDatabaseTool = async (tool: DatabaseTool) => {
    const name = DATABASE_TOOL_NAMES[tool];
    setSwitchingTool(tool);
    setSwitchPercent(0);
    const unlisten = listenIfTauri<DownloadProgress>("download-progress", (event) => {
      setSwitchPercent(event.payload.percent);
    });
    notify({ tone: "info", title: t.switchingDatabaseTool.replace("{name}", name), message: "" });
    try {
      await invoke("switch_database_tool", { tool });
      notify({ tone: "success", title: t.databaseToolSwitched.replace("{name}", name), message: "" });
    } catch (error) {
      notify({
        tone: "error",
        title: t.failedToSwitchDatabaseTool.replace("{name}", name),
        message: String(error),
      });
    } finally {
      unlisten.then((dispose) => dispose());
      setSwitchingTool(null);
      refreshMetadata();
      refreshStatuses();
    }
  };

  // Daily background update check (silent when offline).
  useEffect(() => {
    if (settings?.auto_check_updates === false) return undefined;
    const timer = window.setTimeout(() => {
      autoCheckUpdates();
    }, 4000);
    return () => window.clearTimeout(timer);
  }, [autoCheckUpdates, settings?.auto_check_updates]);

  // Tell the user whenever an AI agent changes a service through the connector.
  useEffect(() => {
    const unlisten = listenIfTauri<AgentActivity>("agent-activity", (event) => {
      const activity = event.payload;
      const service = activity.service ? SERVICE_DISPLAY_NAMES[activity.service] : null;
      const target = activity.target
        ? service
          ? `${activity.target} (${service})`
          : activity.target
        : (service ?? t.stack);
      notify({
        tone: activity.ok ? "success" : "error",
        title: t.agentActivityTitle,
        message: activity.ok
          ? t.agentActivityDone.replace("{action}", activity.tool).replace("{target}", target)
          : (activity.error ?? activity.tool),
      });
      refreshStatuses();
    });
    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, [notify, refreshStatuses, t]);

  const caddyPort = serviceMap[ServiceType.Caddy]?.port || DEFAULT_PORTS[ServiceType.Caddy];
  const webServerUrl = `http://localhost:${caddyPort}`;
  const databaseEngine = getDatabaseEngine(packageSelection);
  const databaseTool = getDatabaseTool(packageSelection);
  const isAdminerSelected = databaseTool === "adminer";
  const databaseToolName = DATABASE_TOOL_NAMES[databaseTool];
  const databaseToolUrl = `${webServerUrl}/${databaseTool}`;

  const openFolder = useCallback(
    async (path?: string) => {
      if (!path) return;
      try {
        await invoke("open_folder", { path });
      } catch (error) {
        notify({
          tone: "error",
          title: t.openFolderFailed,
          message: String(error),
        });
      }
    },
    [notify, t]
  );

  const openTerminal = useCallback(
    async (path?: string) => {
      try {
        await invoke("open_project_terminal", { path: path || null });
      } catch (error) {
        notify({
          tone: "error",
          title: t.openTerminalFailed,
          message: String(error),
        });
      }
    },
    [notify, t]
  );

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Esc to dismiss toast (works in any keyboard layout)
      if (e.code === "Escape" && notice) {
        dismiss();
      }
      // F1 to open the guide
      if (e.key === "F1") {
        e.preventDefault();
        setShowGuide(true);
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
      // Ctrl/Cmd + T to open terminal
      if ((e.ctrlKey || e.metaKey) && e.code === "KeyT") {
        e.preventDefault();
        openTerminal(appPaths?.projects_dir);
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
  }, [
    notice,
    dismiss,
    busy,
    runStackCommand,
    allRunning,
    runningCount,
    appPaths,
    isCaddyRunning,
    webServerUrl,
    databaseToolUrl,
    showSettings,
    showHelp,
    openFolder,
    openTerminal,
  ]);

  /** Open Log Doctor on the first failed service, or on the web server. */
  const openLogDoctor = useCallback(() => {
    const failed = DASHBOARD_SERVICE_TYPES.find(
      (serviceType) => serviceMap[serviceType]?.state === ServiceState.Error
    );
    setDoctorService(failed ?? ServiceType.Caddy);
  }, [serviceMap]);

  const versionRows = useMemo(() => {
    const entries: Array<[string, unknown]> = [
      ["Caddy", installedVersions.caddy],
      ["PHP", installedVersions.php],
      ["MySQL", installedVersions.mysql],
      ["PostgreSQL", installedVersions.postgresql],
      [
        databaseToolName,
        // Both tools can be on disk after switching engines; show the active one.
        isAdminerSelected ? installedVersions.adminer : installedVersions.phpmyadmin,
      ],
      ["ngrok", installedVersions.ngrok],
    ];
    return entries.filter(
      (entry): entry is [string, string] => typeof entry[1] === "string" && entry[1].length > 0
    );
  }, [databaseToolName, installedVersions, isAdminerSelected]);

  const railActions: RailAction[] = [
    {
      id: "projects",
      icon: <Folder size={16} />,
      label: t.projects,
      shortcut: `${MOD_KEY} O`,
      onSelect: () => openFolder(appPaths?.projects_dir),
    },
    {
      id: "terminal",
      icon: <TerminalSquare size={16} />,
      label: t.terminal,
      shortcut: `${MOD_KEY} T`,
      onSelect: () => openTerminal(appPaths?.projects_dir),
    },
    {
      id: "doctor",
      icon: <Stethoscope size={16} />,
      label: t.logDoctor,
      onSelect: openLogDoctor,
    },
    {
      id: "logs",
      icon: <ScrollText size={16} />,
      label: t.logs,
      shortcut: `${MOD_KEY} L`,
      onSelect: () => openFolder(appPaths?.logs_dir),
    },
    {
      id: "runtime",
      icon: <HardDrive size={16} />,
      label: t.runtime,
      onSelect: () => openFolder(appPaths?.runtime_dir),
    },
    {
      id: "guide",
      icon: <BookOpen size={16} />,
      label: t.manual,
      shortcut: "F1",
      onSelect: () => setShowGuide(true),
    },
    {
      id: "config",
      icon: <FolderCog size={16} />,
      label: t.openConfigFolder,
      onSelect: () => openFolder(appPaths?.config_dir),
    },
  ];

  // The request path: browser -> Caddy -> PHP-FPM -> active database.
  const pathNodes = stackServiceTypes.map((serviceType) => ({
    serviceType,
    state: serviceMap[serviceType]?.state ?? ServiceState.Stopped,
    port: serviceMap[serviceType]?.port ?? DEFAULT_PORTS[serviceType],
  }));
  const stackState = allRunning ? "running" : runningCount > 0 ? "partial" : "stopped";
  const stackStateLabel = allRunning ? t.running : runningCount > 0 ? t.active : t.stopped;
  const engineSwitch: EngineSwitch = {
    value: databaseEngine,
    installed: {
      mysql: Boolean(installedVersions.mysql),
      postgresql: Boolean(installedVersions.postgresql),
    },
    switching: switchingEngine,
    percent: switchPercent,
    onChange: switchDatabaseEngine,
  };
  const toolSwitch: ToolSwitch = {
    value: databaseTool,
    installed: {
      phpmyadmin: Boolean(installedVersions.phpmyadmin),
      adminer: Boolean(installedVersions.adminer),
    },
    switching: switchingTool,
    percent: switchPercent,
    onChange: switchDatabaseTool,
  };
  const updateInstalling = updatePhase === "installing";

  return (
    <div className="app-shell" data-testid="dashboard">
      <header className="titlebar">
        <div className="brand">
          <img className="brand-logo" src={champLogo} alt="" />
          <div className="brand-copy">
            <h1>
              CHAMP <span className="brand-version">v{packageInfo.version}</span>
            </h1>
            <p>{t.appDescription}</p>
          </div>
        </div>

        <div className="titlebar-actions">
          <div className="stack-controls" role="group" aria-label={t.stack}>
            <button
              type="button"
              className="btn-command primary"
              onClick={() => {
                AudioManager.playClick();
                runStackCommand("start_all_services");
              }}
              disabled={Boolean(busy) || allRunning}
              title={`${t.startAllServices} (Ctrl+S)`}
            >
              {busyStackCommand === "start_all_services" ? (
                <LoaderCircle size={15} className="spin-icon" />
              ) : (
                <Play size={15} />
              )}
              {busyStackCommand === "start_all_services" ? t.starting : t.start}
            </button>
            <button
              type="button"
              className="btn-command"
              onClick={() => {
                AudioManager.playClick();
                runStackCommand("restart_all_services");
              }}
              disabled={Boolean(busy)}
              title={`${t.restartAllServices} (Ctrl+R)`}
            >
              {busyStackCommand === "restart_all_services" ? (
                <LoaderCircle size={15} className="spin-icon" />
              ) : (
                <RotateCw size={15} />
              )}
              {busyStackCommand === "restart_all_services" ? t.restarting : t.restart}
            </button>
            <button
              type="button"
              className="btn-command danger"
              onClick={() => {
                AudioManager.playClick();
                runStackCommand("stop_all_services");
              }}
              disabled={Boolean(busy) || runningCount === 0}
              title={`${t.stopAllServices} (Ctrl+X)`}
            >
              {busyStackCommand === "stop_all_services" ? (
                <LoaderCircle size={14} className="spin-icon" />
              ) : (
                <Square size={13} />
              )}
              {busyStackCommand === "stop_all_services" ? t.stopping : t.stop}
            </button>
          </div>

          <span className="titlebar-divider" aria-hidden="true" />

          <button
            type="button"
            className="icon-button"
            onClick={() => openUrl(SOURCE_REPO_URL)}
            title={t.sourceRepository}
            aria-label={t.sourceRepository}
          >
            <GitHubIcon size={17} />
          </button>
          <button
            type="button"
            className="icon-button"
            onClick={() => {
              AudioManager.playClick();
              setShowGuide(true);
            }}
            title={`${t.manual} (F1)`}
            aria-label={t.manual}
          >
            <BookOpen size={17} />
          </button>
          <button
            type="button"
            className="icon-button"
            onClick={() => {
              AudioManager.playClick();
              setShowHelp(true);
            }}
            title={`${t.help} (?)`}
            aria-label={t.help}
          >
            <Keyboard size={17} />
          </button>
          <button
            type="button"
            className="icon-button"
            onClick={() => {
              AudioManager.playClick();
              setShowSettings(true);
            }}
            title={`${t.settings} (Ctrl+,)`}
            aria-label={t.settings}
          >
            <Settings size={17} />
          </button>
          <LanguageSelector />
        </div>
      </header>

      {(updatePhase === "available" || updateInstalling) && availableUpdate && (
        <div className="update-banner" role="status">
          <ArrowUpCircle size={17} />
          <p>
            <strong>{t.updateAvailableTitle.replace("{version}", availableUpdate.version)}</strong>
            <span>{updateInstalling ? t.updateInstalling : t.updateRestartNote}</span>
          </p>
          {updateInstalling ? (
            <span className="update-progress">
              <LoaderCircle size={15} className="spin-icon" />
              {updateProgress !== null ? `${updateProgress}%` : t.working}
            </span>
          ) : (
            <>
              <button type="button" className="btn-primary compact" onClick={installUpdate}>
                {t.updateInstall}
              </button>
              <button
                type="button"
                className="icon-button ghost"
                onClick={dismissUpdate}
                aria-label={t.updateLater}
                title={t.updateLater}
              >
                <X size={15} />
              </button>
            </>
          )}
        </div>
      )}

      {notice && (
        <div
          className={`stack-notice ${notice.tone} ${notice.action ?? ""}`}
          role={notice.tone === "error" ? "alert" : "status"}
        >
          <span className="stack-notice-icon" aria-hidden="true">
            {notice.tone === "info" && <LoaderCircle size={17} className="spin-icon" />}
            {notice.tone === "success" && <CheckCircle2 size={17} />}
            {notice.tone === "error" && <AlertTriangle size={17} />}
          </span>
          <span>
            <strong>{notice.title}</strong>
            <small>{notice.message}</small>
          </span>
          <button
            type="button"
            className="notice-close"
            onClick={() => {
              AudioManager.playClick();
              dismiss();
            }}
            aria-label={t.close}
          >
            <X size={15} />
          </button>
        </div>
      )}

      <main className="workspace">
        <section className={`request-path ${stackState}`} aria-label={t.requestPath}>
          <div className="request-path-head">
            <span className="eyebrow">
              <Route size={13} /> {t.requestPath}
            </span>
            <span className={`stack-state ${stackState}`}>
              {stackStateLabel} · {runningCount}/{totalCount} {t.services}
            </span>
          </div>

          <ol className="path-chain">
            <li className={`path-node origin ${isCaddyRunning ? "running" : "stopped"}`}>
              <span className="node-led" aria-hidden="true" />
              <span className="node-name">
                <Monitor size={14} /> {t.browser}
              </span>
              <span className="node-port">localhost:{caddyPort}</span>
            </li>
            {pathNodes.map((node, index) => {
              const previous =
                index === 0 ? isCaddyRunning : pathNodes[index - 1].state === ServiceState.Running;
              const linkLive = previous && node.state === ServiceState.Running;
              return [
                <li
                  key={`${node.serviceType}-link`}
                  className={`path-link ${linkLive ? "live" : ""}`}
                  aria-hidden="true"
                />,
                <li key={node.serviceType} className={`path-node ${node.state}`}>
                  <span className="node-led" aria-hidden="true" />
                  <span className="node-name">
                    <PathIcon serviceType={node.serviceType} />{" "}
                    {SERVICE_DISPLAY_NAMES[node.serviceType]}
                  </span>
                  <span className="node-port">:{node.port}</span>
                </li>,
              ];
            })}
          </ol>

          <div className="request-path-actions">
            <button
              type="button"
              className="btn-path"
              onClick={() => {
                AudioManager.playClick();
                openUrl(webServerUrl);
              }}
              disabled={!isCaddyRunning}
              title={`${t.openWebsite} (Ctrl+W)`}
            >
              <Globe size={16} /> {t.website}
            </button>
            <button
              type="button"
              className="btn-path"
              onClick={() => {
                AudioManager.playClick();
                openUrl(databaseToolUrl);
              }}
              disabled={!isCaddyRunning}
              title={`${t.openDatabaseTool} (Ctrl+D)`}
            >
              <Database size={16} /> {databaseToolName}
            </button>
          </div>
        </section>

        <div className="workspace-grid">
          <section className="service-grid" aria-label={t.services}>
            {stackServiceTypes.map((serviceType) => {
              const service = serviceMap[serviceType];
              if (!service) return null;
              const isDatabase = serviceType === DATABASE_ENGINE_SERVICES[databaseEngine];
              const busyServiceCommand = busy?.endsWith(serviceType)
                ? (busy.split(":")[0] as ServiceCommand)
                : null;
              return (
                <ServiceCard
                  // One card for the database role, whichever engine fills it.
                  key={isDatabase ? "database" : serviceType}
                  serviceType={serviceType}
                  state={service.state}
                  port={service.port}
                  error={service.error_message}
                  engineSwitch={isDatabase ? engineSwitch : undefined}
                  toolSwitch={isDatabase ? toolSwitch : undefined}
                  busy={Boolean(busyServiceCommand)}
                  busyLabel={
                    busyServiceCommand
                      ? t[SERVICE_COMMAND_COPY[busyServiceCommand].buttonLabelKey]
                      : undefined
                  }
                  onStart={() => runServiceCommand("start_service", serviceType)}
                  onStop={() => runServiceCommand("stop_service", serviceType)}
                  onRestart={() => runServiceCommand("restart_service", serviceType)}
                  onDiagnose={() => setDoctorService(serviceType)}
                />
              );
            })}
          </section>

          <aside className="side-rail">
            <section
              className={`rail-card https-tunnel-panel ${tunnelReady ? "ready" : tunnelStatus.running ? "running" : ""}`}
              data-testid="https-tunnel-panel"
            >
              <header className="rail-card-head">
                <ShieldCheck size={16} />
                <h2>{t.httpsPreview}</h2>
              </header>
              <p className={tunnelReady ? "https-tunnel-url" : "rail-muted"}>
                {tunnelReady
                  ? tunnelStatus.url
                  : tunnelHasPendingUrl
                    ? t.httpsTunnelValidating
                    : tunnelStatus.running
                      ? t.httpsTunnelWaitingForDomain
                      : t.httpsTunnelDescription}
              </p>
              {tunnelHasPendingUrl && (
                <small className="https-tunnel-pending-url">{tunnelStatus.url}</small>
              )}
              {tunnelStatus.error && <small className="rail-error">{tunnelStatus.error}</small>}
              {showNgrokTokenForm && (
                <form
                  className="https-tunnel-token"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void submitNgrokToken();
                  }}
                >
                  <label htmlFor="ngrok-authtoken">{t.httpsTunnelTokenLabel}</label>
                  <small className="rail-muted">{t.httpsTunnelTokenHelp}</small>
                  <input
                    id="ngrok-authtoken"
                    type="password"
                    autoComplete="off"
                    spellCheck={false}
                    value={ngrokTokenDraft}
                    onChange={(event) => setNgrokTokenDraft(event.target.value)}
                  />
                  <div className="https-tunnel-actions">
                    <button
                      type="submit"
                      className="btn-primary compact"
                      disabled={!ngrokTokenDraft.trim()}
                    >
                      {t.httpsTunnelTokenSave}
                    </button>
                    <button
                      type="button"
                      className="btn-secondary compact"
                      onClick={() =>
                        openUrl("https://dashboard.ngrok.com/get-started/your-authtoken")
                      }
                    >
                      <ExternalLink size={14} /> {t.httpsTunnelTokenGet}
                    </button>
                  </div>
                </form>
              )}
              <div className="https-tunnel-actions">
                {tunnelReady && (
                  <>
                    <button
                      type="button"
                      className="btn-secondary compact"
                      onClick={() => openUrl(tunnelStatus.url ?? "")}
                      title={t.httpsTunnelOpen}
                    >
                      <ExternalLink size={14} /> {t.open}
                    </button>
                    <button
                      type="button"
                      className="btn-secondary compact"
                      onClick={copyHttpsTunnelUrl}
                      title={t.httpsTunnelCopy}
                    >
                      <Copy size={14} /> {t.httpsTunnelCopy}
                    </button>
                  </>
                )}
                <button
                  type="button"
                  className={`compact ${tunnelStatus.running ? "btn-secondary danger" : "btn-primary"}`}
                  onClick={tunnelStatus.running ? stopHttpsTunnel : startHttpsTunnel}
                  disabled={Boolean(tunnelBusy)}
                >
                  {tunnelBusy ? (
                    <LoaderCircle size={14} className="spin-icon" />
                  ) : tunnelStatus.running ? (
                    <Square size={13} />
                  ) : (
                    <ShieldCheck size={14} />
                  )}
                  {tunnelBusy === "start"
                    ? t.starting
                    : tunnelBusy === "stop"
                      ? t.stopping
                      : tunnelStatus.running
                        ? t.httpsTunnelStop
                        : t.httpsTunnelStart}
                </button>
                {tunnelStatus.authtoken_configured &&
                  !tunnelStatus.running &&
                  !editingNgrokToken && (
                    <button
                      type="button"
                      className="btn-secondary compact"
                      onClick={() => setEditingNgrokToken(true)}
                    >
                      {t.httpsTunnelTokenChange}
                    </button>
                  )}
              </div>
            </section>

            <nav className="rail-card" aria-label={t.shortcuts}>
              <header className="rail-card-head">
                <Zap size={16} />
                <h2>{t.shortcuts}</h2>
              </header>
              <ul className="rail-actions">
                {railActions.map((action) => (
                  <li key={action.id}>
                    <button
                      type="button"
                      className="rail-action"
                      onClick={() => {
                        AudioManager.playClick();
                        action.onSelect();
                      }}
                    >
                      {action.icon}
                      <span>{action.label}</span>
                      {action.shortcut && <kbd>{action.shortcut}</kbd>}
                    </button>
                  </li>
                ))}
              </ul>
            </nav>

            {versionRows.length > 0 && (
              <section className="rail-card" aria-label={t.installedVersionsLabel}>
                <header className="rail-card-head">
                  <Package size={16} />
                  <h2>{t.installedVersionsLabel}</h2>
                </header>
                {phpUpdate && (
                  <div className="version-update">
                    <ArrowUpCircle size={15} />
                    <span>
                      PHP {phpUpdate.version} → <strong>{phpUpdate.latest_version}</strong>
                    </span>
                    <button
                      type="button"
                      className="btn-primary compact"
                      onClick={updatePhp}
                      disabled={phpUpdating}
                    >
                      {phpUpdating ? <LoaderCircle size={14} className="spin-icon" /> : null}
                      {phpUpdating ? t.working : t.phpUpdateNow}
                    </button>
                  </div>
                )}
                <dl className="version-list">
                  {versionRows.map(([name, version]) => (
                    <div key={name}>
                      <dt>{name}</dt>
                      <dd>{version}</dd>
                    </div>
                  ))}
                </dl>
              </section>
            )}
          </aside>
        </div>
      </main>

      <StatusBar
        services={serviceMap}
        appPaths={appPaths || undefined}
        packageSelection={packageSelection}
        data-testid="status-bar"
      />

      {showSettings && (
        <SettingsPanel
          onClose={() => setShowSettings(false)}
          onSettingsChanged={() => {
            refreshStatuses();
            refreshMetadata();
          }}
        />
      )}

      {showHelp && (
        <HelpModal
          onClose={() => setShowHelp(false)}
          onOpenGuide={() => {
            setShowHelp(false);
            setShowGuide(true);
          }}
        />
      )}

      {showGuide && <GuideModal onClose={() => setShowGuide(false)} />}

      {doctorService && (
        <LogDoctorModal
          initialService={doctorService}
          services={serviceMap}
          appPaths={appPaths}
          onClose={() => setDoctorService(null)}
          onOpenSettings={() => {
            setDoctorService(null);
            setShowSettings(true);
          }}
          onStartPhp={() => {
            setDoctorService(null);
            runServiceCommand("start_service", ServiceType.PhpFpm);
          }}
        />
      )}
    </div>
  );
}
