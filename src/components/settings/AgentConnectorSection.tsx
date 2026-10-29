import { Bot, Check, Copy, KeyRound, PlugZap } from "lucide-react";
import { useState } from "react";
import { useAgentConnector } from "../../hooks/useAgentConnector";
import { useTranslation } from "../../stores/languageStore";
import { AudioManager } from "../../utils/audioManager";
import {
  AGENT_CLIENT_IDS,
  AgentClientId,
  agentClientSetup,
  maskToken,
} from "../../utils/agentSnippets";

/** Local MCP server so AI coding agents can read and control the stack. */
export function AgentConnectorSection() {
  const { t } = useTranslation();
  const { status, busy, error, configure, regenerateToken, testResult, testing, test } =
    useAgentConnector();
  const [portDraft, setPortDraft] = useState<string | null>(null);
  const [client, setClient] = useState<AgentClientId>("claude-code");
  const [copied, setCopied] = useState(false);

  if (!status) return null;

  const token = status.token ?? "";
  const setup = agentClientSetup(client, status.url, token);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(setup.snippet);
      AudioManager.playClick();
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
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
          <p className="agent-step">{t.agentConnectorStep1}</p>
          <div className="agent-clients" role="tablist">
            {AGENT_CLIENT_IDS.map((id) => (
              <button
                key={id}
                type="button"
                role="tab"
                aria-selected={client === id}
                className={`agent-client ${client === id ? "selected" : ""}`}
                onClick={() => {
                  AudioManager.playClick();
                  setClient(id);
                  setCopied(false);
                }}
              >
                {agentClientSetup(id, status.url, token).label}
              </button>
            ))}
          </div>

          <p className="agent-step">{t.agentConnectorStep2}</p>
          <div className="agent-snippet">
            <div className="agent-snippet-head">
              <span>
                {setup.kind === "command" ? t.agentConnectorRunIn : t.agentConnectorPasteInto}{" "}
                <span className="agent-snippet-target">{setup.target}</span>
              </span>
              <button
                type="button"
                className="btn-secondary compact"
                onClick={copy}
                disabled={!token}
              >
                {copied ? <Check size={14} /> : <Copy size={14} />}
                {copied ? t.agentConnectorCopied : t.agentConnectorCopy}
              </button>
            </div>
            <pre>{maskToken(setup.snippet, token)}</pre>
          </div>
          {client === "claude-desktop" && (
            <p className="settings-muted">{t.agentConnectorDesktopHint}</p>
          )}
          <p className="settings-muted">{t.agentConnectorRestartHint}</p>

          <p className="agent-step">{t.agentConnectorStep3}</p>
          <div className="settings-inline-actions">
            <button
              type="button"
              className="btn-secondary compact"
              onClick={() => {
                AudioManager.playClick();
                test();
              }}
              disabled={busy || testing || !status.running}
            >
              <PlugZap size={14} /> {testing ? t.agentConnectorTesting : t.agentConnectorTest}
            </button>
          </div>
          {testResult &&
            (testResult.ok ? (
              <div className="success-box">
                <Check size={14} />
                <div>
                  <p className="success-box-text">
                    {t.agentConnectorTestOk.replace("{count}", String(testResult.tools.length))}
                  </p>
                  <p className="agent-tools">{testResult.tools.join(", ")}</p>
                </div>
              </div>
            ) : (
              <div className="error-box">
                <p className="error-box-text">
                  {t.agentConnectorTestFailed}: {testResult.message}
                </p>
              </div>
            ))}

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

          <details className="settings-details">
            <summary>{t.agentConnectorAdvanced}</summary>
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
          </details>
        </>
      )}
    </div>
  );
}
