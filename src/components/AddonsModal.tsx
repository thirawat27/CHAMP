import {
  Boxes,
  Cog,
  Database,
  Download,
  ExternalLink,
  FileCode2,
  HardDrive,
  LoaderCircle,
  Mail,
  Pencil,
  Play,
  Plus,
  Puzzle,
  Radio,
  ScrollText,
  Search,
  Square,
  Trash2,
  X,
} from "lucide-react";
import { ReactNode, useState } from "react";
import { AddonsController } from "../hooks/useAddons";
import { useEscape } from "../hooks/useEscape";
import { useTranslation } from "../stores/preferencesStore";
import { AddonStatus, BuiltinAddonKind, orderAddons, parseAddonPort } from "../types/addons";
import { ServiceState } from "../types/services";
import { AudioManager } from "../utils/audioManager";
import { CustomAddonForm } from "./addons/CustomAddonForm";
import { EnvProfilesPanel } from "./addons/EnvProfilesPanel";
import { WorkersPanel } from "./addons/WorkersPanel";
import { SoundButton } from "./SoundButton";

const ADDON_ICONS: Record<BuiltinAddonKind, ReactNode> = {
  redis: <Database size={17} />,
  mailpit: <Mail size={17} />,
  meilisearch: <Search size={17} />,
  centrifugo: <Radio size={17} />,
  rustfs: <HardDrive size={17} />,
};

type AddonsTab = "services" | "config" | "tasks";
const TAB_STORAGE_KEY = "champ.addons.tab";

function loadTab(): AddonsTab {
  try {
    const saved = window.localStorage.getItem(TAB_STORAGE_KEY);
    if (saved === "services" || saved === "config" || saved === "tasks") return saved;
  } catch {
    // Storage can be unavailable; the first tab is fine.
  }
  return "services";
}

function saveTab(tab: AddonsTab) {
  try {
    window.localStorage.setItem(TAB_STORAGE_KEY, tab);
  } catch {
    // Not worth reporting: only the remembered tab is lost.
  }
}

interface AddonsModalProps {
  controller: AddonsController;
  logsDir?: string;
  onClose: () => void;
  openExternal: (url: string) => void;
  openFolder: (path?: string) => void;
}

/**
 * Add-on services in three tabs: the services themselves (a grid of compact
 * cards, built-in and custom), configuration lines for every framework, and
 * project background tasks.
 */
