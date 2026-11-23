import { Globe } from "lucide-react";
import { useTranslation } from "../../stores/preferencesStore";
import { AudioManager } from "../../utils/audioManager";

/**
 * Project domains (`http://<folder>.localhost`). Saved with the drawer's
 * Save button; a running web server restarts to apply it.
 */
export function ProjectDomainsSection({
  enabled,
  webPort,
  onChange,
}: {
  enabled: boolean;
  webPort: number;
  onChange: (enabled: boolean) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="settings-section" data-testid="project-domains-section">
      <h3>
        <Globe size={16} /> {t.projectDomainsTitle}
      </h3>
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) => {
            AudioManager.playToggle();
            onChange(event.target.checked);
          }}
        />
        <span>{t.projectDomainsToggle}</span>
      </label>
      <p className="settings-muted settings-toggle-hint">
        {(enabled ? t.projectDomainsOnHint : t.projectDomainsOffHint).replace(
          /\{port\}/g,
          String(webPort)
        )}
      </p>
    </div>
  );
}
