import { Boxes, Inbox, Settings2 } from "lucide-react";
import { AddonsController } from "../../hooks/useAddons";
import { useTranslation } from "../../stores/preferencesStore";
import { ServiceState } from "../../types/services";
import { SoundButton } from "../SoundButton";

interface AddonsCardProps {
  controller: AddonsController;
  onManage: () => void;
  openExternal: (url: string) => void;
}

/**
 * Compact status of the add-ons in use. Hidden until one is enabled or
 * running, so the dashboard stays as it was for stacks that do not use them;
 * setup lives in the Add-on services window.
 */
export function AddonsCard({ controller, onManage, openExternal }: AddonsCardProps) {
  const { t } = useTranslation();
  const { addons, mailCount } = controller;
  const shown = addons.filter(
    (addon) => addon.enabled || addon.state !== ServiceState.Stopped || addon.installing
  );
  if (shown.length === 0) return null;
  const mailpit = shown.find((addon) => addon.kind === "mailpit" && addon.web_url);

  return (
    <section className="rail-card addons-card" data-testid="addons-card">
      <header className="rail-card-head">
        <Boxes size={16} />
        <h2>{t.addonsShort}</h2>
        <button
          type="button"
          className="icon-button addons-card-manage"
          onClick={onManage}
          aria-label={t.addonManage}
          title={t.addonManage}
        >
          <Settings2 size={14} />
        </button>
      </header>
      <ul className="addons-card-list">
        {shown.map((addon) => (
          <li key={addon.kind} title={addon.error ?? undefined}>
            <span className={`addon-dot ${addon.installed ? addon.state : "missing"}`} />
            <span className="addons-card-name">{addon.name}</span>
            <code>:{addon.port}</code>
          </li>
        ))}
      </ul>
      {mailpit?.web_url && (
        <SoundButton
          className="btn-secondary compact addons-card-inbox"
          onClick={() => openExternal(mailpit.web_url as string)}
        >
          <Inbox size={14} /> {t.addonOpenInbox}
          {mailCount !== null && (
            <span className="addon-count" aria-label={`${mailCount} ${t.addonMessages}`}>
              {mailCount}
            </span>
          )}
        </SoundButton>
      )}
    </section>
  );
}
