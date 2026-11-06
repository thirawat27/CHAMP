import { BrainCircuit, Download, LoaderCircle, Trash2 } from "lucide-react";
import { useLogDoctorModel } from "../../hooks/useLogDoctorModel";
import { useTranslation } from "../../stores/preferencesStore";
import { formatModelSize } from "../../utils/format";
import { SoundButton } from "../SoundButton";

/** Install or remove the optional AI model of Log Doctor. */
export function LogDoctorSection() {
  const { t } = useTranslation();
  const { status, progress, error, download, remove } = useLogDoctorModel();
  const downloading = progress !== null;

  return (
    <div className="settings-section">
      <h3>
        <BrainCircuit size={16} /> {t.logDoctorAiTitle}
      </h3>
      <p className="settings-muted">{t.logDoctorAiSettingsDescription}</p>
      {error && (
        <div className="error-box">
          <p className="error-box-text">{error}</p>
        </div>
      )}
      {status && (
        <div className="settings-inline-actions">
          <span className="settings-muted">
            {status.model_name} · {status.parameters} · {formatModelSize(status.download_bytes)} ·{" "}
            {status.installed ? t.installed : t.logDoctorAiNotInstalled}
          </span>
          {status.installed ? (
            <SoundButton className="btn-secondary danger compact" onClick={remove}>
              <Trash2 size={14} /> {t.logDoctorRemoveAi}
            </SoundButton>
          ) : (
            <SoundButton className="btn-primary compact" onClick={download} disabled={downloading}>
              {downloading ? (
                <>
                  <LoaderCircle size={14} className="spin-icon" /> {progress}%
                </>
              ) : (
                <>
                  <Download size={14} />
                  {t.logDoctorEnableAi.replace("{size}", formatModelSize(status.download_bytes))}
                </>
              )}
            </SoundButton>
          )}
        </div>
      )}
    </div>
  );
}
