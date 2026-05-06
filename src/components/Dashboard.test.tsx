/**
 * Dashboard Component Tests
 * Phase 3: Process Manager UI
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { Dashboard } from "./Dashboard";
import { ServiceType, ServiceState } from "../types/services";

// Mock Tauri API
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("Dashboard Component", () => {
  const mockServiceMap = {
    [ServiceType.Caddy]: {
      service_type: ServiceType.Caddy,
      state: ServiceState.Stopped,
      port: 8080,
      error_message: null,
    },
    [ServiceType.PhpFpm]: {
      service_type: ServiceType.PhpFpm,
      state: ServiceState.Stopped,
      port: 9000,
      error_message: null,
    },
    [ServiceType.MySQL]: {
      service_type: ServiceType.MySQL,
      state: ServiceState.Stopped,
      port: 3307,
      error_message: null,
    },
  };
  const mockAppPaths = {
    base_dir: "C:\\CHAMP",
    runtime_dir: "C:\\CHAMP\\runtime",
    config_dir: "C:\\CHAMP\\config",
    mysql_data_dir: "C:\\CHAMP\\mysql\\data",
    logs_dir: "C:\\CHAMP\\logs",
    projects_dir: "C:\\CHAMP\\projects",
  };
  const mockSystemMetrics = {
    cpu_usage: 12.5,
    memory_used_bytes: 4 * 1024 * 1024 * 1024,
    memory_total_bytes: 16 * 1024 * 1024 * 1024,
    network_rx_bps: 128_000,
    network_tx_bps: 64_000,
  };

  beforeEach(() => {
    vi.clearAllMocks();
    // Don't use fake timers - they cause issues with async/await in tests
  });

  afterEach(() => {
    vi.clearAllTimers();
  });

  describe("TC-PM-DASH-01: Initial Display", () => {
    it("should render all three service cards", async () => {
      vi.mocked(invoke).mockResolvedValue(mockServiceMap);

      render(<Dashboard />);

      // Wait for initial fetch
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith("get_all_statuses");
      });

      // Check service cards are rendered
      expect(screen.getByTestId("service-card-caddy")).toBeInTheDocument();
      expect(screen.getByTestId("service-card-php-fpm")).toBeInTheDocument();
      expect(screen.getByTestId("service-card-mysql")).toBeInTheDocument();
    });

    it("should fetch service statuses on mount", async () => {
      vi.mocked(invoke).mockResolvedValue(mockServiceMap);

      render(<Dashboard />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith("get_all_statuses");
      });
    });

    it("should display status bar", async () => {
      vi.mocked(invoke).mockResolvedValue(mockServiceMap);

      render(<Dashboard />);

      await waitFor(() => {
        const statusBar = screen.getByTestId("status-bar");
        expect(statusBar).toBeInTheDocument();
      });
    });
  });

  describe("TC-PM-DASH-02: Status Refresh", () => {
    it("should call get_all_statuses multiple times", async () => {
      vi.mocked(invoke).mockResolvedValue(mockServiceMap);

      render(<Dashboard />);

      // Wait for initial fetch
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith("get_all_statuses");
      });

      // Wait a bit and check that it was called again (due to interval)
      await waitFor(
        () => {
          expect(vi.mocked(invoke).mock.calls.length).toBeGreaterThan(1);
        },
        { timeout: 3000 }
      );
    });
  });

  describe("TC-PM-DASH-03: Start Service", () => {
    it("should call start_service when Start button is clicked", async () => {
      vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        switch (cmd) {
          case "get_all_statuses":
            return mockServiceMap;
          case "start_service":
            return mockServiceMap;
          case "get_app_paths":
            return mockAppPaths;
          case "get_installed_versions":
            return {};
          case "get_system_metrics":
            return mockSystemMetrics;
          default:
            return {};
        }
      });

      render(<Dashboard />);

      await waitFor(() => {
        const startButton = screen.getByTestId("start-button-caddy");
        fireEvent.click(startButton);
      });

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith("start_service", { service: ServiceType.Caddy });
      });
    });
  });

  describe("TC-PM-DASH-04: Stop Service", () => {
    it("should call stop_service when Stop button is clicked", async () => {
      const runningMap = {
        ...mockServiceMap,
        [ServiceType.Caddy]: {
          ...mockServiceMap[ServiceType.Caddy],
          state: ServiceState.Running,
        },
      };

      vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        switch (cmd) {
          case "get_all_statuses":
            return runningMap;
          case "stop_service":
            return runningMap;
          case "get_app_paths":
            return mockAppPaths;
          case "get_installed_versions":
            return {};
          case "get_system_metrics":
            return mockSystemMetrics;
          default:
            return {};
        }
      });

      render(<Dashboard />);

      await waitFor(() => {
        const stopButton = screen.getByTestId("stop-button-caddy");
        fireEvent.click(stopButton);
      });

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith("stop_service", { service: ServiceType.Caddy });
      });
    });
  });

  describe("TC-PM-DASH-05: Restart Service", () => {
    it("should call restart_service when Restart button is clicked", async () => {
      const runningMap = {
        ...mockServiceMap,
        [ServiceType.Caddy]: {
          ...mockServiceMap[ServiceType.Caddy],
          state: ServiceState.Running,
        },
      };

      vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        switch (cmd) {
          case "get_all_statuses":
            return runningMap;
          case "restart_service":
            return runningMap;
          case "get_app_paths":
            return mockAppPaths;
          case "get_installed_versions":
            return {};
          case "get_system_metrics":
            return mockSystemMetrics;
          default:
            return {};
        }
      });

      render(<Dashboard />);

      await waitFor(() => {
        const restartButton = screen.getByTestId("restart-button-caddy");
        fireEvent.click(restartButton);
      });

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith("restart_service", { service: ServiceType.Caddy });
      });
    });
  });

  describe("TC-PM-DASH-06: Error Handling", () => {
    it("should display error when service fails to start", async () => {
      const errorMessage = "Port 8080 already in use";
      vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        if (cmd === "get_all_statuses") {
          return {
            ...mockServiceMap,
            [ServiceType.Caddy]: {
              ...mockServiceMap[ServiceType.Caddy],
              state: ServiceState.Error,
              error_message: errorMessage,
            },
          };
        }
        return { success: false, error: errorMessage };
      });

      render(<Dashboard />);

      await waitFor(() => {
        expect(screen.getByText(errorMessage)).toBeInTheDocument();
      });
    });

    it("should show error state badge when service fails", async () => {
      vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        if (cmd === "get_all_statuses") {
          return {
            ...mockServiceMap,
            [ServiceType.PhpFpm]: {
              ...mockServiceMap[ServiceType.PhpFpm],
              state: ServiceState.Error,
              error_message: "Failed to start",
            },
          };
        }
        return {};
      });

      render(<Dashboard />);

      await waitFor(() => {
        const badge = screen.getByTestId("service-state-php-fpm");
        expect(badge).toHaveTextContent("error");
      });
    });
  });

  describe("TC-PM-DASH-07: Quick Actions", () => {
    it("should display version information", async () => {
      vi.mocked(invoke).mockResolvedValue(mockServiceMap);

      render(<Dashboard />);

      await waitFor(() => {
        expect(screen.getByText("CHAMP v1.0.0")).toBeInTheDocument();
      });
    });
  });

  describe("TC-PM-DASH-08: All Services Running", () => {
    it("should show all services as running when all started", async () => {
      const allRunningMap = {
        [ServiceType.Caddy]: {
          ...mockServiceMap[ServiceType.Caddy],
          state: ServiceState.Running,
        },
        [ServiceType.PhpFpm]: {
          ...mockServiceMap[ServiceType.PhpFpm],
          state: ServiceState.Running,
        },
        [ServiceType.MySQL]: {
          ...mockServiceMap[ServiceType.MySQL],
          state: ServiceState.Running,
        },
      };

      vi.mocked(invoke).mockResolvedValue(allRunningMap);

      render(<Dashboard />);

      await waitFor(() => {
        expect(screen.getByTestId("service-state-caddy")).toHaveTextContent("running");
        expect(screen.getByTestId("service-state-php-fpm")).toHaveTextContent("running");
        expect(screen.getByTestId("service-state-mysql")).toHaveTextContent("running");
      });
    });
  });
});
