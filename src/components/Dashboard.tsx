import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "../stores/preferencesStore";
import { AudioManager } from "../utils/audioManager";
import {
  ALL_SERVICE_TYPES,
  DATABASE_ENGINE_SERVICES,
  DATABASE_TOOL_NAMES,
  DEFAULT_PORTS,
  ServiceState,
  ServiceType,
  getDatabaseEngine,
  getDatabaseTool,
} from "../types/services";
import { SERVICE_COMMAND_COPY, ServiceCommand, StackCommand } from "../hooks/commandCopy";
import { useAppMetadata } from "../hooks/useAppMetadata";
import { useAgentActivityNotices, useUpdateChecks } from "../hooks/useDashboardEvents";
import { useDashboardShortcuts } from "../hooks/useDashboardShortcuts";
import { useDatabaseSwitch } from "../hooks/useDatabaseSwitch";
import { useDesktopActions } from "../hooks/useDesktopActions";
import { useEscape } from "../hooks/useEscape";
import { useHttpsTunnel } from "../hooks/useHttpsTunnel";
import { usePhpUpdate } from "../hooks/usePhpUpdate";
import { useServices } from "../hooks/useServices";
import { useStatusPolling } from "../hooks/useStatusPolling";
import { useToast } from "../hooks/useToast";
import { HttpsPreviewCard } from "./dashboard/HttpsPreviewCard";
import { NoticeToast } from "./dashboard/NoticeToast";
import { RailShortcuts } from "./dashboard/RailShortcuts";
import { RequestPath } from "./dashboard/RequestPath";
import { TitleBar } from "./dashboard/TitleBar";
import { UpdateBanner } from "./dashboard/UpdateBanner";
import { VersionsCard } from "./dashboard/VersionsCard";
import { GuideModal } from "./GuideModal";
import { HelpModal } from "./HelpModal";
import { LogDoctorModal } from "./LogDoctorModal";
import { ServiceCard } from "./ServiceCard";
import { SettingsPanel } from "./SettingsPanel";
import { StatusBar } from "./StatusBar";

