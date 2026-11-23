import {
  Boxes,
  Check,
  Copy,
  Database,
  Download,
  ExternalLink,
  LoaderCircle,
  Mail,
  Play,
  ScrollText,
  Search,
  Square,
  X,
} from "lucide-react";
import { ReactNode, useState } from "react";
import { COPIED_FEEDBACK_MS } from "../constants";
import { AddonsController } from "../hooks/useAddons";
import { useEscape } from "../hooks/useEscape";
import { useTranslation } from "../stores/preferencesStore";
import { ADDON_KINDS, AddonKind, AddonStatus, addonDotenv, parseAddonPort } from "../types/addons";
import { ServiceState } from "../types/services";
import { AudioManager } from "../utils/audioManager";
import { SoundButton } from "./SoundButton";

const ADDON_ICONS: Record<AddonKind, ReactNode> = {
  redis: <Database size={18} />,
  mailpit: <Mail size={18} />,
  meilisearch: <Search size={18} />,
};

interface AddonsModalProps {
  controller: AddonsController;
  logsDir?: string;
  onClose: () => void;
  openExternal: (url: string) => void;
  openFolder: (path?: string) => void;
}

/** Install, enable, and configure Redis, Mailpit, and Meilisearch. */
export function AddonsModal({
  controller,
  logsDir,
  onClose,
  openExternal,
  openFolder,
}: AddonsModalProps) {
  const { t } = useTranslation();
  const { addons, loaded, error, clearError } = controller;
  const [copied, setCopied] = useState(false);

  const close = () => {
    AudioManager.playClick();
    onClose();
  };
  useEscape(close);

  const ordered = ADDON_KINDS.map((kind) => addons.find((addon) => addon.kind === kind)).filter(
    (addon): addon is AddonStatus => Boolean(addon)
  );
  const dotenv = addonDotenv(ordered);
  const redisEnabled = ordered.some((addon) => addon.kind === "redis" && addon.enabled);
  const searchEnabled = ordered.some((addon) => addon.kind === "meilisearch" && addon.enabled);

  const copyDotenv = async () => {
    try {
      await navigator.clipboard.writeText(dotenv);
      AudioManager.playClick();
      setCopied(true);
      window.setTimeout(() => setCopied(false), COPIED_FEEDBACK_MS);
    } catch (copyError) {
      console.error("Failed to copy .env lines:", copyError);
    }
  };

  return (
    <div className="modal-overlay" onClick={close}>
      <div
        className="modal-content addons-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="addons-title"
        data-testid="addons-modal"
      >
        <header className="modal-header">
          <div>
            <h2 id="addons-title" className="modal-title-with-icon">
              <Boxes size={19} /> {t.addonsTitle}
            </h2>
            <p className="addons-subtitle">{t.addonsDescription}</p>
          </div>
          <button className="icon-button" onClick={close} aria-label={t.close}>
            <X size={18} />
          </button>
        </header>

        <div className="modal-body addons-body">
          {error && (
            <div className="error-box" role="alert">
              <p className="error-box-text">{error}</p>
              <button className="icon-button" onClick={clearError} aria-label={t.close}>
                <X size={14} />
              </button>
            </div>
          )}

          {!loaded ? (
            <div className="empty-state">{t.loading}</div>
          ) : (
            ordered.map((addon) => (
              <AddonRow
                key={addon.kind}
                addon={addon}
                controller={controller}
                openExternal={openExternal}
              />
            ))
          )}

          <section className="addons-env" aria-labelledby="addons-env-title">
            <div className="addons-env-head">
              <h3 id="addons-env-title">{t.addonEnvTitle}</h3>
              <div className="addons-env-actions">
                {logsDir && (
                  <SoundButton
                    className="btn-secondary compact"
                    onClick={() => openFolder(logsDir)}
                  >
                    <ScrollText size={14} /> {t.addonOpenLog}
                  </SoundButton>
                )}
                <button
                  type="button"
                  className="btn-secondary compact"
                  onClick={copyDotenv}
                  disabled={!dotenv}
                >
                  {copied ? <Check size={14} /> : <Copy size={14} />}{" "}
                  {copied ? t.addonCopied : t.addonCopyEnv}
                </button>
              </div>
            </div>
            {dotenv ? (
              <pre className="addons-env-lines" data-testid="addons-dotenv">
                {dotenv}
              </pre>
            ) : (
              <p className="settings-muted">{t.addonEnvEmpty}</p>
            )}
            {redisEnabled && <p className="settings-muted">{t.addonPredisNote}</p>}
            {searchEnabled && <p className="settings-muted">{t.addonScoutNote}</p>}
          </section>
        </div>
      </div>
    </div>
  );
}

const PURPOSE_KEYS = {
  redis: "addonRedisPurpose",
  mailpit: "addonMailpitPurpose",
  meilisearch: "addonMeilisearchPurpose",
} as const;

function stateLabel(state: ServiceState, t: ReturnType<typeof useTranslation>["t"]) {
  switch (state) {
    case ServiceState.Running:
      return t.running;
    case ServiceState.Starting:
      return t.starting;
    case ServiceState.Stopping:
      return t.stopping;
    case ServiceState.Error:
      return t.error;
    default:
      return t.stopped;
  }
}

interface AddonRowProps {
  addon: AddonStatus;
  controller: AddonsController;
  openExternal: (url: string) => void;
}

