import { invoke } from "@tauri-apps/api/core";
import { HTMLAttributes, useEffect, useState } from "react";
import { useRuntimeInstall, WizardStep } from "../hooks/useRuntimeInstall";
import { useTranslation } from "../stores/preferencesStore";
import {
  EMPTY_PACKAGE_SELECTION,
  PackageSelection,
  PackagesConfig,
  getDatabaseEngine,
  normalizeSelection,
} from "../types/services";
import { GuideModal } from "./GuideModal";
import { LanguageSelector } from "./LanguageSelector";
import { CompleteStep } from "./wizard/CompleteStep";
import { ConfirmStep } from "./wizard/ConfirmStep";
import { DependenciesStep } from "./wizard/DependenciesStep";
import { DownloadStep } from "./wizard/DownloadStep";
import { PackagesStep } from "./wizard/PackagesStep";
import { SetupRail } from "./wizard/SetupRail";
import { WelcomeStep } from "./wizard/WelcomeStep";

interface FirstRunWizardProps extends HTMLAttributes<HTMLDivElement> {
  onComplete: () => void;
}

/** Position in the setup rail: the install spans the download and complete steps. */
const STEP_NUMBERS: Record<WizardStep, number> = {
  welcome: 1,
  packages: 2,
  dependencies: 3,
  confirm: 4,
  download: 5,
  complete: 5,
};

