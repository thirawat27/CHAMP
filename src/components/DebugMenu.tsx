import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../stores/preferencesStore";

const openBackendFolder = async (dirCommand: string, failedMessage: string) => {
  try {
    const path = await invoke<string>(dirCommand);
    await invoke("open_folder", { path });
  } catch (error) {
    console.error(failedMessage, error);
    alert(`${failedMessage}: ${error}`);
  }
};

/** Maintenance actions behind Ctrl+Shift+D. */
export function DebugMenu({
  onClose,
  onShowWizard,
}: {
  onClose: () => void;
  onShowWizard: () => void;
}) {
  const { t } = useTranslation();

  const resetInstallation = async () => {
    if (!confirm(t.resetInstallationConfirm)) return;
    try {
      // reset_installation stops every service before deleting the runtime.
      await invoke("reset_installation");
      onShowWizard();
      onClose();
    } catch (error) {
      console.error("Failed to reset:", error);
      alert(`${t.resetFailed}: ${error}`);
    }
  };

  return (
    <div className="debug-menu">
      <div className="debug-menu-head">
        <span>{t.debugMenu}</span>
        <button className="debug-menu-close" onClick={onClose}>
          ×
        </button>
      </div>
      <div className="debug-menu-actions">
        <button onClick={() => openBackendFolder("get_runtime_dir", t.openFolderFailed)}>
          {t.openRuntimeFolder}
        </button>
        <button onClick={() => openBackendFolder("get_download_dir", t.openDownloadFolderFailed)}>
          {t.viewDownloadFolder}
        </button>
        <button onClick={resetInstallation}>{t.resetInstallation}</button>
        <button onClick={onShowWizard}>{t.showFirstRunWizard}</button>
      </div>
    </div>
  );
}
