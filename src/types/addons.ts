import { ServiceState } from "./services";

/** Optional services that run beside the stack (see `src-tauri/src/addons`). */
export type AddonKind = "redis" | "mailpit" | "meilisearch";

export const ADDON_KINDS: readonly AddonKind[] = ["redis", "mailpit", "meilisearch"];

export interface AddonStatus {
  kind: AddonKind;
  name: string;
  /** Version CHAMP downloads for this platform. */
  version: string;
  installed_version: string | null;
  installed: boolean;
  source: "champ" | "system" | null;
  /** CHAMP can download it here (otherwise see `install_hint`). */
  downloadable: boolean;
  install_hint: string | null;
  update_available: boolean;
  installing: boolean;
  enabled: boolean;
  state: ServiceState;
  port: number;
  web_port: number | null;
  web_url: string | null;
  error: string | null;
  log_path: string | null;
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

/** Laravel-style `.env` lines for the enabled add-ons (mirrors the backend). */
export function addonDotenv(addons: AddonStatus[]): string {
  const byKind = new Map(addons.filter((a) => a.enabled).map((a) => [a.kind, a]));
  const redis = byKind.get("redis");
  const mailpit = byKind.get("mailpit");
  const meilisearch = byKind.get("meilisearch");
  const lines: string[] = [];
  if (redis) {
    lines.push(
      "REDIS_CLIENT=predis",
      "REDIS_HOST=127.0.0.1",
      "REDIS_PASSWORD=null",
      `REDIS_PORT=${redis.port}`,
      "CACHE_STORE=redis",
      "QUEUE_CONNECTION=redis"
    );
  }
  if (mailpit) {
    lines.push(
      "MAIL_MAILER=smtp",
      "MAIL_HOST=127.0.0.1",
      `MAIL_PORT=${mailpit.port}`,
      "MAIL_USERNAME=null",
      "MAIL_PASSWORD=null",
      "MAIL_ENCRYPTION=null"
    );
  }
  if (meilisearch) {
    lines.push(
      "SCOUT_DRIVER=meilisearch",
      `MEILISEARCH_HOST=http://127.0.0.1:${meilisearch.port}`,
      "MEILISEARCH_KEY="
    );
  }
  return lines.join("\n");
}

/** A port the user typed, or `null` when it is not a usable TCP port. */
export function parseAddonPort(value: string): number | null {
  if (!/^\d{1,5}$/.test(value.trim())) return null;
  const port = Number.parseInt(value, 10);
  return port >= 1024 && port <= 65535 ? port : null;
}
