import { HttpsTunnelStatus } from "./services";

export const DEFAULT_TUNNEL_STATUS: HttpsTunnelStatus = {
  running: false,
  url: null,
  ready: false,
  local_url: "",
  error: null,
  log_path: null,
  pid: null,
  authtoken_configured: false,
};

type NoticeTone = "info" | "success" | "error";
type NoticeAction = "start" | "restart" | "stop";

export interface DashboardNotice {
  tone: NoticeTone;
  action?: NoticeAction;
  title: string;
  message: string;
}

export function normalizeTunnelStatus(value: unknown): HttpsTunnelStatus {
  if (
    value &&
    typeof value === "object" &&
    typeof (value as HttpsTunnelStatus).running === "boolean"
  ) {
    return {
      ...DEFAULT_TUNNEL_STATUS,
      ...(value as HttpsTunnelStatus),
      ready: Boolean((value as HttpsTunnelStatus).ready),
    };
  }

  return DEFAULT_TUNNEL_STATUS;
}
