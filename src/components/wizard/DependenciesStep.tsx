import { useTranslation } from "../../stores/preferencesStore";
import { DependencyCheckResult } from "../../types/services";

interface DependenciesStepProps {
  result: DependencyCheckResult;
  onBack: () => void;
  onRetry: () => void;
}

export function DependenciesStep({ result, onBack, onRetry }: DependenciesStepProps) {
  const { t } = useTranslation();

  return (
    <div>
      <p className="setup-deps-title">{t.wizardMissingSystemDependencies}</p>
      <p className="setup-deps-notes">{result.platform_notes}</p>
      <div className="setup-deps-list">
        {result.dependencies
          .filter((dep) => !dep.installed)
          .map((dep) => (
            <div key={dep.name} className="setup-dep">
              <div className="setup-dep-name">{dep.name}</div>
              <div className="setup-dep-description">{dep.description}</div>
              <div className="setup-dep-command-label">{t.wizardInstallCommand}</div>
              <div className="setup-dep-commands">
                {dep.install_commands.map((cmd) => (
                  <div key={cmd.distribution} className="setup-dep-command">
                    <div className="setup-dep-distribution">{cmd.distribution}:</div>
                    <code>{cmd.command}</code>
                  </div>
                ))}
              </div>
            </div>
          ))}
      </div>
      <p className="setup-deps-retry">{t.wizardDependencyRetryCopy}</p>
      <div className="setup-actions">
        <button onClick={onBack} className="btn-secondary setup-action">
          {t.back}
        </button>
        <button onClick={onRetry} className="btn-primary setup-action">
          {t.wizardRetryCheck}
        </button>
      </div>
    </div>
  );
}
