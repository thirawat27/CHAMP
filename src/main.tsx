import React from "react";
import ReactDOM from "react-dom/client";
// Only the subsets and weights the UI uses, to keep the bundle small.
import "@fontsource/ibm-plex-sans-thai/thai-400.css";
import "@fontsource/ibm-plex-sans-thai/latin-400.css";
import "@fontsource/ibm-plex-sans-thai/thai-600.css";
import "@fontsource/ibm-plex-sans-thai/latin-600.css";
import "@fontsource/bai-jamjuree/thai-600.css";
import "@fontsource/bai-jamjuree/latin-600.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";

async function start() {
  // In a plain browser during development, fake the Tauri backend so the UI
  // can be laid out without the desktop shell. Stripped from production.
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    const { installMockTauri } = await import("./dev/mockTauri");
    installMockTauri();
  }

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <ErrorBoundary>
        <App />
      </ErrorBoundary>
    </React.StrictMode>
  );
}

start();
