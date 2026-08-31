import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  AlertTriangle,
  CheckCircle2,
  Copy,
  CircleHelp,
  Database,
  ExternalLink,
  FilePlus2,
  Folder,
  Globe,
  HardDrive,
  LoaderCircle,
  MoreHorizontal,
  Play,
  RefreshCw,
  Settings,
  ShieldCheck,
  Square,
  TerminalSquare,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import packageInfo from "../../package.json";
import champLogo from "../assets/CHAMP.png";
import { useTranslation } from "../stores/languageStore";
import { AudioManager } from "../utils/audioManager";
import {
  AppSettings,
  DEFAULT_PORTS,
  ServiceType,
  isAdminerSelected as isAdminerSelection,
} from "../types/services";
import { useServices } from "../hooks/useServices";
import { useHttpsTunnel } from "../hooks/useHttpsTunnel";
import { useToast } from "../hooks/useToast";
import {
  AppPaths,
  SERVICE_COMMAND_COPY,
  SOURCE_REPO_URL,
  ServiceCommand,
} from "./dashboard/types";
import { GitHubIcon } from "./dashboard/GitHubIcon";
import { HelpModal } from "./HelpModal";
import { LanguageSelector } from "./LanguageSelector";
import { ServiceCard } from "./ServiceCard";
import { SettingsPanel } from "./SettingsPanel";
import { StatusBar } from "./StatusBar";
import { TemplateSelector, ProjectScaffoldResult } from "./TemplateSelector";

const DASHBOARD_SERVICE_TYPES = [
  ServiceType.Caddy,
  ServiceType.PhpFpm,
  ServiceType.MySQL,
  ServiceType.PostgreSQL,
] as const;

