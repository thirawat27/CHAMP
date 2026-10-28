import { ArrowUpCircle, CheckCircle2, LoaderCircle, RefreshCw } from "lucide-react";
import packageInfo from "../../../package.json";
import { useTranslation } from "../../stores/languageStore";
import { useUpdateStore } from "../../stores/updateStore";
import { AudioManager } from "../../utils/audioManager";

interface UpdatesSectionProps {
  autoCheck: boolean;
  onAutoCheckChange: (enabled: boolean) => void;
}

/** Current version, manual update check, and the daily auto-check toggle. */
export function UpdatesSection({ autoCheck, onAutoCheckChange }: UpdatesSectionProps) {
  const { t, language } = useTranslation();
  const { phase, update, progress, error, checkedAt, checkNow, install } = useUpdateStore();
  const checking = phase === "checking";
  const installing = phase === "installing";

  const lastChecked = checkedAt
    ? new Intl.DateTimeFormat(language === "th" ? "th-TH" : "en-US", {
        dateStyle: "medium",
        timeStyle: "short",
      }).format(checkedAt)
    : null;

  return (
    <div className="settings-section">
      <h3>
        <ArrowUpCircle size={16} /> {t.updatesTitle}
      </h3>
      <div className="settings-row">
        <div>
          <strong className="settings-value">CHAMP v{packageInfo.version}</strong>
          <p className="settings-muted">
            {phase === "available" && update
              ? t.updateAvailableTitle.replace("{version}", update.version)
              : phase === "current"
                ? t.updateCurrent
                : lastChecked
                  ? t.updateLastChecked.replace("{time}", lastChecked)
                  : t.updateNeverChecked}
          </p>
        </div>
        {phase === "available" || installing ? (
          <button
            type="button"
            className="btn-primary compact"
            onClick={() => {
              AudioManager.playClick();
              install();
            }}
            disabled={installing}
          >
            {installing ? (
              <>
                <LoaderCircle size={14} className="spin-icon" />
                {progress !== null ? `${progress}%` : t.working}
              </>
            ) : (
              t.updateInstall
            )}
          </button>
        ) : (
          <button
            type="button"
            className="btn-secondary compact"
            onClick={() => {
              AudioManager.playClick();
              checkNow();
            }}
            disabled={checking}
          >
            {checking ? (
              <LoaderCircle size={14} className="spin-icon" />
            ) : phase === "current" ? (
              <CheckCircle2 size={14} />
            ) : (
              <RefreshCw size={14} />
            )}
            {checking ? t.updateChecking : t.updateCheck}
          </button>
        )}
      </div>
      {update?.body && phase === "available" && (
        <details className="settings-details">
          <summary>{t.updateReleaseNotes}</summary>
          <pre>{update.body}</pre>
        </details>
      )}
      {error && (
        <div className="error-box">
          <p className="error-box-text">
            {t.updateFailed}: {error}
          </p>
        </div>
      )}
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={autoCheck}
          onChange={(event) => {
            AudioManager.playToggle();
            onAutoCheckChange(event.target.checked);
          }}
        />
        <span>{t.autoCheckUpdates}</span>
      </label>
    </div>
  );
}