export function Dashboard() {
  const { t } = useTranslation();
  const [showSettings, setShowSettings] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  const [showGuide, setShowGuide] = useState(false);
  const [doctorService, setDoctorService] = useState<ServiceType | null>(null);

  const { notice, notify, dismiss } = useToast();
  const { appPaths, settings, installedVersions, refreshMetadata } = useAppMetadata();
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
    runStackCommand: startStackCommand,
    runServiceCommand,
  } = useServices({ settings, notify });
  const tunnel = useHttpsTunnel({ notify, refreshStatuses, refreshMetadata });
  const { tunnelBusy, refreshTunnelStatus, resetTunnel } = tunnel;
  const { openExternal, openFolder, openTerminal } = useDesktopActions(notify);

  const refreshAll = useCallback(
    () => Promise.all([refreshStatuses(), refreshTunnelStatus()]),
    [refreshStatuses, refreshTunnelStatus]
  );
  useStatusPolling(refreshAll);

  const refreshAfterChange = useCallback(() => {
    refreshMetadata();
    refreshStatuses();
  }, [refreshMetadata, refreshStatuses]);

  const { phpUpdate, phpUpdating, updatePhp, checkPhpUpdate } = usePhpUpdate(
    notify,
    refreshAfterChange
  );
  const { switchingEngine, switchingTool, switchPercent, switchEngine, switchTool } =
    useDatabaseSwitch(notify, refreshAfterChange);

  useAgentActivityNotices(notify, refreshAll);
  useUpdateChecks(settings?.auto_check_updates !== false, notify);
  useEscape(dismiss, Boolean(notice));

  // Initialize audio context on first user interaction
  useEffect(() => {
    window.addEventListener("click", AudioManager.initialize, { once: true });
    return () => window.removeEventListener("click", AudioManager.initialize);
  }, []);

  const runStackCommand = useCallback(
    async (command: StackCommand) => {
      const succeeded = await startStackCommand(command);
      if (succeeded && command === "stop_all_services") resetTunnel();
    },
    [startStackCommand, resetTunnel]
  );

  // Long operations share one lock: starting PHP while an update replaces its
  // files, or switching engines mid-start, would race in the backend.
  const operationBusy =
    Boolean(busy) ||
    phpUpdating ||
    Boolean(switchingEngine || switchingTool) ||
    Boolean(tunnelBusy);

  const caddyPort = serviceMap[ServiceType.Caddy]?.port || DEFAULT_PORTS[ServiceType.Caddy];
  const webServerUrl = `http://localhost:${caddyPort}`;
  const databaseEngine = getDatabaseEngine(packageSelection);
  const databaseTool = getDatabaseTool(packageSelection);
  const databaseToolName = DATABASE_TOOL_NAMES[databaseTool];
  const databaseToolUrl = `${webServerUrl}/${databaseTool}`;

  const openGuide = useCallback(() => setShowGuide(true), []);
  const openHelp = useCallback(() => setShowHelp(true), []);
  const toggleSettings = useCallback(() => setShowSettings((prev) => !prev), []);

  useDashboardShortcuts({
    operationBusy,
    anyModalOpen: showSettings || showHelp || showGuide || doctorService !== null,
    allRunning,
    runningCount,
    isCaddyRunning,
    appPaths,
    webServerUrl,
    databaseToolUrl,
    runStackCommand,
    openExternal,
    openFolder,
    openTerminal,
    openGuide,
    openHelp,
    toggleSettings,
  });

  /** Open Log Doctor on the first failed service, or on the web server. */
  const openLogDoctor = () => {
    const failed = ALL_SERVICE_TYPES.find(
      (serviceType) => serviceMap[serviceType]?.state === ServiceState.Error
    );
    setDoctorService(failed ?? ServiceType.Caddy);
  };

  return (
    <div className="app-shell" data-testid="dashboard">
      <TitleBar
        operationBusy={operationBusy}
        busyStackCommand={busyStackCommand}
        allRunning={allRunning}
        runningCount={runningCount}
        runStackCommand={runStackCommand}
        openExternal={openExternal}
        onOpenGuide={openGuide}
        onOpenHelp={openHelp}
        onOpenSettings={() => setShowSettings(true)}
      />

      <UpdateBanner />

      {notice && <NoticeToast notice={notice} onDismiss={dismiss} />}

      <main className="workspace">
        <RequestPath
          services={serviceMap}
          stackServiceTypes={stackServiceTypes}
          caddyPort={caddyPort}
          isCaddyRunning={isCaddyRunning}
          allRunning={allRunning}
          runningCount={runningCount}
          totalCount={totalCount}
          databaseToolName={databaseToolName}
          onOpenWebsite={() => openExternal(webServerUrl)}
          onOpenDatabaseTool={() => openExternal(databaseToolUrl)}
        />

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
                  engineSwitch={
                    isDatabase
                      ? {
                          value: databaseEngine,
                          installed: {
                            mysql: Boolean(installedVersions.mysql),
                            postgresql: Boolean(installedVersions.postgresql),
                          },
                          switching: switchingEngine,
                          percent: switchPercent,
                          onChange: switchEngine,
                        }
                      : undefined
                  }
                  toolSwitch={
                    isDatabase
                      ? {
                          value: databaseTool,
                          installed: {
                            phpmyadmin: Boolean(installedVersions.phpmyadmin),
                            adminer: Boolean(installedVersions.adminer),
                          },
                          switching: switchingTool,
                          percent: switchPercent,
                          onChange: switchTool,
                        }
                      : undefined
                  }
                  busy={Boolean(busyServiceCommand)}
                  actionsLocked={operationBusy}
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
            <HttpsPreviewCard tunnel={tunnel} openExternal={openExternal} />
            <RailShortcuts
              appPaths={appPaths}
              openFolder={openFolder}
              openTerminal={openTerminal}
              onOpenDoctor={openLogDoctor}
              onOpenGuide={openGuide}
            />
            <VersionsCard
              installedVersions={installedVersions}
              databaseToolName={databaseToolName}
              isAdminerSelected={databaseTool === "adminer"}
              phpUpdate={phpUpdate}
              phpUpdating={phpUpdating}
              operationBusy={operationBusy}
              onUpdatePhp={updatePhp}
            />
          </aside>
        </div>
      </main>

      <StatusBar
        runningCount={runningCount}
        totalCount={totalCount}
        appPaths={appPaths || undefined}
        data-testid="status-bar"
      />

      {showSettings && (
        <SettingsPanel
          onClose={() => setShowSettings(false)}
          onSettingsChanged={() => {
            refreshAfterChange();
            checkPhpUpdate();
          }}
        />
      )}

      {showHelp && (
        <HelpModal
          onClose={() => setShowHelp(false)}
          onOpenGuide={() => {
            setShowHelp(false);
            openGuide();
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
