import { invoke } from "@tauri-apps/api/core";
import { Bug } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "../../stores/preferencesStore";
import { XdebugStatus } from "../../types/addons";
import { AudioManager } from "../../utils/audioManager";

/**
 * Xdebug on/off. Saved at once by its own command (like the agent connector),
 * so the drawer's Save and Cancel do not apply.
 */
export function XdebugToggle() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<XdebugStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.resolve()
      .then(() => invoke<XdebugStatus>("get_xdebug_status"))
      .then((next) => setStatus(next && typeof next.enabled === "boolean" ? next : null))
      .catch(() => setStatus(null));
  }, []);

  if (!status) return null;

  const toggle = async (enabled: boolean) => {
    AudioManager.playToggle();
    setBusy(true);
    setError(null);
    try {
      setStatus(await invoke<XdebugStatus>("set_xdebug", { enabled }));
    } catch (toggleError) {
      setError(String(toggleError));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="xdebug-toggle" data-testid="xdebug-toggle">
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={status.enabled}
          disabled={busy || (!status.available && !status.enabled)}
          onChange={(event) => void toggle(event.target.checked)}
        />
        <span className="modal-title-with-icon">
          <Bug size={14} /> {t.xdebugToggle}
        </span>
      </label>
      <p className="settings-muted">
        {status.available ? (
          t.xdebugHelp.replace("{port}", String(status.client_port))
        ) : (
          <>
            {t.xdebugUnavailable}
            {status.ext_dir && (
              <>
                {" "}
                <code>{status.ext_dir}</code>
              </>
            )}
          </>
        )}
      </p>
      {error && <p className="addon-error">{error}</p>}
    </div>
  );
}
