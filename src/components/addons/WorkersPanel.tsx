import { invoke } from "@tauri-apps/api/core";
import { Cog, LoaderCircle, Play, Plus, Save, Square, Trash2, X } from "lucide-react";
import { FormEvent, useCallback, useEffect, useState } from "react";
import { STATUS_POLL_INTERVAL_MS } from "../../constants";
import { useTranslation } from "../../stores/preferencesStore";
import {
  isValidAddonId,
  ProjectInfo,
  ProjectWorker,
  splitArgs,
  WorkerStatus,
} from "../../types/addons";
import { ServiceState } from "../../types/services";
import { AudioManager } from "../../utils/audioManager";
import { SoundButton } from "../SoundButton";

/** Suggested commands per framework, filled in when a project is picked. */
const SUGGESTED: Record<string, string> = {
  laravel: "artisan queue:work",
  symfony: "bin/console messenger:consume async",
  codeigniter: "spark tasks:work",
  cakephp: "bin/cake queue run",
  yii: "yii queue/listen",
};

/** The saved definition, without the status fields. */
const plain = (worker: WorkerStatus): ProjectWorker => ({
  id: worker.id,
  project: worker.project,
  name: worker.name,
  args: worker.args,
  enabled: worker.enabled,
});

/**
 * Project background PHP commands (queue workers, schedulers). Run with
 * CHAMP's PHP in the project folder; they stop with the stack.
 */
