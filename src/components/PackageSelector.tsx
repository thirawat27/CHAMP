import { useCallback, useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  PackagesConfig,
  PackageSelection,
  PhpPackage,
  MySQLPackage,
  PhpMyAdminPackage,
  EMPTY_PACKAGE_SELECTION,
  getDatabaseDisplayName,
  hasPackageUrlForPlatform,
  isAdminerSelected,
} from "../types/services";
import { useTranslation } from "../stores/languageStore";

// Helper to detect platform
const detectPlatform = (): string => {
  const userAgent = window.navigator.userAgent.toLowerCase();
  if (userAgent.includes("win")) return "windows";
  if (userAgent.includes("mac")) return "darwin";
  return "linux";
};

interface PackageSelectorProps {
  onSelectionChange: (selection: PackageSelection) => void;
  initialSelection?: PackageSelection;
}

export function PackageSelector({ onSelectionChange, initialSelection }: PackageSelectorProps) {
  const { t } = useTranslation();
  const [packages, setPackages] = useState<PackagesConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [currentPlatform, setCurrentPlatform] = useState<string>("");
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

  const loadPackages = useCallback(async () => {
    try {
      const [data, platformKey] = await Promise.all([
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
    // Detect platform for database display name
    setCurrentPlatform(detectPlatform());
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

  const handleDatabaseToolChange = (value: string) => {
    setSelection({ ...selection, phpmyadmin: value });
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
  const adminerSelected = isAdminerSelected(selection);
  const activeDatabaseName = adminerSelected
    ? "PostgreSQL"
    : getDatabaseDisplayName(currentPlatform);
  const activeDatabasePackages = adminerSelected
    ? availablePostgreSQLPackages
    : availableMysqlPackages;
  const activeDatabaseValue = adminerSelected ? selection.postgresql : selection.mysql;
  const handleActiveDatabaseChange = adminerSelected ? handlePostgreSQLChange : handleMySQLChange;

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

      {/* Database Tool Selector */}
      <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
        <label style={{ fontSize: "0.875rem", fontWeight: 500, color: "var(--text-primary)" }}>
          {t.databaseTool}
        </label>
        <select
          value={selection.phpmyadmin}
          onChange={(e) => handleDatabaseToolChange(e.target.value)}
          className="input"
          style={{
            cursor: "pointer",
            padding: "0.375rem 0.5rem",
            fontSize: "0.875rem",
            width: "100%",
          }}
        >
          {packages.phpmyadmin.map((pkg: PhpMyAdminPackage) => (
            <option key={pkg.id} value={pkg.id}>
              {pkg.display_name}
            </option>
          ))}
        </select>
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
              {pkg.display_name}
            </option>
          ))}
        </select>
      </div>

      {/* Package Info Box */}
      <div className="info-box" style={{ padding: "0.5rem", fontSize: "0.875rem" }}>
        <p
          style={{ fontSize: "0.875rem", color: "var(--text-secondary)", margin: "0 0 0.375rem 0" }}
        >
          <strong>{t.wizardDefault}:</strong> PHP 8.5, {activeDatabaseName},{" "}
          {adminerSelected ? "Adminer" : "phpMyAdmin"}
        </p>
        <p style={{ fontSize: "0.875rem", color: "var(--text-secondary)", margin: 0 }}>
          <strong>{t.wizardNote}:</strong> {t.wizardEolNote}
        </p>
      </div>
    </div>
  );
}
