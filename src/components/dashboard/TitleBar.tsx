import { BookOpen, Keyboard, LoaderCircle, Play, RotateCw, Settings, Square } from "lucide-react";
import packageInfo from "../../../package.json";
import champLogo from "../../assets/champ-logo.png";
import { StackCommand } from "../../hooks/commandCopy";
import { useTranslation } from "../../stores/preferencesStore";
import { shortcutHint } from "../../utils/shortcuts";
import { LanguageSelector } from "../LanguageSelector";
import { SoundButton } from "../SoundButton";
import { GitHubIcon } from "./GitHubIcon";

const SOURCE_REPO_URL = "https://github.com/thirawat27/CHAMP";

interface TitleBarProps {
  operationBusy: boolean;
  busyStackCommand: StackCommand | null;
  allRunning: boolean;
  runningCount: number;
  runStackCommand: (command: StackCommand) => void;
  openExternal: (url: string) => void;
  onOpenGuide: () => void;
  onOpenHelp: () => void;
  onOpenSettings: () => void;
}

export function TitleBar({
  operationBusy,
  busyStackCommand,
  allRunning,
  runningCount,
  runStackCommand,
  openExternal,
  onOpenGuide,
  onOpenHelp,
  onOpenSettings,
}: TitleBarProps) {
  const { t } = useTranslation();

  return (
    <header className="titlebar">
      <div className="brand">
        <img className="brand-logo" src={champLogo} alt="" />
        <div className="brand-copy">
          <h1>
            CHAMP <span className="brand-version">v{packageInfo.version}</span>
          </h1>
          <p>{t.appDescription}</p>
        </div>
      </div>

      <div className="titlebar-actions">
        <div className="stack-controls" role="group" aria-label={t.stack}>
          <SoundButton
            className="btn-command primary"
            onClick={() => runStackCommand("start_all_services")}
            disabled={operationBusy || allRunning}
            title={`${t.startAllServices} (${shortcutHint("start")})`}
          >
            {busyStackCommand === "start_all_services" ? (
              <LoaderCircle size={15} className="spin-icon" />
            ) : (
              <Play size={15} />
            )}
            {busyStackCommand === "start_all_services" ? t.starting : t.start}
          </SoundButton>
          <SoundButton
            className="btn-command"
            onClick={() => runStackCommand("restart_all_services")}
            disabled={operationBusy}
            title={`${t.restartAllServices} (${shortcutHint("restart")})`}
          >
            {busyStackCommand === "restart_all_services" ? (
              <LoaderCircle size={15} className="spin-icon" />
            ) : (
              <RotateCw size={15} />
            )}
            {busyStackCommand === "restart_all_services" ? t.restarting : t.restart}
          </SoundButton>
          <SoundButton
            className="btn-command danger"
            onClick={() => runStackCommand("stop_all_services")}
            disabled={operationBusy || runningCount === 0}
            title={`${t.stopAllServices} (${shortcutHint("stop")})`}
          >
            {busyStackCommand === "stop_all_services" ? (
              <LoaderCircle size={14} className="spin-icon" />
            ) : (
              <Square size={13} />
            )}
            {busyStackCommand === "stop_all_services" ? t.stopping : t.stop}
          </SoundButton>
        </div>

        <span className="titlebar-divider" aria-hidden="true" />

        <button
          type="button"
          className="icon-button"
          onClick={() => openExternal(SOURCE_REPO_URL)}
          title={t.sourceRepository}
          aria-label={t.sourceRepository}
        >
          <GitHubIcon size={17} />
        </button>
        <SoundButton
          className="icon-button"
          onClick={onOpenGuide}
          title={`${t.manual} (F1)`}
          aria-label={t.manual}
        >
          <BookOpen size={17} />
        </SoundButton>
        <SoundButton
          className="icon-button"
          onClick={onOpenHelp}
          title={`${t.help} (?)`}
          aria-label={t.help}
        >
          <Keyboard size={17} />
        </SoundButton>
        <SoundButton
          className="icon-button"
          onClick={onOpenSettings}
          title={`${t.settings} (${shortcutHint("settings")})`}
          aria-label={t.settings}
        >
          <Settings size={17} />
        </SoundButton>
        <LanguageSelector />
      </div>
    </header>
  );
}
