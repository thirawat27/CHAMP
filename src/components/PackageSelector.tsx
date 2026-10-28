import { useCallback, useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  PackagesConfig,
  PackageSelection,
  PhpPackage,
  MySQLPackage,
  DATABASE_ENGINES,
  DATABASE_ENGINE_NAMES,
  DatabaseEngine,
  EMPTY_PACKAGE_SELECTION,
  databaseToolIdForEngine,
  getDatabaseEngine,
  hasPackageUrlForPlatform,
} from "../types/services";
import { useTranslation } from "../stores/languageStore";

interface PackageSelectorProps {
  onSelectionChange: (selection: PackageSelection) => void;
  initialSelection?: PackageSelection;
}

export function PackageSelector({ onSelectionChange, initialSelection }: PackageSelectorProps) {
  const { t } = useTranslation();
  const [packages, setPackages] = useState<PackagesConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [runtimePlatformKey, setRuntimePlatformKey] = useState<string>("");
  const [selection, setSelection] = useState<PackageSelection>(
    initialSelection || EMPTY_PACKAGE_SELECTION
  );

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

  const loadPackages = useCallback(async () => {
    try {
      const [data, platformKey] = await Promise.all([
        // The wizard refreshes the catalog on open; read the cached result.
        invoke<PackagesConfig>("get_available_packages_cmd"),
        invoke<string>("get_runtime_platform"),
      ]);
      setPackages(data);
      setRuntimePlatformKey(platformKey);
      if (!initialSelection) {
        setSelection(await invoke<PackageSelection>("get_selected_package_ids"));
      }
    } catch (err) {
      console.error("Failed to load packages:", err);
    } finally {
      setLoading(false);
    }
  }, [initialSelection]);

  useEffect(() => {
    loadPackages();
  }, [loadPackages]);

  useEffect(() => {
    if (!packages || !runtimePlatformKey) return;

    const availablePhp = packages.php.filter((pkg) =>
      hasPackageUrlForPlatform(pkg, runtimePlatformKey)
    );
    const availableMysql = packages.mysql.filter((pkg) =>
      hasPackageUrlForPlatform(pkg, runtimePlatformKey)
    );
    const availablePostgreSQL = packages.postgresql.filter((pkg) =>
      hasPackageUrlForPlatform(pkg, runtimePlatformKey)
    );
    const availableDatabaseTools = packages.phpmyadmin;
    const nextSelection = { ...selection };

    if (!availablePhp.some((pkg) => pkg.id === nextSelection.php) && availablePhp[0]) {
      nextSelection.php = availablePhp[0].id;
    }
    if (!availableMysql.some((pkg) => pkg.id === nextSelection.mysql) && availableMysql[0]) {
      nextSelection.mysql = availableMysql[0].id;
    }
    if (
      !availablePostgreSQL.some((pkg) => pkg.id === nextSelection.postgresql) &&
      availablePostgreSQL[0]
    ) {
      nextSelection.postgresql = availablePostgreSQL[0].id;
    }
    if (
      !availableDatabaseTools.some((pkg) => pkg.id === nextSelection.phpmyadmin) &&
      availableDatabaseTools[0]
    ) {
      nextSelection.phpmyadmin = availableDatabaseTools[0].id;
    }

    if (
      nextSelection.php !== selection.php ||
      nextSelection.mysql !== selection.mysql ||
      nextSelection.postgresql !== selection.postgresql ||
      nextSelection.phpmyadmin !== selection.phpmyadmin
    ) {
      setSelection(nextSelection);
      return;
    }

    onSelectionChange(selection);
  }, [selection, packages, runtimePlatformKey, onSelectionChange]);

  const handlePhpChange = (value: string) => {
    setSelection({ ...selection, php: value });
  };

  const handleMySQLChange = (value: string) => {
    setSelection({ ...selection, mysql: value });
  };

  const handlePostgreSQLChange = (value: string) => {
    setSelection({ ...selection, postgresql: value });
  };

  // The engine is the choice; its web manager follows automatically.
  const handleDatabaseEngineChange = (engine: DatabaseEngine) => {
    const toolId = databaseToolIdForEngine(engine, packages?.phpmyadmin ?? []);
    if (toolId) setSelection({ ...selection, phpmyadmin: toolId });
  };

  if (loading) {
    return (
      <div
        style={{
          textAlign: "center",
          color: "var(--text-secondary)",
          fontSize: "0.875rem",
          padding: "1rem",
        }}
      >
        {t.wizardLoadingPackages}
      </div>
    );
  }

  if (!packages) {
    return (
      <div style={{ textAlign: "center", color: "var(--color-error)", fontSize: "0.875rem" }}>
        {t.wizardFailedToLoadPackages}
      </div>
    );
  }

  const availablePhpPackages = packages.php.filter((pkg) =>
    hasPackageUrlForPlatform(pkg, runtimePlatformKey)
  );
  const availableMysqlPackages = packages.mysql.filter((pkg) =>
    hasPackageUrlForPlatform(pkg, runtimePlatformKey)
  );
  const availablePostgreSQLPackages = packages.postgresql.filter((pkg) =>
    hasPackageUrlForPlatform(pkg, runtimePlatformKey)
  );
  const databaseEngine = getDatabaseEngine(selection);
  const adminerSelected = databaseEngine === "postgresql";
  const activeDatabaseName = DATABASE_ENGINE_NAMES[databaseEngine];
  const activeDatabasePackages = adminerSelected
    ? availablePostgreSQLPackages
    : availableMysqlPackages;
  const activeDatabaseValue = adminerSelected ? selection.postgresql : selection.mysql;
  const handleActiveDatabaseChange = adminerSelected ? handlePostgreSQLChange : handleMySQLChange;
  const selectedPhpPackage = availablePhpPackages.find((pkg) => pkg.id === selection.php);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
      {/* PHP Version Selector */}
      <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
        <label style={{ fontSize: "0.875rem", fontWeight: 500, color: "var(--text-primary)" }}>
          {t.phpVersion}
        </label>
        <select
          value={selection.php}
          onChange={(e) => handlePhpChange(e.target.value)}
          className="input"
          style={{
            cursor: "pointer",
            padding: "0.375rem 0.5rem",
            fontSize: "0.875rem",
            width: "100%",
          }}
        >
          {availablePhpPackages.map((pkg: PhpPackage) => (
            <option key={pkg.id} value={pkg.id}>
              {getPhpLabel(pkg)}
            </option>
          ))}
        </select>
      </div>

      {/* Database engine: MySQL and PostgreSQL fill the same role */}
      <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
        <label style={{ fontSize: "0.875rem", fontWeight: 500, color: "var(--text-primary)" }}>
          {t.databaseEngine}
        </label>
        <select
          value={databaseEngine}
          onChange={(e) => handleDatabaseEngineChange(e.target.value as DatabaseEngine)}
          className="input"
          data-testid="database-engine-select"
          style={{
            cursor: "pointer",
            padding: "0.375rem 0.5rem",
            fontSize: "0.875rem",
            width: "100%",
          }}
        >
          {DATABASE_ENGINES.map((engine) => (
            <option key={engine} value={engine}>
              {DATABASE_ENGINE_NAMES[engine]}
            </option>
          ))}
        </select>
        <small style={{ color: "var(--text-secondary)" }}>
          {t.databaseManagerAuto.replace("{tool}", adminerSelected ? "Adminer" : "phpMyAdmin")}
        </small>
      </div>

      {/* Active Database Version Selector */}
      <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
        <label style={{ fontSize: "0.875rem", fontWeight: 500, color: "var(--text-primary)" }}>
          {activeDatabaseName} {t.version}
        </label>
        <select
          value={activeDatabaseValue}
          onChange={(e) => handleActiveDatabaseChange(e.target.value)}
          className="input"
          style={{
            cursor: "pointer",
            padding: "0.375rem 0.5rem",
            fontSize: "0.875rem",
            width: "100%",
          }}
        >
          {activeDatabasePackages.map((pkg: MySQLPackage) => (
            <option key={pkg.id} value={pkg.id}>
              {getPackageLabel(pkg)}
            </option>
          ))}
        </select>
      </div>

      {/* Package Info Box */}
      <div className="info-box" style={{ padding: "0.5rem", fontSize: "0.875rem" }}>
        <p
          style={{ fontSize: "0.875rem", color: "var(--text-secondary)", margin: "0 0 0.375rem 0" }}
        >
          <strong>{t.wizardDefault}:</strong> {selectedPhpPackage?.display_name || "PHP"},{" "}
          {activeDatabaseName}, {adminerSelected ? "Adminer" : "phpMyAdmin"}
        </p>
        <p style={{ fontSize: "0.875rem", color: "var(--text-secondary)", margin: 0 }}>
          <strong>{t.wizardNote}:</strong> {t.wizardEolNote}
        </p>
      </div>
    </div>
  );
}
