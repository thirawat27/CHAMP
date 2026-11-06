/** Timing and limits of the UI, in one place. */

export const STATUS_POLL_INTERVAL_MS = 3500;
export const METRICS_REFRESH_INTERVAL_MS = 2500;

export const UPDATE_CHECK_DELAY_MS = 4000;
/** The tray keeps CHAMP running for weeks; autoCheck still limits it to once a day. */
export const UPDATE_RECHECK_INTERVAL_MS = 60 * 60 * 1000;

export const TOAST_DISMISS_MS = 4200;
export const SETTINGS_CLOSE_DELAY_MS = 700;
export const COPIED_FEEDBACK_MS = 1600;

/** Longest error shown on a service card; the full text is in the tooltip. */
export const SERVICE_ERROR_PREVIEW_LENGTH = 180;

/** Caddy, PHP, a database, and a database manager. */
export const WIZARD_COMPONENT_COUNT = 4;

/** Mirrors `default_agent_connector_port` in `config/settings.rs`. */
export const DEFAULT_AGENT_CONNECTOR_PORT = 47823;
