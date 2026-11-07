import { useTranslation } from "../../stores/preferencesStore";
import { DownloadProgress } from "../../types/services";
import { componentLabel, isBatchProgress } from "../../utils/downloadProgress";
import { formatBytes } from "../../utils/format";
import { DismissibleError } from "../DismissibleError";

interface DownloadStepProps {
  progress: DownloadProgress;
  error: string | null;
  onDismissError: () => void;
}

export function DownloadStep({ progress, error, onDismissError }: DownloadStepProps) {
  const { t } = useTranslation();
  const shownPercent = Math.max(0, Math.min(100, progress.percent));
  const indeterminate =
    (progress.step === "downloading" && (shownPercent === 0 || progress.totalBytes === 0)) ||
    progress.step === "extracting";

  const component = componentLabel(progress, t);
  const detail = isBatchProgress(progress)
    ? t.wizardComponentCount.replace("{count}", String(progress.totalComponents))
    : progress.currentComponent || progress.componentDisplay;

  const stepLabel = () => {
    switch (progress.step) {
      case "downloading":
        return component ? `${t.wizardDownloadVerb} ${component}` : t.downloading;
      case "extracting":
        return component ? `${t.wizardExtractVerb} ${component}` : t.extracting;
      case "complete":
        return t.installationComplete;
      default:
        return t.wizardPreparing;
    }
  };

  return (
    <div className="setup-download">
      <div className="setup-download-head">
        <h3>{stepLabel()}</h3>
        {progress.step === "downloading" && (
          <span className="progress-percent">
            {progress.totalBytes > 0 && shownPercent > 0 ? `${shownPercent}%` : ""}
          </span>
        )}
      </div>

      {progress.componentDisplay && (
        <div className="setup-download-component">
          <span>{detail}</span>
          {progress.version && <span className="setup-version-badge">{progress.version}</span>}
        </div>
      )}

      <div className="progress-container">
        <div
          className={indeterminate ? "progress-bar-animated" : "progress-bar"}
          style={{ width: indeterminate ? undefined : `${shownPercent}%` }}
        />
      </div>

      {progress.step === "downloading" && progress.totalBytes > 0 && (
        <div className="setup-download-bytes">
          <span>
            {formatBytes(progress.downloadedBytes)} / {formatBytes(progress.totalBytes)}
          </span>
        </div>
      )}

      {error && <DismissibleError message={error} onDismiss={onDismissError} />}
    </div>
  );
}
