//! Running process-manager work from async code.

use crate::process::manager::ProcessManager;
use crate::AppState;
use tauri::{AppHandle, Manager};

/// Run `f` against the process manager on Tokio's blocking pool.
///
/// Service start/stop can block for seconds (port release waits, MySQL init),
/// so it must not run on an async IPC worker thread.
pub(crate) async fn run_with_manager<T, F>(app: &AppHandle, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&mut ProcessManager) -> T + Send + 'static,
{
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut manager = state
            .process_manager
            .lock()
            .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;
        Ok(f(&mut manager))
    })
    .await
    .map_err(|e| format!("Process manager task failed: {}", e))?
}
