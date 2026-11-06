/**
 * Service types for CHAMP application
 */

export enum ServiceType {
  Caddy = "caddy",
  PhpFpm = "php-fpm",
  MySQL = "mysql",
  PostgreSQL = "postgresql",
}

export enum ServiceState {
  Stopped = "stopped",
  Starting = "starting",
  Running = "running",
  Stopping = "stopping",
  Error = "error",
}

interface ServiceInfo {
  service_type: ServiceType;
  state: ServiceState;
  port: number;
  error_message?: string;
}

export type ServiceMap = Record<ServiceType, ServiceInfo>;

export interface AppSettings {
  web_port: number;
  mysql_port: number;
  postgresql_port: number;
  php_port: number;
  auto_start_services?: boolean;
  auto_check_updates?: boolean;
  package_selection?: PackageSelection;
  language?: string;
  sound_enabled?: boolean;
  agent_connector_enabled?: boolean;
  agent_connector_read_only?: boolean;
  agent_connector_port?: number;
}

export interface AgentConnectorStatus {
  enabled: boolean;
  running: boolean;
  read_only: boolean;
  port: number;
  url: string;
  token: string | null;
  error: string | null;
}

export interface AgentConnectorTestResult {
  ok: boolean;
  tools: string[];
  message: string;
}

export interface AgentActivity {
  tool: string;
  service: ServiceType | null;
  /** Non-service target such as a database name or "HTTPS Preview". */
  target?: string | null;
  ok: boolean;
  error: string | null;
}

