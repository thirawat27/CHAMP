/**
 * Custom hook for managing service states and operations
 * 
 * This hook encapsulates all service-related state management including:
 * - Service status polling
 * - Service control commands (start, stop, restart)
 * - Stack-wide operations
 * - Busy state tracking
 * 
 * @example
 * ```tsx
 * const { services, busy, runStackCommand, runServiceCommand, refreshStatuses } = useServices();
 * ```
 */

import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { ServiceMap, ServiceState, ServiceType } from "../types/services";

export interface UseServicesReturn {
  /** Current state of all services */
  services: Partial<ServiceMap>;
  /** Current busy operation identifier (e.g., "stack:start_all_services" or "start_service:caddy") */
  busy: string | null;
  /** Set the busy state */
  setBusy: (busy: string | null) => void;
  /** Refresh service statuses from backend */
  refreshStatuses: () => Promise<void>;
  /** Execute a stack-wide command (start/stop/restart all services) */
  runStackCommand: (
    command: "start_all_services" | "stop_all_services" | "restart_all_services"
  ) => Promise<ServiceMap>;
  /** Execute a command on a specific service */
  runServiceCommand: (
    command: "start_service" | "stop_service" | "restart_service",
    service: ServiceType
  ) => Promise<ServiceMap>;
  /** Mark all services as transitioning (starting or stopping) */
  markStackTransition: (
    command: "start_all_services" | "stop_all_services" | "restart_all_services"
  ) => void;
  /** Mark a specific service as transitioning */
  markServiceTransition: (
    command: "start_service" | "stop_service" | "restart_service",
    service: ServiceType
  ) => void;
}

/**
 * Hook for managing service states and operations
 * 
 * Automatically polls service statuses every 2 seconds and provides
 * methods for controlling services.
 * 
 * @param autoRefresh - Whether to automatically poll service statuses (default: true)
 * @param refreshInterval - Polling interval in milliseconds (default: 2000)
 * @returns Service management interface
 */
export function useServices(
  autoRefresh = true,
  refreshInterval = 2000
): UseServicesReturn {
  const [services, setServices] = useState<Partial<ServiceMap>>({});
  const [busy, setBusy] = useState<string | null>(null);

  /**
   * Fetch current service statuses from the backend
   */
  const refreshStatuses = useCallback(async () => {
    try {
      const statuses = await invoke<ServiceMap>("get_all_statuses");
      setServices(statuses);
    } catch (error) {
      console.error("Failed to get service statuses:", error);
    }
  }, []);

  /**
   * Mark all services as transitioning to a new state
   */
  const markStackTransition = useCallback(
    (command: "start_all_services" | "stop_all_services" | "restart_all_services") => {
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
    },
    []
  );

  /**
   * Mark a specific service as transitioning to a new state
   */
  const markServiceTransition = useCallback(
    (
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
    },
    []
  );

  /**
   * Execute a stack-wide command (affects all services)
   * 
   * @param command - The stack command to execute
   * @returns Promise resolving to updated service statuses
   * @throws Error if the command fails
   */
  const runStackCommand = useCallback(
    async (
      command: "start_all_services" | "stop_all_services" | "restart_all_services"
    ): Promise<ServiceMap> => {
      markStackTransition(command);
      const statuses = await invoke<ServiceMap>(command);
      setServices(statuses);
      return statuses;
    },
    [markStackTransition]
  );

  /**
   * Execute a command on a specific service
   * 
   * @param command - The service command to execute
   * @param service - The service to operate on
   * @returns Promise resolving to updated service statuses
   * @throws Error if the command fails
   */
  const runServiceCommand = useCallback(
    async (
      command: "start_service" | "stop_service" | "restart_service",
      service: ServiceType
    ): Promise<ServiceMap> => {
      markServiceTransition(command, service);
      const statuses = await invoke<ServiceMap>(command, { service });
      setServices(statuses);
      return statuses;
    },
    [markServiceTransition]
  );

  // Auto-refresh service statuses
  useEffect(() => {
    if (!autoRefresh) return undefined;

    refreshStatuses();
    const interval = window.setInterval(refreshStatuses, refreshInterval);
    return () => window.clearInterval(interval);
  }, [autoRefresh, refreshInterval, refreshStatuses]);

  return {
    services,
    busy,
    setBusy,
    refreshStatuses,
    runStackCommand,
    runServiceCommand,
    markStackTransition,
    markServiceTransition,
  };
}
