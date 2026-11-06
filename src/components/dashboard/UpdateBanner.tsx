import { ArrowUpCircle, LoaderCircle, X } from "lucide-react";
import { useTranslation } from "../../stores/preferencesStore";
import { useUpdateStore } from "../../stores/updateStore";

export function UpdateBanner() {
  const { t } = useTranslation();
  const phase = useUpdateStore((state) => state.phase);
  const update = useUpdateStore((state) => state.update);
  const progress = useUpdateStore((state) => state.progress);
  const install = useUpdateStore((state) => state.install);
  const dismiss = useUpdateStore((state) => state.dismiss);
  const installing = phase === "installing";

  if ((phase !== "available" && !installing) || !update) return null;

  return (
    <div className="update-banner" role="status">
      <ArrowUpCircle size={17} />
      <p>
        <strong>{t.updateAvailableTitle.replace("{version}", update.version)}</strong>
        <span>{installing ? t.updateInstalling : t.updateRestartNote}</span>
      </p>
      {installing ? (
        <span className="update-progress">
          <LoaderCircle size={15} className="spin-icon" />
          {progress !== null ? `${progress}%` : t.working}
        </span>
      ) : (
        <>
          <button type="button" className="btn-primary compact" onClick={install}>
            {t.updateInstall}
          </button>
          <button
            type="button"
            className="icon-button"
            onClick={dismiss}
            aria-label={t.updateLater}
            title={t.updateLater}
          >
            <X size={15} />
          </button>
        </>
      )}
    </div>
  );
}
