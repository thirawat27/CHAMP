/**
 * Service types for CHAMP application
 */

export enum ServiceType {
  Caddy = "caddy",
  PhpFpm = "php-fpm",
  MySQL = "mysql",
}

export enum ServiceState {
  Stopped = "stopped",
  Starting = "starting",
  Running = "running",
  Stopping = "stopping",
  Error = "error",
}

export interface ServiceInfo {
  service_type: ServiceType;
  state: ServiceState;
  port: number;
  error_message?: string;
}

export type ServiceMap = Record<ServiceType, ServiceInfo>;

export interface AppSettings {
  web_port: number;
  mysql_port: number;
  php_port: number;
  project_root: string;
  auto_start_services?: boolean;
  package_selection?: PackageSelection;
}

export interface DownloadProgress {
  step: "downloading" | "extracting" | "installing" | "complete" | "error";
  percent: number;
  currentComponent: string;
  componentDisplay: string;
  version: string;
  totalComponents: number;
  downloadedBytes: number;
  totalBytes: number;
}

// Package selection types
export interface PhpPackage {
  id: string;
  version: string;
  display_name: string;
  windowsX64: string;
  windowsArm64: string;
  linuxX64: string;
  linuxArm64: string;
  macOSX64: string;
  macOSArm64: string;
  eol: boolean;
  lts: boolean;
  recommended: boolean;
}

export interface MySQLPackage {
  id: string;
  version: string;
  display_name: string;
  windowsX64: string;
  windowsArm64: string;
  linuxX64: string;
  linuxArm64: string;
  macOSX64: string;
  macOSArm64: string;
  eol: boolean;
  lts: boolean;
  recommended: boolean;
}

export interface PhpMyAdminPackage {
  id: string;
  version: string;
  display_name: string;
  url: string;
  eol: boolean;
  lts: boolean;
  recommended: boolean;
}

export interface PackagesConfig {
  php: PhpPackage[];
  mysql: MySQLPackage[];
  phpmyadmin: PhpMyAdminPackage[];
}

export interface PackageSelection {
  php: string;
  mysql: string;
  phpmyadmin: string;
}

export type RuntimePlatformKey =
  | "windowsX64"
  | "windowsArm64"
  | "linuxX64"
  | "linuxArm64"
  | "macOSX64"
  | "macOSArm64";

export const getPackageUrlForPlatform = (
  pkg: Pick<
    PhpPackage,
    "windowsX64" | "windowsArm64" | "linuxX64" | "linuxArm64" | "macOSX64" | "macOSArm64"
  >,
  platformKey: string
): string => {
  switch (platformKey) {
    case "windowsX64":
      return pkg.windowsX64;
    case "windowsArm64":
      return pkg.windowsArm64;
    case "linuxX64":
      return pkg.linuxX64;
    case "linuxArm64":
      return pkg.linuxArm64;
    case "macOSX64":
      return pkg.macOSX64;
    case "macOSArm64":
      return pkg.macOSArm64;
    default:
      return "";
  }
};

export const hasPackageUrlForPlatform = (
  pkg: Pick<
    PhpPackage,
    "windowsX64" | "windowsArm64" | "linuxX64" | "linuxArm64" | "macOSX64" | "macOSArm64"
  >,
  platformKey: string
): boolean => getPackageUrlForPlatform(pkg, platformKey).trim().length > 0;

export interface InstalledPhpVersion {
  id: string;
  version: string;
  display_name: string;
  installed: boolean;
  active: boolean;
  eol: boolean;
  lts: boolean;
  recommended: boolean;
  path?: string;
}

export const DEFAULT_PORTS = {
  [ServiceType.Caddy]: 8080,
  [ServiceType.PhpFpm]: 9000,
  [ServiceType.MySQL]: 3306,
} as const;

export const SERVICE_DISPLAY_NAMES = {
  [ServiceType.Caddy]: "Caddy",
  [ServiceType.PhpFpm]: "PHP-FPM",
  [ServiceType.MySQL]: "MySQL",
} as const;

// CHAMP currently ships official MySQL portable packages for supported platforms.
export const getDatabaseDisplayName = (platform?: string): string => {
  void platform;
  return "MySQL";
};

export const SERVICE_DESCRIPTIONS = {
  [ServiceType.Caddy]: "Web Server",
  [ServiceType.PhpFpm]: "PHP Runtime",
  [ServiceType.MySQL]: "Database Server",
} as const;

// System dependency types
export interface InstallCommand {
  distribution: string;
  command: string;
}

export interface Dependency {
  name: string;
  installed: boolean;
  description: string;
  install_commands: InstallCommand[];
}

export interface DependencyCheckResult {
  dependencies: Dependency[];
  all_satisfied: boolean;
  platform_notes: string;
}
