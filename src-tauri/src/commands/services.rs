//! Starting and stopping services, HTTPS Preview, and live status.

use crate::process::manager::ProcessManager;
use crate::process::runner::run_with_manager;
use crate::process::{ServiceMap, ServiceState, ServiceType};
use tauri::AppHandle;

/// Run a service command, then refresh health and return all statuses.
///
/// The error is returned only after statuses were refreshed, so the frontend's
/// next poll sees the error state.
pub(super) async fn run_service_command(
    app: &AppHandle,
    command: impl FnOnce(&mut ProcessManager) -> Result<(), String> + Send + 'static,
) -> Result<ServiceMap, String> {
    let (result, statuses) = run_with_manager(app, |m| {
        let result = command(m);
        m.update_health();
        (result, m.get_all_statuses())
    })
    .await?;

    result?;
    Ok(statuses)
}

/// Start a service
#[tauri::command]
pub async fn start_service(service: ServiceType, app: AppHandle) -> Result<ServiceMap, String> {
    run_service_command(&app, move |m| m.start(service)).await
}

/// Stop a service
#[tauri::command]
pub async fn stop_service(service: ServiceType, app: AppHandle) -> Result<ServiceMap, String> {
    run_service_command(&app, move |m| m.stop(service)).await
}

/// Restart a service
#[tauri::command]
pub async fn restart_service(service: ServiceType, app: AppHandle) -> Result<ServiceMap, String> {
    run_service_command(&app, move |m| m.restart(service)).await
}

#[tauri::command]
pub async fn start_all_services(app: AppHandle) -> Result<ServiceMap, String> {
    run_service_command(&app, |m| m.start_all()).await
}

#[tauri::command]
pub async fn stop_all_services(app: AppHandle) -> Result<ServiceMap, String> {
    run_service_command(&app, |m| {
        let _ = crate::tunnel::stop_https_tunnel();
        m.stop_stack()
    })
    .await
}

#[tauri::command]
pub async fn restart_all_services(app: AppHandle) -> Result<ServiceMap, String> {
    run_service_command(&app, |m| m.restart_all()).await
}

/// Get the status of all services
#[tauri::command]
pub async fn get_all_statuses(app: AppHandle) -> Result<ServiceMap, String> {
    run_with_manager(&app, |m| {
        m.update_health();
        m.get_all_statuses()
    })
    .await
}

/// Start a free public HTTPS URL for the local CHAMP web server.
///
/// This uses an ngrok agent (`*.ngrok-free.app` on the free plan) and starts
/// the selected CHAMP stack first when the web server is not already running.
#[tauri::command]
pub async fn start_https_tunnel(
    app: AppHandle,
) -> Result<crate::tunnel::HttpsTunnelStatus, String> {
    let web_port = run_with_manager(&app, |manager| -> Result<u16, String> {
        manager.update_health();
        let caddy_running = manager.status(ServiceType::Caddy) == ServiceState::Running;
        if !caddy_running {
            manager.start_all()?;
            manager.update_health();
        }

        Ok(manager
            .get_all_statuses()
            .get(&ServiceType::Caddy)
            .map(|service| service.port)
            .unwrap_or(ServiceType::Caddy.default_port()))
    })
    .await??;

    crate::tunnel::start_https_tunnel(web_port).await
}

/// Stop the current public HTTPS tunnel, if one is running.
#[tauri::command]
pub async fn stop_https_tunnel() -> Result<crate::tunnel::HttpsTunnelStatus, String> {
    crate::tunnel::stop_https_tunnel()
}

/// Get the current public HTTPS tunnel status.
#[tauri::command]
pub async fn get_https_tunnel_status() -> Result<crate::tunnel::HttpsTunnelStatus, String> {
    crate::tunnel::get_https_tunnel_status()
}

/// Save (or clear, when empty) the ngrok authtoken used by HTTPS Preview.
#[tauri::command]
pub async fn set_ngrok_authtoken(
    token: String,
) -> Result<crate::tunnel::HttpsTunnelStatus, String> {
    crate::tunnel::set_ngrok_authtoken(&token)
}

/// Stop the public tunnel and every service.
pub(crate) async fn stop_tunnel_and_services(
    app: &AppHandle,
) -> Result<Result<(), String>, String> {
    run_with_manager(app, |manager| {
        let _ = crate::tunnel::stop_https_tunnel();
        manager.stop_all()
    })
    .await
}

/// Stop the tunnel and every service so the installer can replace the app
/// without leaving orphaned Caddy/PHP/database processes behind.
#[tauri::command]
pub async fn prepare_for_update(app: AppHandle) -> Result<(), String> {
    crate::agent::stop();
    stop_tunnel_and_services(&app).await?
}

#[tauri::command]
pub async fn get_system_metrics() -> Result<crate::metrics::SystemMetricsDto, String> {
    crate::metrics::current_metrics()
}
