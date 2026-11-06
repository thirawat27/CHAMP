import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Dashboard } from "./components/Dashboard";
import { DebugMenu } from "./components/DebugMenu";
import { FirstRunWizard } from "./components/FirstRunWizard";
import { initializeLanguage, useTranslation } from "./stores/preferencesStore";
import { listenIfTauri } from "./utils/tauri";
import "./App.css";

function App() {
  const { t } = useTranslation();
  const [isFirstRun, setIsFirstRun] = useState<boolean | null>(null);
  const [showDebugMenu, setShowDebugMenu] = useState(false);

  useEffect(() => {
    initializeLanguage();
    invoke<boolean>("check_runtime_installed")
      .then((installed) => setIsFirstRun(!installed))
      .catch((error) => {
        console.error("Failed to check runtime status:", error);
        // Default to showing wizard if check fails
        setIsFirstRun(true);
      });

    const blockDevtoolsShortcuts = (e: KeyboardEvent) => {
      const key = e.key.toLowerCase();
      const opensDevtools =
        e.key === "F12" ||
        ((e.ctrlKey || e.metaKey) && e.shiftKey && ["i", "j", "c"].includes(key));

      if (opensDevtools) {
        e.preventDefault();
        e.stopPropagation();
        return;
      }

      // A webview reload would drop in-flight commands (and the wizard's place)
      // while the backend keeps working. Ctrl+R itself is the stack shortcut.
      const reloads = e.key === "F5" || ((e.ctrlKey || e.metaKey) && e.code === "KeyR");
      if (reloads && import.meta.env.PROD) {
        e.preventDefault();
      }
    };

    // Debug mode: press Ctrl+Shift+D to toggle debug menu
    const handleKeyPress = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.code === "KeyD") {
        setShowDebugMenu((prev) => !prev);
      }
    };

    window.addEventListener("keydown", blockDevtoolsShortcuts, true);
    window.addEventListener("keydown", handleKeyPress);

    const unlistenWizard = listenIfTauri("show-wizard", () => {
      setIsFirstRun(true);
    });

    // NOTE: Closing the window must NOT stop services. The Rust side
    // (lib.rs on_window_event CloseRequested) hides the window to the tray and
    // calls prevent_close(), keeping the stack running. Service cleanup happens
    // only on real exit via the tray "Quit CHAMP" item (lib.rs tray-quit).
    // A frontend onCloseRequested -> cleanup_all_services handler here used to
    // contradict that and silently stopped Caddy/PHP/MySQL/PostgreSQL. (R-02)

    return () => {
      window.removeEventListener("keydown", handleKeyPress);
      window.removeEventListener("keydown", blockDevtoolsShortcuts, true);
      unlistenWizard.then((fn) => fn());
    };
  }, []);

  if (isFirstRun === null) {
    return (
      <div className="app-loading">
        <p>{t.loading}</p>
      </div>
    );
  }

  if (isFirstRun) {
    return <FirstRunWizard onComplete={() => setIsFirstRun(false)} />;
  }

  return (
    <>
      {showDebugMenu && (
        <DebugMenu
          onClose={() => setShowDebugMenu(false)}
          onShowWizard={() => setIsFirstRun(true)}
        />
      )}
      <Dashboard />
    </>
  );
}

export default App;