export interface DownloadProgress {
  step: "downloading" | "extracting" | "complete";
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

interface PhpMyAdminPackage {
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
  postgresql: MySQLPackage[];
  phpmyadmin: PhpMyAdminPackage[];
}

export interface PackageSelection {
  php: string;
  mysql: string;
  postgresql: string;
  /** Web database manager package id (`phpmyadmin-*` or `adminer-*`). */
  phpmyadmin: string;
  /**
   * Engine that fills the database role. Chosen separately from the manager;
   * missing in settings saved before that, where Adminer meant PostgreSQL.
   */
  database_engine?: string;
}

export const EMPTY_PACKAGE_SELECTION: PackageSelection = {
  php: "",
  mysql: "",
  postgresql: "",
  phpmyadmin: "",
  database_engine: "",
};

type DatabaseSelection = Pick<PackageSelection, "phpmyadmin" | "database_engine">;

export const isAdminerSelected = (selection?: DatabaseSelection | null): boolean =>
  selection?.phpmyadmin?.startsWith("adminer") ?? false;

/**
 * MySQL and PostgreSQL fill the same "database" role of the stack; the user
 * picks one. The web manager is a separate choice: Adminer works with both,
 * phpMyAdmin only with MySQL.
 */
export type DatabaseEngine = "mysql" | "postgresql";

export type DatabaseTool = "phpmyadmin" | "adminer";

export const DATABASE_TOOLS: readonly DatabaseTool[] = ["phpmyadmin", "adminer"];

export const DATABASE_TOOL_NAMES: Record<DatabaseTool, string> = {
  phpmyadmin: "phpMyAdmin",
  adminer: "Adminer",
};

export const getDatabaseTool = (selection?: DatabaseSelection | null): DatabaseTool =>
  isAdminerSelected(selection) ? "adminer" : "phpmyadmin";

/** Whether a manager can serve an engine. phpMyAdmin only speaks MySQL. */
export const databaseToolSupportsEngine = (tool: DatabaseTool, engine: DatabaseEngine): boolean =>
  tool === "adminer" || engine === "mysql";

export const DATABASE_ENGINES: readonly DatabaseEngine[] = ["mysql", "postgresql"];

export const DATABASE_ENGINE_NAMES: Record<DatabaseEngine, string> = {
  mysql: "MySQL",
  postgresql: "PostgreSQL",
};

export const DATABASE_ENGINE_SERVICES: Record<DatabaseEngine, ServiceType> = {
  mysql: ServiceType.MySQL,
  postgresql: ServiceType.PostgreSQL,
};

export const getDatabaseEngine = (selection?: DatabaseSelection | null): DatabaseEngine => {
  const engine = selection?.database_engine;
  if (engine === "mysql" || engine === "postgresql") return engine;
  return isAdminerSelected(selection) ? "postgresql" : "mysql";
};

/** Package id of a web database manager in the catalog. */
const databaseToolId = (
  tool: DatabaseTool,
  tools: readonly Pick<PhpMyAdminPackage, "id">[]
): string | undefined => tools.find((pkg) => pkg.id.startsWith(tool))?.id;

/**
 * The selection after choosing an engine. The manager is kept when it can
 * serve the engine; phpMyAdmin moves to Adminer for PostgreSQL.
 */
export const selectionWithEngine = (
  selection: PackageSelection,
  engine: DatabaseEngine,
  tools: readonly Pick<PhpMyAdminPackage, "id">[]
): PackageSelection => {
  const next = { ...selection, database_engine: engine };
  if (!databaseToolSupportsEngine(getDatabaseTool(selection), engine)) {
    next.phpmyadmin = databaseToolId("adminer", tools) ?? selection.phpmyadmin;
  }
  return next;
};

/** The selection after choosing a manager; ignored when it cannot serve the engine. */
export const selectionWithTool = (
  selection: PackageSelection,
  tool: DatabaseTool,
  tools: readonly Pick<PhpMyAdminPackage, "id">[]
): PackageSelection => {
  const engine = getDatabaseEngine(selection);
  const id = databaseToolId(tool, tools);
  if (!id || !databaseToolSupportsEngine(tool, engine)) return selection;
  return { ...selection, phpmyadmin: id, database_engine: engine };
};

const getActiveDatabaseService = (selection?: DatabaseSelection | null): ServiceType =>
  DATABASE_ENGINE_SERVICES[getDatabaseEngine(selection)];

export const getStackServiceTypes = (selection?: DatabaseSelection | null): ServiceType[] => [
  ServiceType.Caddy,
  ServiceType.PhpFpm,
  getActiveDatabaseService(selection),
];

/** Every service CHAMP manages, in display order. */
export const ALL_SERVICE_TYPES = [
  ServiceType.Caddy,
  ServiceType.PhpFpm,
  ServiceType.MySQL,
  ServiceType.PostgreSQL,
] as const;

type PlatformUrls = Pick<
  PhpPackage,
  "windowsX64" | "windowsArm64" | "linuxX64" | "linuxArm64" | "macOSX64" | "macOSArm64"
>;

const getPackageUrlForPlatform = (pkg: PlatformUrls, platformKey: string): string => {
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

export const hasPackageUrlForPlatform = (pkg: PlatformUrls, platformKey: string): boolean =>
  getPackageUrlForPlatform(pkg, platformKey).trim().length > 0;

/** Packages of a catalog list that have a download for this platform. */
export const packagesForPlatform = <T extends PlatformUrls>(
  packages: readonly T[],
  platformKey: string
): T[] => packages.filter((pkg) => hasPackageUrlForPlatform(pkg, platformKey));

const availableOrFirst = (id: string, packages: readonly { id: string }[]): string =>
  packages.some((pkg) => pkg.id === id) || !packages[0] ? id : packages[0].id;

/** The selection with every id that this platform's catalog lacks moved to its first entry. */
export const normalizeSelection = (
  selection: PackageSelection,
  packages: PackagesConfig,
  platformKey: string
): PackageSelection => ({
  ...selection,
  php: availableOrFirst(selection.php, packagesForPlatform(packages.php, platformKey)),
  mysql: availableOrFirst(selection.mysql, packagesForPlatform(packages.mysql, platformKey)),
  postgresql: availableOrFirst(
    selection.postgresql,
    packagesForPlatform(packages.postgresql, platformKey)
  ),
  phpmyadmin: availableOrFirst(selection.phpmyadmin, packages.phpmyadmin),
});

export interface InstalledPhpVersion {
  id: string;
  version: string;
  display_name: string;
  installed: boolean;
  active: boolean;
  eol: boolean;
  lts: boolean;
  recommended: boolean;
  path: string | null;
  /** Newest patch release of this branch in the catalog */
  latest_version: string;
  /** Installed, but older than latest_version */
  update_available: boolean;
}

export interface AppPaths {
  base_dir: string;
  portable: boolean;
  runtime_dir: string;
  config_dir: string;
  mysql_data_dir: string;
  postgresql_data_dir: string;
  logs_dir: string;
  projects_dir: string;
}

export interface HttpsTunnelStatus {
  running: boolean;
  url: string | null;
  ready: boolean;
  local_url: string;
  error: string | null;
  log_path: string | null;
  pid: number | null;
  /** An ngrok authtoken is saved in CHAMP, in NGROK_AUTHTOKEN, or in ngrok's config. */
  authtoken_configured: boolean;
}

/**
 * UI pre-load fallback port numbers, used only before real settings arrive
 * from the backend. The Rust backend (`config/settings.rs` DEFAULT_PORTS) is
 * the source of truth once settings load via get_settings / get_all_statuses.
 */
export const DEFAULT_PORTS = {
  [ServiceType.Caddy]: 8080,
  [ServiceType.PhpFpm]: 9000,
  [ServiceType.MySQL]: 3306,
  [ServiceType.PostgreSQL]: 5432,
} as const;

export const SERVICE_DISPLAY_NAMES = {
  [ServiceType.Caddy]: "Caddy",
  [ServiceType.PhpFpm]: "PHP-FPM",
  [ServiceType.MySQL]: "MySQL",
  [ServiceType.PostgreSQL]: "PostgreSQL",
} as const;

export const SERVICE_DESCRIPTION_KEYS = {
  [ServiceType.Caddy]: "serviceDescriptionCaddy",
  [ServiceType.PhpFpm]: "serviceDescriptionPhp",
  [ServiceType.MySQL]: "serviceDescriptionMysql",
  [ServiceType.PostgreSQL]: "serviceDescriptionPostgresql",
} as const;

// System dependency types
interface InstallCommand {
  distribution: string;
  command: string;
}

interface Dependency {
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

// Log Doctor
interface LocalizedText {
  en: string;
  th: string;
}

interface LocalizedSteps {
  en: string[];
  th: string[];
}

export type LogDoctorAction =
  | "open_settings"
  | "open_logs"
  | "open_url"
  | "start_php"
  | "open_mysql_data"
  | "open_postgresql_data"
  | "open_runtime";

export interface DiagnosisMatch {
  id: string;
  title: LocalizedText;
  cause: LocalizedText;
  steps: LocalizedSteps;
  confidence: number;
  evidence: string;
  method: "signature" | "ai";
  action: LogDoctorAction | null;
  url: string | null;
}

export interface Diagnosis {
  service: ServiceType;
  ai_used: boolean;
  matches: DiagnosisMatch[];
  excerpt: string[];
  log_files: string[];
}

export interface LogDoctorStatus {
  installed: boolean;
  downloading: boolean;
  model_name: string;
  parameters: string;
  download_bytes: number;
}

export interface LogDoctorProgress {
  downloaded_bytes: number;
  total_bytes: number;
  percent: number;
}
