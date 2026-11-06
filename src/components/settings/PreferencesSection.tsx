import { Power, Volume2 } from "lucide-react";
import { usePreferencesStore, useTranslation } from "../../stores/preferencesStore";
import { AudioManager } from "../../utils/audioManager";

/** Sound effects and auto-start. */
export function PreferencesSection({
  autoStart,
  onAutoStartChange,
}: {
  autoStart: boolean;
  onAutoStartChange: (enabled: boolean) => void;
}) {
  const { t } = useTranslation();
  const soundEnabled = usePreferencesStore((state) => state.soundEnabled);
  const toggleSound = usePreferencesStore((state) => state.toggleSound);

  return (
    <>
      <div className="settings-section">
        <h3>
          <Volume2 size={16} /> {t.soundEffects}
        </h3>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={soundEnabled}
            onChange={() => {
              toggleSound();
              AudioManager.playToggle();
            }}
          />
          <span>{t.enableSoundEffects}</span>
        </label>
        <p className="settings-muted settings-toggle-hint">{t.soundEffectsDescription}</p>
      </div>

      <div className="settings-section">
        <h3>
          <Power size={16} /> {t.startup}
        </h3>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={autoStart}
            onChange={(e) => {
              AudioManager.playToggle();
              onAutoStartChange(e.target.checked);
            }}
          />
          <span>{t.autoStartServices}</span>
        </label>
        <p className="settings-muted settings-toggle-hint">{t.autoStartDescription}</p>
      </div>
    </>
  );
}