export function WorkersPanel() {
  const { t } = useTranslation();
  const [workers, setWorkers] = useState<WorkerStatus[]>([]);
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<WorkerStatus[]>("get_workers");
      if (Array.isArray(next)) setWorkers(next);
    } catch (refreshError) {
      console.warn("Failed to load background tasks:", refreshError);
    }
  }, []);

  useEffect(() => {
    void refresh();
    Promise.resolve()
      .then(() => invoke<ProjectInfo[]>("get_project_info", { name: null }))
      .then((list) => setProjects(Array.isArray(list) ? list : []))
      .catch(() => setProjects([]));
  }, [refresh]);

  const active = workers.some((worker) => worker.state !== ServiceState.Stopped);
  useEffect(() => {
    if (!active) return undefined;
    const timer = window.setInterval(() => void refresh(), STATUS_POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [active, refresh]);

  const run = async (id: string, command: string, args: Record<string, unknown>) => {
    setBusy(id);
    setError(null);
    try {
      const next = await invoke<WorkerStatus[]>(command, args);
      if (Array.isArray(next)) setWorkers(next);
      return true;
    } catch (runError) {
      setError(String(runError));
      await refresh();
      return false;
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="addons-env" aria-labelledby="workers-title" data-testid="workers-panel">
      <div className="addons-env-head">
        <h3 id="workers-title" className="modal-title-with-icon">
          <Cog size={15} /> {t.workersTitle}
        </h3>
        {!adding && (
          <SoundButton className="btn-secondary compact" onClick={() => setAdding(true)}>
            <Plus size={14} /> {t.workerAdd}
          </SoundButton>
        )}
      </div>
      <p className="settings-muted">{t.workersHelp}</p>
      {error && <p className="addon-error">{error}</p>}

      {adding && (
        <WorkerForm
          projects={projects}
          busy={busy !== null}
          onCancel={() => setAdding(false)}
          onSave={(worker) => run(worker.id, "save_worker", { worker })}
        />
      )}

      {workers.length === 0 && !adding && <p className="settings-muted">{t.workerEmpty}</p>}
      {workers.map((worker) => {
        const running = worker.state === ServiceState.Running;
        const working = busy === worker.id;
        return (
          <article
            key={worker.id}
            className={`addon-row ${worker.state}`}
            data-testid={`worker-${worker.id}`}
          >
            <div className="addon-row-head">
              <div className="addon-title">
                <strong>{worker.name}</strong>
                <small>
                  {worker.project} · <code>{worker.command_line}</code>
                </small>
              </div>
              <span className={`addon-state ${worker.state}`}>
                {running ? t.running : worker.state === ServiceState.Error ? t.error : t.stopped}
              </span>
            </div>
            {worker.error && <p className="addon-error">{worker.error}</p>}
            <div className="addon-actions">
              <label className="toggle-row addon-toggle">
                <input
                  type="checkbox"
                  checked={worker.enabled}
                  disabled={busy !== null}
                  onChange={(event) => {
                    AudioManager.playToggle();
                    void run(worker.id, "save_worker", {
                      worker: { ...plain(worker), enabled: event.target.checked },
                    });
                  }}
                />
                <span>{t.addonStartWithStack}</span>
              </label>
              {running ? (
                <SoundButton
                  className="btn-secondary compact danger"
                  disabled={busy !== null}
                  onClick={() => run(worker.id, "stop_worker", { id: worker.id })}
                >
                  {working ? (
                    <LoaderCircle size={14} className="spin-icon" />
                  ) : (
                    <Square size={14} />
                  )}{" "}
                  {t.stop}
                </SoundButton>
              ) : (
                <SoundButton
                  className="btn-secondary compact"
                  disabled={busy !== null}
                  onClick={() => run(worker.id, "start_worker", { id: worker.id })}
                >
                  {working ? <LoaderCircle size={14} className="spin-icon" /> : <Play size={14} />}{" "}
                  {t.start}
                </SoundButton>
              )}
              <SoundButton
                className="btn-secondary compact"
                disabled={busy !== null}
                onClick={() => run(worker.id, "remove_worker", { id: worker.id })}
                aria-label={`${t.workerRemove} ${worker.name}`}
              >
                <Trash2 size={14} /> {t.workerRemove}
              </SoundButton>
            </div>
          </article>
        );
      })}
    </section>
  );
}

interface WorkerFormProps {
  projects: ProjectInfo[];
  busy: boolean;
  onSave: (worker: ProjectWorker) => Promise<boolean>;
  onCancel: () => void;
}

function WorkerForm({ projects, busy, onSave, onCancel }: WorkerFormProps) {
  const { t } = useTranslation();
  const [project, setProject] = useState("");
  const [id, setId] = useState("queue");
  const [name, setName] = useState("");
  const [command, setCommand] = useState("");
  const [enabled, setEnabled] = useState(true);

  const pickProject = (value: string) => {
    setProject(value);
    const framework = projects.find((item) => item.name === value)?.framework;
    const suggestion = framework ? SUGGESTED[framework] : undefined;
    if (suggestion && !command) setCommand(suggestion);
    if (!name) setName(`${value} queue`);
  };

  const args = splitArgs(command);
  const valid = project !== "" && isValidAddonId(id) && name.trim() !== "" && args.length > 0;

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!valid) return;
    if (await onSave({ id, project, name: name.trim(), args, enabled })) onCancel();
  };

  return (
    <form className="custom-addon-form" onSubmit={submit} data-testid="worker-form">
      <div className="custom-addon-grid">
        <label className="addon-port">
          <span>{t.envProject}</span>
          <select value={project} onChange={(event) => pickProject(event.target.value)}>
            <option value="" disabled>
              {t.workerPickProject}
            </option>
            {projects.map((item) => (
              <option key={item.name} value={item.name}>
                {item.name}
              </option>
            ))}
          </select>
        </label>
        <label className="addon-port">
          <span>{t.addonCustomId}</span>
          <input
            value={id}
            aria-invalid={!isValidAddonId(id)}
            onChange={(event) => setId(event.target.value.toLowerCase())}
          />
        </label>
        <label className="addon-port">
          <span>{t.addonCustomName}</span>
          <input value={name} onChange={(event) => setName(event.target.value)} />
        </label>
      </div>
      <label className="addon-port wide">
        <span>{t.workerCommand}</span>
        <input
          value={command}
          placeholder="artisan queue:work"
          onChange={(event) => setCommand(event.target.value)}
        />
        <small className="settings-muted">{t.workerCommandHelp}</small>
      </label>
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) => setEnabled(event.target.checked)}
        />
        <span>{t.addonStartWithStack}</span>
      </label>
      <div className="addon-actions">
        <button type="submit" className="btn-primary compact" disabled={!valid || busy}>
          <Save size={14} /> {t.save}
        </button>
        <button type="button" className="btn-secondary compact" onClick={onCancel}>
          <X size={14} /> {t.cancel}
        </button>
      </div>
    </form>
  );
}