export function Dashboard() {
  const { t } = useTranslation();
  const [showSettings, setShowSettings] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  const [appPaths, setAppPaths] = useState<AppPaths | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [installedVersions, setInstalledVersions] = useState<Record<string, string>>({});
  const [showProjectCreator, setShowProjectCreator] = useState(false);
  const [showQuickActionsMenu, setShowQuickActionsMenu] = useState(false);

  const { notice, notify, dismiss } = useToast();

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
  } = tunnel;

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

  const caddyPort =
    serviceMap[ServiceType.Caddy]?.port || DEFAULT_PORTS[ServiceType.Caddy];
  const webServerUrl = `http://localhost:${caddyPort}`;
  const isAdminerSelected = isAdminerSelection(packageSelection);
  const databaseToolName = isAdminerSelected ? "Adminer" : "phpMyAdmin";
  const databaseToolUrl = `${webServerUrl}/${isAdminerSelected ? "adminer" : "phpmyadmin"}`;

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

  useEffect(() => {
    if (!showProjectCreator) return undefined;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.code === "Escape") {
        setShowProjectCreator(false);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [showProjectCreator]);

  useEffect(() => {
    if (!showQuickActionsMenu) return undefined;

    const closeMenu = () => setShowQuickActionsMenu(false);
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.code === "Escape") {
        setShowQuickActionsMenu(false);
      }
    };

    window.addEventListener("click", closeMenu);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("click", closeMenu);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [showQuickActionsMenu]);

  const handleProjectCreated = async (result: ProjectScaffoldResult) => {
    notify({
      tone: "success",
      title: t.projectCreated,
      message: `${result.name} -> ${result.path}`,
    });
    setShowProjectCreator(false);
    await refreshMetadata();
  };

  const handleProjectError = (error: string) => {
    notify({
      tone: "error",
      title: t.projectCreateFailed,
      message: error,
    });
  };

  const versionBadges = useMemo(() => {
    const entries: Array<[string, unknown]> = [
      ["Caddy", installedVersions.caddy],
      ["PHP", installedVersions.php],
      ["MySQL", installedVersions.mysql],
      ["PostgreSQL", installedVersions.postgresql],
      [databaseToolName, installedVersions.phpmyadmin || installedVersions.adminer],
      ["cloudflared", installedVersions.cloudflared],
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
            {t.appName} <span>v{packageInfo.version}</span>
          </h1>
          <p>{t.appDescription}</p>
        </div>
        <div className="titlebar-actions">
          <button
            className="btn-command primary"
            onClick={() => {
              AudioManager.playClick();
              runStackCommand("start_all_services");
            }}
            disabled={Boolean(busy) || allRunning}
            title={`${t.startAllServices} (Ctrl+S)`}
            onMouseEnter={() => AudioManager.playHover()}
          >
            {busyStackCommand === "start_all_services" ? (
              <LoaderCircle size={16} className="spin-icon" />
            ) : (
              <Play size={16} />
            )}
            {busyStackCommand === "start_all_services" ? t.starting : t.start}
          </button>
          <button
            className="btn-command"
            onClick={() => {
              AudioManager.playClick();
              runStackCommand("restart_all_services");
            }}
            disabled={Boolean(busy)}
            title={`${t.restartAllServices} (Ctrl+R)`}
            onMouseEnter={() => AudioManager.playHover()}
          >
            {busyStackCommand === "restart_all_services" ? (
              <LoaderCircle size={16} className="spin-icon" />
            ) : (
              <RefreshCw size={16} />
            )}
            {busyStackCommand === "restart_all_services" ? t.restarting : t.restart}
          </button>
          <button
            className="btn-command danger"
            onClick={() => {
              AudioManager.playClick();
              runStackCommand("stop_all_services");
            }}
            disabled={Boolean(busy) || runningCount === 0}
            title={`${t.stopAllServices} (Ctrl+X)`}
            onMouseEnter={() => AudioManager.playHover()}
          >
            {busyStackCommand === "stop_all_services" ? (
              <LoaderCircle size={15} className="spin-icon" />
            ) : (
              <Square size={15} />
            )}
            {busyStackCommand === "stop_all_services" ? t.stopping : t.stop}
          </button>
          <button
            className="icon-button github"
            onClick={() => openUrl(SOURCE_REPO_URL)}
            title={t.sourceRepository}
            aria-label={t.sourceRepository}
          >
            <GitHubIcon size={18} />
          </button>
          <button
            className="icon-button"
            onClick={() => {
              AudioManager.playClick();
              setShowHelp(true);
            }}
            title={`${t.help} (?)`}
            aria-label={t.help}
            onMouseEnter={() => AudioManager.playHover()}
          >
            <CircleHelp size={18} />
          </button>
          <button
            className="icon-button"
            onClick={() => {
              AudioManager.playClick();
              setShowSettings(true);
            }}
            title={`${t.settings} (Ctrl+,)`}
            aria-label={t.settings}
            onMouseEnter={() => AudioManager.playHover()}
          >
            <Settings size={18} />
          </button>
          <LanguageSelector variant="toggle" />
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
          <button
            className="notice-close"
            onClick={() => {
              AudioManager.playClick();
              dismiss();
            }}
            aria-label={t.close}
            onMouseEnter={() => AudioManager.playHover()}
          >
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
              {allRunning ? t.running : runningCount > 0 ? t.active : t.stopped}
            </span>
            <h2>
              {runningCount}/{totalCount} {t.services}
            </h2>
          </div>
          <div className="quick-actions">
            <button
              className="btn-quick-action action-site"
              onClick={() => {
                AudioManager.playClick();
                openUrl(webServerUrl);
              }}
              disabled={!isCaddyRunning}
              title={`${t.openWebsite} (Ctrl+W)`}
              onMouseEnter={() => AudioManager.playHover()}
            >
              <Globe size={16} /> {t.website}
            </button>
            <button
              className="btn-quick-action action-database"
              onClick={() => {
                AudioManager.playClick();
                openUrl(databaseToolUrl);
              }}
              disabled={!isCaddyRunning}
              title={`${t.openDatabaseTool} (Ctrl+D)`}
              onMouseEnter={() => AudioManager.playHover()}
            >
              <Database size={16} /> {databaseToolName}
            </button>
            <button
              className="btn-quick-action action-projects"
              onClick={() => {
                AudioManager.playClick();
                openFolder(appPaths?.projects_dir);
              }}
              title={`${t.openProjectsFolder} (Ctrl+O)`}
              onMouseEnter={() => AudioManager.playHover()}
            >
              <Folder size={16} /> {t.projects}
            </button>
            <button
              className="btn-quick-action action-runtime"
              onClick={() => {
                AudioManager.playClick();
                openFolder(appPaths?.runtime_dir);
              }}
              title={t.openRuntimeFolder}
              onMouseEnter={() => AudioManager.playHover()}
            >
              <HardDrive size={16} /> {t.runtime}
            </button>
            <div className="quick-actions-menu-wrap" onClick={(event) => event.stopPropagation()}>
              <button
                className="btn-quick-action action-more"
                onClick={() => {
                  AudioManager.playClick();
                  setShowQuickActionsMenu((current) => !current);
                }}
                title={t.more || "More"}
                aria-haspopup="menu"
                aria-expanded={showQuickActionsMenu}
                onMouseEnter={() => AudioManager.playHover()}
              >
                <MoreHorizontal size={16} /> {t.more || "More"}
              </button>
              {showQuickActionsMenu && (
                <div className="quick-actions-menu" role="menu">
                  <button
                    className="quick-menu-action"
                    role="menuitem"
                    onClick={() => {
                      AudioManager.playClick();
                      setShowQuickActionsMenu(false);
                      openTerminal(appPaths?.projects_dir);
                    }}
                  >
                    <TerminalSquare size={16} /> {t.terminal}
                  </button>
                  <button
                    className="quick-menu-action"
                    role="menuitem"
                    onClick={() => {
                      AudioManager.playClick();
                      setShowQuickActionsMenu(false);
                      setShowProjectCreator(true);
                    }}
                  >
                    <FilePlus2 size={16} /> {t.createProject}
                  </button>
                  <button
                    className="quick-menu-action"
                    role="menuitem"
                    onClick={() => {
                      AudioManager.playClick();
                      setShowQuickActionsMenu(false);
                      openFolder(appPaths?.logs_dir);
                    }}
                  >
                    <TerminalSquare size={16} /> {t.logs}
                  </button>
                  <button
                    className="quick-menu-action"
                    role="menuitem"
                    onClick={() => {
                      AudioManager.playClick();
                      setShowQuickActionsMenu(false);
                      openFolder(appPaths?.config_dir);
                    }}
                  >
                    <HardDrive size={16} /> {t.settings}
                  </button>
                  <button
                    className="quick-menu-action"
                    role="menuitem"
                    onClick={() => {
                      setShowQuickActionsMenu(false);
                      openUrl(SOURCE_REPO_URL);
                    }}
                  >
                    <GitHubIcon size={16} /> GitHub
                  </button>
                </div>
              )}
            </div>
          </div>
        </section>

        <section
          className={`https-tunnel-panel ${tunnelReady ? "ready" : tunnelStatus.running ? "running" : ""}`}
          data-testid="https-tunnel-panel"
        >
          <div className="https-tunnel-copy">
            <span className="https-tunnel-icon" aria-hidden="true">
              <ShieldCheck size={18} />
            </span>
            <div className="https-tunnel-main">
              <span className="https-tunnel-kicker">{t.httpsPreview}</span>
              <h3>{t.publicHttpsDomain}</h3>
              <p className={tunnelStatus.url ? "https-tunnel-url" : undefined}>
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
              {tunnelStatus.error && <small>{tunnelStatus.error}</small>}
            </div>
          </div>
          <div className="https-tunnel-actions">
            {tunnelReady && (
              <>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => openUrl(tunnelStatus.url ?? "")}
                  title={t.httpsTunnelOpen}
                  onMouseEnter={() => AudioManager.playHover()}
                >
                  <ExternalLink size={15} /> {t.open}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={copyHttpsTunnelUrl}
                  title={t.httpsTunnelCopy}
                  onMouseEnter={() => AudioManager.playHover()}
                >
                  <Copy size={15} /> {t.httpsTunnelCopy}
                </button>
              </>
            )}
            <button
              className={tunnelStatus.running ? "btn-secondary danger" : "btn-primary"}
              type="button"
              onClick={tunnelStatus.running ? stopHttpsTunnel : startHttpsTunnel}
              disabled={Boolean(tunnelBusy)}
              onMouseEnter={() => AudioManager.playHover()}
            >
              {tunnelBusy ? (
                <LoaderCircle size={15} className="spin-icon" />
              ) : tunnelStatus.running ? (
                <Square size={15} />
              ) : (
                <ShieldCheck size={15} />
              )}
              {tunnelBusy === "start"
                ? t.starting
                : tunnelBusy === "stop"
                  ? t.stopping
                  : tunnelStatus.running
                    ? t.httpsTunnelStop
                    : t.httpsTunnelStart}
            </button>
          </div>
        </section>

        {versionBadges.length > 0 && (
          <section className="version-strip" aria-label={t.installedVersionsLabel}>
            {versionBadges.map(([name, version]) => (
              <span key={name}>
                {name} {version}
              </span>
            ))}
          </section>
        )}

        <section className="service-grid-responsive">
          {DASHBOARD_SERVICE_TYPES.map((serviceType) => {
            const service = serviceMap[serviceType];
            if (!service) return null;
            const busyServiceCommand = busy?.endsWith(serviceType)
              ? (busy.split(":")[0] as ServiceCommand)
              : null;
            return (
              <ServiceCard
                key={serviceType}
                serviceType={serviceType}
                state={service.state}
                port={service.port}
                error={service.error_message}
                busy={Boolean(busyServiceCommand)}
                busyLabel={
                  busyServiceCommand
                    ? t[SERVICE_COMMAND_COPY[busyServiceCommand].buttonLabelKey]
                    : undefined
                }
                onStart={() => runServiceCommand("start_service", serviceType)}
                onStop={() => runServiceCommand("stop_service", serviceType)}
                onRestart={() => runServiceCommand("restart_service", serviceType)}
              />
            );
          })}
        </section>
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

      {showHelp && <HelpModal onClose={() => setShowHelp(false)} />}

      {showProjectCreator && (
        <TemplateSelector
          appPaths={appPaths}
          installedVersions={installedVersions}
          onClose={() => setShowProjectCreator(false)}
          onProjectCreated={handleProjectCreated}
          onError={handleProjectError}
        />
      )}
    </div>
  );
}
