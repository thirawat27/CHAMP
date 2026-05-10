import { X } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "../stores/languageStore";
import { AudioManager } from "../utils/audioManager";

interface HelpModalProps {
  onClose: () => void;
}

export function HelpModal({ onClose }: HelpModalProps) {
  const { t } = useTranslation();

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.code === "Escape") {
        AudioManager.playClick();
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return (
    <div className="modal-overlay" onClick={() => { AudioManager.playClick(); onClose(); }}>
      <div className="modal-content help-modal" onClick={(e) => e.stopPropagation()}>
        <header className="modal-header">
          <h2>{t.shortcutsTitle}</h2>
          <button 
            className="icon-button" 
            onClick={() => { AudioManager.playClick(); onClose(); }} 
            aria-label={t.close}
            onMouseEnter={() => AudioManager.playHover()}
          >
            <X size={20} />
          </button>
        </header>

        <div className="modal-body">
          <section className="shortcut-section">
            <h3>{t.services}</h3>
            <div className="shortcut-list">
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>S</kbd>
                <span>{t.shortcutStart}</span>
              </div>
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>R</kbd>
                <span>{t.shortcutRestart}</span>
              </div>
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>X</kbd>
                <span>{t.shortcutStop}</span>
              </div>
            </div>
          </section>

          <section className="shortcut-section">
            <h3>{t.quickAccess}</h3>
            <div className="shortcut-list">
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>W</kbd>
                <span>{t.shortcutWebsite}</span>
              </div>
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>D</kbd>
                <span>{t.shortcutDatabase}</span>
              </div>
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>O</kbd>
                <span>{t.shortcutProjects}</span>
              </div>
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>L</kbd>
                <span>{t.shortcutLogs}</span>
              </div>
            </div>
          </section>

          <section className="shortcut-section">
            <h3>UI</h3>
            <div className="shortcut-list">
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>,</kbd>
                <span>{t.shortcutSettings}</span>
              </div>
              <div className="shortcut-item">
                <kbd>?</kbd>
                <span>{t.shortcutHelp}</span>
              </div>
              <div className="shortcut-item">
                <kbd>Esc</kbd>
                <span>{t.close}</span>
              </div>
            </div>
          </section>

          <footer className="help-footer">
            <p>
              <strong>Note:</strong> {t.language === "th" ? "บน macOS ใช้" : "On macOS, use"} <kbd>Cmd</kbd> {t.language === "th" ? "แทน" : "instead of"} <kbd>Ctrl</kbd>
            </p>
            <p className="help-hint">
              {t.language === "th" 
                ? "คีย์ลัดทำงานได้ทุกภาษา (ไทย, อังกฤษ, ฯลฯ)" 
                : "Shortcuts work in any keyboard language (Thai, English, etc.)"}
            </p>
          </footer>
        </div>
      </div>
    </div>
  );
}
