import { useTranslation } from "../../stores/preferencesStore";
import { PackageSelection, PackagesConfig } from "../../types/services";
import { PackageSelector } from "../PackageSelector";

interface PackagesStepProps {
  selection: PackageSelection;
  packages: PackagesConfig | null;
  platformKey: string;
  loadFailed: boolean;
  onSelectionChange: (selection: PackageSelection) => void;
  onBack: () => void;
  onInstall: () => void;
}

export function PackagesStep({
  selection,
  packages,
  platformKey,
  loadFailed,
  onSelectionChange,
  onBack,
  onInstall,
}: PackagesStepProps) {
  const { t } = useTranslation();

  return (
    <div>
      <p className="setup-intro">{t.wizardPackageIntro}</p>
      <PackageSelector
        selection={selection}
        packages={packages}
        platformKey={platformKey}
        loadFailed={loadFailed}
        onSelectionChange={onSelectionChange}
      />
      <div className="setup-actions">
        <button onClick={onBack} className="btn-secondary setup-action">
          {t.back}
        </button>
        <button onClick={onInstall} className="btn-primary setup-action" disabled={!packages}>
          {t.wizardDownloadAndInstall}
        </button>
      </div>
    </div>
  );
}
