import { useState, FormEvent, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Folder, FilePlus2, LoaderCircle, Globe, TerminalSquare, HardDrive, Download } from "lucide-react";
import { useTranslation } from "../stores/languageStore";
import { AudioManager } from "../utils/audioManager";
import { AppPaths, PackagesConfig, PackageSelection } from "../types/services";

export type ProjectTemplateId = "static" | "php" | "node" | "python" | "go" | "ruby";

export interface ProjectScaffoldResult {
  name: string;
  template: ProjectTemplateId;
  path: string;
  entry_file: string;
}

export const PROJECT_TEMPLATES: Array<{
  id: ProjectTemplateId;
  icon: typeof Globe;
  labelKey: "staticTemplate" | "phpTemplate" | "nodeTemplate" | "pythonTemplate" | "goTemplate" | "rubyTemplate";
  descriptionKey:
    | "staticTemplateDescription"
    | "phpTemplateDescription"
    | "nodeTemplateDescription"
    | "pythonTemplateDescription"
    | "goTemplateDescription"
    | "rubyTemplateDescription";
}> = [
  {
    id: "static",
    icon: Globe,
    labelKey: "staticTemplate",
    descriptionKey: "staticTemplateDescription",
  },
  {
    id: "php",
    icon: TerminalSquare,
    labelKey: "phpTemplate",
    descriptionKey: "phpTemplateDescription",
  },
  {
    id: "node",
    icon: HardDrive,
    labelKey: "nodeTemplate",
    descriptionKey: "nodeTemplateDescription",
  },
  {
    id: "python",
    icon: FilePlus2,
    labelKey: "pythonTemplate",
    descriptionKey: "pythonTemplateDescription",
  },
  {
    id: "go",
    icon: TerminalSquare,
    labelKey: "goTemplate",
    descriptionKey: "goTemplateDescription",
  },
  {
    id: "ruby",
    icon: FilePlus2,
    labelKey: "rubyTemplate",
    descriptionKey: "rubyTemplateDescription",
  },
];

interface TemplateSelectorProps {
  appPaths: AppPaths | null;
  installedVersions: Record<string, string>;
  onClose: () => void;
  onProjectCreated: (result: ProjectScaffoldResult) => void;
  onError: (error: string) => void;
}

