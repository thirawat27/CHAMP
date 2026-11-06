import { BookOpen, Keyboard, LayoutGrid, Play, X, Zap } from "lucide-react";
import { useEscape } from "../hooks/useEscape";
import { useTranslation } from "../stores/preferencesStore";
import { AudioManager } from "../utils/audioManager";
import { shortcutKeys } from "../utils/shortcuts";

interface HelpModalProps {
  onClose: () => void;
  onOpenGuide: () => void;
}

export function HelpModal({ onClose, onOpenGuide }: HelpModalProps) {
  const { t } = useTranslation();

  useEscape(() => {
    AudioManager.playClick();
    onClose();
  });

  const groups = [
    {
      icon: Play,
      title: t.services,
      items: [
        [shortcutKeys("start"), t.shortcutStart],
        [shortcutKeys("restart"), t.shortcutRestart],
        [shortcutKeys("stop"), t.shortcutStop],
      ],
    },
    {
      icon: Zap,
      title: t.quickAccess,
      items: [
        [shortcutKeys("website"), t.shortcutWebsite],
        [shortcutKeys("database"), t.shortcutDatabase],
        [shortcutKeys("projects"), t.shortcutProjects],
        [shortcutKeys("logs"), t.shortcutLogs],
        [shortcutKeys("terminal"), t.terminal],
      ],
    },
    {
      icon: LayoutGrid,
      title: t.shortcutGroupWindows,
      items: [
        [shortcutKeys("settings"), t.shortcutSettings],
        ["F1", t.manual],
        ["?", t.shortcutHelp],
        ["Esc", t.close],
      ],
    },
  ];

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-content help-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="help-title"
      >
        <header className="modal-header">
          <h2 id="help-title" className="modal-title-with-icon">
            <Keyboard size={19} /> {t.shortcutsTitle}
          </h2>
          <button className="icon-button" onClick={onClose} aria-label={t.close}>
            <X size={18} />
          </button>
        </header>

        <div className="modal-body">
          {groups.map((group) => {
            const Icon = group.icon;
            return (
              <section className="shortcut-section" key={group.title}>
                <h3>
                  <Icon size={13} /> {group.title}
                </h3>
                <div className="shortcut-list">
                  {group.items.map(([keys, label]) => (
                    <div className="shortcut-item" key={keys}>
                      {keys.split(" ").map((key) => (
                        <kbd key={key}>{key}</kbd>
                      ))}
                      <span>{label}</span>
                    </div>
                  ))}
                </div>
              </section>
            );
          })}

          <footer className="help-footer">
            <p>{t.shortcutsAnyLayout}</p>
            <button type="button" className="btn-secondary compact" onClick={onOpenGuide}>
              <BookOpen size={14} /> {t.manualTitle}
            </button>
          </footer>
        </div>
      </div>
    </div>
  );
}
