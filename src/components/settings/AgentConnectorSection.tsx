import { Bot, Check, Copy, KeyRound } from "lucide-react";
import { useState } from "react";
import { useAgentConnector } from "../../hooks/useAgentConnector";
import { useTranslation } from "../../stores/languageStore";
import { AudioManager } from "../../utils/audioManager";

type SnippetId = "claude" | "json";

/** Local MCP server so AI coding agents can read and control the stack. */
export function AgentConnectorSection() {
  const { t } = useTranslation();
  const { status, busy, error, configure, regenerateToken } = useAgentConnector();
  const [portDraft, setPortDraft] = useState<string | null>(null);
  const [copied, setCopied] = useState<SnippetId | null>(null);

  if (!status) return null;

  const token = status.token ?? "";
  const snippets: Record<SnippetId, string> = {
    claude: `claude mcp add --transport http champ ${status.url} --header "Authorization: Bearer ${token}"`,
    json: JSON.stringify(
      {
        mcpServers: {
          champ: { type: "http", url: status.url, headers: { Authorization: `Bearer ${token}` } },
        },
      },
      null,
      2
    ),
  };

  const copy = async (id: SnippetId) => {
    try {
      await navigator.clipboard.writeText(snippets[id]);
      AudioManager.playClick();
      setCopied(id);
      window.setTimeout(() => setCopied((current) => (current === id ? null : current)), 1600);
    } catch (copyError) {
      console.error("Failed to copy agent config:", copyError);
    }
  };

  const commitPort = () => {
    if (portDraft === null) return;
    const port = Number.parseInt(portDraft, 10);
    setPortDraft(null);
    if (Number.isInteger(port) && port !== status.port) {
      configure({ port });
    }
  };

  return (
    <div className="settings-section">
      <h3>
        <Bot size={16} /> {t.agentConnectorTitle}
      </h3>
      <p className="settings-muted">{t.agentConnectorDescription}</p>

      <label className="toggle-row">
        <input
          type="checkbox"
          checked={status.enabled}
          disabled={busy}
          onChange={(event) => {
            AudioManager.playToggle();
            configure({ enabled: event.target.checked });
          }}
        />
        <span>{t.agentConnectorEnable}</span>
        {status.enabled && (
          <span className={`agent-state ${status.running ? "running" : "error"}`}>
            {status.running ? t.agentConnectorRunning : t.agentConnectorStopped}
          </span>
        )}
      </label>

      {error && (
        <div className="error-box">
          <p className="error-box-text">{error}</p>
        </div>
      )}
      {status.error && (
        <div className="error-box">
          <p className="error-box-text">{status.error}</p>
        </div>
      )}

      {status.enabled && (
        <>
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={status.read_only}
              disabled={busy}
              onChange={(event) => {
                AudioManager.playToggle();
                configure({ read_only: event.target.checked });
              }}
            />
            <span>{t.agentConnectorReadOnly}</span>
          </label>
          <p className="settings-muted settings-toggle-hint">
            {status.read_only ? t.agentConnectorReadOnlyHint : t.agentConnectorControlHint}
          </p>

          <div className="settings-grid">
            <label>
              <span>{t.agentConnectorPort}</span>
              <input
                className="input"
                type="number"
                min={1024}
                max={65535}
                value={portDraft ?? status.port}
                onChange={(event) => setPortDraft(event.target.value)}
                onBlur={commitPort}
                onKeyDown={(event) => {
                  if (event.key === "Enter") commitPort();
                }}
              />
            </label>
            <label>
              <span>{t.agentConnectorEndpoint}</span>
              <input className="input mono" value={status.url} readOnly />
            </label>
          </div>

          <div className="agent-snippets">
            {(["claude", "json"] as const).map((id) => (
              <div className="agent-snippet" key={id}>
                <div className="agent-snippet-head">
                  <span>{id === "claude" ? t.agentConnectorClaudeCode : t.agentConnectorJson}</span>
                  <button
                    type="button"
                    className="btn-secondary compact"
                    onClick={() => copy(id)}
                    disabled={!token}
                  >
                    {copied === id ? <Check size={14} /> : <Copy size={14} />}
                    {copied === id ? t.agentConnectorCopied : t.agentConnectorCopy}
                  </button>
                </div>
                <pre>{token ? snippets[id].replace(token, "••••••••") : snippets[id]}</pre>
              </div>
            ))}
          </div>

          <div className="settings-inline-actions">
            <span className="settings-muted">{t.agentConnectorTokenHint}</span>
            <button
              type="button"
              className="btn-secondary compact"
              onClick={() => {
                AudioManager.playClick();
                regenerateToken();
              }}
              disabled={busy}
            >
              <KeyRound size={14} /> {t.agentConnectorRegenerate}
            </button>
          </div>
        </>
      )}
    </div>
  );
}
