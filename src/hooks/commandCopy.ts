/** Translation keys and sound action of each stack and service command. */

export const STACK_COMMAND_COPY = {
  start_all_services: {
    pendingTitleKey: "stackStartingTitle",
    pendingMessageKey: "stackStartingMessage",
    successTitleKey: "stackStartedTitle",
    successMessageKey: "stackStartedMessage",
    action: "start",
  },
  restart_all_services: {
    pendingTitleKey: "stackRestartingTitle",
    pendingMessageKey: "stackRestartingMessage",
    successTitleKey: "stackRestartedTitle",
    successMessageKey: "stackRestartedMessage",
    action: "restart",
  },
  stop_all_services: {
    pendingTitleKey: "stackStoppingTitle",
    pendingMessageKey: "stackStoppingMessage",
    successTitleKey: "stackStoppedTitle",
    successMessageKey: "stackStoppedMessage",
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
