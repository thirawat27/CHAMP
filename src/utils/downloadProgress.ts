import { Translations } from "../i18n/translations";
import { DownloadProgress } from "../types/services";

/** The backend's fixed label for a parallel download of several components. */
const BATCH_DISPLAY = "All Components";

export const isBatchProgress = (progress: Pick<DownloadProgress, "componentDisplay">) =>
  progress.componentDisplay === BATCH_DISPLAY;

/** The component name to show: real names as they are, a batch in the selected language. */
export const componentLabel = (
  progress: Pick<DownloadProgress, "componentDisplay">,
  t: Pick<Translations, "wizardAllComponents">
) => (isBatchProgress(progress) ? t.wizardAllComponents : progress.componentDisplay);
