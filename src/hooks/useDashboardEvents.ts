import { useEffect } from "react";
import { UPDATE_CHECK_DELAY_MS, UPDATE_RECHECK_INTERVAL_MS } from "../constants";
import { getTranslations } from "../stores/preferencesStore";
import { useUpdateStore } from "../stores/updateStore";
import { DashboardNotice } from "../types/dashboard";
import { AgentActivity, SERVICE_DISPLAY_NAMES } from "../types/services";
import { listenIfTauri } from "../utils/tauri";

/** Tell the user whenever an AI agent changes something through the connector. */
export function useAgentActivityNotices(
  notify: (notice: DashboardNotice) => void,
  refresh: () => void
) {
  useEffect(() => {
    const unlisten = listenIfTauri<AgentActivity>("agent-activity", (event) => {
      const t = getTranslations();
      const activity = event.payload;
      const service = activity.service ? SERVICE_DISPLAY_NAMES[activity.service] : null;
      const target = activity.target
        ? service
          ? `${activity.target} (${service})`
          : activity.target
        : (service ?? t.stack);
      notify({
        tone: activity.ok ? "success" : "error",
        title: t.agentActivityTitle,
        message: activity.ok
          ? t.agentActivityDone.replace("{action}", activity.tool).replace("{target}", target)
          : (activity.error ?? activity.tool),
      });
      refresh();
    });
    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, [notify, refresh]);
}

/** Background update checks (silent when offline), and a toast when an install fails. */
export function useUpdateChecks(
  autoCheckEnabled: boolean,
  notify: (notice: DashboardNotice) => void
) {
  const autoCheck = useUpdateStore((state) => state.autoCheck);

  useEffect(() => {
    if (!autoCheckEnabled) return undefined;
    const timer = window.setTimeout(autoCheck, UPDATE_CHECK_DELAY_MS);
    const interval = window.setInterval(autoCheck, UPDATE_RECHECK_INTERVAL_MS);
    return () => {
      window.clearTimeout(timer);
      window.clearInterval(interval);
    };
  }, [autoCheck, autoCheckEnabled]);

  // The banner hides when an install fails, so say what happened.
  useEffect(
    () =>
      useUpdateStore.subscribe((state, previous) => {
        if (previous.phase === "installing" && state.phase === "error") {
          notify({
            tone: "error",
            title: getTranslations().updateFailed,
            message: state.error ?? "",
          });
        }
      }),
    [notify]
  );
}
