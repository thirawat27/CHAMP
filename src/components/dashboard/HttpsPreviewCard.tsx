import { Copy, ExternalLink, LoaderCircle, ShieldCheck, Square } from "lucide-react";
import { useState } from "react";
import { HttpsTunnelController } from "../../hooks/useHttpsTunnel";
import { useTranslation } from "../../stores/preferencesStore";

const NGROK_TOKEN_URL = "https://dashboard.ngrok.com/get-started/your-authtoken";

export function HttpsPreviewCard({
  tunnel,
  openExternal,
}: {
  tunnel: HttpsTunnelController;
  openExternal: (url: string) => void;
}) {
  const { t } = useTranslation();
  const {
    tunnelStatus,
    tunnelStatusLoaded,
    tunnelBusy,
    tunnelReady,
    tunnelHasPendingUrl,
    startHttpsTunnel,
    stopHttpsTunnel,
    copyHttpsTunnelUrl,
    saveNgrokAuthtoken,
  } = tunnel;
  const [tokenDraft, setTokenDraft] = useState("");
  const [editingToken, setEditingToken] = useState(false);
  // Hidden until the first status loads, so a saved token never flashes the form.
  const showTokenForm =
    tunnelStatusLoaded &&
    !tunnelStatus.running &&
    (!tunnelStatus.authtoken_configured || editingToken);

  const submitToken = async () => {
    if (await saveNgrokAuthtoken(tokenDraft.trim())) {
      setTokenDraft("");
      setEditingToken(false);
    }
  };

  return (
    <section
      className={`rail-card https-tunnel-panel ${tunnelReady ? "ready" : tunnelStatus.running ? "running" : ""}`}
      data-testid="https-tunnel-panel"
    >
      <header className="rail-card-head">
        <ShieldCheck size={16} />
        <h2>{t.httpsPreview}</h2>
      </header>
      <p className={tunnelReady ? "https-tunnel-url" : "rail-muted"}>
        {tunnelReady
          ? tunnelStatus.url
          : tunnelHasPendingUrl
            ? t.httpsTunnelValidating
            : tunnelStatus.running
              ? t.httpsTunnelWaitingForDomain
              : t.httpsTunnelDescription}
      </p>
      {tunnelHasPendingUrl && (
        <small className="https-tunnel-pending-url">{tunnelStatus.url}</small>
      )}
      {tunnelStatus.error && <small className="rail-error">{tunnelStatus.error}</small>}
      {showTokenForm && (
        <form
          className="https-tunnel-token"
          onSubmit={(event) => {
            event.preventDefault();
            void submitToken();
          }}
        >
          <label htmlFor="ngrok-authtoken">{t.httpsTunnelTokenLabel}</label>
          <small className="rail-muted">{t.httpsTunnelTokenHelp}</small>
          <input
            id="ngrok-authtoken"
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={tokenDraft}
            onChange={(event) => setTokenDraft(event.target.value)}
          />
          <div className="https-tunnel-actions">
            <button
              type="submit"
              className="btn-primary compact"
              // An empty save clears a saved token.
              disabled={!tokenDraft.trim() && !tunnelStatus.authtoken_configured}
            >
              {t.httpsTunnelTokenSave}
            </button>
            {editingToken && (
              <button
                type="button"
                className="btn-secondary compact"
                onClick={() => {
                  setTokenDraft("");
                  setEditingToken(false);
                }}
              >
                {t.cancel}
              </button>
            )}
            <button
              type="button"
              className="btn-secondary compact"
              onClick={() => openExternal(NGROK_TOKEN_URL)}
            >
              <ExternalLink size={14} /> {t.httpsTunnelTokenGet}
            </button>
          </div>
        </form>
      )}
      <div className="https-tunnel-actions">
        {tunnelReady && (
          <>
            <button
              type="button"
              className="btn-secondary compact"
              onClick={() => openExternal(tunnelStatus.url ?? "")}
              title={t.httpsTunnelOpen}
            >
              <ExternalLink size={14} /> {t.open}
            </button>
            <button
              type="button"
              className="btn-secondary compact"
              onClick={copyHttpsTunnelUrl}
              title={t.httpsTunnelCopy}
            >
              <Copy size={14} /> {t.httpsTunnelCopy}
            </button>
          </>
        )}
        <button
          type="button"
          className={`compact ${tunnelStatus.running ? "btn-secondary danger" : "btn-primary"}`}
          onClick={tunnelStatus.running ? stopHttpsTunnel : startHttpsTunnel}
          disabled={Boolean(tunnelBusy)}
        >
          {tunnelBusy ? (
            <LoaderCircle size={14} className="spin-icon" />
          ) : tunnelStatus.running ? (
            <Square size={13} />
          ) : (
            <ShieldCheck size={14} />
          )}
          {tunnelBusy === "start"
            ? t.starting
            : tunnelBusy === "stop"
              ? t.stopping
              : tunnelStatus.running
                ? t.httpsTunnelStop
                : t.httpsTunnelStart}
        </button>
        {tunnelStatus.authtoken_configured && !tunnelStatus.running && !editingToken && (
          <button
            type="button"
            className="btn-secondary compact"
            onClick={() => setEditingToken(true)}
          >
            {t.httpsTunnelTokenChange}
          </button>
        )}
      </div>
    </section>
  );
}
