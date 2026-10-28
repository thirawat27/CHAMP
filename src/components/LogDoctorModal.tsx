import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  BrainCircuit,
  ExternalLink,
  FolderOpen,
  LoaderCircle,
  Play,
  RefreshCw,
  Settings,
  Stethoscope,
  X,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { formatModelSize, useLogDoctorModel } from "../hooks/useLogDoctorModel";
import { Translations } from "../i18n/translations";
import { useTranslation } from "../stores/languageStore";
import { AudioManager } from "../utils/audioManager";
import { withInlineCode } from "../utils/inlineCode";
import {
  AppPaths,
  Diagnosis,
  DiagnosisMatch,
  LogDoctorAction,
  SERVICE_DISPLAY_NAMES,
  ServiceMap,
  ServiceState,
  ServiceType,
} from "../types/services";

const DOCTOR_SERVICES = [
  ServiceType.Caddy,
  ServiceType.PhpFpm,
  ServiceType.MySQL,
  ServiceType.PostgreSQL,
] as const;

interface LogDoctorModalProps {
  initialService: ServiceType;
  services: Partial<ServiceMap>;
  appPaths: AppPaths | null;
  onClose: () => void;
  onOpenSettings: () => void;
  onStartPhp: () => void;
}

function actionLabel(action: LogDoctorAction, t: Translations): string {
  switch (action) {
    case "open_settings":
      return t.doctorActionOpenSettings;
    case "open_logs":
      return t.openLogsFolder;
    case "open_url":
      return t.doctorActionOpenUrl;
    case "start_php":
      return t.doctorActionStartPhp;
    case "open_mysql_data":
      return t.doctorActionOpenMysqlData;
    case "open_postgresql_data":
      return t.doctorActionOpenPostgresqlData;
    case "open_runtime":
      return t.openRuntimeFolder;
  }
}

function actionIcon(action: LogDoctorAction) {
  switch (action) {
    case "open_settings":
      return <Settings size={15} />;
    case "open_url":
      return <ExternalLink size={15} />;
    case "start_php":
      return <Play size={15} />;
    default:
      return <FolderOpen size={15} />;
  }
}