export function TemplateSelector({
  appPaths,
  installedVersions,
  onClose,
  onProjectCreated,
  onError,
}: TemplateSelectorProps) {
  const { t } = useTranslation();
  const [projectTemplate, setProjectTemplate] = useState<ProjectTemplateId>("static");
  const [projectName, setProjectName] = useState("");
  const [isCreatingProject, setIsCreatingProject] = useState(false);
  const [isDownloadingRuntime, setIsDownloadingRuntime] = useState(false);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.code === "Escape" && !isCreatingProject && !isDownloadingRuntime) {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isCreatingProject, isDownloadingRuntime, onClose]);

  const openFolder = async (path?: string) => {
    if (!path) return;
    try {
      await invoke("open_folder", { path });
    } catch (error) {
      onError(String(error));
    }
  };

  const isRuntimeMissing = useMemo(() => {
    if (projectTemplate === "node") return !installedVersions.node;
    if (projectTemplate === "python") return !installedVersions.python;
    if (projectTemplate === "go") return !installedVersions.go;
    if (projectTemplate === "ruby") return !installedVersions.ruby;
    return false;
  }, [projectTemplate, installedVersions]);

  const handleDownloadRuntime = async () => {
    setIsDownloadingRuntime(true);
    try {
      const packages = await invoke<PackagesConfig>("get_available_packages_cmd");
      const currentSelection = await invoke<PackageSelection>("get_selected_package_ids");

      let nextSelection = { ...currentSelection };

      if (projectTemplate === "node" && packages.node?.[0]) {
        nextSelection.node = packages.node[0].id;
      } else if (projectTemplate === "python" && packages.python?.[0]) {
        nextSelection.python = packages.python[0].id;
      } else if (projectTemplate === "go" && packages.go?.[0]) {
        nextSelection.go = packages.go[0].id;
      } else if (projectTemplate === "ruby" && packages.ruby?.[0]) {
        nextSelection.ruby = packages.ruby[0].id;
      }

      await invoke("update_package_selection", { packageSelection: nextSelection });

      await invoke("download_runtime_with_skip", {
        packageSelection: nextSelection,
        skipList: ["caddy", "php", "mysql", "postgresql", "adminer", "phpmyadmin"],
      });

      // After download completes, trigger project creation
      createProject(null);
    } catch (error) {
      onError(String(error));
      setIsDownloadingRuntime(false);
    }
  };

  const createProject = async (event: FormEvent<HTMLFormElement> | null) => {
    if (event) event.preventDefault();

    const trimmedName = projectName.trim();
    if (!trimmedName) {
      onError(t.projectNameRequired);
      return;
    }

    if (isRuntimeMissing && !isDownloadingRuntime) {
      await handleDownloadRuntime();
      return;
    }

    setIsCreatingProject(true);
    try {
      const result = await invoke<ProjectScaffoldResult>("create_project_template", {
        projectName: trimmedName,
        template: projectTemplate,
      });
      onProjectCreated(result);
    } catch (error) {
      onError(String(error));
      setIsCreatingProject(false);
    }
  };

  return (
    <div
      className="modal-overlay project-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="project-modal-title"
    >
      <div className="modal-content project-modal-content">
        <div className="modal-header">
          <div>
            <span className="project-modal-eyebrow">{t.projectTemplates}</span>
            <h2 id="project-modal-title">{t.createProject}</h2>
          </div>
          <button
            className="icon-button"
            type="button"
            onClick={() => {
              AudioManager.playClick();
              onClose();
            }}
            disabled={isCreatingProject || isDownloadingRuntime}
            aria-label={t.close}
            onMouseEnter={() => AudioManager.playHover()}
          >
            ×
          </button>
        </div>

        <div className="modal-body project-modal-body">
          <div className="project-template-grid">
            {PROJECT_TEMPLATES.map((template) => {
              const TemplateIcon = template.icon;
              const selected = projectTemplate === template.id;
              return (
                <button
                  key={template.id}
                  type="button"
                  className={`project-template-option ${selected ? "selected" : ""}`}
                  onClick={() => {
                    AudioManager.playClick();
                    setProjectTemplate(template.id);
                  }}
                  onMouseEnter={() => AudioManager.playHover()}
                  aria-pressed={selected}
                >
                  <TemplateIcon size={17} />
                  <span>
                    <strong>{t[template.labelKey]}</strong>
                    <small>{t[template.descriptionKey]}</small>
                  </span>
                </button>
              );
            })}
          </div>

          <form className="project-create-form" onSubmit={createProject}>
            <label className="sr-only" htmlFor="project-name">
              {t.projectName}
            </label>
            <input
              id="project-name"
              value={projectName}
              onChange={(event) => setProjectName(event.target.value)}
              placeholder={t.projectName}
              disabled={isCreatingProject || isDownloadingRuntime}
              autoFocus
            />
            {isRuntimeMissing ? (
              <button
                className="btn-primary"
                type="submit"
                disabled={isCreatingProject || isDownloadingRuntime || projectName.trim().length === 0}
              >
                {isDownloadingRuntime ? (
                  <LoaderCircle size={15} className="spin-icon" />
                ) : (
                  <Download size={15} />
                )}
                {isDownloadingRuntime ? t.downloading : `${t.install} ${projectTemplate} Runtime`}
              </button>
            ) : (
              <button
                className="btn-primary"
                type="submit"
                disabled={isCreatingProject || isDownloadingRuntime || projectName.trim().length === 0}
              >
                {isCreatingProject ? (
                  <LoaderCircle size={15} className="spin-icon" />
                ) : (
                  <FilePlus2 size={15} />
                )}
                {isCreatingProject ? t.working : t.createProject}
              </button>
            )}
          </form>

          <div className="project-modal-footer">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => {
                AudioManager.playClick();
                openFolder(appPaths?.projects_dir);
              }}
              onMouseEnter={() => AudioManager.playHover()}
            >
              <Folder size={16} /> {t.projects}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
