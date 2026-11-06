import { ExistingComponent } from "../../hooks/useRuntimeInstall";
import { useTranslation } from "../../stores/preferencesStore";
import { DismissibleError } from "../DismissibleError";

interface ConfirmStepProps {
  components: ExistingComponent[];
  /** Version each component would be installed at */
  newVersions: Record<string, string | undefined>;
  error: string | null;
  onDismissError: () => void;
  onBack: () => void;
  onUseExisting: () => void;
  onKeepAndInstall: () => void;
  onInstallAll: () => void;
}

export function ConfirmStep({
  components,
  newVersions,
  error,
  onDismissError,
  onBack,
  onUseExisting,
  onKeepAndInstall,
  onInstallAll,
}: ConfirmStepProps) {
  const { t } = useTranslation();

  return (
    <div>
      <p className="setup-summary-intro">{t.wizardInstallationSummary}</p>
      <div className="setup-component-list">
        {components.map((component) => (
          <div
            key={component.name}
            className={`setup-component ${component.isExisting ? "existing" : ""}`}
          >
            <span className="setup-component-name">{component.displayName}</span>
            <span className="setup-component-version">
              {newVersions[component.name] || t.unknownError}
            </span>
          </div>
        ))}
      </div>
      {error && <DismissibleError message={error} onDismiss={onDismissError} />}
      <div className="setup-actions">
        <button onClick={onBack} className="btn-secondary setup-action tight">
          {t.back}
        </button>
        {components.some((component) => component.isExisting) && (
          <button onClick={onUseExisting} className="btn-secondary setup-action tight">
            {t.wizardUseExisting}
          </button>
        )}
        <button onClick={onKeepAndInstall} className="btn-secondary setup-action tight">
          {t.wizardKeepAndInstall}
        </button>
        <button onClick={onInstallAll} className="btn-primary setup-action">
          {t.wizardInstallAll}
        </button>
      </div>
    </div>
  );
}
