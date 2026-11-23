import { ServiceState } from "./services";

/** Built-in optional services (see `src-tauri/src/addons/spec.rs`). */
export type BuiltinAddonKind = "redis" | "mailpit" | "meilisearch" | "centrifugo" | "rustfs";

/** A built-in key or a custom add-on's id. */
export type AddonKind = string;

export const ADDON_KINDS: readonly BuiltinAddonKind[] = [
  "redis",
  "mailpit",
  "meilisearch",
  "centrifugo",
  "rustfs",
];

export function isBuiltinAddon(kind: string): kind is BuiltinAddonKind {
  return (ADDON_KINDS as readonly string[]).includes(kind);
}

export interface AddonPort {
  role: string;
  label: string;
  port: number;
}

/** A service the user runs from their own binary (never downloaded by CHAMP). */
export interface CustomAddon {
  id: string;
  name: string;
  /** Absolute path to the executable. */
  command: string;
  /** `{port}` and `{data_dir}` are replaced at start. */
  args: string[];
  port: number;
  web: boolean;
  enabled: boolean;
}

export interface AddonStatus {
  kind: AddonKind;
  name: string;
  category: string;
  custom: boolean;
  /** Version CHAMP downloads for this platform (empty for custom add-ons). */
  version: string;
  installed_version: string | null;
  installed: boolean;
  source: "champ" | "system" | "custom" | null;
  /** CHAMP can download it here (otherwise see `install_hint`). */
  downloadable: boolean;
  install_hint: string | null;
  update_available: boolean;
  installing: boolean;
  enabled: boolean;
  state: ServiceState;
  port: number;
  web_port: number | null;
  ports?: AddonPort[];
  web_url: string | null;
  error: string | null;
  log_path: string | null;
  definition?: CustomAddon | null;
}

export interface AddonChange {
  enabled?: boolean;
  port?: number;
  web_port?: number;
}

export interface AddonInstallProgress {
  kind: AddonKind;
  percent: number;
}

/** One framework's configuration lines (built by `src-tauri/src/envprofile.rs`). */
export interface EnvProfile {
  id: string;
  label: string;
  target: string;
  format: "dotenv" | "php";
  content: string;
  notes: string[];
}

export interface EnvProfilesReply {
  project: string | null;
  framework: string | null;
  recommended: string;
  profiles: EnvProfile[];
}

export interface ProjectInfo {
  name: string;
  path: string;
  framework: string | null;
  framework_name: string | null;
  web_root: string;
  php_requirement: string | null;
  has_composer: boolean;
  has_vendor: boolean;
  has_env: boolean;
  has_env_example: boolean;
}

/** A long-running PHP command for one project. */
export interface ProjectWorker {
  id: string;
  project: string;
  name: string;
  /** Arguments after `php`; the first is a script inside the project. */
  args: string[];
  enabled: boolean;
}

export interface WorkerStatus extends ProjectWorker {
  state: ServiceState;
  error: string | null;
  command_line: string;
  log_path: string | null;
}

export interface XdebugStatus {
  enabled: boolean;
  available: boolean;
  extension_path: string | null;
  ext_dir: string | null;
  client_port: number;
  hint: string | null;
}

/** Built-ins in catalog order, then custom add-ons in the order they were added. */
export function orderAddons(addons: AddonStatus[]): AddonStatus[] {
  const builtins = ADDON_KINDS.map((kind) => addons.find((addon) => addon.kind === kind)).filter(
    (addon): addon is AddonStatus => Boolean(addon)
  );
  return [...builtins, ...addons.filter((addon) => addon.custom)];
}

/** A port the user typed, or `null` when it is not a usable TCP port. */
export function parseAddonPort(value: string): number | null {
  if (!/^\d{1,5}$/.test(value.trim())) return null;
  const port = Number.parseInt(value, 10);
  return port >= 1024 && port <= 65535 ? port : null;
}

/** Lowercase letters, digits, and inner hyphens, starting with a letter. */
export function isValidAddonId(id: string): boolean {
  return /^[a-z](?:[a-z0-9-]{0,30}[a-z0-9])?$/.test(id);
}

/** Split a typed command line into arguments (quotes group words). */
export function splitArgs(line: string): string[] {
  const args: string[] = [];
  const pattern = /"([^"]*)"|'([^']*)'|(\S+)/g;
  for (const match of line.matchAll(pattern)) {
    args.push(match[1] ?? match[2] ?? match[3]);
  }
  return args;
}
