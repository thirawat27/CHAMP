import { Boxes, ExternalLink, X } from "lucide-react";
import { AddonsController } from "../../hooks/useAddons";
import { useTranslation } from "../../stores/preferencesStore";
import { ADDON_KINDS, AddonStatus } from "../../types/addons";
import { AddonRow } from "../AddonsModal";
import { SoundButton } from "../SoundButton";

interface AddonsSectionProps {
  controller: AddonsController;
  openExternal: (url: string) => void;
  /** Opens the Add-on services window (with the `.env` lines). */
  onOpenWindow?: () => void;
}

/**
 * Redis, Memcached, and Mailpit in Settings. Like the agent connector, every
 * change is saved at once by its own command, so Save and Cancel of the
 * drawer do not apply to it.
 */
export function AddonsSection({ controller, openExternal, onOpenWindow }: AddonsSectionProps) {
  const { t } = useTranslation();
  const { addons, loaded, error, clearError } = controller;
  const ordered = ADDON_KINDS.map((kind) => addons.find((addon) => addon.kind === kind)).filter(
    (addon): addon is AddonStatus => Boolean(addon)
  );

  return (
    <div className="settings-section addons-settings" data-testid="addons-settings">
      <h3>
        <Boxes size={16} /> {t.addonsTitle}
      </h3>
      <p className="settings-muted">{t.addonsDescription}</p>
      <p className="settings-muted">{t.addonSettingsSavedAtOnce}</p>

      {error && (
        <div className="error-box" role="alert">
          <p className="error-box-text">{error}</p>
          <button type="button" className="icon-button" onClick={clearError} aria-label={t.close}>
            <X size={14} />
          </button>
        </div>
      )}

      {!loaded ? (
        <div className="empty-state">{t.loading}</div>
      ) : (
        <div className="addons-settings-list">
          {ordered.map((addon) => (
            <AddonRow
              key={addon.kind}
              addon={addon}
              controller={controller}
              openExternal={openExternal}
            />
          ))}
        </div>
      )}

      {onOpenWindow && (
        <SoundButton className="btn-secondary compact" onClick={onOpenWindow}>
          <ExternalLink size={14} /> {t.addonOpenWindow}
        </SoundButton>
      )}
    </div>
  );
}
