import { invoke } from "@tauri-apps/api/core";
import { Network, Radar } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "../../stores/preferencesStore";
import { AppSettings } from "../../types/services";
import { SoundButton } from "../SoundButton";

export type PortField = "web_port" | "php_port" | "mysql_port" | "postgresql_port";

interface PortCheck {
  web: { port: number; available: boolean };
  php: { port: number; available: boolean };
  mysql: { port: number; available: boolean };
  postgresql: { port: number; available: boolean };
}

/** Typing keeps its own draft, so a field can be emptied before the next port is entered. */
function PortInput({
  label,
  value,
  check,
  onChange,
}: {
  label: string;
  value: number;
  check?: { available: boolean };
  onChange: (port: number) => void;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<string | null>(null);
  return (
    <label>
      <span>{label}</span>
      <input
        className="input"
        type="number"
        min={1}
        max={65535}
        value={draft ?? value}
        onChange={(e) => {
          setDraft(e.target.value);
          const port = Number.parseInt(e.target.value, 10);
          if (port >= 1 && port <= 65535) onChange(port);
        }}
        onBlur={() => setDraft(null)}
      />
      {check && (
        <small className={check.available ? "ok" : "warn"}>
          {check.available ? t.portAvailable : t.portInUse}
        </small>
      )}
    </label>
  );
}

export function PortsSection({
  settings,
  onChange,
  onError,
}: {
  settings: AppSettings;
  onChange: (field: PortField, port: number) => void;
  onError: (message: string) => void;
}) {
  const { t } = useTranslation();
  const [portCheck, setPortCheck] = useState<PortCheck | null>(null);

  const changePort = (field: PortField, port: number) => {
    onChange(field, port);
    setPortCheck(null);
  };

  const checkPorts = async () => {
    try {
      setPortCheck(
        await invoke<PortCheck>("check_ports", {
          webPort: settings.web_port,
          phpPort: settings.php_port,
          mysqlPort: settings.mysql_port,
          postgresqlPort: settings.postgresql_port,
        })
      );
    } catch (error) {
      onError(String(error));
    }
  };

  return (
    <div className="settings-section">
      <h3>
        <Network size={16} /> {t.ports}
      </h3>
      <div className="settings-grid">
        <PortInput
          label={t.httpPort}
          value={settings.web_port}
          check={portCheck?.web}
          onChange={(port) => changePort("web_port", port)}
        />
        <PortInput
          label={t.phpPort}
          value={settings.php_port}
          check={portCheck?.php}
          onChange={(port) => changePort("php_port", port)}
        />
        <PortInput
          label={t.mysqlPort}
          value={settings.mysql_port}
          check={portCheck?.mysql}
          onChange={(port) => changePort("mysql_port", port)}
        />
        <PortInput
          label="PostgreSQL"
          value={settings.postgresql_port}
          check={portCheck?.postgresql}
          onChange={(port) => changePort("postgresql_port", port)}
        />
      </div>
      <SoundButton className="btn-secondary compact" onClick={checkPorts}>
        <Radar size={14} /> {t.checkPorts}
      </SoundButton>
    </div>
  );
}
