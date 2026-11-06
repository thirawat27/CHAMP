import {
  BookOpen,
  Folder,
  FolderCog,
  HardDrive,
  ScrollText,
  Stethoscope,
  TerminalSquare,
  Zap,
} from "lucide-react";
import { ReactNode } from "react";
import { useTranslation } from "../../stores/preferencesStore";
import { AppPaths } from "../../types/services";
import { shortcutKeys } from "../../utils/shortcuts";
import { SoundButton } from "../SoundButton";

interface RailAction {
  id: string;
  icon: ReactNode;
  label: string;
  shortcut?: string;
  onSelect: () => void;
}

interface RailShortcutsProps {
  appPaths: AppPaths | null;
  openFolder: (path?: string) => void;
  openTerminal: (path?: string) => void;
  onOpenDoctor: () => void;
  onOpenGuide: () => void;
}

export function RailShortcuts({
  appPaths,
  openFolder,
  openTerminal,
  onOpenDoctor,
  onOpenGuide,
}: RailShortcutsProps) {
  const { t } = useTranslation();

  const actions: RailAction[] = [
    {
      id: "projects",
      icon: <Folder size={16} />,
      label: t.projects,
      shortcut: shortcutKeys("projects"),
      onSelect: () => openFolder(appPaths?.projects_dir),
    },
    {
      id: "terminal",
      icon: <TerminalSquare size={16} />,
      label: t.terminal,
      shortcut: shortcutKeys("terminal"),
      onSelect: () => openTerminal(appPaths?.projects_dir),
    },
    {
      id: "doctor",
      icon: <Stethoscope size={16} />,
      label: t.logDoctor,
      onSelect: onOpenDoctor,
    },
    {
      id: "logs",
      icon: <ScrollText size={16} />,
      label: t.logs,
      shortcut: shortcutKeys("logs"),
      onSelect: () => openFolder(appPaths?.logs_dir),
    },
    {
      id: "runtime",
      icon: <HardDrive size={16} />,
      label: t.runtime,
      onSelect: () => openFolder(appPaths?.runtime_dir),
    },
    {
      id: "guide",
      icon: <BookOpen size={16} />,
      label: t.manual,
      shortcut: "F1",
      onSelect: onOpenGuide,
    },
    {
      id: "config",
      icon: <FolderCog size={16} />,
      label: t.openConfigFolder,
      onSelect: () => openFolder(appPaths?.config_dir),
    },
  ];

  return (
    <nav className="rail-card" aria-label={t.shortcuts}>
      <header className="rail-card-head">
        <Zap size={16} />
        <h2>{t.shortcuts}</h2>
      </header>
      <ul className="rail-actions">
        {actions.map((action) => (
          <li key={action.id}>
            <SoundButton className="rail-action" onClick={action.onSelect}>
              {action.icon}
              <span>{action.label}</span>
              {action.shortcut && <kbd>{action.shortcut}</kbd>}
            </SoundButton>
          </li>
        ))}
      </ul>
    </nav>
  );
}
