import { openUrl } from "@tauri-apps/plugin-opener";
import { Database, Globe2, Play, RefreshCw, Square, Terminal } from "lucide-react";
import {
  DEFAULT_PORTS,
  SERVICE_DESCRIPTIONS,
  SERVICE_DISPLAY_NAMES,
  ServiceState,
  ServiceType,
} from "../types/services";

interface ServiceCardProps {
  serviceType: ServiceType;
  state: ServiceState;
  port?: number;
  error?: string;
  busy?: boolean;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  [key: string]: unknown;
}

const SERVICE_ICONS = {
  [ServiceType.Caddy]: Globe2,
  [ServiceType.PhpFpm]: Terminal,
  [ServiceType.MySQL]: Database,
};

function getServiceUrl(serviceType: ServiceType, port: number) {
  return {
    [ServiceType.Caddy]: `http://localhost:${port}`,
    [ServiceType.PhpFpm]: `tcp://127.0.0.1:${port}`,
    [ServiceType.MySQL]: `mysql://127.0.0.1:${port}`,
  }[serviceType];
}

export function ServiceCard({
  serviceType,
  state,
  port = DEFAULT_PORTS[serviceType],
  error,
  busy = false,
  onStart,
  onStop,
  onRestart,
  ...props
}: ServiceCardProps) {
  const Icon = SERVICE_ICONS[serviceType];
  const displayName = SERVICE_DISPLAY_NAMES[serviceType];
  const description = SERVICE_DESCRIPTIONS[serviceType];
  const serviceUrl = getServiceUrl(serviceType, port);
  const isRunning = state === ServiceState.Running;
  const isTransitioning =
    busy || state === ServiceState.Starting || state === ServiceState.Stopping;
  const isError = state === ServiceState.Error;
  const statusClass = {
    [ServiceState.Stopped]: "status-gray",
    [ServiceState.Starting]: "status-blue",
    [ServiceState.Running]: "status-green",
    [ServiceState.Stopping]: "status-orange",
    [ServiceState.Error]: "status-red",
  }[state];

  return (
    <article
      className={`service-card ${isError ? "has-error" : ""}`}
      data-testid={`service-card-${serviceType}`}
      {...props}
    >
      <div className="service-card-header">
        <div className="service-identity">
          <span className="service-icon">
            <Icon size={18} />
          </span>
          <div>
            <h3>{displayName}</h3>
            <p>{description}</p>
          </div>
        </div>
        <span
          className={`status-pill ${state} ${statusClass}`}
          data-testid={`service-state-${serviceType}`}
        >
          {state}
        </span>
      </div>

      <div className="service-meta">
        <span>Port: {port}</span>
        <span>
          URL:{" "}
          <button
            type="button"
            className="service-url-button"
            onClick={() =>
              openUrl(serviceUrl).catch((openError) =>
                console.error("Failed to open service URL:", openError)
              )
            }
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
            onClick={onStart}
            disabled={isTransitioning}
            className="btn-start"
            data-testid={`start-button-${serviceType}`}
          >
            <Play size={15} /> Start
          </button>
        ) : (
          <>
            <button
              onClick={onStop}
              disabled={isTransitioning}
              className="btn-stop"
              data-testid={`stop-button-${serviceType}`}
            >
              <Square size={14} /> Stop
            </button>
            <button
              onClick={onRestart}
              disabled={isTransitioning}
              className="btn-restart"
              data-testid={`restart-button-${serviceType}`}
            >
              <RefreshCw size={15} /> Restart
            </button>
          </>
        )}
      </div>
    </article>
  );
}