/** One add-on with its toggle, ports, and actions (window and Settings). */
export function AddonRow({ addon, controller, openExternal }: AddonRowProps) {
  const { t } = useTranslation();
  const { busyKind, progress, install, start, stop, configure, mailCount } = controller;
  const busy = busyKind === addon.kind || addon.installing;
  const anyBusy = busyKind !== null;
  const running = addon.state === ServiceState.Running;
  const percent = progress[addon.kind];
  const isMailpit = addon.kind === "mailpit";

  return (
    <article
      className={`addon-row ${addon.state}`}
      data-testid={`addon-${addon.kind}`}
      aria-label={addon.name}
    >
      <div className="addon-row-head">
        <span className="addon-icon">{ADDON_ICONS[addon.kind]}</span>
        <div className="addon-title">
          <strong>{addon.name}</strong>
          <small>
            {t[PURPOSE_KEYS[addon.kind]]}
            {addon.installed_version
              ? ` · ${addon.installed_version}`
              : addon.source === "system"
                ? ` · ${t.addonSystemSource}`
                : ""}
          </small>
        </div>
        <span className={`addon-state ${addon.installed ? addon.state : "missing"}`}>
          {addon.installed ? stateLabel(addon.state, t) : t.addonNotInstalled}
        </span>
      </div>

      <div className="addon-controls">
        <label className="toggle-row addon-toggle">
          <input
            type="checkbox"
            checked={addon.enabled}
            disabled={anyBusy}
            onChange={(event) => {
              AudioManager.playToggle();
              void configure(addon.kind, { enabled: event.target.checked });
            }}
          />
          <span>{t.addonStartWithStack}</span>
        </label>

        <PortInput
          id={`${addon.kind}-port`}
          label={isMailpit ? t.addonSmtpPort : t.addonPort}
          value={addon.port}
          disabled={anyBusy}
          onCommit={(port) => configure(addon.kind, { port })}
        />
        {isMailpit && addon.web_port !== null && (
          <PortInput
            id="mailpit-web-port"
            label={t.addonWebPort}
            value={addon.web_port}
            disabled={anyBusy}
            onCommit={(web_port) => configure(addon.kind, { web_port })}
          />
        )}
      </div>

      {!addon.installed && !addon.downloadable && addon.install_hint && (
        <p className="addon-hint">
          {t.addonSystemInstallHint} <code>{addon.install_hint}</code>
        </p>
      )}
      {isMailpit && addon.enabled && <p className="addon-hint">{t.addonMailHint}</p>}
      {addon.error && (
        <p className="addon-error" role="status">
          {addon.error}
        </p>
      )}

      <div className="addon-actions">
        {addon.downloadable && (!addon.installed_version || addon.update_available) && (
          <SoundButton
            className="btn-secondary compact"
            onClick={() => install(addon.kind)}
            disabled={anyBusy}
          >
            {busy && percent !== undefined ? (
              <>
                <LoaderCircle size={14} className="spin-icon" /> {t.addonInstalling} {percent}%
              </>
            ) : (
              <>
                <Download size={14} />{" "}
                {addon.update_available ? `${t.addonUpdate} ${addon.version}` : t.addonInstall}
              </>
            )}
          </SoundButton>
        )}
        {addon.installed &&
          (running ? (
            <SoundButton
              className="btn-secondary compact danger"
              onClick={() => stop(addon.kind)}
              disabled={anyBusy}
            >
              {busy ? <LoaderCircle size={14} className="spin-icon" /> : <Square size={14} />}{" "}
              {t.stop}
            </SoundButton>
          ) : (
            <SoundButton
              className="btn-secondary compact"
              onClick={() => start(addon.kind)}
              disabled={anyBusy || addon.state === ServiceState.Stopping}
            >
              {busy ? <LoaderCircle size={14} className="spin-icon" /> : <Play size={14} />}{" "}
              {t.start}
            </SoundButton>
          ))}
        {isMailpit && addon.web_url && (
          <SoundButton
            className="btn-primary compact"
            onClick={() => openExternal(addon.web_url as string)}
          >
            <ExternalLink size={14} /> {t.addonOpenInbox}
            {mailCount !== null && <span className="addon-count">{mailCount}</span>}
          </SoundButton>
        )}
        {addon.kind === "meilisearch" && addon.web_url && (
          <SoundButton
            className="btn-primary compact"
            onClick={() => openExternal(addon.web_url as string)}
          >
            <ExternalLink size={14} /> {t.addonOpenDashboard}
          </SoundButton>
        )}
      </div>
    </article>
  );
}

interface PortInputProps {
  id: string;
  label: string;
  value: number;
  disabled: boolean;
  onCommit: (port: number) => Promise<boolean>;
}

/** Saves on blur or Enter; an invalid value is shown and never sent. */
function PortInput({ id, label, value, disabled, onCommit }: PortInputProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<string | null>(null);
  const shown = draft ?? String(value);
  const invalid = draft !== null && parseAddonPort(draft) === null;

  const commit = async () => {
    if (draft === null) return;
    const port = parseAddonPort(draft);
    if (port === null) return;
    if (port === value) {
      setDraft(null);
      return;
    }
    if (await onCommit(port)) setDraft(null);
  };

  return (
    <label className="addon-port" htmlFor={id}>
      <span>{label}</span>
      <input
        id={id}
        inputMode="numeric"
        value={shown}
        disabled={disabled}
        aria-invalid={invalid}
        title={invalid ? t.addonPortInvalid : undefined}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => void commit()}
        onKeyDown={(event) => {
          if (event.key === "Enter") void commit();
          if (event.key === "Escape" && draft !== null) {
            // Undo the edit instead of closing the window.
            event.stopPropagation();
            setDraft(null);
          }
        }}
      />
      {invalid && <small className="addon-port-error">{t.addonPortInvalid}</small>}
    </label>
  );
}
