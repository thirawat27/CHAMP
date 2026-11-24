import { invoke } from "@tauri-apps/api/core";
import { Check, Copy, FileCode2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useCopiedFlag } from "../../hooks/useCopiedFlag";
import { useTranslation } from "../../stores/preferencesStore";
import { EnvProfilesReply, ProjectInfo } from "../../types/addons";
import { AudioManager } from "../../utils/audioManager";

interface EnvProfilesPanelProps {
  /** Changes whenever an add-on is enabled or moves port, to reload the lines. */
  refreshKey: string;
}

/**
 * Configuration lines for the database and enabled add-ons in each
 * framework's convention. The backend builds them (the agent connector
 * reads the same profiles), so this only renders.
 */
export function EnvProfilesPanel({ refreshKey }: EnvProfilesPanelProps) {
  const { t } = useTranslation();
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [project, setProject] = useState("");
  const [reply, setReply] = useState<EnvProfilesReply | null>(null);
  const [chosen, setChosen] = useState<string | null>(null);
  const { copied, markCopied } = useCopiedFlag();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.resolve()
      .then(() => invoke<ProjectInfo[]>("get_project_info", { name: null }))
      .then((list) => setProjects(Array.isArray(list) ? list : []))
      .catch(() => setProjects([]));
  }, []);

  // A new object whenever the project or an add-on changes, so the lines reload.
  const request = useMemo(() => ({ project, refreshKey }), [project, refreshKey]);

  useEffect(() => {
    let cancelled = false;
    Promise.resolve()
      .then(() =>
        invoke<EnvProfilesReply>("get_env_profiles", { project: request.project || null })
      )
      .then((next) => {
        if (cancelled || !next || !Array.isArray(next.profiles)) return;
        setReply(next);
        setError(null);
      })
      .catch((loadError) => {
        if (!cancelled) setError(String(loadError));
      });
    return () => {
      cancelled = true;
    };
  }, [request]);

  const pickProject = (name: string) => {
    setProject(name);
    // A new project brings its own recommendation.
    setChosen(null);
  };

  const selectedId = chosen ?? reply?.recommended ?? "laravel";
  const profile = reply?.profiles.find((item) => item.id === selectedId) ?? reply?.profiles[0];
  const detected = projects.find((item) => item.name === project);

  const copy = async () => {
    if (!profile) return;
    try {
      await navigator.clipboard.writeText(profile.content);
      AudioManager.playClick();
      markCopied();
    } catch (copyError) {
      console.error("Failed to copy configuration:", copyError);
    }
  };

  return (
    <section className="addons-env" aria-labelledby="addons-env-title" data-testid="env-profiles">
      <div className="addons-env-head">
        <h3 id="addons-env-title" className="modal-title-with-icon">
          <FileCode2 size={15} /> {t.addonEnvTitle}
        </h3>
        <button type="button" className="btn-secondary compact" onClick={copy} disabled={!profile}>
          {copied ? <Check size={14} /> : <Copy size={14} />}{" "}
          {copied ? t.addonCopied : t.addonCopyEnv}
        </button>
      </div>

      <div className="env-profile-pickers">
        <label className="addon-port">
          <span>{t.envProject}</span>
          <select
            value={project}
            onChange={(event) => pickProject(event.target.value)}
            data-testid="env-project"
          >
            <option value="">{t.envWholeStack}</option>
            {projects.map((item) => (
              <option key={item.name} value={item.name}>
                {item.framework_name ? `${item.name} (${item.framework_name})` : item.name}
              </option>
            ))}
          </select>
        </label>
        <label className="addon-port">
          <span>{t.envFramework}</span>
          <select
            value={profile?.id ?? ""}
            onChange={(event) => setChosen(event.target.value)}
            data-testid="env-profile"
          >
            {reply?.profiles.map((item) => (
              <option key={item.id} value={item.id}>
                {item.label}
                {item.id === reply.recommended ? ` · ${t.envRecommended}` : ""}
              </option>
            ))}
          </select>
        </label>
      </div>

      {error && <p className="addon-error">{error}</p>}
      {detected && (
        <p className="settings-muted">
          {detected.framework_name ? `${t.envDetected} ${detected.framework_name}` : t.envPlainPhp}
          {detected.php_requirement ? ` · PHP ${detected.php_requirement}` : ""}
          {detected.has_composer && !detected.has_vendor ? ` · ${t.envNeedsComposer}` : ""}
        </p>
      )}
      {profile && (
        <>
          <p className="settings-muted">
            {t.envTarget} <code>{profile.target}</code>
          </p>
          <pre className="addons-env-lines" data-testid="addons-dotenv">
            {profile.content}
          </pre>
          {profile.notes.map((note) => (
            <p key={note} className="settings-muted" lang="en">
              {note}
            </p>
          ))}
        </>
      )}
    </section>
  );
}
