import { HttpsTunnelStatus } from "../../types/services";

export type { AppPaths } from "../../types/services";

export const SOURCE_REPO_URL = "https://github.com/thirawat27/CHAMP";

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

export type NoticeTone = "info" | "success" | "error";
export type NoticeAction = "start" | "restart" | "stop";

export interface DashboardNotice {
  tone: NoticeTone;
  action?: NoticeAction;
  title: string;
  message: string;
}

export const STACK_COMMAND_COPY = {
  start_all_services: {
    pendingTitleKey: "stackStartingTitle",
    pendingMessageKey: "stackStartingMessage",
    successTitleKey: "stackStartedTitle",
    successMessageKey: "stackStartedMessage",
    buttonLabelKey: "starting",
    action: "start",
  },
  restart_all_services: {
    pendingTitleKey: "stackRestartingTitle",
    pendingMessageKey: "stackRestartingMessage",
    successTitleKey: "stackRestartedTitle",
    successMessageKey: "stackRestartedMessage",
    buttonLabelKey: "restarting",
    action: "restart",
  },
  stop_all_services: {
    pendingTitleKey: "stackStoppingTitle",
    pendingMessageKey: "stackStoppingMessage",
    successTitleKey: "stackStoppedTitle",
    successMessageKey: "stackStoppedMessage",
    buttonLabelKey: "stopping",
    action: "stop",
  },
} as const;

export const SERVICE_COMMAND_COPY = {
  start_service: {
    pendingTitleKey: "serviceStartingTitle",
    pendingMessageKey: "serviceStartingMessage",
    successTitleKey: "serviceStarted",
    buttonLabelKey: "starting",
    action: "start",
  },
  restart_service: {
    pendingTitleKey: "serviceRestartingTitle",
    pendingMessageKey: "serviceRestartingMessage",
    successTitleKey: "serviceRestarted",
    buttonLabelKey: "restarting",
    action: "restart",
  },
  stop_service: {
    pendingTitleKey: "serviceStoppingTitle",
    pendingMessageKey: "serviceStoppingMessage",
    successTitleKey: "serviceStopped",
    buttonLabelKey: "stopping",
    action: "stop",
  },
} as const;

export type StackCommand = keyof typeof STACK_COMMAND_COPY;
export type ServiceCommand = keyof typeof SERVICE_COMMAND_COPY;

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
