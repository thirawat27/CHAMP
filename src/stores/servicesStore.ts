/**
 * Services Store - Zustand
 * 
 * Manages service states and operations using Zustand for global state management.
 * Provides actions for controlling services and automatic status polling.
 */

import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { ServiceMap, ServiceState, ServiceType } from "../types/services";

interface ServicesState {
  /** Current state of all services */
  services: Partial<ServiceMap>;
  /** Current busy operation identifier */
  busy: string | null;
  /** Auto-refresh interval ID */
  refreshInterval: number | null;
  
  // Actions
  /** Set services state */
  setServices: (services: Partial<ServiceMap>) => void;
  /** Set busy state */
  setBusy: (busy: string | null) => void;
  /** Refresh service statuses from backend */
  refreshStatuses: () => Promise<void>;
  /** Start auto-refresh polling */
  startAutoRefresh: (interval?: number) => void;
  /** Stop auto-refresh polling */
  stopAutoRefresh: () => void;
  /** Execute a stack-wide command */
  runStackCommand: (
    command: "start_all_services" | "stop_all_services" | "restart_all_services"
  ) => Promise<ServiceMap>;
  /** Execute a command on a specific service */
  runServiceCommand: (
    command: "start_service" | "stop_service" | "restart_service",
    service: ServiceType
  ) => Promise<ServiceMap>;
  /** Mark all services as transitioning */
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
 * Services store for managing service states and operations
 */
export const useServicesStore = create<ServicesState>((set, get) => ({
  services: {},
  busy: null,
  refreshInterval: null,

  setServices: (services) => set({ services }),

  setBusy: (busy) => set({ busy }),

  refreshStatuses: async () => {
    try {
      const statuses = await invoke<ServiceMap>("get_all_statuses");
      set({ services: statuses });
    } catch (error) {
      console.error("Failed to get service statuses:", error);
    }
  },

  startAutoRefresh: (interval = 2000) => {
    const { refreshInterval, stopAutoRefresh, refreshStatuses } = get();
    
    // Stop existing interval if any
    if (refreshInterval !== null) {
      stopAutoRefresh();
    }

    // Start initial refresh
    refreshStatuses();

    // Set up interval
    const id = window.setInterval(() => {
      refreshStatuses();
    }, interval);

    set({ refreshInterval: id });
  },

  stopAutoRefresh: () => {
    const { refreshInterval } = get();
    if (refreshInterval !== null) {
      window.clearInterval(refreshInterval);
      set({ refreshInterval: null });
    }
  },

  markStackTransition: (command) => {
    const transitionState =
      command === "stop_all_services" ? ServiceState.Stopping : ServiceState.Starting;

    set((state) => {
      const next = { ...state.services };
      for (const serviceType of [ServiceType.Caddy, ServiceType.PhpFpm, ServiceType.MySQL]) {
        const service = next[serviceType];
        if (!service) continue;
        next[serviceType] = {
          ...service,
          state: transitionState,
          error_message: undefined,
        };
      }
      return { services: next };
    });
  },

  markServiceTransition: (command, service) => {
    const transitionState =
      command === "stop_service" ? ServiceState.Stopping : ServiceState.Starting;

    set((state) => {
      const selected = state.services[service];
      if (!selected) return state;
      return {
        services: {
          ...state.services,
          [service]: {
            ...selected,
            state: transitionState,
            error_message: undefined,
          },
        },
      };
    });
  },

  runStackCommand: async (command) => {
    const { markStackTransition } = get();
    markStackTransition(command);
    const statuses = await invoke<ServiceMap>(command);
    set({ services: statuses });
    return statuses;
  },

  runServiceCommand: async (command, service) => {
    const { markServiceTransition } = get();
    markServiceTransition(command, service);
    const statuses = await invoke<ServiceMap>(command, { service });
    set({ services: statuses });
    return statuses;
  },
}));
