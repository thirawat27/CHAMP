import { Database, Globe, Monitor, Route } from "lucide-react";
import { useTranslation } from "../../stores/preferencesStore";
import {
  DEFAULT_PORTS,
  SERVICE_DISPLAY_NAMES,
  ServiceMap,
  ServiceState,
  ServiceType,
} from "../../types/services";
import { shortcutHint } from "../../utils/shortcuts";
import { SERVICE_ICONS } from "../serviceIcons";
import { SoundButton } from "../SoundButton";

interface RequestPathProps {
  services: Partial<ServiceMap>;
  stackServiceTypes: ServiceType[];
  caddyPort: number;
  isCaddyRunning: boolean;
  allRunning: boolean;
  runningCount: number;
  totalCount: number;
  databaseToolName: string;
  onOpenWebsite: () => void;
  onOpenDatabaseTool: () => void;
}

/** The request path: browser -> Caddy -> PHP-FPM -> active database. */
export function RequestPath({
  services,
  stackServiceTypes,
  caddyPort,
  isCaddyRunning,
  allRunning,
  runningCount,
  totalCount,
  databaseToolName,
  onOpenWebsite,
  onOpenDatabaseTool,
}: RequestPathProps) {
  const { t } = useTranslation();
  const pathNodes = stackServiceTypes.map((serviceType) => ({
    serviceType,
    state: services[serviceType]?.state ?? ServiceState.Stopped,
    port: services[serviceType]?.port ?? DEFAULT_PORTS[serviceType],
  }));
  const stackState = allRunning ? "running" : runningCount > 0 ? "partial" : "stopped";
  const stackStateLabel = allRunning ? t.running : runningCount > 0 ? t.active : t.stopped;

  return (
    <section className={`request-path ${stackState}`} aria-label={t.requestPath}>
      <div className="request-path-head">
        <span className="eyebrow">
          <Route size={13} /> {t.requestPath}
        </span>
        <span className={`stack-state ${stackState}`}>
          {stackStateLabel} · {runningCount}/{totalCount} {t.services}
        </span>
      </div>

      <ol className="path-chain">
        <li className={`path-node ${isCaddyRunning ? "running" : "stopped"}`}>
          <span className="node-led" aria-hidden="true" />
          <span className="node-name">
            <Monitor size={14} /> {t.browser}
          </span>
          <span className="node-port">localhost:{caddyPort}</span>
        </li>
        {pathNodes.map((node, index) => {
          const previous =
            index === 0 ? isCaddyRunning : pathNodes[index - 1].state === ServiceState.Running;
          const linkLive = previous && node.state === ServiceState.Running;
          const Icon = SERVICE_ICONS[node.serviceType];
          return [
            <li
              key={`${node.serviceType}-link`}
              className={`path-link ${linkLive ? "live" : ""}`}
              aria-hidden="true"
            />,
            <li key={node.serviceType} className={`path-node ${node.state}`}>
              <span className="node-led" aria-hidden="true" />
              <span className="node-name">
                <Icon size={14} /> {SERVICE_DISPLAY_NAMES[node.serviceType]}
              </span>
              <span className="node-port">:{node.port}</span>
            </li>,
          ];
        })}
      </ol>

      <div className="request-path-actions">
        <SoundButton
          className="btn-path"
          onClick={onOpenWebsite}
          disabled={!isCaddyRunning}
          title={`${t.openWebsite} (${shortcutHint("website")})`}
        >
          <Globe size={16} /> {t.website}
        </SoundButton>
        <SoundButton
          className="btn-path"
          onClick={onOpenDatabaseTool}
          disabled={!isCaddyRunning}
          title={`${t.openDatabaseTool} (${shortcutHint("database")})`}
        >
          <Database size={16} /> {databaseToolName}
        </SoundButton>
      </div>
    </section>
  );
}
