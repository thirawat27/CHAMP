import { useTranslation } from "../stores/preferencesStore";

/** An error box with a small Close button. */
export function DismissibleError({
  message,
  onDismiss,
}: {
  message: string;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="error-box dismissible-error">
      <p className="error-box-text">
        <strong>{t.error}:</strong> {message}
      </p>
      <button type="button" className="dismissible-error-close" onClick={onDismiss}>
        {t.close}
      </button>
    </div>
  );
}
