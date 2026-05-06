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
  const [settings, setSettings] = useState<AppSettings>({
    web_port: 8080,
    php_port: 9000,
    mysql_port: 3307,
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
            <h2>Settings</h2>
            <p>Ports, project folder, and startup behavior</p>
          </div>
          <button className="icon-button" onClick={onClose} aria-label="Close settings">
            <X size={18} />
          </button>
        </header>

        <div className="settings-content">
          {loading ? (
            <div className="empty-state">Loading settings...</div>
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
                <h3>Ports</h3>
                <div className="settings-grid">
                  <label>
                    <span>HTTP</span>
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
                        {portCheck.web.available ? "Available" : "In use"}
                      </small>
                    )}
                  </label>
                  <label>
                    <span>PHP FastCGI</span>
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
                        {portCheck.php.available ? "Available" : "In use"}
                      </small>
                    )}
                  </label>
                  <label>
                    <span>MySQL</span>
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
                        {portCheck.mysql.available ? "Available" : "In use"}
                      </small>
                    )}
                  </label>
                </div>
                <button className="btn-secondary compact" onClick={checkPorts}>
                  Check Ports
                </button>
              </div>

              <div className="settings-section">
                <h3>PHP Versions</h3>
                <label className="project-row">
                  <span>Active PHP runtime</span>
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
                      className={`php-version-chip ${php.active ? "active" : ""}`}
                      onClick={() => updateSelectedPhpSetting(php.id)}
                    >
                      <strong>{php.display_name}</strong>
                      <span>
                        {php.active ? "Active" : php.installed ? "Installed" : "Available"}
                        {php.lts ? " · LTS" : ""}
                        {php.eol ? " · EOL" : ""}
                      </span>
                    </button>
                  ))}
                </div>
                <div className="settings-inline-actions">
                  <button
                    className="btn-secondary"
                    onClick={installPhpVersion}
                    disabled={
                      phpBusy || phpVersions.find((php) => php.id === selectedPhpId)?.installed
                    }
                  >
                    {phpBusy ? "Working..." : "Install Selected"}
                  </button>
                  <button
                    className="btn-primary"
                    onClick={switchPhpVersion}
                    disabled={
                      phpBusy ||
                      !phpVersions.find((php) => php.id === selectedPhpId)?.installed ||
                      phpVersions.find((php) => php.id === selectedPhpId)?.active
                    }
                  >
                    Switch PHP
                  </button>
                </div>
              </div>

              <div className="settings-section">
                <h3>Database Tool</h3>
                <label className="project-row">
                  <span>Web database manager</span>
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

              <div className="settings-section">
                <h3>Workspace</h3>
                <label className="project-row">
                  <span>Projects folder</span>
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
                      onClick={openProjectRoot}
                      type="button"
                      title="Open projects folder"
                      aria-label="Open projects folder"
                    >
                      <FolderOpen size={18} />
                    </button>
                  </div>
                </label>
              </div>

              <div className="settings-section">
                <label className="toggle-row">
                  <input
                    type="checkbox"
                    checked={settings.auto_start_services ?? false}
                    onChange={(e) =>
                      setSettings((current) => ({
                        ...current,
                        auto_start_services: e.target.checked,
                      }))
                    }
                  />
                  <span>Start stack when CHAMP opens</span>
                </label>
              </div>
            </>
          )}
        </div>

        <footer className="settings-footer">
          <button className="btn-secondary danger" onClick={onClose} disabled={saving}>
            Cancel
          </button>
          <button className="btn-primary success" onClick={handleSave} disabled={saving || loading}>
            {saving && saveProgress?.step === "downloading"
              ? `Downloading ${saveProgress.componentDisplay || "database tool"}`
              : saving
                ? "Saving..."
                : "Save"}
          </button>
        </footer>
      </section>
    </div>
  );
}
