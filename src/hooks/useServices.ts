import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Translations } from "../i18n/translations";
import { AudioManager } from "../utils/audioManager";
import {
  ServiceCommand,
  SERVICE_COMMAND_COPY,
  StackCommand,
  STACK_COMMAND_COPY,
} from "../components/dashboard/types";
import {
  AppSettings,
  DEFAULT_PORTS,
  PackageSelection,
  SERVICE_DISPLAY_NAMES,
  ServiceMap,
  ServiceState,
  ServiceType,
  getStackServiceTypes,
} from "../types/services";
import { DashboardNotice } from "../components/dashboard/types";

interface UseServicesOptions {
  t: Translations;
  settings: AppSettings | null;
  notify: (notice: DashboardNotice) => void;
  /**
   * Refreshes HTTPS tunnel status. Runs alongside `get_all_statuses` on mount
   * and on every visibility-gated poll tick, matching the original combined
   * fetch behaviour.
   */
  refreshTunnel: () => Promise<void>;
  /** Invoked after a successful `stop_all_services` so the tunnel panel resets. */
  resetTunnel: () => void;
}

export interface ServicesController {
  services: Partial<ServiceMap>;
  busy: string | null;
  busyStackCommand: StackCommand | null;
  runningCount: number;
  totalCount: number;
  allRunning: boolean;
  isCaddyRunning: boolean;
  stackServiceTypes: ServiceType[];
  packageSelection: PackageSelection | null;
  refreshStatuses: () => Promise<void>;
  runStackCommand: (command: StackCommand) => Promise<void>;
  runServiceCommand: (command: ServiceCommand, service: ServiceType) => Promise<void>;
}

/**
 * Owns service status state and the stack/service command lifecycle.
 *
 * Preserves the original dashboard behaviour: a single in-flight guard for the
 * 3.5s visibility-gated status poll, optimistic transition states, port
 * fallback messaging, and busy flags that disable the relevant buttons.
 */
export function useServices({
  t,
  settings,
  notify,
  refreshTunnel,
  resetTunnel,
}: UseServicesOptions): ServicesController {
  const [services, setServices] = useState<Partial<ServiceMap>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const statusRefreshInFlight = useRef(false);

  const packageSelection = settings?.package_selection ?? null;
  const stackServiceTypes = useMemo(
    () => getStackServiceTypes(packageSelection),
    [packageSelection]
  );
  const runningCount = stackServiceTypes.filter(
    (serviceType) => services[serviceType]?.state === ServiceState.Running
  ).length;
  const totalCount = stackServiceTypes.length;
  const isCaddyRunning = services[ServiceType.Caddy]?.state === ServiceState.Running;
  const allRunning = runningCount === totalCount;
  const busyStackCommand = busy?.startsWith("stack:")
    ? (busy.slice("stack:".length) as StackCommand)
    : null;

  const expectedPorts = useMemo(
    () => ({
      [ServiceType.Caddy]: settings?.web_port ?? DEFAULT_PORTS[ServiceType.Caddy],
      [ServiceType.PhpFpm]: settings?.php_port ?? DEFAULT_PORTS[ServiceType.PhpFpm],
      [ServiceType.MySQL]: settings?.mysql_port ?? DEFAULT_PORTS[ServiceType.MySQL],
      [ServiceType.PostgreSQL]:
        settings?.postgresql_port ?? DEFAULT_PORTS[ServiceType.PostgreSQL],
    }),
    [settings]
  );

  const refreshStatuses = useCallback(async () => {
    if (statusRefreshInFlight.current) return;
    statusRefreshInFlight.current = true;
    try {
      const [statuses] = await Promise.all([
        invoke<ServiceMap>("get_all_statuses"),
        refreshTunnel(),
      ]);
      setServices(statuses);
    } catch (error) {
      console.error("Failed to refresh dashboard status:", error);
    } finally {
      statusRefreshInFlight.current = false;
    }
  }, [refreshTunnel]);

  useEffect(() => {
    refreshStatuses();
    const interval = window.setInterval(() => {
      if (document.visibilityState === "visible") {
        refreshStatuses();
      }
    }, 3500);
    return () => window.clearInterval(interval);
  }, [refreshStatuses]);

  const markStackTransition = useCallback(
    (command: StackCommand) => {
      const transitionState =
        command === "stop_all_services" ? ServiceState.Stopping : ServiceState.Starting;

      setServices((current) => {
        const next = { ...current };
        for (const serviceType of stackServiceTypes) {
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
    },
    [stackServiceTypes]
  );

  const markServiceTransition = useCallback(
    (command: ServiceCommand, service: ServiceType) => {
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
    },
    []
  );

  const fallbackPortMessage = useMemo(() => {
    return (statuses: ServiceMap, fallbackMessage: string) => {
      const changedPorts = [
        ServiceType.Caddy,
        ServiceType.PhpFpm,
        ServiceType.MySQL,
        ServiceType.PostgreSQL,
      ]
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

      return t.fallbackPortsUsed.replace("{ports}", changedPorts.join(", "));
    };
  }, [expectedPorts, t]);

  const runStackCommand = useCallback(
    async (command: StackCommand) => {
      const copy = STACK_COMMAND_COPY[command];
      setBusy(`stack:${command}`);
      notify({
        tone: "info",
        action: copy.action,
        title: t[copy.pendingTitleKey],
        message: t[copy.pendingMessageKey],
      });
      markStackTransition(command);
      try {
        const statuses = await invoke<ServiceMap>(command);
        setServices(statuses);
        if (command === "stop_all_services") {
          resetTunnel();
        }
        AudioManager.playNotification("success", copy.action);
        notify({
          tone: "success",
          action: copy.action,
          title: t[copy.successTitleKey],
          message: fallbackPortMessage(statuses, t[copy.successMessageKey]),
        });
      } catch (error) {
        AudioManager.playNotification("error");
        notify({
          tone: "error",
          title: t.commandFailed,
          message: String(error),
        });
        await refreshStatuses();
      } finally {
        setBusy(null);
      }
    },
    [fallbackPortMessage, markStackTransition, notify, refreshStatuses, resetTunnel, t]
  );

  const runServiceCommand = useCallback(
    async (command: ServiceCommand, service: ServiceType) => {
      const copy = SERVICE_COMMAND_COPY[command];
      const displayName = SERVICE_DISPLAY_NAMES[service];
      setBusy(`${command}:${service}`);
      notify({
        tone: "info",
        action: copy.action,
        title: `${t[copy.pendingTitleKey]}: ${displayName}`,
        message: t[copy.pendingMessageKey],
      });
      markServiceTransition(command, service);
      try {
        const statuses = await invoke<ServiceMap>(command, { service });
        setServices(statuses);
        AudioManager.playNotification("success", copy.action);
        notify({
          tone: "success",
          action: copy.action,
          title: `${t[copy.successTitleKey]}: ${displayName}`,
          message: fallbackPortMessage(statuses, t.dashboardRefreshingStatus),
        });
      } catch (error) {
        AudioManager.playNotification("error");
        notify({
          tone: "error",
          title: `${t.commandFailed}: ${displayName}`,
          message: String(error),
        });
        await refreshStatuses();
      } finally {
        setBusy(null);
      }
    },
    [fallbackPortMessage, markServiceTransition, notify, refreshStatuses, t]
  );

  return {
    services,
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
  };
}