export function AddonsModal({
  controller,
  logsDir,
  onClose,
  openExternal,
  openFolder,
}: AddonsModalProps) {
  const { t } = useTranslation();
  const { addons, loaded, error, clearError, busyKind, saveCustom } = controller;
  const [tab, setTab] = useState<AddonsTab>(loadTab);
  const [addingCustom, setAddingCustom] = useState(false);

  const close = () => {
    AudioManager.playClick();
    onClose();
  };
  useEscape(close);

  const ordered = orderAddons(addons);
  const enabledCount = ordered.filter((addon) => addon.enabled).length;
  // Reload the configuration lines when an add-on is toggled or moves port.
  const refreshKey = ordered
    .map((addon) => `${addon.kind}:${addon.enabled}:${addon.port}:${addon.web_port}`)
    .join("|");

  const pickTab = (next: AddonsTab) => {
    setTab(next);
    saveTab(next);
  };

  const tabs: { id: AddonsTab; label: string; icon: ReactNode; badge?: number }[] = [
    { id: "services", label: t.addonsTabServices, icon: <Boxes size={14} />, badge: enabledCount },
    { id: "config", label: t.addonEnvTitle, icon: <FileCode2 size={14} /> },
    { id: "tasks", label: t.workersTitle, icon: <Cog size={14} /> },
  ];

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
        <header className="modal-header addons-header">
          <div className="addons-header-text">
            <h2 id="addons-title" className="modal-title-with-icon">
              <Boxes size={19} /> {t.addonsTitle}
            </h2>
            <p className="addons-subtitle">{t.addonsDescription}</p>
          </div>
          <div className="addons-header-actions">
            {logsDir && (
              <SoundButton
                className="icon-button"
                onClick={() => openFolder(logsDir)}
                aria-label={t.addonOpenLog}
                title={t.addonOpenLog}
              >
                <ScrollText size={16} />
              </SoundButton>
            )}
            <button className="icon-button" onClick={close} aria-label={t.close}>
              <X size={18} />
            </button>
          </div>
        </header>

        <div className="modal-body addons-body">
          <div className="addons-tabs" role="tablist" aria-label={t.addonsTitle}>
            {tabs.map((item) => (
              <SoundButton
                key={item.id}
                role="tab"
                aria-selected={tab === item.id}
                className={`addons-tab ${tab === item.id ? "selected" : ""}`}
                onClick={() => pickTab(item.id)}
                data-testid={`addons-tab-${item.id}`}
              >
                {item.icon}
                <span>{item.label}</span>
                {item.badge ? <span className="addons-tab-badge">{item.badge}</span> : null}
              </SoundButton>
            ))}
          </div>

          {error && (
            <div className="error-box" role="alert">
              <p className="error-box-text">{error}</p>
              <button className="icon-button" onClick={clearError} aria-label={t.close}>
                <X size={14} />
              </button>
            </div>
          )}

          {tab === "services" &&
            (!loaded ? (
              <div className="empty-state">{t.loading}</div>
            ) : (
              <div className="addons-grid" role="tabpanel">
                {ordered.map((addon) => (
                  <AddonRow
                    key={addon.kind}
                    addon={addon}
                    controller={controller}
                    openExternal={openExternal}
                  />
                ))}
                {addingCustom ? (
                  <div className="addons-grid-wide">
                    <CustomAddonForm
                      busy={busyKind !== null}
                      onSave={saveCustom}
                      onCancel={() => setAddingCustom(false)}
                    />
                  </div>
                ) : (
                  <button
                    type="button"
                    className="addon-add-card"
                    onClick={() => {
                      AudioManager.playClick();
                      setAddingCustom(true);
                    }}
                  >
                    <Plus size={18} />
                    <strong>{t.addonCustomAdd}</strong>
                    <small>{t.addonCustomHelp}</small>
                  </button>
                )}
              </div>
            ))}

          {tab === "config" && (
            <div role="tabpanel">
              <EnvProfilesPanel refreshKey={refreshKey} />
            </div>
          )}
          {tab === "tasks" && (
            <div role="tabpanel">
              <WorkersPanel />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

const PURPOSE_KEYS = {
  redis: "addonRedisPurpose",
  mailpit: "addonMailpitPurpose",
  meilisearch: "addonMeilisearchPurpose",
  centrifugo: "addonCentrifugoPurpose",
  rustfs: "addonRustfsPurpose",
} as const;

const OPEN_KEYS = {
  mailpit: "addonOpenInbox",
  meilisearch: "addonOpenDashboard",
  centrifugo: "addonOpenAdmin",
  rustfs: "addonOpenConsole",
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

/** One add-on as a compact card (window grid and Settings list). */
export function AddonRow({ addon, controller, openExternal }: AddonRowProps) {
  const { t } = useTranslation();
  const {
    busyKind,
    progress,
    install,
    start,
    stop,
    configure,
    mailCount,
    saveCustom,
    removeCustom,
  } = controller;
  const [editing, setEditing] = useState(false);
  const busy = busyKind === addon.kind || addon.installing;
  const anyBusy = busyKind !== null;
  const running = addon.state === ServiceState.Running;
  const percent = progress[addon.kind];
  const builtin = addon.custom ? null : (addon.kind as BuiltinAddonKind);
  const isMailpit = builtin === "mailpit";
  const secondPort = addon.ports && addon.ports.length > 1 ? addon.ports[1] : null;

  const purpose =
    builtin && PURPOSE_KEYS[builtin] ? t[PURPOSE_KEYS[builtin]] : t.addonCustomPurpose;
  const openKey =
    builtin && builtin in OPEN_KEYS ? OPEN_KEYS[builtin as keyof typeof OPEN_KEYS] : null;
  const showInstall = addon.downloadable && (!addon.installed_version || addon.update_available);
  // One short hint at most, so cards stay the same height.
  const hint =
    !addon.installed && !addon.downloadable && addon.install_hint ? (
      <>
        {t.addonSystemInstallHint} <code>{addon.install_hint}</code>
      </>
    ) : addon.custom && addon.definition ? (
      <code title={addon.definition.command}>{addon.definition.command}</code>
    ) : builtin === "rustfs" && addon.enabled ? (
      t.addonRustfsHint
    ) : isMailpit && addon.enabled ? (
      t.addonMailHint
    ) : null;

  if (editing && addon.definition) {
    return (
      <article
        className="addon-row addons-grid-wide"
        data-testid={`addon-${addon.kind}`}
        aria-label={addon.name}
      >
        <CustomAddonForm
          initial={addon.definition}
          busy={anyBusy}
          onSave={saveCustom}
          onCancel={() => setEditing(false)}
        />
      </article>
    );
  }

  return (
    <article
      className={`addon-row ${addon.installed ? addon.state : "missing"}`}
      data-testid={`addon-${addon.kind}`}
      aria-label={addon.name}
    >
      <div className="addon-row-head">
        <span className="addon-icon">{builtin ? ADDON_ICONS[builtin] : <Puzzle size={17} />}</span>
        <div className="addon-title">
          <strong>{addon.name}</strong>
          <small title={purpose}>
            {purpose}
            {addon.installed_version
              ? ` · ${addon.installed_version}`
              : addon.source === "system"
                ? ` · ${t.addonSystemSource}`
                : ""}
          </small>
        </div>
        <span className={`addon-state ${addon.installed ? addon.state : "missing"}`}>
          {addon.installed
            ? stateLabel(addon.state, t)
            : addon.custom
              ? t.addonCustomMissing
              : t.addonNotInstalled}
        </span>
      </div>

      <div className="addon-controls">
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
        {!isMailpit && secondPort && (
          <span className="addon-port readonly">
            <span>{secondPort.label}</span>
            <code>{secondPort.port}</code>
          </span>
        )}
        <label className="addon-switch" title={t.addonStartWithStack}>
          <input
            type="checkbox"
            checked={addon.enabled}
            disabled={anyBusy}
            onChange={(event) => {
              AudioManager.playToggle();
              void configure(addon.kind, { enabled: event.target.checked });
            }}
          />
          <span className="addon-switch-track" aria-hidden="true" />
          <span className="addon-switch-label">{t.addonStartWithStack}</span>
        </label>
      </div>

      {hint && <p className="addon-hint">{hint}</p>}
      {addon.error && (
        <p className="addon-error" role="status" title={addon.error}>
          {addon.error}
        </p>
      )}

      <div className="addon-actions">
        {showInstall && (
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
        {addon.web_url && (
          <SoundButton
            className="btn-primary compact"
            onClick={() => openExternal(addon.web_url as string)}
          >
            <ExternalLink size={14} /> {openKey ? t[openKey] : t.addonOpenPage}
            {isMailpit && mailCount !== null && <span className="addon-count">{mailCount}</span>}
          </SoundButton>
        )}
        {addon.custom && (
          <span className="addon-actions-end">
            <SoundButton
              className="icon-button"
              onClick={() => setEditing(true)}
              disabled={anyBusy}
              aria-label={`${t.addonCustomEdit} ${addon.name}`}
              title={t.addonCustomEdit}
            >
              <Pencil size={14} />
            </SoundButton>
            <SoundButton
              className="icon-button"
              onClick={() => removeCustom(addon.kind)}
              disabled={anyBusy}
              aria-label={`${t.workerRemove} ${addon.name}`}
              title={t.workerRemove}
            >
              <Trash2 size={14} />
            </SoundButton>
          </span>
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
