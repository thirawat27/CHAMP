import { invoke } from "@tauri-apps/api/core";
import { CheckCircle2, Settings2, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { SETTINGS_CLOSE_DELAY_MS } from "../constants";
import { useEscape } from "../hooks/useEscape";
import { useTranslation } from "../stores/preferencesStore";
import {
  AppSettings,
  DEFAULT_PORTS,
  DownloadProgress,
  EMPTY_PACKAGE_SELECTION,
  InstalledPhpVersion,
  PackagesConfig,
  ServiceType,
  normalizeSelection,
  packagesForPlatform,
} from "../types/services";
import { AudioManager } from "../utils/audioManager";
import { listenIfTauri } from "../utils/tauri";
import { AgentConnectorSection } from "./settings/AgentConnectorSection";
import { DatabaseSection } from "./settings/DatabaseSection";
import { LogDoctorSection } from "./settings/LogDoctorSection";
import { PhpSection } from "./settings/PhpSection";
import { PortField, PortsSection } from "./settings/PortsSection";
import { PreferencesSection } from "./settings/PreferencesSection";
import { UpdatesSection } from "./settings/UpdatesSection";
import { SoundButton } from "./SoundButton";
import { componentLabel } from "../utils/downloadProgress";

interface SettingsPanelProps {
  onClose: () => void;
  onSettingsChanged?: () => void;
}

export function SettingsPanel({ onClose, onSettingsChanged }: SettingsPanelProps) {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<AppSettings>({
    web_port: DEFAULT_PORTS[ServiceType.Caddy],
    php_port: DEFAULT_PORTS[ServiceType.PhpFpm],
    mysql_port: DEFAULT_PORTS[ServiceType.MySQL],
    postgresql_port: DEFAULT_PORTS[ServiceType.PostgreSQL],
    auto_start_services: false,
    package_selection: EMPTY_PACKAGE_SELECTION,
  });
  const [packages, setPackages] = useState<PackagesConfig | null>(null);
  const [runtimePlatformKey, setRuntimePlatformKey] = useState("");
  const [phpVersions, setPhpVersions] = useState<InstalledPhpVersion[]>([]);
  const [selectedPhpId, setSelectedPhpId] = useState(EMPTY_PACKAGE_SELECTION.php);
  const [saving, setSaving] = useState(false);
  const [saveProgress, setSaveProgress] = useState<DownloadProgress | null>(null);
  const [loading, setLoading] = useState(true);
  // Defaults stay in the form after a failed load; saving them would overwrite
  // the real settings.
  const [loadFailed, setLoadFailed] = useState(false);
  const closeTimer = useRef<number | undefined>(undefined);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const requestClose = () => {
    // A close during a save would hide its result.
    if (saving) return;
    AudioManager.playClick();
    onClose();
  };

  useEscape(requestClose);

  useEffect(() => () => window.clearTimeout(closeTimer.current), []);

  const loadSettings = useCallback(async () => {
    try {
      const [loaded, availablePackages, installedPhp, platformKey] = await Promise.all([
        invoke<AppSettings>("get_settings"),
        invoke<PackagesConfig>("get_available_packages_cmd"),
        invoke<InstalledPhpVersion[]>("get_installed_php_versions"),
        invoke<string>("get_runtime_platform"),
      ]);
      const normalizedPackageSelection = normalizeSelection(
        { ...EMPTY_PACKAGE_SELECTION, ...loaded.package_selection },
        availablePackages,
        platformKey
      );
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
      setLoadFailed(true);
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

  const handlePortChange = (field: PortField, port: number) => {
    setSettings((current) => ({ ...current, [field]: port }));
  };

  const clearResult = () => {
    setError(null);
    setMessage(null);
  };

  const handleSave = async () => {
    setSaving(true);
    setSaveProgress(null);
    clearResult();

    // The PHP dropdown also browses versions for Install. Only an installed
    // version can become active; otherwise keep the one in use.
    const selectedPhp = phpVersions.find((php) => php.id === selectedPhpId);
    const activePhpId =
      phpVersions.find((php) => php.active)?.id ?? settings.package_selection?.php ?? "";
    const toSave: AppSettings = {
      ...settings,
      package_selection: {
        ...(settings.package_selection ?? EMPTY_PACKAGE_SELECTION),
        php: selectedPhp?.installed ? selectedPhpId : activePhpId,
      },
    };

    try {
      // Validate before saving so duplicate/invalid ports are blocked up front
      // instead of silently persisting and later surfacing as fallback-port behavior.
      try {
        await invoke<string[]>("validate_settings", { settings: toSave });
      } catch (validationErrors) {
        const list = Array.isArray(validationErrors)
          ? validationErrors.join("; ")
          : String(validationErrors);
        setError(`${t.failedToSaveSettings}: ${list}`);
        return;
      }

      await invoke("save_settings", { settings: toSave });
      setMessage(t.settingsSaved);
      onSettingsChanged?.();
      closeTimer.current = window.setTimeout(onClose, SETTINGS_CLOSE_DELAY_MS);
    } catch (e) {
      setError(`${t.failedToSaveSettings}: ${e}`);
    } finally {
      setSaving(false);
      setSaveProgress(null);
    }
  };

  const reloadPhpVersions = async () => {
    setPhpVersions(await invoke<InstalledPhpVersion[]>("get_installed_php_versions"));
  };

  return (
    <div className="modal-backdrop" onClick={requestClose}>
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
          <button className="icon-button" onClick={requestClose} aria-label={t.close}>
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

              <PortsSection settings={settings} onChange={handlePortChange} onError={setError} />

              <PhpSection
                availablePackages={packagesForPlatform(packages?.php ?? [], runtimePlatformKey)}
                versions={phpVersions}
                selectedId={selectedPhpId}
                onSelect={setSelectedPhpId}
                onVersionsChanged={async () => {
                  await reloadPhpVersions();
                  onSettingsChanged?.();
                }}
                onSuccess={setMessage}
                onError={setError}
                onCommandStart={clearResult}
              />

              <DatabaseSection
                selection={settings.package_selection}
                packages={packages}
                onChange={(packageSelection) =>
                  setSettings((current) => ({ ...current, package_selection: packageSelection }))
                }
              />

              <LogDoctorSection />

              <AgentConnectorSection />

              <UpdatesSection
                autoCheck={settings.auto_check_updates ?? true}
                onAutoCheckChange={(enabled) =>
                  setSettings((current) => ({ ...current, auto_check_updates: enabled }))
                }
              />

              <PreferencesSection
                autoStart={settings.auto_start_services ?? false}
                onAutoStartChange={(enabled) =>
                  setSettings((current) => ({ ...current, auto_start_services: enabled }))
                }
              />
            </>
          )}
        </div>

        <footer className="settings-footer">
          <button className="btn-secondary danger" onClick={requestClose} disabled={saving}>
            {t.cancel}
          </button>
          <SoundButton
            className="btn-primary success"
            onClick={handleSave}
            disabled={saving || loading || loadFailed}
          >
            {saving && saveProgress?.step === "downloading"
              ? `${t.downloading} ${componentLabel(saveProgress, t) || t.databaseTool}`
              : saving
                ? `${t.save}...`
                : t.save}
          </SoundButton>
        </footer>
      </section>
    </div>
  );
}
