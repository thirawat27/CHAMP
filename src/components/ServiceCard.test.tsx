/**
 * ServiceCard Component Tests
 * Phase 3: Process Manager UI
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ServiceCard } from './ServiceCard';
import { ServiceType, ServiceState } from '../types/services';

describe('ServiceCard Component', () => {
  const mockHandlers = {
    onStart: vi.fn(),
    onStop: vi.fn(),
    onRestart: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('TC-PM-UI-01: Rendering', () => {
    it('should display service name', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Stopped}
          port={8080}
          {...mockHandlers}
        />
      );
      expect(screen.getByText('Caddy')).toBeInTheDocument();
    });

    it('should display service description', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Stopped}
          port={8080}
          {...mockHandlers}
        />
      );
      expect(screen.getByText('Web Server')).toBeInTheDocument();
    });

    it('should display service port', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Stopped}
          port={8080}
          {...mockHandlers}
        />
      );
      expect(screen.getByText('Port: 8080')).toBeInTheDocument();
    });

    it('should have proper data-testid attributes for testing', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Stopped}
          port={8080}
          {...mockHandlers}
        />
      );
      expect(screen.getByTestId('service-card-caddy')).toBeInTheDocument();
      expect(screen.getByTestId('service-state-caddy')).toBeInTheDocument();
      expect(screen.getByTestId('start-button-caddy')).toBeInTheDocument();
    });
  });

  describe('TC-PM-UI-02: Stopped State', () => {
    it('should show gray badge when stopped', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Stopped}
          port={8080}
          {...mockHandlers}
        />
      );

      const badge = screen.getByTestId('service-state-caddy');
      expect(badge).toHaveTextContent('stopped');
      expect(badge).toHaveClass('status-gray');
    });

    it('should enable Start button when stopped', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Stopped}
          port={8080}
          {...mockHandlers}
        />
      );

      const startButton = screen.getByTestId('start-button-caddy');
      expect(startButton).not.toBeDisabled();
    });

    it('should disable Stop and Restart buttons when stopped', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Stopped}
          port={8080}
          {...mockHandlers}
        />
      );

      expect(screen.queryByTestId('stop-button-caddy')).not.toBeInTheDocument();
      expect(screen.queryByTestId('restart-button-caddy')).not.toBeInTheDocument();
    });

    it('should call onStart when Start button is clicked', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Stopped}
          port={8080}
          {...mockHandlers}
        />
      );

      const startButton = screen.getByTestId('start-button-caddy');
      fireEvent.click(startButton);
      expect(mockHandlers.onStart).toHaveBeenCalledTimes(1);
    });
  });

  describe('TC-PM-UI-03: Running State', () => {
    it('should show green badge when running', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Running}
          port={8080}
          {...mockHandlers}
        />
      );

      const badge = screen.getByTestId('service-state-caddy');
      expect(badge).toHaveTextContent('running');
      expect(badge).toHaveClass('status-green');
    });

    it('should disable Start button when running', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Running}
          port={8080}
          {...mockHandlers}
        />
      );

      expect(screen.queryByTestId('start-button-caddy')).not.toBeInTheDocument();
    });

    it('should enable Stop and Restart buttons when running', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Running}
          port={8080}
          {...mockHandlers}
        />
      );

      const stopButton = screen.getByTestId('stop-button-caddy');
      const restartButton = screen.getByTestId('restart-button-caddy');
      expect(stopButton).not.toBeDisabled();
      expect(restartButton).not.toBeDisabled();
    });

    it('should call onStop when Stop button is clicked', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Running}
          port={8080}
          {...mockHandlers}
        />
      );

      const stopButton = screen.getByTestId('stop-button-caddy');
      fireEvent.click(stopButton);
      expect(mockHandlers.onStop).toHaveBeenCalledTimes(1);
    });

    it('should call onRestart when Restart button is clicked', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Running}
          port={8080}
          {...mockHandlers}
        />
      );

      const restartButton = screen.getByTestId('restart-button-caddy');
      fireEvent.click(restartButton);
      expect(mockHandlers.onRestart).toHaveBeenCalledTimes(1);
    });
  });

  describe('TC-PM-UI-04: Starting State', () => {
    it('should show blue badge when starting', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Starting}
          port={8080}
          {...mockHandlers}
        />
      );

      const badge = screen.getByTestId('service-state-caddy');
      expect(badge).toHaveTextContent('starting');
      expect(badge).toHaveClass('status-blue');
    });

    it('should disable Start button when starting', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Starting}
          port={8080}
          {...mockHandlers}
        />
      );

      const startButton = screen.getByTestId('start-button-caddy');
      expect(startButton).toBeDisabled();
    });
  });

  describe('TC-PM-UI-05: Stopping State', () => {
    it('should show orange badge when stopping', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Stopping}
          port={8080}
          {...mockHandlers}
        />
      );

      const badge = screen.getByTestId('service-state-caddy');
      expect(badge).toHaveTextContent('stopping');
      expect(badge).toHaveClass('status-orange');
    });

    it('should disable Start button when stopping', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Stopping}
          port={8080}
          {...mockHandlers}
        />
      );

      // When stopping, isRunning is false, so Start button is shown but disabled
      const startButton = screen.getByTestId('start-button-caddy');
      expect(startButton).toBeDisabled();
    });
  });

  describe('TC-PM-UI-06: Error State', () => {
    it('should show red badge when in error state', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Error}
          port={8080}
          error="Port 8080 already in use"
          {...mockHandlers}
        />
      );

      const badge = screen.getByTestId('service-state-caddy');
      expect(badge).toHaveTextContent('error');
      expect(badge).toHaveClass('status-red');
    });

    it('should display error message', () => {
      const errorMessage = 'Port 8080 already in use';
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Error}
          port={8080}
          error={errorMessage}
          {...mockHandlers}
        />
      );

      expect(screen.getByText(errorMessage)).toBeInTheDocument();
    });

    it('should show Start button for retry when in error state', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Error}
          port={8080}
          error="Failed to start"
          {...mockHandlers}
        />
      );

      const startButton = screen.getByTestId('start-button-caddy');
      expect(startButton).not.toBeDisabled();
    });
  });

  describe('TC-PM-UI-07: All Service Types', () => {
    it('should render Caddy service card correctly', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.Caddy}
          state={ServiceState.Stopped}
          port={8080}
          {...mockHandlers}
        />
      );

      expect(screen.getByText('Caddy')).toBeInTheDocument();
      expect(screen.getByText('Port: 8080')).toBeInTheDocument();
      expect(screen.getByText('Web Server')).toBeInTheDocument();
    });

    it('should render PHP-FPM service card correctly', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.PhpFpm}
          state={ServiceState.Stopped}
          port={9000}
          {...mockHandlers}
        />
      );

      expect(screen.getByText('PHP-FPM')).toBeInTheDocument();
      expect(screen.getByText('Port: 9000')).toBeInTheDocument();
      expect(screen.getByText('PHP Runtime')).toBeInTheDocument();
    });

    it('should render MySQL service card correctly', () => {
      render(
        <ServiceCard
          serviceType={ServiceType.MySQL}
          state={ServiceState.Stopped}
          port={3307}
          {...mockHandlers}
        />
      );

      expect(screen.getByText('MySQL')).toBeInTheDocument();
      expect(screen.getByText('Port: 3307')).toBeInTheDocument();
      expect(screen.getByText('Database Server')).toBeInTheDocument();
    });
  });
});
