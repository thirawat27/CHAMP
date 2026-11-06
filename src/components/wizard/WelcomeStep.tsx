import { BookOpen } from "lucide-react";
import { useTranslation } from "../../stores/preferencesStore";

interface WelcomeStepProps {
  hasExisting: boolean;
  onUseExisting: () => void;
  onNext: () => void;
  onOpenGuide: () => void;
}

export function WelcomeStep({ hasExisting, onUseExisting, onNext, onOpenGuide }: WelcomeStepProps) {
  const { t } = useTranslation();

  return (
    <div>
      <p className="setup-body-text">{t.wizardWelcomeBody}</p>
      <div className="setup-summary">
        <div className="setup-summary-item">
          <strong>{t.wizardUserSpaceRuntime}</strong>
          <span>{t.wizardUserSpaceRuntimeCopy}</span>
        </div>
        <div className="setup-summary-item">
          <strong>{t.wizardDevFirstPorts}</strong>
          <span>{t.wizardDevFirstPortsCopy}</span>
        </div>
        <div className="setup-summary-item">
          <strong>{t.wizardDatabaseToolReady}</strong>
          <span>{t.wizardDatabaseToolReadyCopy}</span>
        </div>
      </div>
      {hasExisting ? (
        <div className="setup-callout">
          <strong>{t.wizardExistingDetected}</strong>
        </div>
      ) : (
        <div className="info-box setup-info">
          <strong>{t.wizardEstimatedDownloadSize}</strong> {t.wizardVariesByPlatform}
        </div>
      )}
      <div className="setup-actions">
        {hasExisting && (
          <button onClick={onUseExisting} className="btn-secondary setup-action">
            {t.wizardUseExisting}
          </button>
        )}
        <button onClick={onNext} className="btn-primary setup-action">
          {hasExisting ? t.wizardDownloadFresh : t.wizardGetStarted}
        </button>
        <button
          type="button"
          onClick={onOpenGuide}
          className="btn-secondary"
          title={t.wizardReadUserManual}
        >
          <BookOpen size={15} /> {t.manual}
        </button>
      </div>
    </div>
  );
}
