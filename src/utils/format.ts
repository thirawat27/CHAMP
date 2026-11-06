/** Human-readable byte counts: "0 B", "512 B", "1.5 MB". */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";

  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** exponent;
  const precision = exponent === 0 ? 0 : 1;

  return `${value.toFixed(precision)} ${units[exponent]}`;
}

/** Whole megabytes, for download size labels such as "Enable AI (29 MB)". */
export const formatModelSize = (bytes: number): string => `${Math.round(bytes / 1024 / 1024)} MB`;
