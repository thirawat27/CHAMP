import { BookOpen, Keyboard, LayoutGrid, Play, X, Zap } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "../stores/languageStore";
import { AudioManager } from "../utils/audioManager";

interface HelpModalProps {
  onClose: () => void;
  onOpenGuide: () => void;
}

const MOD = /mac/i.test(navigator.userAgent) ? "⌘" : "Ctrl";

export function HelpModal({ onClose, onOpenGuide }: HelpModalProps) {
  const { t, language } = useTranslation();

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

  const groups = [
    {
      icon: Play,
      title: t.services,
      items: [
        [`${MOD} S`, t.shortcutStart],
        [`${MOD} R`, t.shortcutRestart],
        [`${MOD} X`, t.shortcutStop],
      ],
    },
    {
      icon: Zap,
      title: t.quickAccess,
      items: [
        [`${MOD} W`, t.shortcutWebsite],
        [`${MOD} D`, t.shortcutDatabase],
        [`${MOD} O`, t.shortcutProjects],
        [`${MOD} L`, t.shortcutLogs],
        [`${MOD} T`, t.terminal],
      ],
    },
    {
      icon: LayoutGrid,
      title: language === "th" ? "หน้าต่าง" : "Windows",
      items: [
        [`${MOD} ,`, t.shortcutSettings],
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
            <p>
              {language === "th"
                ? "คีย์ลัดทำงานได้ทุกภาษาของแป้นพิมพ์ (ไทย, อังกฤษ ฯลฯ)"
                : "Shortcuts work in any keyboard layout (Thai, English, and others)."}
            </p>
            <button type="button" className="btn-secondary compact" onClick={onOpenGuide}>
              <BookOpen size={14} /> {t.manualTitle}
            </button>
          </footer>
        </div>
      </div>
    </div>
  );
}
