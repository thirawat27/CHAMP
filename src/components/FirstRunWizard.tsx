import { useState, useEffect, type HTMLAttributes } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  DownloadProgress as DownloadProgressType,
  PackageSelection,
  DependencyCheckResult,
  getDatabaseDisplayName,
} from "../types/services";
import { CheckCircle2 } from "lucide-react";
import champLogo from "../assets/CHAMP.png";
import { PackageSelector } from "./PackageSelector";

// Helper to detect platform
const detectPlatform = (): string => {
  const userAgent = window.navigator.userAgent.toLowerCase();
  if (userAgent.includes("win")) return "windows";
  if (userAgent.includes("mac")) return "darwin";
  return "linux";
};

interface FirstRunWizardProps extends HTMLAttributes<HTMLDivElement> {
  onComplete: () => void;
}

type WizardStep = "welcome" | "packages" | "dependencies" | "confirm" | "download" | "complete";

interface ExistingComponent {
  name: string;
  version: string;
  displayName: string;
  isExisting: boolean;
}

// Package versions for display
const packages = {
  php: [
    { id: "php-8.5", version: "8.5.x" },
    { id: "php-8.4", version: "8.4.16" },
    { id: "php-8.3", version: "8.3.29" },
    { id: "php-8.2", version: "8.2.30" },
    { id: "php-7.4", version: "7.4.33" },
  ],
  mysql: [
    { id: "mysql-9.7", version: "9.7.0" },
  ],
  phpmyadmin: [
    { id: "phpmyadmin-5.2", version: "5.2.3", name: "phpMyAdmin" },
    { id: "adminer-5.4", version: "5.4.2", name: "Adminer" },
  ],
};

const SETUP_STEPS = [
  { title: "Welcome", copy: "Runtime overview" },
  { title: "Packages", copy: "Choose versions" },
  { title: "Checks", copy: "System readiness" },
  { title: "Review", copy: "Keep or replace" },
  { title: "Install", copy: "Download runtime" },
];

