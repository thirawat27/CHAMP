import { ChangeEvent, ReactNode } from "react";
import {
  PackagesConfig,
  PackageSelection,
  PhpPackage,
  MySQLPackage,
  DATABASE_ENGINES,
  DATABASE_ENGINE_NAMES,
  DATABASE_TOOLS,
  DATABASE_TOOL_NAMES,
  DatabaseEngine,
  DatabaseTool,
  databaseToolSupportsEngine,
  getDatabaseEngine,
  getDatabaseTool,
  packagesForPlatform,
  selectionWithEngine,
  selectionWithTool,
} from "../types/services";
import { useTranslation } from "../stores/preferencesStore";

interface PackageSelectorProps {
  selection: PackageSelection;
  /** Catalog loaded by the wizard; null while it loads */
  packages: PackagesConfig | null;
  platformKey: string;
  loadFailed: boolean;
  onSelectionChange: (selection: PackageSelection) => void;
}

function LabeledSelect({
  label,
  value,
  testId,
  note,
  onChange,
  children,
}: {
  label: string;
  value: string;
  testId?: string;
  note?: string;
  onChange: (value: string) => void;
  children: ReactNode;
}) {
  return (
    <label className="labeled-select">
      <span>{label}</span>
      <select
        className="input"
        value={value}
        data-testid={testId}
        onChange={(event: ChangeEvent<HTMLSelectElement>) => onChange(event.target.value)}
      >
        {children}
      </select>
      {note && <small>{note}</small>}
    </label>
  );
}

/** Version and database choices of the first-run wizard, which owns the selection. */
export function PackageSelector({
  selection,
  packages,
  platformKey,
  loadFailed,
  onSelectionChange,
}: PackageSelectorProps) {
  const { t } = useTranslation();

  if (!packages) {
    return loadFailed ? (
      <div className="package-selector-status error">{t.wizardFailedToLoadPackages}</div>
    ) : (
      <div className="package-selector-status">{t.wizardLoadingPackages}</div>
    );
  }

  const getPhpLabel = (pkg: PhpPackage): string => {
    const badges: string[] = [];
    if (pkg.lts) badges.push("LTS");
    if (pkg.eol) badges.push("EOL");
    if (pkg.recommended) badges.push(t.wizardRecommended);
    return badges.length > 0 ? `${pkg.display_name} [${badges.join(", ")}]` : pkg.display_name;
  };

  const getPackageLabel = (pkg: Pick<PhpPackage, "display_name" | "lts" | "eol">): string => {
    const badges = [pkg.lts ? t.ltsBadge : t.stableBadge, pkg.eol ? t.eolBadge : ""].filter(
      Boolean
    );
    return `${pkg.display_name} [${badges.join(", ")}]`;
  };

  const availablePhpPackages = packagesForPlatform(packages.php, platformKey);
  const databaseEngine = getDatabaseEngine(selection);
  const databaseTool = getDatabaseTool(selection);
  const postgresqlSelected = databaseEngine === "postgresql";
  const activeDatabaseName = DATABASE_ENGINE_NAMES[databaseEngine];
  const activeDatabasePackages = packagesForPlatform(
    postgresqlSelected ? packages.postgresql : packages.mysql,
    platformKey
  );
  const selectedPhpPackage = availablePhpPackages.find((pkg) => pkg.id === selection.php);

  return (
    <div className="package-selector">
      <LabeledSelect
        label={t.phpVersion}
        value={selection.php}
        onChange={(php) => onSelectionChange({ ...selection, php })}
      >
        {availablePhpPackages.map((pkg: PhpPackage) => (
          <option key={pkg.id} value={pkg.id}>
            {getPhpLabel(pkg)}
          </option>
        ))}
      </LabeledSelect>

      {/* Database engine: MySQL and PostgreSQL fill the same role */}
      <LabeledSelect
        label={t.databaseEngine}
        value={databaseEngine}
        testId="database-engine-select"
        onChange={(engine) =>
          // PostgreSQL moves a phpMyAdmin selection onto Adminer; otherwise the
          // manager is kept.
          onSelectionChange(
            selectionWithEngine(selection, engine as DatabaseEngine, packages.phpmyadmin)
          )
        }
      >
        {DATABASE_ENGINES.map((engine) => (
          <option key={engine} value={engine}>
            {DATABASE_ENGINE_NAMES[engine]}
          </option>
        ))}
      </LabeledSelect>

      {/* Web database manager: phpMyAdmin is MySQL-only, Adminer serves both */}
      <LabeledSelect
        label={t.databaseManager}
        value={databaseTool}
        testId="database-tool-select"
        note={t.databaseManagerNote}
        onChange={(tool) =>
          onSelectionChange(selectionWithTool(selection, tool as DatabaseTool, packages.phpmyadmin))
        }
      >
        {DATABASE_TOOLS.filter((tool) => databaseToolSupportsEngine(tool, databaseEngine)).map(
          (tool) => (
            <option key={tool} value={tool}>
              {DATABASE_TOOL_NAMES[tool]}
            </option>
          )
        )}
      </LabeledSelect>

      <LabeledSelect
        label={`${activeDatabaseName} ${t.version}`}
        value={postgresqlSelected ? selection.postgresql : selection.mysql}
        onChange={(id) =>
          onSelectionChange(
            postgresqlSelected ? { ...selection, postgresql: id } : { ...selection, mysql: id }
          )
        }
      >
        {activeDatabasePackages.map((pkg: MySQLPackage) => (
          <option key={pkg.id} value={pkg.id}>
            {getPackageLabel(pkg)}
          </option>
        ))}
      </LabeledSelect>

      <div className="info-box package-selector-info">
        <p>
          <strong>{t.wizardDefault}:</strong> {selectedPhpPackage?.display_name || "PHP"},{" "}
          {activeDatabaseName}, {postgresqlSelected ? "Adminer" : "phpMyAdmin"}
        </p>
        <p>
          <strong>{t.wizardNote}:</strong> {t.wizardEolNote}
        </p>
      </div>
    </div>
  );
}
