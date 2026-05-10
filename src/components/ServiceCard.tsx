/**
 * ServiceCard Component
 * 
 * Displays a service's status, port, and control buttons.
 * Shows real-time state updates and error messages.
 */

import { openUrl } from "@tauri-apps/plugin-opener";
import { Database, Globe2, LoaderCircle, Play, RefreshCw, Square, Terminal } from "lucide-react";
import { useTranslation } from "../stores/languageStore";
import { AudioManager } from "../utils/audioManager";
import { DEFAULT_PORTS, SERVICE_DESCRIPTIONS, SERVICE_DISPLAY_NAMES, ServiceState, ServiceType } from "../types/services";

/**
 * Props for the ServiceCard component
 */
interface ServiceCardProps {
  /** Type of service (Caddy, PHP-FPM, or MySQL) */
  serviceType: ServiceType;
  /** Current state of the service */
  state: ServiceState;
  /** Port number the service is running on */
  port?: number;
  /** Error message if service is in error state */
  error?: string;
  /** Whether the service is currently processing a command */
  busy?: boolean;
  /** Label to show when busy (e.g., "Starting...", "Stopping...") */
  busyLabel?: string;
  /** Callback when start button is clicked */
  onStart: () => void;
  /** Callback when stop button is clicked */
  onStop: () => void;
  /** Callback when restart button is clicked */
  onRestart: () => void;
  /** Additional HTML attributes */
  [key: string]: unknown;
}

/**
 * Icon mapping for each service type
 */
const SERVICE_ICONS = {
  [ServiceType.Caddy]: Globe2,
  [ServiceType.PhpFpm]: Terminal,
  [ServiceType.MySQL]: Database,
};

/**
 * Generate the service URL based on service type and port
 * 
 * @param serviceType - The type of service
 * @param port - The port number
 * @returns The service URL string
 */
function getServiceUrl(serviceType: ServiceType, port: number) {
  return {
    [ServiceType.Caddy]: `http://localhost:${port}`,
    [ServiceType.PhpFpm]: `tcp://127.0.0.1:${port}`,
    [ServiceType.MySQL]: `mysql://127.0.0.1:${port}`,
  }[serviceType];
}

/**
 * ServiceCard component displays a service's status and controls
 * 
 * Shows the service name, description, current state, port, and URL.
 * Provides buttons to start, stop, and restart the service.
 * Displays error messages when the service is in an error state.
 * 
 * @param props - Component props
 * @returns ServiceCard component
 */
export function ServiceCard({
  serviceType,
  state,
  port = DEFAULT_PORTS[serviceType],
  error,
  busy = false,
  busyLabel,
  onStart,
  onStop,
  onRestart,
  ...props
}: ServiceCardProps) {
  const { t } = useTranslation();
  const Icon = SERVICE_ICONS[serviceType];
  const displayName = SERVICE_DISPLAY_NAMES[serviceType];
  const description = SERVICE_DESCRIPTIONS[serviceType];
  const serviceUrl = getServiceUrl(serviceType, port);
  const isRunning = state === ServiceState.Running;
  const isTransitioning = busy || state === ServiceState.Starting || state === ServiceState.Stopping;
  const isError = state === ServiceState.Error;
  const actionLabel = busyLabel || (state === ServiceState.Stopping ? t.stopping : t.starting);
  const statusClass = {
    [ServiceState.Stopped]: "status-gray",
    [ServiceState.Starting]: "status-blue",
    [ServiceState.Running]: "status-green",
    [ServiceState.Stopping]: "status-orange",
    [ServiceState.Error]: "status-red",
  }[state];

  // Map service states to translation keys
  const stateTranslations: Record<ServiceState, string> = {
    [ServiceState.Stopped]: t.stopped,
    [ServiceState.Starting]: t.starting,
    [ServiceState.Running]: t.running,
    [ServiceState.Stopping]: t.stopping,
    [ServiceState.Error]: t.error,
  };

  return (
    <article className={`service-card ${isError ? "has-error" : ""}`} data-testid={`service-card-${serviceType}`} {...props}>
      <div className="service-card-header">
        <div className="service-identity">
          <span className="service-icon"><Icon size={18} /></span>
          <div>
            <h3>{displayName}</h3>
            <p>{description}</p>
          </div>
        </div>
        <span className={`status-pill ${state} ${statusClass}`} data-testid={`service-state-${serviceType}`}>{stateTranslations[state]}</span>
      </div>

      <div className="service-meta">
        <span>{t.port}: {port}</span>
        <span>
          URL:{" "}
          <button
            type="button"
            className="service-url-button"
            onClick={() => {
              AudioManager.playClick();
              openUrl(serviceUrl).catch((openError) => console.error("Failed to open service URL:", openError));
            }}
            onMouseEnter={() => AudioManager.playHover()}
          >
            {serviceUrl}
          </button>
        </span>
      </div>

      {isError && error && (
        <div className="service-error" title={error}>
          {error.length > 180 ? `${error.substring(0, 180)}...` : error}
        </div>
      )}

      <div className="service-actions">
        {!isRunning ? (
          <button 
            onClick={() => {
              AudioManager.playClick();
              onStart();
            }} 
            disabled={isTransitioning} 
            className="btn-start" 
            data-testid={`start-button-${serviceType}`}
            onMouseEnter={() => AudioManager.playHover()}
          >
            {isTransitioning ? <LoaderCircle size={15} className="spin-icon" /> : <Play size={15} />}
            {isTransitioning ? actionLabel : t.start}
          </button>
        ) : (
          <>
            <button 
              onClick={() => {
                AudioManager.playClick();
                onStop();
              }} 
              disabled={isTransitioning} 
              className="btn-stop" 
              data-testid={`stop-button-${serviceType}`}
              onMouseEnter={() => AudioManager.playHover()}
            >
              {isTransitioning ? <LoaderCircle size={14} className="spin-icon" /> : <Square size={14} />}
              {isTransitioning ? actionLabel : t.stop}
            </button>
            <button 
              onClick={() => {
                AudioManager.playClick();
                onRestart();
              }} 
              disabled={isTransitioning} 
              className="btn-restart" 
              data-testid={`restart-button-${serviceType}`}
              onMouseEnter={() => AudioManager.playHover()}
            >
              {busy ? <LoaderCircle size={15} className="spin-icon" /> : <RefreshCw size={15} />}
              {busy ? t.restarting : t.restart}
            </button>
          </>
        )}
      </div>
    </article>
  );
}