export function FirstRunWizard({ onComplete, ...props }: FirstRunWizardProps) {
  const [step, setStep] = useState<WizardStep>("welcome");
  const [currentPlatform, setCurrentPlatform] = useState<string>("linux");
  const [progress, setProgress] = useState<DownloadProgressType>({
    step: "downloading",
    percent: 0,
    currentComponent: "",
    componentDisplay: "",
    version: "",
    totalComponents: 4,
    downloadedBytes: 0,
    totalBytes: 0,
  });
  const [error, setError] = useState<string | null>(null);
  const [packageSelection, setPackageSelection] = useState<PackageSelection>({
    php: "php-8.5",
    mysql: "mysql-9.7",
    phpmyadmin: "phpmyadmin-5.2",
  });
  const [existingComponents, setExistingComponents] = useState<ExistingComponent[]>([]);
  const [hasExistingOnWelcome, setHasExistingOnWelcome] = useState(false);
  const [dependencyCheckResult, setDependencyCheckResult] = useState<DependencyCheckResult | null>(
    null
  );

  // Check for existing components when welcome step loads
  useEffect(() => {
    const checkExisting = async () => {
      try {
        const existing = await invoke<Record<string, string>>("check_existing_components");
        const hasExisting = Object.keys(existing).length > 0;
        setHasExistingOnWelcome(hasExisting);
      } catch (err) {
        console.error("Failed to check existing components:", err);
        setHasExistingOnWelcome(false);
      }
    };

    if (step === "welcome") {
      checkExisting();
    }
  }, [step]);

  // Detect platform on mount
  useEffect(() => {
    setCurrentPlatform(detectPlatform());
  }, []);

  // Listen for download progress events
  useEffect(() => {
    const unlisten = listen<DownloadProgressType>("download-progress", (event) => {
      setProgress(event.payload);
      if (event.payload.step === "complete") {
        setStep("complete");
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Keyboard shortcut handler for Ctrl+Shift+D / Cmd+Shift+D
  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === "d") {
        e.preventDefault();
        try {
          const downloadDir = await invoke<string>("get_download_dir");
          await invoke("open_folder", { path: downloadDir });
        } catch (err) {
          console.error("Failed to open download folder:", err);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const startDownload = async () => {
    setError(null);

    // First, check system dependencies
    try {
      const depsResult = await invoke<DependencyCheckResult>("check_system_dependencies");
      setDependencyCheckResult(depsResult);

      if (!depsResult.all_satisfied) {
        setStep("dependencies");
        return;
      }
    } catch (err) {
      console.error("Failed to check dependencies:", err);
      // Continue anyway if dependency check fails
    }

    // Then, check for existing components
    try {
      const existing = await invoke<Record<string, string>>("check_existing_components");

      const isAdminerSelected = packageSelection.phpmyadmin.startsWith("adminer");
      const existingDatabaseToolVersion = isAdminerSelected
        ? existing.adminer || ""
        : existing.phpmyadmin || "";

      // All components that should be shown
      const allComponents: ExistingComponent[] = [
        {
          name: "caddy",
          version: existing.caddy || "",
          displayName: "Caddy",
          isExisting: !!existing.caddy,
        },
        {
          name: "php",
          version: existing.php || "",
          displayName: "PHP",
          isExisting: !!existing.php,
        },
        {
          name: "mysql",
          version: existing.mysql || "",
          displayName: getDatabaseDisplayName(currentPlatform),
          isExisting: !!existing.mysql,
        },
        {
          name: packageSelection.phpmyadmin.startsWith("adminer") ? "adminer" : "phpmyadmin",
          displayName: packageSelection.phpmyadmin.startsWith("adminer") ? "Adminer" : "phpMyAdmin",
          isExisting: !!existingDatabaseToolVersion,
          version: existingDatabaseToolVersion,
        },
      ];

      setExistingComponents(allComponents);
      setStep("confirm");
      return;
    } catch (err) {
      console.error("Failed to check existing components:", err);
    }

    // No existing components, proceed with download
    proceedWithDownload([]);
  };

  const proceedWithDownload = async (skipList: string[]) => {
    setError(null);
    setStep("download");

    try {
      // Save the package selection to settings
      await invoke("update_package_selection", { packageSelection });

      if (skipList.length > 0) {
        const result = await invoke<string>("download_runtime_with_skip", {
          packageSelection,
          skipList,
        });
        console.log(result);
      } else {
        const result = await invoke<string>("download_runtime_with_packages", { packageSelection });
        console.log(result);
      }
    } catch (err) {
      console.error("Download error:", err);
      setError(err as string);
      setStep("confirm");
    }
  };

  const handleOverwriteAll = () => {
    proceedWithDownload([]);
  };

  const handleSkipExisting = () => {
    const skipList = existingComponents.filter((c) => c.isExisting).map((c) => c.name);
    proceedWithDownload(skipList);
  };

  const handleCancel = () => {
    setStep("packages");
    setExistingComponents([]);
  };

  const handleSkipToDashboard = () => {
    onComplete();
  };

  const handleSkipFromWelcome = async () => {
    try {
      const existing = await invoke<Record<string, string>>("check_existing_components");
      const existingList = Object.keys(existing);

      if (existingList.length > 0) {
        onComplete();
      } else {
        alert(
          "No existing installation found. Please download the runtime components to continue."
        );
      }
    } catch (err) {
      console.error("Failed to check existing components:", err);
    }
  };

  const handleNext = () => {
    if (step === "welcome") {
      setStep("packages");
    }
  };

  const handleBack = () => {
    if (step === "packages") {
      setStep("welcome");
    } else if (step === "dependencies") {
      setStep("packages");
    }
  };

  const handlePackageChange = (selection: PackageSelection) => {
    setPackageSelection(selection);
  };

  const getStepLabel = () => {
    switch (progress.step) {
      case "downloading":
        return progress.componentDisplay
          ? `Download ${progress.componentDisplay}`
          : "Downloading...";
      case "extracting":
        return progress.componentDisplay ? `Extract ${progress.componentDisplay}` : "Extracting...";
      case "installing":
        return "Installing...";
      case "complete":
        return "Installation complete!";
      case "error":
        return "An error occurred";
      default:
        return "Preparing...";
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  };

  const getStepNumber = () => {
    switch (step) {
      case "welcome":
        return 1;
      case "packages":
        return 2;
      case "dependencies":
        return 3;
      case "confirm":
        return 4;
      case "download":
        return 5;
      case "complete":
        return 5;
      default:
        return 1;
    }
  };

  const currentStepNum = getStepNumber();
  const currentStep = SETUP_STEPS[currentStepNum - 1];
  const shownPercent = Math.max(0, Math.min(100, progress.percent));
  const isProgressIndeterminate =
    (progress.step === "downloading" && (shownPercent === 0 || progress.totalBytes === 0)) ||
    progress.step === "extracting" ||
    progress.step === "installing";

  return (
    <div className="setup-shell" {...props}>
      <div className="setup-card">
        <aside className="setup-rail">
          <div className="setup-brand">
            <img className="setup-brand-logo" src={champLogo} alt="" />
            <h1>CHAMP</h1>
          </div>
          <p>Caddy + HTTP(S) + phpMyAdmin/Adminer + MySQL + PHP</p>
          <div className="setup-steps">
            {SETUP_STEPS.map((setupStep, index) => {
              const stepIndex = index + 1;
              const stateClass =
                currentStepNum === stepIndex ? "active" : currentStepNum > stepIndex ? "done" : "";
              return (
                <div key={setupStep.title} className={`setup-step ${stateClass}`}>
                  <span className="setup-step-index">{stepIndex}</span>
                  <span>
                    <span className="setup-step-title">{setupStep.title}</span>
                    <span className="setup-step-copy">{setupStep.copy}</span>
                  </span>
                </div>
              );
            })}
          </div>
        </aside>

        <section className="setup-main">
          <h2>{currentStep.title}</h2>
          <p>{currentStep.copy}</p>

          {/* Content */}
          <div className="setup-content">
            {/* Welcome Step */}
            {step === "welcome" && (
              <div>
                <p
                  style={{
                    fontSize: "0.875rem",
                    lineHeight: 1.5,
                    color: "var(--text-primary)",
                    marginBottom: "0.75rem",
                  }}
                >
                  CHAMP installs Caddy, PHP-FPM, {getDatabaseDisplayName(currentPlatform)}, and
                  phpMyAdmin or Adminer into your user profile so the stack can run without writing config files
                  into protected system folders.
                </p>
                <div className="setup-summary">
                  <div className="setup-summary-item">
                    <strong>User-space runtime</strong>
                    <span>No admin write access for generated configs.</span>
                  </div>
                  <div className="setup-summary-item">
                    <strong>Dev-first ports</strong>
                    <span>HTTP 8080, PHP 9000, MySQL 3307.</span>
                  </div>
                  <div className="setup-summary-item">
                    <strong>phpMyAdmin ready</strong>
                    <span>Database tools open at /phpmyadmin by default.</span>
                  </div>
                </div>
                {hasExistingOnWelcome ? (
                  <div className="setup-callout">
                    <strong>Existing installation detected!</strong>
                  </div>
                ) : (
                  <div
                    className="info-box"
                    style={{ marginBottom: "0.75rem", padding: "0.5rem", fontSize: "0.875rem" }}
                  >
                    <strong>Estimated download size:</strong> ~150 MB
                  </div>
                )}
                <div className="setup-actions">
                  {hasExistingOnWelcome && (
                    <button
                      onClick={handleSkipFromWelcome}
                      className="btn-secondary setup-btn-existing"
                      style={{ fontSize: "0.875rem", padding: "0.5rem 1rem" }}
                    >
                      Use Existing
                    </button>
                  )}
                  <button
                    onClick={handleNext}
                    className="btn-primary setup-btn-next"
                    style={{ fontSize: "0.875rem", padding: "0.5rem 1rem" }}
                  >
                    {hasExistingOnWelcome ? "Download Fresh" : "Get Started"}
                  </button>
                  <button
                    onClick={async () => {
                      try {
                        await invoke("open_manual");
                      } catch (err) {
                        console.error("Failed to open manual:", err);
                      }
                    }}
                    className="btn-secondary setup-btn-help"
                    title="Read User Manual"
                    style={{ fontSize: "0.875rem", padding: "0.5rem 0.75rem" }}
                  >
                    ?
                  </button>
                </div>
              </div>
            )}

            {/* Package Selection Step */}
            {step === "packages" && (
              <div>
                <p style={{ fontSize: "0.875rem", marginBottom: "0.75rem" }}>
                  Select the versions of PHP, {getDatabaseDisplayName(currentPlatform)}, and
                  phpMyAdmin or Adminer.
                </p>
                <PackageSelector
                  onSelectionChange={handlePackageChange}
                  initialSelection={packageSelection}
                />
                <div className="setup-actions">
                  <button
                    onClick={handleBack}
                    className="btn-secondary setup-btn-back"
                    style={{ fontSize: "0.875rem", padding: "0.5rem 1rem" }}
                  >
                    Back
                  </button>
                  <button
                    onClick={startDownload}
                    className="btn-primary setup-btn-next"
                    style={{ fontSize: "0.875rem", padding: "0.5rem 1rem" }}
                  >
                    Download & Install
                  </button>
                </div>
              </div>
            )}

            {/* Dependencies Step */}
            {step === "dependencies" && dependencyCheckResult && (
              <div>
                <p
                  style={{
                    fontSize: "0.875rem",
                    marginBottom: "0.5rem",
                    color: "var(--color-error)",
                    fontWeight: 600,
                  }}
                >
                  Missing System Dependencies
                </p>
                <p
                  style={{
                    fontSize: "0.875rem",
                    marginBottom: "0.75rem",
                    color: "var(--text-secondary)",
                  }}
                >
                  {dependencyCheckResult.platform_notes}
                </p>
                <div
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: "0.5rem",
                    margin: "0.5rem 0",
                    padding: "0.5rem",
                    backgroundColor: "rgba(239, 68, 68, 0.1)",
                    borderRadius: "0.375rem",
                    border: "1px solid var(--color-error)",
                  }}
                >
                  {dependencyCheckResult.dependencies
                    .filter((dep) => !dep.installed)
                    .map((dep) => (
                      <div key={dep.name} style={{ marginBottom: "0.5rem" }}>
                        <div
                          style={{ fontWeight: 600, marginBottom: "0.25rem", fontSize: "0.875rem" }}
                        >
                          {dep.name}
                        </div>
                        <div
                          style={{
                            fontSize: "0.8125rem",
                            color: "var(--text-secondary)",
                            marginBottom: "0.375rem",
                          }}
                        >
                          {dep.description}
                        </div>
                        <div
                          style={{
                            fontSize: "0.8125rem",
                            fontWeight: 500,
                            marginBottom: "0.25rem",
                          }}
                        >
                          Install command for your distribution:
                        </div>
                        <div
                          style={{
                            display: "flex",
                            flexDirection: "column",
                            gap: "0.25rem",
                          }}
                        >
                          {dep.install_commands.map((cmd) => (
                            <div
                              key={cmd.distribution}
                              style={{
                                backgroundColor: "var(--bg-card)",
                                padding: "0.375rem 0.5rem",
                                borderRadius: "0.25rem",
                                fontSize: "0.75rem",
                              }}
                            >
                              <div style={{ fontWeight: 600, marginBottom: "0.125rem" }}>
                                {cmd.distribution}:
                              </div>
                              <code
                                style={{
                                  fontFamily: "monospace",
                                  fontSize: "0.75rem",
                                  wordBreak: "break-all",
                                }}
                              >
                                {cmd.command}
                              </code>
                            </div>
                          ))}
                        </div>
                      </div>
                    ))}
                </div>
                <p
                  style={{
                    fontSize: "0.8125rem",
                    color: "var(--text-secondary)",
                    marginBottom: "0.75rem",
                  }}
                >
                  After installing the dependencies, click &quot;Retry Check&quot; to continue.
                </p>
                <div className="setup-actions">
                  <button
                    onClick={handleBack}
                    className="btn-secondary setup-btn-back"
                    style={{ fontSize: "0.875rem", padding: "0.5rem 1rem" }}
                  >
                    Back
                  </button>
                  <button
                    onClick={startDownload}
                    className="btn-primary setup-btn-next"
                    style={{ fontSize: "0.875rem", padding: "0.5rem 1rem" }}
                  >
                    Retry Check
                  </button>
                </div>
              </div>
            )}

            {/* Confirm Overwrite Step */}
            {step === "confirm" && (
              <div>
                <p style={{ fontSize: "0.875rem", marginBottom: "0.5rem" }}>
                  Installation summary:
                </p>
                <div
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: "0.375rem",
                    margin: "0.5rem 0",
                    padding: "0.5rem",
                    backgroundColor: "var(--bg-card-secondary)",
                    borderRadius: "0.375rem",
                    border: "1px solid var(--border-color)",
                  }}
                >
                  {existingComponents.map((component) => {
                    const newVersion =
                      component.name === "php"
                        ? packages.php.find((p) => p.id === packageSelection.php)?.version
                        : component.name === "mysql"
                          ? packages.mysql.find((p) => p.id === packageSelection.mysql)?.version
                          : component.name === "adminer" || component.name === "phpmyadmin"
                            ? packages.phpmyadmin.find((p) => p.id === packageSelection.phpmyadmin)
                                ?.version
                            : component.name === "caddy"
                              ? "2.11.2"
                              : component.version;

                    return (
                      <div
                        key={component.name}
                        style={{
                          display: "flex",
                          justifyContent: "space-between",
                          alignItems: "center",
                          padding: "0.375rem",
                          backgroundColor: "var(--bg-card)",
                          borderRadius: "0.25rem",
                          fontSize: "0.875rem",
                          border: component.isExisting
                            ? "1px solid var(--color-warning)"
                            : "1px solid transparent",
                        }}
                      >
                        <span style={{ fontWeight: 500 }}>
                          {component.displayName}
                        </span>
                        <div style={{ display: "flex", alignItems: "center", gap: "0.375rem" }}>
                          <span
                            style={{
                              fontSize: "0.75rem",
                              color: "var(--color-success)",
                              fontWeight: 500,
                            }}
                          >
                            {newVersion || "Unknown"}
                          </span>
                        </div>
                      </div>
                    );
                  })}
                </div>
                {error && (
                  <div
                    className="error-box"
                    style={{ marginBottom: "0.5rem", padding: "0.5rem", fontSize: "0.875rem" }}
                  >
                    <p className="error-box-text" style={{ margin: "0 0 0.375rem 0" }}>
                      <strong>Error:</strong> {error}
                    </p>
                    <button
                      onClick={() => setError(null)}
                      style={{
                        padding: "0.25rem 0.5rem",
                        borderRadius: "0.25rem",
                        border: "1px solid var(--color-error)",
                        backgroundColor: "transparent",
                        color: "var(--color-error)",
                        fontSize: "0.75rem",
                        cursor: "pointer",
                      }}
                    >
                      Dismiss
                    </button>
                  </div>
                )}
                <div className="setup-actions">
                  <button
                    onClick={handleCancel}
                    className="btn-secondary setup-btn-back"
                    style={{ fontSize: "0.875rem", padding: "0.5rem 0.75rem" }}
                  >
                    Back
                  </button>
                  <button
                    onClick={handleSkipToDashboard}
                    className="btn-secondary setup-btn-existing"
                    style={{ fontSize: "0.875rem", padding: "0.5rem 0.75rem" }}
                  >
                    Use Existing
                  </button>
                  <button
                    onClick={handleSkipExisting}
                    className="btn-secondary setup-btn-keep"
                    style={{ fontSize: "0.875rem", padding: "0.5rem 0.75rem" }}
                  >
                    Keep & Install
                  </button>
                  <button
                    onClick={handleOverwriteAll}
                    className="btn-primary setup-btn-install-all"
                    style={{ fontSize: "0.875rem", padding: "0.5rem 1rem" }}
                  >
                    Install All
                  </button>
                </div>
              </div>
            )}

            {/* Download Step */}
            {step === "download" && (
              <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
                {/* Progress Header */}
                <div
                  style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}
                >
                  <h3 style={{ fontSize: "1rem", fontWeight: 600 }}>{getStepLabel()}</h3>
                  {progress.step === "downloading" && (
                    <span className="progress-percent">
                      {progress.totalBytes > 0 && shownPercent > 0 ? `${shownPercent}%` : ""}
                    </span>
                  )}
                </div>

                {/* Current Component Info */}
                {progress.componentDisplay && (
                  <div
                    style={{
                      backgroundColor: "var(--bg-card-secondary)",
                      borderRadius: "0.375rem",
                      padding: "0.5rem",
                    }}
                  >
                    <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                      <span style={{ fontSize: "0.875rem", fontWeight: 600 }}>
                        {progress.currentComponent || progress.componentDisplay}
                      </span>
                      {progress.version && (
                        <span
                          style={{
                            padding: "0.125rem 0.375rem",
                            backgroundColor: "var(--color-primary)",
                            borderRadius: "0.25rem",
                            fontSize: "0.75rem",
                            fontWeight: 600,
                            color: "white",
                          }}
                        >
                          {progress.version}
                        </span>
                      )}
                    </div>
                  </div>
                )}

                {/* Progress Bar */}
                <div className="progress-container">
                  <div
                    className={isProgressIndeterminate ? "progress-bar-animated" : "progress-bar"}
                    style={{
                      width: isProgressIndeterminate ? undefined : `${shownPercent}%`,
                    }}
                  />
                </div>

                {/* Download Details */}
                {progress.step === "downloading" && progress.totalBytes > 0 && (
                  <div
                    style={{
                      textAlign: "center",
                      fontSize: "0.875rem",
                      color: "var(--text-secondary)",
                    }}
                  >
                    <span>
                      {formatBytes(progress.downloadedBytes)} / {formatBytes(progress.totalBytes)}
                    </span>
                  </div>
                )}

                {/* Error Display */}
                {error && (
                  <div className="error-box" style={{ padding: "0.5rem", fontSize: "0.875rem" }}>
                    <p className="error-box-text" style={{ margin: "0 0 0.375rem 0" }}>
                      Error: {error}
                    </p>
                    <button
                      onClick={() => setError(null)}
                      style={{
                        padding: "0.25rem 0.5rem",
                        borderRadius: "0.25rem",
                        border: "1px solid var(--color-error)",
                        backgroundColor: "transparent",
                        color: "var(--color-error)",
                        fontSize: "0.75rem",
                        cursor: "pointer",
                      }}
                    >
                      Dismiss
                    </button>
                  </div>
                )}
              </div>
            )}

            {/* Complete Step */}
            {step === "complete" && (
              <div className="setup-complete">
                <div className="setup-complete-icon" aria-hidden="true">
                  <CheckCircle2 size={34} strokeWidth={2.4} />
                </div>
                <h3>Runtime installed</h3>
                <p>CHAMP is ready to start the local web stack.</p>
                <div className="setup-complete-packages">
                  {[
                    { name: "Caddy", version: "2.11.2" },
                    {
                      name: "PHP",
                      version:
                        packages.php.find((p) => p.id === packageSelection.php)?.version || "8.5.x",
                    },
                    {
                      name: getDatabaseDisplayName(currentPlatform),
                      version:
                        packages.mysql.find((p) => p.id === packageSelection.mysql)?.version ||
                        "9.7.0",
                    },
                    {
                      name:
                        packages.phpmyadmin.find((p) => p.id === packageSelection.phpmyadmin)
                          ?.name || "phpMyAdmin",
                      version:
                        packages.phpmyadmin.find((p) => p.id === packageSelection.phpmyadmin)
                        ?.version || "5.2.3",
                    },
                  ].map((pkg) => (
                    <div key={pkg.name} className="setup-complete-package">
                      <span>{pkg.name}</span>
                      <strong>{pkg.version}</strong>
                    </div>
                  ))}
                </div>
                <button
                  onClick={onComplete}
                  className="btn-primary setup-btn-next"
                  style={{ fontSize: "0.875rem", padding: "0.5rem 1rem" }}
                >
                  Continue to Dashboard
                </button>
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
