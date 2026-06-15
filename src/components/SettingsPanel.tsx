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
  EMPTY_PACKAGE_SELECTION,
  hasPackageUrlForPlatform,
  isAdminerSelected,
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
  postgresql: { port: number; available: boolean };
}

const defaultPackageSelection: PackageSelection = EMPTY_PACKAGE_SELECTION;

export function SettingsPanel({ onClose, onSettingsChanged, ...props }: SettingsPanelProps) {
  const { t, language } = useTranslation();
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
    postgresql_port: 5432,
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
  const [installedRuntimes, setInstalledRuntimes] = useState<Record<string, string>>({});
  const [runtimeBusy, setRuntimeBusy] = useState<Record<string, boolean>>({
    node: false,
    python: false,
    go: false,
    ruby: false,
  });

  const loadSettings = useCallback(async () => {
    try {
      const [loaded, availablePackages, installedPhp, platformKey, installedRuntimesData] = await Promise.all([
        invoke<AppSettings>("get_settings"),
        invoke<PackagesConfig>("get_available_packages_cmd"),
        invoke<InstalledPhpVersion[]>("get_installed_php_versions"),
        invoke<string>("get_runtime_platform"),
        invoke<Record<string, string>>("get_installed_versions"),
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
      setInstalledRuntimes(installedRuntimesData || {});
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

  const updateSelectedOptionalRuntime = (
    runtimeKey: "node" | "python" | "go" | "ruby",
    packageId: string
  ) => {
    setSettings((current) => ({
      ...current,
      package_selection: {
        ...(current.package_selection ?? defaultPackageSelection),
        [runtimeKey]: packageId,
      },
    }));
  };

  const installOptionalRuntime = async (runtimeKey: "node" | "python" | "go" | "ruby") => {
    const packageId = settings.package_selection?.[runtimeKey];
    if (!packageId) return;

    setRuntimeBusy((prev) => ({ ...prev, [runtimeKey]: true }));
    setError(null);
    setMessage(null);

    try {
      // Build selection where only this runtime is chosen to download
      const selection: PackageSelection = {
        ...EMPTY_PACKAGE_SELECTION,
        [runtimeKey]: packageId,
      };

      // Build skip list of all other components
      const allComponents = ["caddy", "php", "mysql", "postgresql", "adminer", "phpmyadmin", "node", "python", "go", "ruby"];
      const skipList = allComponents.filter((item) => item !== runtimeKey);

      // Trigger download via tauri command
      await invoke("download_runtime_with_skip", {
        packageSelection: selection,
        skipList,
      });

      // Save settings to persist the choice
      await invoke("save_settings", { settings });

      // Reload installed versions
      const installedRuntimesData = await invoke<Record<string, string>>("get_installed_versions");
      setInstalledRuntimes(installedRuntimesData || {});

      setMessage(`${runtimeKey.toUpperCase()} installed successfully!`);
      onSettingsChanged?.();
    } catch (e) {
      setError(`Failed to install ${runtimeKey}: ${e}`);
    } finally {
      setRuntimeBusy((prev) => ({ ...prev, [runtimeKey]: false }));
      setSaveProgress(null);
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
  const selectedDatabaseToolId =
    settings.package_selection?.phpmyadmin ?? defaultPackageSelection.phpmyadmin;
  const activeDatabaseName = isAdminerSelected(settings.package_selection) ? "PostgreSQL" : "MySQL";

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
                    value={selectedDatabaseToolId}
                    onChange={(event) => updateSelectedDatabaseTool(event.target.value)}
                  >
                    {(packages?.phpmyadmin ?? []).map((pkg) => (
                      <option key={pkg.id} value={pkg.id}>
                        {pkg.display_name}
                      </option>
                    ))}
                  </select>
                  <small>{activeDatabaseName} will be used when starting the main stack.</small>
                </label>
              </div>

              <div className="settings-section">
                <h3>{language === "th" ? "รันไทม์เพิ่มเติม (Optional)" : "Additional Runtimes (Optional)"}</h3>
                <p style={{ fontSize: "0.8rem", color: "var(--text-secondary)", marginBottom: "12px" }}>
                  {language === "th" 
                    ? "เลือกและดาวน์โหลดรันไทม์ภาษาอื่นๆ สำหรับเครื่องมือในโครงการของคุณ" 
                    : "Select and download additional language runtimes for your projects."}
                </p>
                <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                  {/* Node.js */}
                  {packages?.node && packages.node.length > 0 && (
                    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "12px", background: "var(--bg-card-secondary)", padding: "10px 14px", borderRadius: "8px", border: "1px solid var(--border-color)" }}>
                      <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                        <span style={{ fontWeight: 600, fontSize: "0.9rem" }}>Node.js</span>
                        {installedRuntimes?.node ? (
                          <span style={{ fontSize: "0.75rem", color: "#16a34a", display: "flex", alignItems: "center", gap: "4px", fontWeight: "bold" }}>
                            <CheckCircle2 size={12} /> {t.installed} ({installedRuntimes.node})
                          </span>
                        ) : (
                          <span style={{ fontSize: "0.75rem", color: "var(--text-secondary)" }}>
                            {language === "th" ? "ยังไม่ได้ติดตั้ง" : "Not installed"}
                          </span>
                        )}
                      </div>
                      
                      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                        <select
                          className="input"
                          style={{ minWidth: "130px", padding: "4px 8px", fontSize: "0.85rem", cursor: "pointer" }}
                          value={settings.package_selection?.node || ""}
                          onChange={(e) => updateSelectedOptionalRuntime("node", e.target.value)}
                        >
                          <option value="">{t.wizardNotSelected || "Do not install"}</option>
                          {packages.node.map((pkg) => (
                            <option key={pkg.id} value={pkg.id}>
                              {pkg.display_name}
                            </option>
                          ))}
                        </select>
                        
                        {settings.package_selection?.node && (
                          <button
                            className="btn-primary"
                            style={{ padding: "0 10px", fontSize: "0.8rem", minHeight: "28px" }}
                            onClick={() => {
                              AudioManager.playClick();
                              installOptionalRuntime("node");
                            }}
                            disabled={
                              runtimeBusy.node || 
                              installedRuntimes?.node === packages.node.find(p => p.id === settings.package_selection?.node)?.version
                            }
                          >
                            {runtimeBusy.node ? (
                              saveProgress && saveProgress.currentComponent === "node" ? (
                                `${t.downloading} ${saveProgress.percent}%`
                              ) : (
                                t.working
                              )
                            ) : installedRuntimes?.node === packages.node.find(p => p.id === settings.package_selection?.node)?.version ? (
                              t.installed
                            ) : (
                              t.install
                            )}
                          </button>
                        )}
                      </div>
                    </div>
                  )}

                  {/* Python */}
                  {packages?.python && packages.python.length > 0 && (
                    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "12px", background: "var(--bg-card-secondary)", padding: "10px 14px", borderRadius: "8px", border: "1px solid var(--border-color)" }}>
                      <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                        <span style={{ fontWeight: 600, fontSize: "0.9rem" }}>Python</span>
                        {installedRuntimes?.python ? (
                          <span style={{ fontSize: "0.75rem", color: "#16a34a", display: "flex", alignItems: "center", gap: "4px", fontWeight: "bold" }}>
                            <CheckCircle2 size={12} /> {t.installed} ({installedRuntimes.python})
                          </span>
                        ) : (
                          <span style={{ fontSize: "0.75rem", color: "var(--text-secondary)" }}>
                            {language === "th" ? "ยังไม่ได้ติดตั้ง" : "Not installed"}
                          </span>
                        )}
                      </div>
                      
                      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                        <select
                          className="input"
                          style={{ minWidth: "130px", padding: "4px 8px", fontSize: "0.85rem", cursor: "pointer" }}
                          value={settings.package_selection?.python || ""}
                          onChange={(e) => updateSelectedOptionalRuntime("python", e.target.value)}
                        >
                          <option value="">{t.wizardNotSelected || "Do not install"}</option>
                          {packages.python.map((pkg) => (
                            <option key={pkg.id} value={pkg.id}>
                              {pkg.display_name}
                            </option>
                          ))}
                        </select>
                        
                        {settings.package_selection?.python && (
                          <button
                            className="btn-primary"
                            style={{ padding: "0 10px", fontSize: "0.8rem", minHeight: "28px" }}
                            onClick={() => {
                              AudioManager.playClick();
                              installOptionalRuntime("python");
                            }}
                            disabled={
                              runtimeBusy.python || 
                              installedRuntimes?.python === packages.python.find(p => p.id === settings.package_selection?.python)?.version
                            }
                          >
                            {runtimeBusy.python ? (
                              saveProgress && saveProgress.currentComponent === "python" ? (
                                `${t.downloading} ${saveProgress.percent}%`
                              ) : (
                                t.working
                              )
                            ) : installedRuntimes?.python === packages.python.find(p => p.id === settings.package_selection?.python)?.version ? (
                              t.installed
                            ) : (
                              t.install
                            )}
                          </button>
                        )}
                      </div>
                    </div>
                  )}

                  {/* Go */}
                  {packages?.go && packages.go.length > 0 && (
                    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "12px", background: "var(--bg-card-secondary)", padding: "10px 14px", borderRadius: "8px", border: "1px solid var(--border-color)" }}>
                      <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                        <span style={{ fontWeight: 600, fontSize: "0.9rem" }}>Go</span>
                        {installedRuntimes?.go ? (
                          <span style={{ fontSize: "0.75rem", color: "#16a34a", display: "flex", alignItems: "center", gap: "4px", fontWeight: "bold" }}>
                            <CheckCircle2 size={12} /> {t.installed} ({installedRuntimes.go})
                          </span>
                        ) : (
                          <span style={{ fontSize: "0.75rem", color: "var(--text-secondary)" }}>
                            {language === "th" ? "ยังไม่ได้ติดตั้ง" : "Not installed"}
                          </span>
                        )}
                      </div>
                      
                      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                        <select
                          className="input"
                          style={{ minWidth: "130px", padding: "4px 8px", fontSize: "0.85rem", cursor: "pointer" }}
                          value={settings.package_selection?.go || ""}
                          onChange={(e) => updateSelectedOptionalRuntime("go", e.target.value)}
                        >
                          <option value="">{t.wizardNotSelected || "Do not install"}</option>
                          {packages.go.map((pkg) => (
                            <option key={pkg.id} value={pkg.id}>
                              {pkg.display_name}
                            </option>
                          ))}
                        </select>
                        
                        {settings.package_selection?.go && (
                          <button
                            className="btn-primary"
                            style={{ padding: "0 10px", fontSize: "0.8rem", minHeight: "28px" }}
                            onClick={() => {
                              AudioManager.playClick();
                              installOptionalRuntime("go");
                            }}
                            disabled={
                              runtimeBusy.go || 
                              installedRuntimes?.go === packages.go.find(p => p.id === settings.package_selection?.go)?.version
                            }
                          >
                            {runtimeBusy.go ? (
                              saveProgress && saveProgress.currentComponent === "go" ? (
                                `${t.downloading} ${saveProgress.percent}%`
                              ) : (
                                t.working
                              )
                            ) : installedRuntimes?.go === packages.go.find(p => p.id === settings.package_selection?.go)?.version ? (
                              t.installed
                            ) : (
                              t.install
                            )}
                          </button>
                        )}
                      </div>
                    </div>
                  )}

                  {/* Ruby */}
                  {packages?.ruby && packages.ruby.length > 0 && (
                    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "12px", background: "var(--bg-card-secondary)", padding: "10px 14px", borderRadius: "8px", border: "1px solid var(--border-color)" }}>
                      <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                        <span style={{ fontWeight: 600, fontSize: "0.9rem" }}>Ruby</span>
                        {installedRuntimes?.ruby ? (
                          <span style={{ fontSize: "0.75rem", color: "#16a34a", display: "flex", alignItems: "center", gap: "4px", fontWeight: "bold" }}>
                            <CheckCircle2 size={12} /> {t.installed} ({installedRuntimes.ruby})
                          </span>
                        ) : (
                          <span style={{ fontSize: "0.75rem", color: "var(--text-secondary)" }}>
                            {language === "th" ? "ยังไม่ได้ติดตั้ง" : "Not installed"}
                          </span>
                        )}
                      </div>
                      
                      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                        <select
                          className="input"
                          style={{ minWidth: "130px", padding: "4px 8px", fontSize: "0.85rem", cursor: "pointer" }}
                          value={settings.package_selection?.ruby || ""}
                          onChange={(e) => updateSelectedOptionalRuntime("ruby", e.target.value)}
                        >
                          <option value="">{t.wizardNotSelected || "Do not install"}</option>
                          {packages.ruby.map((pkg) => (
                            <option key={pkg.id} value={pkg.id}>
                              {pkg.display_name}
                            </option>
                          ))}
                        </select>
                        
                        {settings.package_selection?.ruby && (
                          <button
                            className="btn-primary"
                            style={{ padding: "0 10px", fontSize: "0.8rem", minHeight: "28px" }}
                            onClick={() => {
                              AudioManager.playClick();
                              installOptionalRuntime("ruby");
                            }}
                            disabled={
                              runtimeBusy.ruby || 
                              installedRuntimes?.ruby === packages.ruby.find(p => p.id === settings.package_selection?.ruby)?.version
                            }
                          >
                            {runtimeBusy.ruby ? (
                              saveProgress && saveProgress.currentComponent === "ruby" ? (
                                `${t.downloading} ${saveProgress.percent}%`
                              ) : (
                                t.working
                              )
                            ) : installedRuntimes?.ruby === packages.ruby.find(p => p.id === settings.package_selection?.ruby)?.version ? (
                              t.installed
                            ) : (
                              t.install
                            )}
                          </button>
                        )}
                      </div>
                    </div>
                  )}
                </div>
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
                <p
                  style={{
                    fontSize: "0.75rem",
                    color: "var(--text-secondary)",
                    marginTop: "4px",
                    marginLeft: "24px",
                  }}
                >
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
                <p
                  style={{
                    fontSize: "0.75rem",
                    color: "var(--text-secondary)",
                    marginTop: "4px",
                    marginLeft: "24px",
                  }}
                >
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
