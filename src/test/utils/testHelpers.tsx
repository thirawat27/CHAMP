/**
 * Test helper utilities for CAMPP UI testing
 */

import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { vi, expect } from 'vitest';

/**
 * Render a component with default Tauri mocks
 */
export function renderWithTauri(ui: React.ReactElement) {
  return render(ui);
}

/**
 * Find a service card by service name
 */
export function findServiceCard(serviceName: string) {
  return screen.getByTestId(`service-card-${serviceName}`);
}

/**
 * Find a service state badge
 */
export function findServiceStateBadge(serviceName: string) {
  return screen.getByTestId(`service-state-${serviceName}`);
}

/**
 * Find a service start button
 */
export function findStartButton(serviceName: string) {
  return screen.getByTestId(`start-button-${serviceName}`);
}

/**
 * Find a service stop button
 */
export function findStopButton(serviceName: string) {
  return screen.getByTestId(`stop-button-${serviceName}`);
}

/**
 * Find a service restart button
 */
export function findRestartButton(serviceName: string) {
  return screen.getByTestId(`restart-button-${serviceName}`);
}

/**
 * Wait for service state to change
 */
export async function waitForServiceState(
  serviceName: string,
  expectedState: string,
  timeout = 3000
) {
  await waitFor(
    () => {
      const badge = findServiceStateBadge(serviceName);
      expect(badge).toHaveTextContent(expectedState);
    },
    { timeout }
  );
}

/**
 * Click a service button and wait for result
 */
export async function clickServiceButton(
  serviceName: string,
  buttonType: 'start' | 'stop' | 'restart'
) {
  const buttonMap = {
    start: () => findStartButton(serviceName),
    stop: () => findStopButton(serviceName),
    restart: () => findRestartButton(serviceName),
  };

  const button = buttonMap[buttonType]();
  fireEvent.click(button);

  // Wait for the Tauri command to be called
  await waitFor(() => {
    expect(invoke).toHaveBeenCalled();
  });
}

/**
 * Mock service status response
 */
export function mockServiceStatus(serviceName: string, state: string) {
  vi.mocked(invoke).mockResolvedValue({
    name: serviceName,
    displayName: serviceName,
    port: 8080,
    state,
    error_message: null,
  });
}

/**
 * Mock all service statuses
 */
export function mockAllServiceStatuses(statuses: Record<string, string>) {
  vi.mocked(invoke).mockResolvedValue({
    Caddy: {
      name: 'Caddy',
      displayName: 'Caddy',
      port: 8080,
      state: statuses.Caddy || 'Stopped',
      error_message: null,
    },
    PhpFpm: {
      name: 'PhpFpm',
      displayName: 'PHP-FPM',
      port: 9000,
      state: statuses.PhpFpm || 'Stopped',
      error_message: null,
    },
    MySQL: {
      name: 'MySQL',
      displayName: 'MySQL',
      port: 3307,
      state: statuses.MySQL || 'Stopped',
      error_message: null,
    },
  });
}

/**
 * Helper to simulate service state transition
 */
export async function simulateServiceTransition(
  _fromState: string,
  toState: string,
  delay = 100
) {
  // Reset mock to return intermediate state
  await new Promise(resolve => setTimeout(resolve, delay / 2));

  // Return final state
  return toState;
}
