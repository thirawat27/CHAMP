/**
 * Mock utilities for testing CAMPP UI
 * Provides mock data and functions for testing without actual runtime binaries
 */

import { vi } from 'vitest';
import type { ServiceMap, ServiceInfo, ServiceType, ServiceState } from '../../types/services';

/**
 * Create a mock service info object
 */
export function createMockServiceInfo(
  overrides: Partial<ServiceInfo> = {}
): ServiceInfo {
  return {
    name: 'Caddy',
    displayName: 'Caddy',
    description: 'Web server',
    port: 8080,
    state: 'Stopped' as ServiceState,
    error_message: null,
    ...overrides,
  };
}

/**
 * Create mock service map with all services
 */
export function createMockServiceMap(
  overrides: Partial<ServiceMap> = {}
): ServiceMap {
  return {
    Caddy: createMockServiceInfo({
      name: 'Caddy',
      displayName: 'Caddy',
      description: 'Web server',
      port: 8080,
    }),
    PhpFpm: createMockServiceInfo({
      name: 'PhpFpm',
      displayName: 'PHP-FPM',
      description: 'PHP runtime',
      port: 9000,
    }),
    MySQL: createMockServiceInfo({
      name: 'MySQL',
      displayName: 'MySQL',
      description: 'Database',
      port: 3307,
    }),
    ...overrides,
  };
}

/**
 * Create mock service map with specific states
 */
export function createMockServiceMapWithStates(
  caddyState: ServiceState = 'Stopped',
  phpState: ServiceState = 'Stopped',
  mysqlState: ServiceState = 'Stopped'
): ServiceMap {
  return {
    Caddy: createMockServiceInfo({
      name: 'Caddy',
      displayName: 'Caddy',
      description: 'Web server',
      port: 8080,
      state: caddyState,
    }),
    PhpFpm: createMockServiceInfo({
      name: 'PhpFpm',
      displayName: 'PHP-FPM',
      description: 'PHP runtime',
      port: 9000,
      state: phpState,
    }),
    MySQL: createMockServiceInfo({
      name: 'MySQL',
      displayName: 'MySQL',
      description: 'Database',
      port: 3307,
      state: mysqlState,
    }),
  };
}

/**
 * Mock Tauri invoke function
 */
export function createMockInvoke() {
  const invoke = vi.fn();

  // Mock return values for common commands
  invoke.mockImplementation(async (cmd: string, args?: unknown) => {
    switch (cmd) {
      case 'get_all_statuses':
        return createMockServiceMap();

      case 'get_service_status':
        return createMockServiceInfo({ name: args as ServiceType });

      case 'start_service':
        // Simulate service starting
        await new Promise(resolve => setTimeout(resolve, 100));
        return { success: true };

      case 'stop_service':
        // Simulate service stopping
        await new Promise(resolve => setTimeout(resolve, 100));
        return { success: true };

      case 'restart_service':
        // Simulate service restart
        await new Promise(resolve => setTimeout(resolve, 200));
        return { success: true };

      case 'check_runtime_installed':
        return false; // Default to not installed

      case 'get_settings':
        return {
          web_port: 8080,
          mysql_port: 3307,
          php_port: 9000,
        };

      default:
        return undefined;
    }
  });

  return invoke;
}

/**
 * Wait for async operations to complete
 */
export function waitFor(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Helper to get service state text
 */
export function getServiceStateText(state: ServiceState): string {
  const stateMap: Record<ServiceState, string> = {
    Stopped: 'Stopped',
    Starting: 'Starting',
    Running: 'Running',
    Stopping: 'Stopping',
    Error: 'Error',
  };
  return stateMap[state] || state;
}

/**
 * Helper to get service state color class
 */
export function getServiceStateColor(state: ServiceState): string {
  const colorMap: Record<ServiceState, string> = {
    Stopped: 'text-gray-500',
    Starting: 'text-blue-500',
    Running: 'text-green-500',
    Stopping: 'text-orange-500',
    Error: 'text-red-500',
  };
  return colorMap[state] || 'text-gray-500';
}
