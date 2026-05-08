import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  PackagesConfig,
  PackageSelection,
  PhpPackage,
  MySQLPackage,
  PhpMyAdminPackage,
  getDatabaseDisplayName,
  hasPackageUrlForPlatform,
} from "../types/services";

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
  const [packages, setPackages] = useState<PackagesConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [currentPlatform, setCurrentPlatform] = useState<string>("");
  const [runtimePlatformKey, setRuntimePlatformKey] = useState<string>("");
  const [selection, setSelection] = useState<PackageSelection>(
    initialSelection || {
      php: "php-8.5",
      mysql: "mysql-9.7",
      phpmyadmin: "phpmyadmin-5.2",
    }
  );

  const getPhpLabel = (pkg: PhpPackage): string => {
    const badges: string[] = [];
    if (pkg.lts) badges.push("LTS");
    if (pkg.eol) badges.push("EOL");
    if (pkg.recommended) badges.push("Recommended");
    return badges.length > 0 ? `${pkg.display_name} [${badges.join(", ")}]` : pkg.display_name;
  };

  useEffect(() => {
    loadPackages();
    // Detect platform for database display name
    setCurrentPlatform(detectPlatform());
  }, []);

  useEffect(() => {
    if (!packages || !runtimePlatformKey) return;

    const availablePhp = packages.php.filter((pkg) =>
      hasPackageUrlForPlatform(pkg, runtimePlatformKey)
    );
    const availableMysql = packages.mysql.filter((pkg) =>
      hasPackageUrlForPlatform(pkg, runtimePlatformKey)
    );
    const nextSelection = { ...selection };

    if (!availablePhp.some((pkg) => pkg.id === nextSelection.php) && availablePhp[0]) {
      nextSelection.php = availablePhp[0].id;
    }
    if (!availableMysql.some((pkg) => pkg.id === nextSelection.mysql) && availableMysql[0]) {
      nextSelection.mysql = availableMysql[0].id;
    }

    if (nextSelection.php !== selection.php || nextSelection.mysql !== selection.mysql) {
      setSelection(nextSelection);
      return;
    }

    onSelectionChange(selection);
  }, [selection, packages, runtimePlatformKey, onSelectionChange]);

  const loadPackages = async () => {
    try {
      const [data, platformKey] = await Promise.all([
        invoke<PackagesConfig>("get_available_packages_cmd"),
        invoke<string>("get_runtime_platform"),
      ]);
      setPackages(data);
      setRuntimePlatformKey(platformKey);
    } catch (err) {
      console.error("Failed to load packages:", err);
    } finally {
      setLoading(false);
    }
  };

  const handlePhpChange = (value: string) => {
    setSelection({ ...selection, php: value });
  };

  const handleMySQLChange = (value: string) => {
    setSelection({ ...selection, mysql: value });
  };

  const handleDatabaseToolChange = (value: string) => {
    setSelection({ ...selection, phpmyadmin: value });
  };

  if (loading) {
    return (
      <div style={{ textAlign: "center", color: "var(--text-secondary)", fontSize: "0.875rem", padding: "1rem" }}>
        Loading available packages...
      </div>
    );
  }

  if (!packages) {
    return (
      <div style={{ textAlign: "center", color: "var(--color-error)", fontSize: "0.875rem" }}>
        Failed to load available packages
      </div>
    );
  }

  const availablePhpPackages = packages.php.filter((pkg) =>
    hasPackageUrlForPlatform(pkg, runtimePlatformKey)
  );
  const availableMysqlPackages = packages.mysql.filter((pkg) =>
    hasPackageUrlForPlatform(pkg, runtimePlatformKey)
  );

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
      {/* PHP Version Selector */}
      <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
        <label style={{ fontSize: "0.875rem", fontWeight: 500, color: "var(--text-primary)" }}>
          PHP Version
        </label>
        <select
          value={selection.php}
          onChange={(e) => handlePhpChange(e.target.value)}
          className="input"
          style={{ cursor: "pointer", padding: "0.375rem 0.5rem", fontSize: "0.875rem", width: "100%" }}
        >
          {availablePhpPackages.map((pkg: PhpPackage) => (
            <option key={pkg.id} value={pkg.id}>
              {getPhpLabel(pkg)}
            </option>
          ))}
        </select>
      </div>

      {/* Database Version Selector */}
      <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
        <label style={{ fontSize: "0.875rem", fontWeight: 500, color: "var(--text-primary)" }}>
          {getDatabaseDisplayName(currentPlatform)} Version
        </label>
        <select
          value={selection.mysql}
          onChange={(e) => handleMySQLChange(e.target.value)}
          className="input"
          style={{ cursor: "pointer", padding: "0.375rem 0.5rem", fontSize: "0.875rem", width: "100%" }}
        >
          {availableMysqlPackages.map((pkg: MySQLPackage) => (
            <option key={pkg.id} value={pkg.id}>
              {pkg.display_name}
            </option>
          ))}
        </select>
      </div>

      {/* Database Tool Selector */}
      <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
        <label style={{ fontSize: "0.875rem", fontWeight: 500, color: "var(--text-primary)" }}>
          Database Tool
        </label>
        <select
          value={selection.phpmyadmin}
          onChange={(e) => handleDatabaseToolChange(e.target.value)}
          className="input"
          style={{ cursor: "pointer", padding: "0.375rem 0.5rem", fontSize: "0.875rem", width: "100%" }}
        >
          {packages.phpmyadmin.map((pkg: PhpMyAdminPackage) => (
            <option key={pkg.id} value={pkg.id}>
              {pkg.display_name}
            </option>
          ))}
        </select>
      </div>

      {/* Package Info Box */}
      <div className="info-box" style={{ padding: "0.5rem", fontSize: "0.875rem" }}>
        <p style={{ fontSize: "0.875rem", color: "var(--text-secondary)", margin: "0 0 0.375rem 0" }}>
          <strong>Default:</strong> PHP 8.5, {getDatabaseDisplayName(currentPlatform)} 9.7, phpMyAdmin 5.2
        </p>
        <p style={{ fontSize: "0.875rem", color: "var(--text-secondary)", margin: 0 }}>
          <strong>Note:</strong> EOL versions are unsupported and may have security vulnerabilities. PHP 5.5–7.3 are available on Windows only.
        </p>
      </div>
    </div>
  );
}
