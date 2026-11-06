import { CheckCircle2 } from "lucide-react";
import { useTranslation } from "../../stores/preferencesStore";

interface CompleteStepProps {
  installed: Array<{ name: string; version: string }>;
  onContinue: () => void;
}

export function CompleteStep({ installed, onContinue }: CompleteStepProps) {
  const { t } = useTranslation();

  return (
    <div className="setup-complete">
      <div className="setup-complete-icon" aria-hidden="true">
        <CheckCircle2 size={34} strokeWidth={2.4} />
      </div>
      <h3>{t.wizardRuntimeInstalled}</h3>
      <p>{t.wizardReadyToStartStack}</p>
      <div className="setup-complete-packages">
        {installed.map((pkg) => (
          <div key={pkg.name} className="setup-complete-package">
            <span>{pkg.name}</span>
            <strong>{pkg.version}</strong>
          </div>
        ))}
      </div>
      <button onClick={onContinue} className="btn-primary setup-action">
        {t.wizardContinueToDashboard}
      </button>
    </div>
  );
}
