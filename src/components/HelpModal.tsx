import { X } from "lucide-react";
import { useEffect } from "react";

interface HelpModalProps {
  onClose: () => void;
}

export function HelpModal({ onClose }: HelpModalProps) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.code === "Escape") {
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content help-modal" onClick={(e) => e.stopPropagation()}>
        <header className="modal-header">
          <h2>Keyboard Shortcuts</h2>
          <button className="icon-button" onClick={onClose} aria-label="Close help">
            <X size={20} />
          </button>
        </header>

        <div className="modal-body">
          <section className="shortcut-section">
            <h3>Service Control</h3>
            <div className="shortcut-list">
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>S</kbd>
                <span>Start all services</span>
              </div>
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>R</kbd>
                <span>Restart all services</span>
              </div>
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>X</kbd>
                <span>Stop all services</span>
              </div>
            </div>
          </section>

          <section className="shortcut-section">
            <h3>Quick Access</h3>
            <div className="shortcut-list">
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>W</kbd>
                <span>Open website (localhost)</span>
              </div>
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>D</kbd>
                <span>Open database tool</span>
              </div>
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>O</kbd>
                <span>Open projects folder</span>
              </div>
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>L</kbd>
                <span>Open logs folder</span>
              </div>
            </div>
          </section>

          <section className="shortcut-section">
            <h3>UI Navigation</h3>
            <div className="shortcut-list">
              <div className="shortcut-item">
                <kbd>Ctrl</kbd> + <kbd>,</kbd>
                <span>Toggle Settings</span>
              </div>
              <div className="shortcut-item">
                <kbd>?</kbd>
                <span>Show this help</span>
              </div>
              <div className="shortcut-item">
                <kbd>Esc</kbd>
                <span>Close modal or dismiss notification</span>
              </div>
            </div>
          </section>

          <footer className="help-footer">
            <p>
              <strong>Note:</strong> On macOS, use <kbd>Cmd</kbd> instead of <kbd>Ctrl</kbd>
            </p>
            <p className="help-hint">
              All shortcuts work regardless of keyboard language (Thai, English, etc.)
            </p>
          </footer>
        </div>
      </div>
    </div>
  );
}