export function LogDoctorModal({
  initialService,
  services,
  appPaths,
  onClose,
  onOpenSettings,
  onStartPhp,
}: LogDoctorModalProps) {
  const { t, language } = useTranslation();
  const model = useLogDoctorModel();
  const [service, setService] = useState<ServiceType>(initialService);
  const [diagnosis, setDiagnosis] = useState<Diagnosis | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const runDiagnosis = useCallback(async (target: ServiceType) => {
    setLoading(true);
    setError(null);
    try {
      setDiagnosis(await invoke<Diagnosis>("diagnose_service", { service: target }));
    } catch (diagnoseError) {
      setDiagnosis(null);
      setError(String(diagnoseError));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    runDiagnosis(service);
  }, [runDiagnosis, service]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.code === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const openFolder = async (path?: string) => {
    if (!path) return;
    try {
      await invoke("open_folder", { path });
    } catch (openError) {
      setError(String(openError));
    }
  };

  const runAction = (match: DiagnosisMatch) => {
    AudioManager.playClick();
    switch (match.action) {
      case "open_settings":
        onOpenSettings();
        break;
      case "open_logs":
        openFolder(appPaths?.logs_dir);
        break;
      case "open_url":
        if (match.url) openUrl(match.url).catch((urlError) => setError(String(urlError)));
        break;
      case "start_php":
        onStartPhp();
        break;
      case "open_mysql_data":
        openFolder(appPaths?.mysql_data_dir);
        break;
      case "open_postgresql_data":
        openFolder(appPaths?.postgresql_data_dir);
        break;
      case "open_runtime":
        openFolder(appPaths?.runtime_dir);
        break;
    }
  };

  const enableAi = async () => {
    AudioManager.playClick();
    if (await model.download()) {
      runDiagnosis(service);
    }
  };

  const modelReady = Boolean(model.status?.installed);
  const downloading = model.progress !== null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <section
        className="modal-content log-doctor-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="log-doctor-title"
        data-testid="log-doctor"
      >
        <header className="modal-header">
          <div className="log-doctor-heading">
            <span className="log-doctor-icon" aria-hidden="true">
              <Stethoscope size={18} />
            </span>
            <div>
              <h2 id="log-doctor-title">{t.logDoctor}</h2>
              <p>{t.logDoctorDescription}</p>
            </div>
          </div>
          <button className="icon-button" onClick={onClose} aria-label={t.close}>
            <X size={18} />
          </button>
        </header>

        <div className="modal-body log-doctor-body">
          <div className="log-doctor-tabs" role="tablist">
            {DOCTOR_SERVICES.map((item) => (
              <button
                key={item}
                type="button"
                role="tab"
                aria-selected={item === service}
                className={`log-doctor-tab ${item === service ? "selected" : ""}`}
                onClick={() => {
                  AudioManager.playClick();
                  setService(item);
                }}
              >
                {services[item]?.state === ServiceState.Error && (
                  <span className="log-doctor-tab-dot" aria-hidden="true" />
                )}
                {SERVICE_DISPLAY_NAMES[item]}
              </button>
            ))}
          </div>

          <div className={`log-doctor-ai ${modelReady ? "ready" : ""}`}>
            <BrainCircuit size={16} />
            <span>
              {modelReady
                ? t.logDoctorAiReady
                    .replace("{name}", model.status?.model_name ?? "")
                    .replace("{params}", model.status?.parameters ?? "")
                : downloading
                  ? t.logDoctorDownloading.replace("{percent}", String(model.progress ?? 0))
                  : t.logDoctorAiOff}
            </span>
            {!modelReady && model.status && (
              <button
                type="button"
                className="btn-secondary compact"
                onClick={enableAi}
                disabled={downloading}
              >
                {downloading ? (
                  <LoaderCircle size={14} className="spin-icon" />
                ) : (
                  t.logDoctorEnableAi.replace(
                    "{size}",
                    formatModelSize(model.status.download_bytes)
                  )
                )}
              </button>
            )}
          </div>
          {model.error && <div className="error-box error-box-text">{model.error}</div>}
          {error && <div className="error-box error-box-text">{error}</div>}

          {loading ? (
            <div className="empty-state">
              <LoaderCircle size={18} className="spin-icon" /> {t.logDoctorReading}
            </div>
          ) : diagnosis && diagnosis.matches.length > 0 ? (
            <div className="log-doctor-matches">
              {diagnosis.matches.map((match, index) => (
                <article
                  key={match.id}
                  className={`log-doctor-match ${index === 0 ? "primary" : ""}`}
                  data-testid={`diagnosis-${match.id}`}
                >
                  <header>
                    <h3>{match.title[language]}</h3>
                    <span className={`log-doctor-badge ${match.method}`}>
                      {match.method === "ai" ? t.logDoctorAiMatch : t.logDoctorKnownError} ·{" "}
                      {Math.round(match.confidence * 100)}%
                    </span>
                  </header>
                  <p>{withInlineCode(match.cause[language])}</p>
                  <h4>{t.logDoctorSteps}</h4>
                  <ol>
                    {match.steps[language].map((step) => (
                      <li key={step}>{withInlineCode(step)}</li>
                    ))}
                  </ol>
                  <div className="log-doctor-evidence">
                    <small>{t.logDoctorEvidence}</small>
                    <code>{match.evidence}</code>
                  </div>
                  {match.action && (
                    <button
                      type="button"
                      className={index === 0 ? "btn-primary" : "btn-secondary"}
                      onClick={() => runAction(match)}
                    >
                      {actionIcon(match.action)} {actionLabel(match.action, t)}
                    </button>
                  )}
                </article>
              ))}
            </div>
          ) : diagnosis ? (
            <div className="log-doctor-empty">
              <strong>
                {diagnosis.excerpt.length > 0 ? t.logDoctorNoMatch : t.logDoctorNoLogs}
              </strong>
              {diagnosis.excerpt.length > 0 && <p>{t.logDoctorNoMatchHint}</p>}
            </div>
          ) : null}

          {diagnosis && diagnosis.excerpt.length > 0 && (
            <details className="log-doctor-excerpt">
              <summary>
                {t.logDoctorLogLines} ({diagnosis.excerpt.length})
              </summary>
              <pre>{diagnosis.excerpt.join("\n")}</pre>
            </details>
          )}

          <footer className="log-doctor-footer">
            <button
              type="button"
              className="btn-secondary"
              onClick={() => openFolder(appPaths?.logs_dir)}
            >
              <FolderOpen size={15} /> {t.openLogsFolder}
            </button>
            <button
              type="button"
              className="btn-secondary"
              onClick={() => {
                AudioManager.playClick();
                runDiagnosis(service);
              }}
              disabled={loading}
            >
              <RefreshCw size={15} /> {t.logDoctorRecheck}
            </button>
          </footer>
        </div>
      </section>
    </div>
  );
}
