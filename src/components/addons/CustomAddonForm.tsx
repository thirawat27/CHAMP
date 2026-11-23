import { Save, X } from "lucide-react";
import { FormEvent, useState } from "react";
import { useTranslation } from "../../stores/preferencesStore";
import { CustomAddon, isValidAddonId, parseAddonPort, splitArgs } from "../../types/addons";

interface CustomAddonFormProps {
  /** Editing an existing add-on (its id cannot change). */
  initial?: CustomAddon;
  busy: boolean;
  onSave: (addon: CustomAddon) => Promise<boolean>;
  onCancel: () => void;
}

/** Register a service the user runs from their own binary. */
export function CustomAddonForm({ initial, busy, onSave, onCancel }: CustomAddonFormProps) {
  const { t } = useTranslation();
  const [id, setId] = useState(initial?.id ?? "");
  const [name, setName] = useState(initial?.name ?? "");
  const [command, setCommand] = useState(initial?.command ?? "");
  const [args, setArgs] = useState(
    (initial?.args ?? []).map((arg) => (/\s/.test(arg) ? `"${arg}"` : arg)).join(" ")
  );
  const [port, setPort] = useState(initial ? String(initial.port) : "");
  const [web, setWeb] = useState(initial?.web ?? false);

  const parsedPort = parseAddonPort(port);
  const valid =
    isValidAddonId(id) &&
    name.trim().length > 0 &&
    command.trim().length > 0 &&
    parsedPort !== null;

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!valid || parsedPort === null) return;
    const saved = await onSave({
      id,
      name: name.trim(),
      command: command.trim(),
      args: splitArgs(args),
      port: parsedPort,
      web,
      enabled: initial?.enabled ?? false,
    });
    if (saved) onCancel();
  };

  return (
    <form className="custom-addon-form" onSubmit={submit} data-testid="custom-addon-form">
      <p className="addon-hint">{t.addonCustomWarning}</p>
      <div className="custom-addon-grid">
        <label className="addon-port">
          <span>{t.addonCustomId}</span>
          <input
            value={id}
            disabled={Boolean(initial)}
            placeholder="valkey"
            aria-invalid={id !== "" && !isValidAddonId(id)}
            onChange={(event) => setId(event.target.value.toLowerCase())}
          />
        </label>
        <label className="addon-port">
          <span>{t.addonCustomName}</span>
          <input
            value={name}
            placeholder="Valkey"
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <label className="addon-port">
          <span>{t.addonPort}</span>
          <input
            inputMode="numeric"
            value={port}
            placeholder="6380"
            aria-invalid={port !== "" && parsedPort === null}
            onChange={(event) => setPort(event.target.value)}
          />
        </label>
      </div>
      <label className="addon-port wide">
        <span>{t.addonCustomCommand}</span>
        <input
          value={command}
          placeholder="C:\tools\valkey\valkey-server.exe"
          onChange={(event) => setCommand(event.target.value)}
        />
      </label>
      <label className="addon-port wide">
        <span>{t.addonCustomArgs}</span>
        <input
          value={args}
          placeholder="--port {port} --bind 127.0.0.1 --dir {data_dir}"
          onChange={(event) => setArgs(event.target.value)}
        />
        <small className="settings-muted">{t.addonCustomArgsHelp}</small>
      </label>
      <label className="toggle-row">
        <input type="checkbox" checked={web} onChange={(event) => setWeb(event.target.checked)} />
        <span>{t.addonCustomWeb}</span>
      </label>
      <div className="addon-actions">
        <button type="submit" className="btn-primary compact" disabled={!valid || busy}>
          <Save size={14} /> {t.save}
        </button>
        <button type="button" className="btn-secondary compact" onClick={onCancel}>
          <X size={14} /> {t.cancel}
        </button>
      </div>
    </form>
  );
}
