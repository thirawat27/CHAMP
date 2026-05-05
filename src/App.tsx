import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Dashboard } from "./components/Dashboard";
import { FirstRunWizard } from "./components/FirstRunWizard";
import "./App.css";

function App() {
  const [isFirstRun, setIsFirstRun] = useState<boolean | null>(null);
  const [showDebugMenu, setShowDebugMenu] = useState(false);

  useEffect(() => {
    checkRuntimeInstalled();

    // Debug mode: press Ctrl+Shift+D to toggle debug menu
    const handleKeyPress = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === "D") {
        setShowDebugMenu((prev) => !prev);
      }
    };

    window.addEventListener("keydown", handleKeyPress);

    // Listen for show-wizard event from menu
    const unlisten = listen("show-wizard", () => {
      setIsFirstRun(true);
    });

    // Cleanup all services when window closes
    const handleBeforeUnload = () => {
      invoke("cleanup_all_services").catch((error) => {
        console.error("Failed to cleanup services:", error);
      });
    };

    window.addEventListener("beforeunload", handleBeforeUnload);

    return () => {
      window.removeEventListener("keydown", handleKeyPress);
      window.removeEventListener("beforeunload", handleBeforeUnload);
      unlisten.then((fn) => fn());
    };
  }, []);

  const checkRuntimeInstalled = async () => {
    try {
      const installed = await invoke<boolean>("check_runtime_installed");
      setIsFirstRun(!installed);
    } catch (error) {
      console.error("Failed to check runtime status:", error);
      // Default to showing wizard if check fails
      setIsFirstRun(true);
    }
  };

  const handleWizardComplete = () => {
    setIsFirstRun(false);
  };

  const handleResetInstallation = async () => {
    if (confirm("Reset installation? This will stop all services and delete runtime binaries.")) {
      try {
        // Stop all services first
        await invoke("cleanup_all_services");
        await invoke("reset_installation");
        setIsFirstRun(true);
        setShowDebugMenu(false);
      } catch (error) {
        console.error("Failed to reset:", error);
        alert("Failed to reset: " + error);
      }
    }
  };

  const handleOpenRuntimeFolder = async () => {
    try {
      const runtimeDir = await invoke<string>("get_runtime_dir");
      await invoke("open_folder", { path: runtimeDir });
    } catch (error) {
      console.error("Failed to open folder:", error);
      alert("Failed to open folder: " + error);
    }
  };

  const handleOpenDownloadFolder = async () => {
    try {
      const downloadDir = await invoke<string>("get_download_dir");
      await invoke("open_folder", { path: downloadDir });
    } catch (error) {
      console.error("Failed to open download folder:", error);
      alert("Failed to open download folder: " + error);
    }
  };

  if (isFirstRun === null) {
    return (
      <div
        style={{
          minHeight: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-secondary)",
        }}
      >
        <p style={{ fontSize: "1.25rem" }}>Loading...</p>
      </div>
    );
  }

  if (isFirstRun) {
    return <FirstRunWizard onComplete={handleWizardComplete} />;
  }

  return (
    <>
      {showDebugMenu && (
        <div
          style={{
            position: "fixed",
            top: "0.625rem",
            right: "0.625rem",
            backgroundColor: "var(--bg-card)",
            border: "1px solid var(--border-color)",
            borderRadius: "0.5rem",
            boxShadow: "0 4px 12px rgba(0, 0, 0, 0.15)",
            zIndex: 9999,
            minWidth: "12.5rem",
          }}
        >
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "0.5rem 1rem",
              borderBottom: "1px solid var(--border-color)",
              fontWeight: 500,
            }}
          >
            <span>Debug Menu</span>
            <button
              onClick={() => setShowDebugMenu(false)}
              style={{
                background: "none",
                border: "none",
                fontSize: "1.25rem",
                cursor: "pointer",
                padding: 0,
                lineHeight: 1,
              }}
            >
              ×
            </button>
          </div>
          <div style={{ display: "flex", flexDirection: "column", padding: "0.5rem", gap: "0.5rem" }}>
            <button
              onClick={handleOpenRuntimeFolder}
              style={{
                padding: "0.5rem 1rem",
                borderRadius: "0.375rem",
                border: "1px solid var(--border-color)",
                backgroundColor: "var(--bg-card-secondary)",
                cursor: "pointer",
                textAlign: "left",
                fontSize: "0.875rem",
                color: "var(--text-primary)",
              }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = "var(--bg-card)"; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = "var(--bg-card-secondary)"; }}
            >
              Open Runtime Folder
            </button>
            <button
              onClick={handleOpenDownloadFolder}
              style={{
                padding: "0.5rem 1rem",
                borderRadius: "0.375rem",
                border: "1px solid var(--border-color)",
                backgroundColor: "var(--bg-card-secondary)",
                cursor: "pointer",
                textAlign: "left",
                fontSize: "0.875rem",
                color: "var(--text-primary)",
              }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = "var(--bg-card)"; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = "var(--bg-card-secondary)"; }}
            >
              View Download Folder
            </button>
            <button
              onClick={handleResetInstallation}
              style={{
                padding: "0.5rem 1rem",
                borderRadius: "0.375rem",
                border: "1px solid var(--border-color)",
                backgroundColor: "var(--bg-card-secondary)",
                cursor: "pointer",
                textAlign: "left",
                fontSize: "0.875rem",
                color: "var(--text-primary)",
              }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = "var(--bg-card)"; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = "var(--bg-card-secondary)"; }}
            >
              Reset Installation
            </button>
            <button
              onClick={() => setIsFirstRun(true)}
              style={{
                padding: "0.5rem 1rem",
                borderRadius: "0.375rem",
                border: "1px solid var(--border-color)",
                backgroundColor: "var(--bg-card-secondary)",
                cursor: "pointer",
                textAlign: "left",
                fontSize: "0.875rem",
                color: "var(--text-primary)",
              }}
              onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = "var(--bg-card)"; }}
              onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = "var(--bg-card-secondary)"; }}
            >
              Show First-Run Wizard
            </button>
          </div>
        </div>
      )}
      <Dashboard />
    </>
  );
}

export default App;
