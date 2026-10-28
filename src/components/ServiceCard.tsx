/**
 * ServiceCard Component
 *
 * One module of the local stack: status light, port readout, last error, and
 * the start/stop/restart controls.
 */

import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Database,
  Download,
  FileCode2,
  Globe2,
  LoaderCircle,
  Play,
  RotateCw,
  Square,
  Stethoscope,
} from "lucide-react";
import { useTranslation } from "../stores/languageStore";
import { AudioManager } from "../utils/audioManager";
import {
  DEFAULT_PORTS,
  SERVICE_DESCRIPTIONS,
  SERVICE_DISPLAY_NAMES,
  ServiceState,
  ServiceType,
} from "../types/services";

const SERVICE_ICONS = {
  [ServiceType.Caddy]: Globe2,
  [ServiceType.PhpFpm]: FileCode2,
  [ServiceType.MySQL]: Database,
  [ServiceType.PostgreSQL]: Database,
};

interface ServiceCardProps {
  serviceType: ServiceType;
  state: ServiceState;
  port?: number;
  error?: string;
  /** Whether the service is currently processing a command */
  busy?: boolean;
  /** Label to show when busy (e.g., "Starting...", "Stopping...") */
  busyLabel?: string;
  /** True for the database that is not part of the selected stack */
  standby?: boolean;
  /** Install the runtime first; replaces the start control while set */
  onInstall?: () => void;
  /** Download progress (0-100) while onInstall is running */
  installPercent?: number | null;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  /** Open Log Doctor for this service (shown on errors) */
  onDiagnose?: () => void;
  [key: string]: unknown;
}

/** Address shown under the port: a link for the web server, plain text otherwise. */
function serviceAddress(serviceType: ServiceType, port: number) {
  return {
    [ServiceType.Caddy]: `http://localhost:${port}`,
    [ServiceType.PhpFpm]: `127.0.0.1:${port}`,
    [ServiceType.MySQL]: `127.0.0.1:${port}`,
    [ServiceType.PostgreSQL]: `127.0.0.1:${port}`,
  }[serviceType];
}

export function ServiceCard({
  serviceType,
  state,
  port = DEFAULT_PORTS[serviceType],
  error,
  busy = false,
  busyLabel,
  standby = false,
  onInstall,
  installPercent = null,
  onStart,
  onStop,
  onRestart,
  onDiagnose,
  ...props
}: ServiceCardProps) {
  const { t } = useTranslation();
  const address = serviceAddress(serviceType, port);
  const Icon = SERVICE_ICONS[serviceType];
  const isRunning = state === ServiceState.Running;
  const isTransitioning =
    busy || state === ServiceState.Starting || state === ServiceState.Stopping;
  const isError = state === ServiceState.Error;
  const actionLabel = busyLabel || (state === ServiceState.Stopping ? t.stopping : t.starting);
  const needsInstall = Boolean(onInstall) && !isRunning;
  const installing = installPercent !== null;

  const stateLabels: Record<ServiceState, string> = {
    [ServiceState.Stopped]: t.stopped,
    [ServiceState.Starting]: t.starting,
    [ServiceState.Running]: t.running,
    [ServiceState.Stopping]: t.stopping,
    [ServiceState.Error]: t.error,
  };

  return (
    <article
      className={`service-card state-${state}${standby ? " is-standby" : ""}`}
      data-testid={`service-card-${serviceType}`}
      {...props}
    >
      <header className="service-card-header">
        <span className="service-icon" aria-hidden="true">
          <Icon size={17} />
          <span className="service-led" />
        </span>
        <div className="service-identity">
          <h3>{SERVICE_DISPLAY_NAMES[serviceType]}</h3>
          <p>{SERVICE_DESCRIPTIONS[serviceType]}</p>
        </div>
        <span className={`status-pill ${state}`} data-testid={`service-state-${serviceType}`}>
          {needsInstall ? t.notInstalled : stateLabels[state]}
        </span>
      </header>

      <div className="service-readout">
        <span className="readout-label">{t.port}</span>
        <span className="readout-value" data-testid={`service-port-${serviceType}`}>
          {port}
        </span>
        {serviceType === ServiceType.Caddy ? (
          <button
            type="button"
            className="readout-address link"
            onClick={() => {
              AudioManager.playClick();
              openUrl(address).catch((openError) =>
                console.error("Failed to open service URL:", openError)
              );
            }}
          >
            {address}
          </button>
        ) : (
          <code className="readout-address">{address}</code>
        )}
      </div>

      {standby && !isRunning && (
        <p className="service-standby">{needsInstall ? t.standbyNotInstalled : t.notInStack}</p>
      )}

      {isError && error && (
        <div className="service-error" title={error}>
          {error.length > 180 ? `${error.substring(0, 180)}…` : error}
        </div>
      )}

      <div className="service-actions">
        {isError && onDiagnose && (
          <button
            type="button"
            onClick={() => {
              AudioManager.playClick();
              onDiagnose();
            }}
            className="btn-diagnose"
            data-testid={`diagnose-button-${serviceType}`}
          >
            <Stethoscope size={15} />
            {t.diagnose}
          </button>
        )}
        {needsInstall ? (
          <button
            type="button"
            onClick={() => {
              AudioManager.playClick();
              onInstall?.();
            }}
            disabled={installing}
            className="btn-start"
            data-testid={`install-button-${serviceType}`}
          >
            {installing ? (
              <LoaderCircle size={15} className="spin-icon" />
            ) : (
              <Download size={15} />
            )}
            {installing ? `${installPercent}%` : t.install}
          </button>
        ) : !isRunning ? (
          <button
            type="button"
            onClick={() => {
              AudioManager.playClick();
              onStart();
            }}
            disabled={isTransitioning}
            className="btn-start"
            data-testid={`start-button-${serviceType}`}
          >
            {isTransitioning ? (
              <LoaderCircle size={15} className="spin-icon" />
            ) : (
              <Play size={15} />
            )}
            {isTransitioning ? actionLabel : t.start}
          </button>
        ) : (
          <>
            <button
              type="button"
              onClick={() => {
                AudioManager.playClick();
                onStop();
              }}
              disabled={isTransitioning}
              className="btn-stop"
              data-testid={`stop-button-${serviceType}`}
            >
              {isTransitioning ? (
                <LoaderCircle size={14} className="spin-icon" />
              ) : (
                <Square size={13} />
              )}
              {isTransitioning ? actionLabel : t.stop}
            </button>
            <button
              type="button"
              onClick={() => {
                AudioManager.playClick();
                onRestart();
              }}
              disabled={isTransitioning}
              className="btn-restart"
              data-testid={`restart-button-${serviceType}`}
              aria-label={busy ? t.restarting : t.restart}
              title={t.restart}
            >
              {busy ? <LoaderCircle size={15} className="spin-icon" /> : <RotateCw size={15} />}
            </button>
          </>
        )}
      </div>
    </article>
  );
}
