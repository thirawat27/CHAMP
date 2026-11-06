import { AlertTriangle, CheckCircle2, LoaderCircle, X } from "lucide-react";
import { useTranslation } from "../../stores/preferencesStore";
import { DashboardNotice } from "../../types/dashboard";
import { SoundButton } from "../SoundButton";

export function NoticeToast({
  notice,
  onDismiss,
}: {
  notice: DashboardNotice;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();

  return (
    <div
      className={`stack-notice ${notice.tone} ${notice.action ?? ""}`}
      role={notice.tone === "error" ? "alert" : "status"}
    >
      <span className="stack-notice-icon" aria-hidden="true">
        {notice.tone === "info" && <LoaderCircle size={17} className="spin-icon" />}
        {notice.tone === "success" && <CheckCircle2 size={17} />}
        {notice.tone === "error" && <AlertTriangle size={17} />}
      </span>
      <span>
        <strong>{notice.title}</strong>
        <small>{notice.message}</small>
      </span>
      <SoundButton className="notice-close" onClick={onDismiss} aria-label={t.close}>
        <X size={15} />
      </SoundButton>
    </div>
  );
}