export function FirstRunWizard({ onComplete, ...props }: FirstRunWizardProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState<WizardStep>("welcome");
  const [showGuide, setShowGuide] = useState(false);
  const [packageSelection, setPackageSelection] =
    useState<PackageSelection>(EMPTY_PACKAGE_SELECTION);
  const [caddyVersion, setCaddyVersion] = useState("");
  const [hasExistingOnWelcome, setHasExistingOnWelcome] = useState(false);
  const [availablePackages, setAvailablePackages] = useState<PackagesConfig | null>(null);
  const [runtimePlatformKey, setRuntimePlatformKey] = useState("");
  const [metadataFailed, setMetadataFailed] = useState(false);
  const install = useRuntimeInstall(packageSelection, setStep);

  // Check for existing components when welcome step loads
  useEffect(() => {
    if (step !== "welcome") return;
    invoke<Record<string, string>>("check_existing_components")
      .then((existing) => setHasExistingOnWelcome(Object.keys(existing).length > 0))
      .catch((err) => {
        console.error("Failed to check existing components:", err);
        setHasExistingOnWelcome(false);
      });
  }, [step]);

  // Load package metadata on mount. The catalog is refreshed first so the
  // default selection is the newest PHP (and database) release.
  useEffect(() => {
    invoke<PackagesConfig>("refresh_runtime_catalog")
      .catch(() => invoke<PackagesConfig>("get_available_packages_cmd"))
      .then((packages) =>
        Promise.all([
          packages,
          invoke<PackageSelection>("get_selected_package_ids"),
          invoke<Record<string, string>>("get_installed_versions"),
          invoke<string>("get_runtime_platform"),
        ])
      )
      .then(([packages, selectedPackages, versions, platformKey]) => {
        setAvailablePackages(packages);
        setRuntimePlatformKey(platformKey);
        setPackageSelection(normalizeSelection(selectedPackages, packages, platformKey));
        setCaddyVersion(versions.caddy || "");
      })
      .catch((err) => {
        console.error("Failed to load package metadata:", err);
        setMetadataFailed(true);
      });
  }, []);

  const handleSkipFromWelcome = async () => {
    try {
      const existing = await invoke<Record<string, string>>("check_existing_components");

      if (Object.keys(existing).length > 0) {
        onComplete();
      } else {
        alert(t.wizardNoExistingInstallation);
      }
    } catch (err) {
      console.error("Failed to check existing components:", err);
    }
  };

  const setupSteps = [
    { title: t.welcome, copy: t.wizardRuntimeOverview },
    { title: t.wizardPackagesStep, copy: t.wizardChooseVersions },
    { title: t.wizardChecksStep, copy: t.wizardSystemReadiness },
    { title: t.wizardReviewStep, copy: t.wizardKeepOrReplace },
    { title: t.wizardInstallStep, copy: t.wizardDownloadRuntime },
  ];
  const currentStepNumber = STEP_NUMBERS[step];
  const currentStep = setupSteps[currentStepNumber - 1];
  const selectedPhp = availablePackages?.php.find((pkg) => pkg.id === packageSelection.php);
  const selectedMysql = availablePackages?.mysql.find((pkg) => pkg.id === packageSelection.mysql);
  const selectedPostgreSQL = availablePackages?.postgresql.find(
    (pkg) => pkg.id === packageSelection.postgresql
  );
  const selectedDatabaseTool = availablePackages?.phpmyadmin.find(
    (pkg) => pkg.id === packageSelection.phpmyadmin
  );
  const postgresqlSelected = getDatabaseEngine(packageSelection) === "postgresql";

  return (
    <div className="setup-shell" {...props}>
      <div className="setup-card">
        <div className="setup-language-switcher">
          <LanguageSelector />
        </div>
        <SetupRail steps={setupSteps} current={currentStepNumber} />

        <section className="setup-main">
          <h2>{currentStep.title}</h2>
          <p>{currentStep.copy}</p>

          <div className="setup-content">
            {step === "welcome" && (
              <WelcomeStep
                hasExisting={hasExistingOnWelcome}
                onUseExisting={handleSkipFromWelcome}
                onNext={() => setStep("packages")}
                onOpenGuide={() => setShowGuide(true)}
              />
            )}

            {step === "packages" && (
              <PackagesStep
                selection={packageSelection}
                packages={availablePackages}
                platformKey={runtimePlatformKey}
                loadFailed={metadataFailed}
                onSelectionChange={setPackageSelection}
                onBack={() => setStep("welcome")}
                onInstall={install.startDownload}
              />
            )}

            {step === "dependencies" && install.dependencyCheckResult && (
              <DependenciesStep
                result={install.dependencyCheckResult}
                onBack={() => setStep("packages")}
                onRetry={install.startDownload}
              />
            )}

            {step === "confirm" && (
              <ConfirmStep
                components={install.existingComponents}
                newVersions={{
                  caddy: caddyVersion,
                  php: selectedPhp?.version,
                  mysql: selectedMysql?.version,
                  postgresql: selectedPostgreSQL?.version,
                  adminer: selectedDatabaseTool?.version,
                  phpmyadmin: selectedDatabaseTool?.version,
                }}
                error={install.error}
                onDismissError={install.clearError}
                onBack={install.cancelReview}
                onUseExisting={onComplete}
                onKeepAndInstall={install.keepExistingAndInstall}
                onInstallAll={install.installAll}
              />
            )}

            {step === "download" && (
              <DownloadStep
                progress={install.progress}
                error={install.error}
                onDismissError={install.clearError}
              />
            )}

            {step === "complete" && (
              <CompleteStep
                installed={[
                  { name: "Caddy", version: caddyVersion },
                  { name: "PHP", version: selectedPhp?.version || packageSelection.php },
                  postgresqlSelected
                    ? {
                        name: "PostgreSQL",
                        version: selectedPostgreSQL?.version || packageSelection.postgresql,
                      }
                    : {
                        name: "MySQL",
                        version: selectedMysql?.version || packageSelection.mysql,
                      },
                  {
                    name: selectedDatabaseTool?.display_name.split(" ")[0] || "phpMyAdmin",
                    version: selectedDatabaseTool?.version || packageSelection.phpmyadmin,
                  },
                ]}
                onContinue={onComplete}
              />
            )}
          </div>
        </section>
      </div>
      {showGuide && <GuideModal onClose={() => setShowGuide(false)} initialSection="start" />}
    </div>
  );
}
