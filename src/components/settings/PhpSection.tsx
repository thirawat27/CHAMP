import { invoke } from "@tauri-apps/api/core";
import { ArrowUpCircle, Download, FileCode2, LoaderCircle, Repeat } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "../../stores/preferencesStore";
import { InstalledPhpVersion, PhpPackage } from "../../types/services";
import { SoundButton } from "../SoundButton";

interface PhpSectionProps {
  availablePackages: PhpPackage[];
  versions: InstalledPhpVersion[];
  selectedId: string;
  onSelect: (id: string) => void;
  onVersionsChanged: () => Promise<void>;
  onSuccess: (message: string) => void;
  onError: (message: string) => void;
  /** Starts a command: clears the previous result message. */
  onCommandStart: () => void;
}

export function PhpSection({
  availablePackages,
  versions,
  selectedId,
  onSelect,
  onVersionsChanged,
  onSuccess,
  onError,
  onCommandStart,
}: PhpSectionProps) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const selected = versions.find((php) => php.id === selectedId);
  const availableIds = new Set(availablePackages.map((pkg) => pkg.id));

  const run = async (command: string, successMessage: string, failurePrefix: string) => {
    setBusy(true);
    onCommandStart();
    try {
      await invoke(command, { phpId: selectedId });
      await onVersionsChanged();
      onSuccess(successMessage);
    } catch (error) {
      onError(`${failurePrefix}: ${error}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="settings-section">
      <h3>
        <FileCode2 size={16} /> {t.phpVersions}
      </h3>
      <label className="project-row">
        <span>{t.activePhpRuntime}</span>
        <select
          className="input"
          value={selectedId}
          onChange={(event) => onSelect(event.target.value)}
        >
          {availablePackages.map((pkg) => {
            const installed = versions.find((php) => php.id === pkg.id)?.installed;
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
        {versions
          .filter((php) => availableIds.has(php.id))
          .map((php) => (
            <button
              key={php.id}
              type="button"
              className={`php-version-chip ${php.active ? "active" : ""} ${php.id === selectedId ? "selected" : ""}`.trim()}
              onClick={() => onSelect(php.id)}
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
        <SoundButton
          className="btn-secondary"
          onClick={() =>
            run("download_php_version", t.phpVersionInstalled, t.failedToInstallPhpVersion)
          }
          disabled={busy || (selected?.installed && !selected.update_available)}
        >
          {busy ? (
            <LoaderCircle size={14} className="spin-icon" />
          ) : selected?.update_available ? (
            <ArrowUpCircle size={14} />
          ) : (
            <Download size={14} />
          )}
          {busy
            ? t.working
            : selected?.update_available
              ? t.phpUpdateTo.replace("{version}", selected.latest_version)
              : t.installSelected}
        </SoundButton>
        <SoundButton
          className="btn-primary"
          onClick={() => run("switch_php_version", t.activePhpChanged, t.failedToSwitchPhpVersion)}
          disabled={busy || !selected?.installed || selected.active}
        >
          <Repeat size={14} /> {t.switchPhp}
        </SoundButton>
      </div>
    </div>
  );
}
