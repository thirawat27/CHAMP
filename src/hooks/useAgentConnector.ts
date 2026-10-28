import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { AgentConnectorStatus } from "../types/services";

export interface AgentConnectorController {
  status: AgentConnectorStatus | null;
  busy: boolean;
  error: string | null;
  configure: (
    changes: Partial<Pick<AgentConnectorStatus, "enabled" | "read_only" | "port">>
  ) => Promise<void>;
  regenerateToken: () => Promise<void>;
}

/** Status and settings of the local MCP server for AI coding agents. */
export function useAgentConnector(): AgentConnectorController {
  const [status, setStatus] = useState<AgentConnectorStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<AgentConnectorStatus>("get_agent_connector_status")
      .then(setStatus)
      .catch((statusError) => setError(String(statusError)));
  }, []);

  const run = useCallback(async (action: () => Promise<AgentConnectorStatus>) => {
    setBusy(true);
    setError(null);
    try {
      setStatus(await action());
    } catch (actionError) {
      setError(String(actionError));
    } finally {
      setBusy(false);
    }
  }, []);

  const configure = useCallback<AgentConnectorController["configure"]>(
    (changes) =>
      run(() =>
        invoke<AgentConnectorStatus>("configure_agent_connector", {
          enabled: changes.enabled ?? status?.enabled ?? false,
          readOnly: changes.read_only ?? status?.read_only ?? false,
          port: changes.port ?? status?.port ?? 47823,
        })
      ),
    [run, status]
  );

  const regenerateToken = useCallback(
    () => run(() => invoke<AgentConnectorStatus>("regenerate_agent_token")),
    [run]
  );

  return { status, busy, error, configure, regenerateToken };
}
