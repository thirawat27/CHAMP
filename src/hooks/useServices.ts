import { invoke } from "@tauri-apps/api/core";
import { useCallback, useMemo, useRef, useState } from "react";
import { getTranslations } from "../stores/preferencesStore";
import { AudioManager } from "../utils/audioManager";
import { DashboardNotice } from "../types/dashboard";
import {
  ServiceCommand,
  SERVICE_COMMAND_COPY,
  StackCommand,
  STACK_COMMAND_COPY,
} from "./commandCopy";
import {
  ALL_SERVICE_TYPES,
  AppSettings,
  DEFAULT_PORTS,
  PackageSelection,
  SERVICE_DISPLAY_NAMES,
  ServiceMap,
  ServiceState,
  ServiceType,
  getStackServiceTypes,
} from "../types/services";

interface UseServicesOptions {
  settings: AppSettings | null;
  notify: (notice: DashboardNotice) => void;
}

interface ServicesController {
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
  /** Resolves true when the command succeeded. */
  runStackCommand: (command: StackCommand) => Promise<boolean>;
  runServiceCommand: (command: ServiceCommand, service: ServiceType) => Promise<void>;
}

/** Polls return a new object every tick; keep the old one when nothing changed. */
function sameServices(current: Partial<ServiceMap>, next: ServiceMap): boolean {
  if (!next) return false;
  const keys = Object.keys(next) as ServiceType[];
  if (keys.length !== Object.keys(current).length) return false;
  return keys.every((key) => {
    const a = current[key];
    const b = next[key];
    return a?.state === b?.state && a?.port === b?.port && a?.error_message === b?.error_message;
  });
}

/**
 * Owns service status state and the stack/service command lifecycle: one
 * command at a time, optimistic transition states, port fallback messaging, and
 * busy flags that disable the relevant buttons.
 */
export function useServices({ settings, notify }: UseServicesOptions): ServicesController {
  const [services, setServices] = useState<Partial<ServiceMap>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const statusRefreshInFlight = useRef(false);
  // Single-flight guard for commands: read synchronously, unlike `busy`.
  const commandRunning = useRef(false);
  // A poll that started before a command must not overwrite the command's state.
  const statusGeneration = useRef(0);

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
      [ServiceType.PostgreSQL]: settings?.postgresql_port ?? DEFAULT_PORTS[ServiceType.PostgreSQL],
    }),
    [settings]
  );

  const refreshStatuses = useCallback(async () => {
    if (statusRefreshInFlight.current) return;
    statusRefreshInFlight.current = true;
    const generation = statusGeneration.current;
    try {
      const statuses = await invoke<ServiceMap>("get_all_statuses");
      if (generation === statusGeneration.current) {
        setServices((current) => (sameServices(current, statuses) ? current : statuses));
      }
    } catch (error) {
      console.error("Failed to refresh dashboard status:", error);
    } finally {
      statusRefreshInFlight.current = false;
    }
  }, []);

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

  const markServiceTransition = useCallback((command: ServiceCommand, service: ServiceType) => {
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
  }, []);

  const fallbackPortMessage = useCallback(
    (statuses: ServiceMap, fallbackMessage: string) => {
      const changedPorts = ALL_SERVICE_TYPES.map((serviceType) => {
        const service = statuses[serviceType];
        const expectedPort = expectedPorts[serviceType];
        if (!service || service.port === expectedPort) return null;
        return `${SERVICE_DISPLAY_NAMES[serviceType]} ${service.port}`;
      }).filter((value): value is string => Boolean(value));

      if (changedPorts.length === 0) {
        return fallbackMessage;
      }

      return getTranslations().fallbackPortsUsed.replace("{ports}", changedPorts.join(", "));
    },
    [expectedPorts]
  );

  const runStackCommand = useCallback(
    async (command: StackCommand) => {
      if (commandRunning.current) return false;
      commandRunning.current = true;
      statusGeneration.current += 1;
      const t = getTranslations();
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
        statusGeneration.current += 1;
        setServices(statuses);
        AudioManager.playNotification("success", copy.action);
        notify({
          tone: "success",
          action: copy.action,
          title: t[copy.successTitleKey],
          message: fallbackPortMessage(statuses, t[copy.successMessageKey]),
        });
        return true;
      } catch (error) {
        AudioManager.playNotification("error");
        notify({
          tone: "error",
          title: t.commandFailed,
          message: String(error),
        });
        statusGeneration.current += 1;
        await refreshStatuses();
        return false;
      } finally {
        commandRunning.current = false;
        setBusy(null);
      }
    },
    [fallbackPortMessage, markStackTransition, notify, refreshStatuses]
  );

  const runServiceCommand = useCallback(
    async (command: ServiceCommand, service: ServiceType) => {
      if (commandRunning.current) return;
      commandRunning.current = true;
      statusGeneration.current += 1;
      const t = getTranslations();
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
        statusGeneration.current += 1;
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
        statusGeneration.current += 1;
        await refreshStatuses();
      } finally {
        commandRunning.current = false;
        setBusy(null);
      }
    },
    [fallbackPortMessage, markServiceTransition, notify, refreshStatuses]
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
