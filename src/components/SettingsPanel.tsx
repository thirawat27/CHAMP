import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, FolderOpen, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import {
  AppSettings,
  DownloadProgress,
  InstalledPhpVersion,
  PackagesConfig,
  PackageSelection,
  hasPackageUrlForPlatform,
} from "../types/services";
import { useLanguageStore, useTranslation } from "../stores/languageStore";
import { AudioManager } from "../utils/audioManager";

interface SettingsPanelProps {
  onClose: () => void;
  onSettingsChanged?: () => void;
  [key: string]: unknown;
}

interface PortCheck {
  web: { port: number; available: boolean };
  php: { port: number; available: boolean };
  mysql: { port: number; available: boolean };
}

const defaultPackageSelection: PackageSelection = {
  php: "php-8.5",
  mysql: "mysql-9.7",
  phpmyadmin: "phpmyadmin-5.2",
};

export function SettingsPanel({ onClose, onSettingsChanged, ...props }: SettingsPanelProps) {
  const { t } = useTranslation();
  const { soundEnabled, toggleSound } = useLanguageStore();

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
    web_port: 8080,
    php_port: 9000,
    mysql_port: 3306,
    project_root: "",
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
      const normalizedPackageSelection = { ...packageSelection };
      if (
        !availablePhpPackages.some((pkg) => pkg.id === normalizedPackageSelection.php) &&
        availablePhpPackages[0]
      ) {
        normalizedPackageSelection.php = availablePhpPackages[0].id;
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
    } catch (e) {
      setError(`Failed to load settings: ${e}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  useEffect(() => {
    const unlisten = listen<DownloadProgress>("download-progress", (event) => {
      setSaveProgress(event.payload);
    });

    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, []);

  const handlePortChange = (field: "web_port" | "php_port" | "mysql_port", value: string) => {
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
    });
    setPortCheck(result);
  };

  const handleSave = async () => {
    setSaving(true);
    setSaveProgress(null);
    setError(null);
    setMessage(null);

    try {
      await invoke("save_settings", { settings });
      setMessage("Settings saved");
      onSettingsChanged?.();
      window.setTimeout(onClose, 700);
    } catch (e) {
      setError(`Failed to save settings: ${e}`);
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
      setMessage("PHP version installed");
      onSettingsChanged?.();
    } catch (e) {
      setError(`Failed to install PHP version: ${e}`);
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
      setMessage("Active PHP version changed");
      onSettingsChanged?.();
    } catch (e) {
      setError(`Failed to switch PHP version: ${e}`);
    } finally {
      setPhpBusy(false);
    }
  };

  const openProjectRoot = async () => {
    if (!settings.project_root) return;
    await invoke("open_folder", { path: settings.project_root });
  };

  const availablePhpPackages = (packages?.php ?? []).filter((pkg) =>
    hasPackageUrlForPlatform(pkg, runtimePlatformKey)
  );
  const availablePhpIds = new Set(availablePhpPackages.map((pkg) => pkg.id));
  const visiblePhpVersions = phpVersions.filter((php) => availablePhpIds.has(php.id));

  return (
    <div className="modal-backdrop" onClick={onClose} {...props}>
      <section
        className="settings-panel"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
      >
        <header className="settings-header">
          <div>
            <h2>{t.settingsTitle}</h2>
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
                <h3>{t.ports}</h3>
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
                </div>
                <button 
                  className="btn-secondary compact" 
                  onClick={() => {
                    AudioManager.playClick();
                    checkPorts();
                  }}
                  onMouseEnter={() => AudioManager.playHover()}
                >
                  {t.checkPorts}
                </button>
              </div>

              <div className="settings-section">
                <h3>{t.phpVersions}</h3>
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
                          {installed ? " - installed" : ""}
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
                        {php.active ? t.active : php.installed ? t.installed : t.available}
                        {php.lts ? " · LTS" : ""}
                        {php.eol ? " · EOL" : ""}
                      </span>
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
                    disabled={
                      phpBusy || phpVersions.find((php) => php.id === selectedPhpId)?.installed
                    }
                    onMouseEnter={() => AudioManager.playHover()}
                  >
                    {phpBusy ? t.working : t.installSelected}
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
                    {t.switchPhp}
                  </button>
                </div>
              </div>

              <div className="settings-section">
                <h3>{t.databaseToolSelect}</h3>
                <label className="project-row">
                  <span>{t.webDatabaseManager}</span>
                  <select
                    className="input"
                    value={settings.package_selection?.phpmyadmin ?? defaultPackageSelection.phpmyadmin}
                    onChange={(event) => updateSelectedDatabaseTool(event.target.value)}
                  >
                    {(packages?.phpmyadmin ?? []).map((pkg) => (
                      <option key={pkg.id} value={pkg.id}>
                        {pkg.display_name}
                      </option>
                    ))}
                  </select>
                </label>
              </div>

              {/* Sound Effects Section */}
              <div className="settings-section">
                <h3>{t.soundEffects}</h3>
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
                <p style={{ fontSize: "0.75rem", color: "var(--text-secondary)", marginTop: "4px", marginLeft: "24px" }}>
                  {t.soundEffectsDescription}
                </p>
              </div>

              <div className="settings-section">
                <h3>{t.workspace}</h3>
                <label className="project-row">
                  <span>{t.projectsFolder}</span>
                  <div className="input-with-button">
                    <input
                      className="input"
                      value={settings.project_root}
                      onChange={(e) =>
                        setSettings((current) => ({ ...current, project_root: e.target.value }))
                      }
                    />
                    <button
                      className="icon-button"
                      onClick={() => {
                        AudioManager.playClick();
                        openProjectRoot();
                      }}
                      type="button"
                      title={t.openProjectsFolder}
                      aria-label={t.openProjectsFolder}
                      onMouseEnter={() => AudioManager.playHover()}
                    >
                      <FolderOpen size={18} />
                    </button>
                  </div>
                </label>
              </div>

              <div className="settings-section">
                <h3>{t.startup}</h3>
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
                <p style={{ fontSize: "0.75rem", color: "var(--text-secondary)", marginTop: "4px", marginLeft: "24px" }}>
                  {t.autoStartDescription}
                </p>
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
