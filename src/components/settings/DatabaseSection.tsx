import { Database } from "lucide-react";
import { useTranslation } from "../../stores/preferencesStore";
import {
  DATABASE_ENGINES,
  DATABASE_ENGINE_NAMES,
  DATABASE_TOOLS,
  DATABASE_TOOL_NAMES,
  DatabaseEngine,
  DatabaseTool,
  EMPTY_PACKAGE_SELECTION,
  PackageSelection,
  PackagesConfig,
  databaseToolSupportsEngine,
  getDatabaseEngine,
  getDatabaseTool,
  selectionWithEngine,
  selectionWithTool,
} from "../../types/services";

export function DatabaseSection({
  selection,
  packages,
  onChange,
}: {
  selection?: PackageSelection;
  packages: PackagesConfig | null;
  onChange: (selection: PackageSelection) => void;
}) {
  const { t } = useTranslation();
  const current = selection ?? EMPTY_PACKAGE_SELECTION;
  const tools = packages?.phpmyadmin ?? [];
  const engine = getDatabaseEngine(current);
  const tool = getDatabaseTool(selection);

  return (
    <div className="settings-section">
      <h3>
        <Database size={16} /> {t.databaseEngine}
      </h3>
      <label className="project-row">
        <span>{t.databaseEngine}</span>
        <select
          className="input"
          value={engine}
          onChange={(event) =>
            // PostgreSQL moves a phpMyAdmin selection onto Adminer; otherwise
            // the manager is kept.
            onChange(selectionWithEngine(current, event.target.value as DatabaseEngine, tools))
          }
        >
          {DATABASE_ENGINES.map((option) => (
            <option key={option} value={option}>
              {DATABASE_ENGINE_NAMES[option]}
            </option>
          ))}
        </select>
        <small>
          {t.databaseUsedWhenStartingStack.replace("{name}", DATABASE_ENGINE_NAMES[engine])}
        </small>
      </label>
      <label className="project-row">
        <span>{t.databaseManager}</span>
        <select
          className="input"
          value={tool}
          data-testid="settings-database-tool"
          onChange={(event) =>
            onChange(selectionWithTool(current, event.target.value as DatabaseTool, tools))
          }
        >
          {DATABASE_TOOLS.filter((option) => databaseToolSupportsEngine(option, engine)).map(
            (option) => (
              <option key={option} value={option}>
                {DATABASE_TOOL_NAMES[option]}
              </option>
            )
          )}
        </select>
        <small>{t.databaseManagerNote}</small>
      </label>
    </div>
  );
}
