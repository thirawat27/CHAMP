import { invoke } from "@tauri-apps/api/core";
import {
  ArrowUpCircle,
  BrainCircuit,
  CheckCircle2,
  Database,
  Download,
  FileCode2,
  LoaderCircle,
  Network,
  Power,
  Radar,
  Repeat,
  Settings2,
  Trash2,
  Volume2,
  X,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import {
  AppSettings,
  DEFAULT_PORTS,
  DownloadProgress,
  InstalledPhpVersion,
  PackagesConfig,
  PackageSelection,
  ServiceType,
  EMPTY_PACKAGE_SELECTION,
  hasPackageUrlForPlatform,
  isAdminerSelected,
} from "../types/services";
import { formatModelSize, useLogDoctorModel } from "../hooks/useLogDoctorModel";
import { useLanguageStore, useTranslation } from "../stores/languageStore";
import { AudioManager } from "../utils/audioManager";
import { listenIfTauri } from "../utils/tauri";
import { AgentConnectorSection } from "./settings/AgentConnectorSection";
import { UpdatesSection } from "./settings/UpdatesSection";

interface SettingsPanelProps {
  onClose: () => void;
  onSettingsChanged?: () => void;
  [key: string]: unknown;
}

interface PortCheck {
  web: { port: number; available: boolean };
  php: { port: number; available: boolean };
  mysql: { port: number; available: boolean };
  postgresql: { port: number; available: boolean };
}

const defaultPackageSelection: PackageSelection = EMPTY_PACKAGE_SELECTION;

export function SettingsPanel({ onClose, onSettingsChanged, ...props }: SettingsPanelProps) {
  const { t } = useTranslation();
  const { soundEnabled, toggleSound } = useLanguageStore();
  const doctorModel = useLogDoctorModel();

  // ESC to close
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.code === "Escape") {
        e.preventDefault();
        AudioManager.playClick();
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);
  const [settings, setSettings] = useState<AppSettings>({
    web_port: DEFAULT_PORTS[ServiceType.Caddy],
    php_port: DEFAULT_PORTS[ServiceType.PhpFpm],
    mysql_port: DEFAULT_PORTS[ServiceType.MySQL],
    postgresql_port: DEFAULT_PORTS[ServiceType.PostgreSQL],
    auto_start_services: false,
    package_selection: defaultPackageSelection,
  });
  const [packages, setPackages] = useState<PackagesConfig | null>(null);
  const [runtimePlatformKey, setRuntimePlatformKey] = useState("");
  const [phpVersions, setPhpVersions] = useState<InstalledPhpVersion[]>([]);
  const [selectedPhpId, setSelectedPhpId] = useState(defaultPackageSelection.php);
  const [phpBusy, setPhpBusy] = useState(false);
  const [portCheck, setPortCheck] = useState<PortCheck | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveProgress, setSaveProgress] = useState<DownloadProgress | null>(null);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadSettings = useCallback(async () => {
    try {
      const [loaded, availablePackages, installedPhp, platformKey] = await Promise.all([
        invoke<AppSettings>("get_settings"),
        invoke<PackagesConfig>("get_available_packages_cmd"),
        invoke<InstalledPhpVersion[]>("get_installed_php_versions"),
        invoke<string>("get_runtime_platform"),
      ]);
      const packageSelection = loaded.package_selection ?? defaultPackageSelection;
      const availablePhpPackages = availablePackages.php.filter((pkg) =>
        hasPackageUrlForPlatform(pkg, platformKey)
      );
      const availableMysqlPackages = availablePackages.mysql.filter((pkg) =>
        hasPackageUrlForPlatform(pkg, platformKey)
      );
      const availablePostgreSQLPackages = availablePackages.postgresql.filter((pkg) =>
        hasPackageUrlForPlatform(pkg, platformKey)
      );
      const normalizedPackageSelection = {
        ...defaultPackageSelection,
        ...packageSelection,
      };
      if (
        !availablePhpPackages.some((pkg) => pkg.id === normalizedPackageSelection.php) &&
        availablePhpPackages[0]
      ) {
        normalizedPackageSelection.php = availablePhpPackages[0].id;
      }
      if (
        !availableMysqlPackages.some((pkg) => pkg.id === normalizedPackageSelection.mysql) &&
        availableMysqlPackages[0]
      ) {
        normalizedPackageSelection.mysql = availableMysqlPackages[0].id;
      }
      if (
        !availablePostgreSQLPackages.some(
          (pkg) => pkg.id === normalizedPackageSelection.postgresql
        ) &&
        availablePostgreSQLPackages[0]
      ) {
        normalizedPackageSelection.postgresql = availablePostgreSQLPackages[0].id;
      }
      if (
        !availablePackages.phpmyadmin.some(
          (pkg) => pkg.id === normalizedPackageSelection.phpmyadmin
        ) &&
        availablePackages.phpmyadmin[0]
      ) {
        normalizedPackageSelection.phpmyadmin = availablePackages.phpmyadmin[0].id;
      }
      setSettings({
        ...loaded,
        auto_start_services: loaded.auto_start_services ?? false,
        package_selection: normalizedPackageSelection,
      });
      setPackages(availablePackages);
      setRuntimePlatformKey(platformKey);
      setPhpVersions(installedPhp);
      setSelectedPhpId(normalizedPackageSelection.php);

      invoke<PackagesConfig>("refresh_runtime_catalog")
        .then((freshPackages) => setPackages(freshPackages))
        .catch((refreshError) => {
          console.warn(t.runtimeCatalogUpdateFailed, refreshError);
        });
    } catch (e) {
      setError(`${t.failedToLoadSettings}: ${e}`);
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  useEffect(() => {
    const unlisten = listenIfTauri<DownloadProgress>("download-progress", (event) => {
      setSaveProgress(event.payload);
    });

    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, []);

  const handlePortChange = (
    field: "web_port" | "php_port" | "mysql_port" | "postgresql_port",
    value: string
  ) => {
    const next = Number.parseInt(value, 10);
    if (Number.isNaN(next) || next < 1 || next > 65535) return;
    setSettings((current) => ({ ...current, [field]: next }));
    setPortCheck(null);
  };

  const checkPorts = async () => {
    const result = await invoke<PortCheck>("check_ports", {
      webPort: settings.web_port,
      phpPort: settings.php_port,
      mysqlPort: settings.mysql_port,
      postgresqlPort: settings.postgresql_port,
    });
    setPortCheck(result);
  };

  const handleSave = async () => {
    setSaving(true);
    setSaveProgress(null);
    setError(null);
    setMessage(null);

    try {
      // Validate before saving so duplicate/invalid ports are blocked up front
      // instead of silently persisting and later surfacing as fallback-port behavior.
      try {
        await invoke<string[]>("validate_settings", { settings });
      } catch (validationErrors) {
        const list = Array.isArray(validationErrors)
          ? validationErrors.join("; ")
          : String(validationErrors);
        setError(`${t.failedToSaveSettings}: ${list}`);
        setSaving(false);
        setSaveProgress(null);
        return;
      }

      await invoke("save_settings", { settings });
      setMessage(t.settingsSaved);
      onSettingsChanged?.();
      window.setTimeout(onClose, 700);
    } catch (e) {
      setError(`${t.failedToSaveSettings}: ${e}`);
    } finally {
      setSaving(false);
      setSaveProgress(null);
    }
  };

  const reloadPhpVersions = async () => {
    const installedPhp = await invoke<InstalledPhpVersion[]>("get_installed_php_versions");
    setPhpVersions(installedPhp);
  };

  const updateSelectedPhpSetting = (phpId: string) => {
    setSelectedPhpId(phpId);
    setSettings((current) => ({
      ...current,
      package_selection: {
        ...(current.package_selection ?? defaultPackageSelection),
        php: phpId,
      },
    }));
  };

  const updateSelectedDatabaseTool = (toolId: string) => {
    setSettings((current) => ({
      ...current,
      package_selection: {
        ...(current.package_selection ?? defaultPackageSelection),
        phpmyadmin: toolId,
      },
    }));
  };

  const installPhpVersion = async () => {
    setPhpBusy(true);
    setError(null);
    setMessage(null);

    try {
      await invoke("download_php_version", { phpId: selectedPhpId });
      await reloadPhpVersions();
      setMessage(t.phpVersionInstalled);
      onSettingsChanged?.();
    } catch (e) {
      setError(`${t.failedToInstallPhpVersion}: ${e}`);
    } finally {
      setPhpBusy(false);
    }
  };

  const switchPhpVersion = async () => {
    setPhpBusy(true);
    setError(null);
    setMessage(null);

    try {
      await invoke("switch_php_version", { phpId: selectedPhpId });
      await reloadPhpVersions();
      setMessage(t.activePhpChanged);
      onSettingsChanged?.();
    } catch (e) {
      setError(`${t.failedToSwitchPhpVersion}: ${e}`);
    } finally {
      setPhpBusy(false);
    }
  };

  const availablePhpPackages = (packages?.php ?? []).filter((pkg) =>
    hasPackageUrlForPlatform(pkg, runtimePlatformKey)
  );
  const availablePhpIds = new Set(availablePhpPackages.map((pkg) => pkg.id));
  const visiblePhpVersions = phpVersions.filter((php) => availablePhpIds.has(php.id));
  const selectedPhp = phpVersions.find((php) => php.id === selectedPhpId);
  const selectedDatabaseToolId =
    settings.package_selection?.phpmyadmin ?? defaultPackageSelection.phpmyadmin;
  const activeDatabaseName = isAdminerSelected(settings.package_selection) ? "PostgreSQL" : "MySQL";
  const doctorModelStatus = doctorModel.status;
  const doctorModelDownloading = doctorModel.progress !== null;

  return (
    <div className="modal-backdrop" onClick={onClose} {...props}>
      <section
        className="settings-panel"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={t.settings}
      >
        <header className="settings-header">
          <div>
            <h2>
              <Settings2 size={19} /> {t.settingsTitle}
            </h2>
            <p>{t.settingsDescription}</p>
          </div>
          <button
            className="icon-button"
            onClick={() => {
              AudioManager.playClick();
              onClose();
            }}
            aria-label={t.close}
            onMouseEnter={() => AudioManager.playHover()}
          >
            <X size={18} />
          </button>
        </header>

        <div className="settings-content">
          {loading ? (
            <div className="empty-state">{t.loading}</div>
          ) : (
            <>
              {error && (
                <div className="error-box">
                  <p className="error-box-text">{error}</p>
                </div>
              )}
              {message && (
                <div className="success-box">
                  <CheckCircle2 size={16} />
                  <p className="success-box-text">{message}</p>
                </div>
              )}

              <div className="settings-section">
                <h3>
                  <Network size={16} /> {t.ports}
                </h3>
                <div className="settings-grid">
                  <label>
                    <span>{t.httpPort}</span>
                    <input
                      className="input"
                      type="number"
                      min={1}
                      max={65535}
                      value={settings.web_port}
                      onChange={(e) => handlePortChange("web_port", e.target.value)}
                    />
                    {portCheck && (
                      <small className={portCheck.web.available ? "ok" : "warn"}>
                        {portCheck.web.available ? t.portAvailable : t.portInUse}
                      </small>
                    )}
                  </label>
                  <label>
                    <span>{t.phpPort}</span>
                    <input
                      className="input"
                      type="number"
                      min={1}
                      max={65535}
                      value={settings.php_port}
                      onChange={(e) => handlePortChange("php_port", e.target.value)}
                    />
                    {portCheck && (
                      <small className={portCheck.php.available ? "ok" : "warn"}>
                        {portCheck.php.available ? t.portAvailable : t.portInUse}
                      </small>
                    )}
                  </label>
                  <label>
                    <span>{t.mysqlPort}</span>
                    <input
                      className="input"
                      type="number"
                      min={1}
                      max={65535}
                      value={settings.mysql_port}
                      onChange={(e) => handlePortChange("mysql_port", e.target.value)}
                    />
                    {portCheck && (
                      <small className={portCheck.mysql.available ? "ok" : "warn"}>
                        {portCheck.mysql.available ? t.portAvailable : t.portInUse}
                      </small>
                    )}
                  </label>
                  <label>
                    <span>PostgreSQL</span>
                    <input
                      className="input"
                      type="number"
                      min={1}
                      max={65535}
                      value={settings.postgresql_port}
                      onChange={(e) => handlePortChange("postgresql_port", e.target.value)}
                    />
                    {portCheck && (
                      <small className={portCheck.postgresql.available ? "ok" : "warn"}>
                        {portCheck.postgresql.available ? t.portAvailable : t.portInUse}
                      </small>
                    )}
                  </label>
                </div>
                <button
                  className="btn-secondary compact"
                  onClick={() => {
                    AudioManager.playClick();
                    checkPorts();
                  }}
                  onMouseEnter={() => AudioManager.playHover()}
                >
                  <Radar size={14} /> {t.checkPorts}
                </button>
              </div>

              <div className="settings-section">
                <h3>
                  <FileCode2 size={16} /> {t.phpVersions}
                </h3>
                <label className="project-row">
                  <span>{t.activePhpRuntime}</span>
                  <select
                    className="input"
                    value={selectedPhpId}
                    onChange={(event) => updateSelectedPhpSetting(event.target.value)}
                  >
                    {availablePhpPackages.map((pkg) => {
                      const installed = phpVersions.find((php) => php.id === pkg.id)?.installed;
                      return (
                        <option key={pkg.id} value={pkg.id}>
                          {pkg.display_name}
                          {installed ? ` - ${t.installedSuffix}` : ""}
                        </option>
                      );
                    })}
                  </select>
                </label>
                <div className="php-version-grid">
                  {visiblePhpVersions.map((php) => (
                    <button
                      key={php.id}
                      type="button"
                      className={`php-version-chip ${php.active ? "active" : ""} ${php.id === selectedPhpId ? "selected" : ""}`.trim()}
                      onClick={() => updateSelectedPhpSetting(php.id)}
                    >
                      <strong>{php.display_name}</strong>
                      <span>
                        {php.installed ? php.version : php.latest_version} ·{" "}
                        {php.active ? t.active : php.installed ? t.installed : t.available}
                        {php.eol ? " · EOL" : ""}
                      </span>
                      {php.update_available && (
                        <span className="php-update-badge">
                          <ArrowUpCircle size={12} /> {php.latest_version}
                        </span>
                      )}
                    </button>
                  ))}
                </div>
                <div className="settings-inline-actions">
                  <button
                    className="btn-secondary"
                    onClick={() => {
                      AudioManager.playClick();
                      installPhpVersion();
                    }}
                    disabled={phpBusy || (selectedPhp?.installed && !selectedPhp.update_available)}
                    onMouseEnter={() => AudioManager.playHover()}
                  >
                    {phpBusy ? (
                      <LoaderCircle size={14} className="spin-icon" />
                    ) : selectedPhp?.update_available ? (
                      <ArrowUpCircle size={14} />
                    ) : (
                      <Download size={14} />
                    )}
                    {phpBusy
                      ? t.working
                      : selectedPhp?.update_available
                        ? t.phpUpdateTo.replace("{version}", selectedPhp.latest_version)
                        : t.installSelected}
                  </button>
                  <button
                    className="btn-primary"
                    onClick={() => {
                      AudioManager.playClick();
                      switchPhpVersion();
                    }}
                    disabled={
                      phpBusy ||
                      !phpVersions.find((php) => php.id === selectedPhpId)?.installed ||
                      phpVersions.find((php) => php.id === selectedPhpId)?.active
                    }
                    onMouseEnter={() => AudioManager.playHover()}
                  >
                    <Repeat size={14} /> {t.switchPhp}
                  </button>
                </div>
              </div>

              <div className="settings-section">
                <h3>
                  <Database size={16} /> {t.databaseToolSelect}
                </h3>
                <label className="project-row">
                  <span>{t.webDatabaseManager}</span>
                  <select
                    className="input"
                    value={selectedDatabaseToolId}
                    onChange={(event) => updateSelectedDatabaseTool(event.target.value)}
                  >
                    {(packages?.phpmyadmin ?? []).map((pkg) => (
                      <option key={pkg.id} value={pkg.id}>
                        {pkg.display_name}
                      </option>
                    ))}
                  </select>
                  <small>
                    {t.databaseUsedWhenStartingStack.replace("{name}", activeDatabaseName)}
                  </small>
                </label>
              </div>

              <div className="settings-section">
                <h3>
                  <BrainCircuit size={16} /> {t.logDoctorAiTitle}
                </h3>
                <p className="settings-muted">{t.logDoctorAiSettingsDescription}</p>
                {doctorModel.error && (
                  <div className="error-box">
                    <p className="error-box-text">{doctorModel.error}</p>
                  </div>
                )}
                {doctorModelStatus && (
                  <div className="settings-inline-actions">
                    <span className="settings-muted">
                      {doctorModelStatus.model_name} · {doctorModelStatus.parameters} ·{" "}
                      {formatModelSize(doctorModelStatus.download_bytes)} ·{" "}
                      {doctorModelStatus.installed ? t.installed : t.logDoctorAiNotInstalled}
                    </span>
                    {doctorModelStatus.installed ? (
                      <button
                        className="btn-secondary danger compact"
                        onClick={() => {
                          AudioManager.playClick();
                          doctorModel.remove();
                        }}
                      >
                        <Trash2 size={14} /> {t.logDoctorRemoveAi}
                      </button>
                    ) : (
                      <button
                        className="btn-primary compact"
                        onClick={() => {
                          AudioManager.playClick();
                          doctorModel.download();
                        }}
                        disabled={doctorModelDownloading}
                      >
                        {doctorModelDownloading ? (
                          <>
                            <LoaderCircle size={14} className="spin-icon" /> {doctorModel.progress}%
                          </>
                        ) : (
                          <>
                            <Download size={14} />
                            {t.logDoctorEnableAi.replace(
                              "{size}",
                              formatModelSize(doctorModelStatus.download_bytes)
                            )}
                          </>
                        )}
                      </button>
                    )}
                  </div>
                )}
              </div>

              <AgentConnectorSection />

              <UpdatesSection
                autoCheck={settings.auto_check_updates ?? true}
                onAutoCheckChange={(enabled) =>
                  setSettings((current) => ({ ...current, auto_check_updates: enabled }))
                }
              />

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
                    checked={settings.auto_start_services ?? false}
                    onChange={(e) => {
                      AudioManager.playToggle();
                      setSettings((current) => ({
                        ...current,
                        auto_start_services: e.target.checked,
                      }));
                    }}
                  />
                  <span>{t.autoStartServices}</span>
                </label>
                <p className="settings-muted settings-toggle-hint">{t.autoStartDescription}</p>
              </div>
            </>
          )}
        </div>

        <footer className="settings-footer">
          <button
            className="btn-secondary danger"
            onClick={() => {
              AudioManager.playClick();
              onClose();
            }}
            disabled={saving}
            onMouseEnter={() => AudioManager.playHover()}
          >
            {t.cancel}
          </button>
          <button
            className="btn-primary success"
            onClick={() => {
              AudioManager.playClick();
              handleSave();
            }}
            disabled={saving || loading}
            onMouseEnter={() => AudioManager.playHover()}
          >
            {saving && saveProgress?.step === "downloading"
              ? `${t.downloading} ${saveProgress.componentDisplay || t.databaseTool}`
              : saving
                ? `${t.save}...`
                : t.save}
          </button>
        </footer>
      </section>
    </div>
  );
}
