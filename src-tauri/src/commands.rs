//! Tauri IPC Commands
//!
//! This module contains all Tauri commands that are exposed to the frontend.
//! Commands are invoked from the React frontend using the `invoke()` function.
//!
//! # Command Categories
//!
//! - **Folder Operations**: `open_folder`, `open_project_terminal`
//! - **Service Control**: `start_service`, `stop_service`, `restart_service`, `start_all_services`, `stop_all_services`, `restart_all_services`
//! - **Service Status**: `get_all_statuses`
//! - **HTTPS Tunnel**: `start_https_tunnel`, `stop_https_tunnel`, `get_https_tunnel_status`
//! - **Settings**: `get_settings`, `save_settings`, `validate_settings`
//! - **Port Checking**: `check_ports`
//! - **Runtime Management**: `check_runtime_installed`, `download_runtime_with_packages`, `download_runtime_with_skip`
//! - **Installation**: `reset_installation`, `get_runtime_dir`, `get_app_paths`
//! - **System Metrics**: `get_system_metrics`
//! - **Package Management**: `get_available_packages_cmd`, `get_selected_package_ids`, `update_package_selection`
//! - **PHP Version Management**: `get_installed_php_versions`, `switch_php_version`, `download_php_version`
//! - **Version Info**: `get_installed_versions`, `check_existing_components`
//! - **Dependencies**: `check_system_dependencies`
//!
//! Service commands run the blocking process-manager work on Tokio's blocking
//! pool so slow starts (MySQL init, port waits) never stall async IPC workers.

use crate::config::AppSettings;
use crate::constants::SYSTEM_METRICS_MIN_SAMPLE_INTERVAL_MS;
use crate::process::manager::ProcessManager;
use crate::process::{ServiceMap, ServiceState, ServiceType};
use crate::runtime::deps::DependencyCheckResult;
use crate::runtime::downloader::{read_marker_version, Platform, RuntimeDownloader};
use crate::runtime::locator::get_app_data_paths;
use crate::runtime::packages::{PackageSelection, PackagesConfig};
use crate::AppState;
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, Networks, RefreshKind, System};
use tauri::{AppHandle, Emitter, Manager};

/// Installing runtimes without a configured SHA-256 is currently allowed because
/// upstream projects do not publish stable per-platform checksums for every
/// package in `runtime-config.json`. A configured checksum that MISMATCHES still
/// hard-fails regardless of this flag (see `finalize_downloaded_file`).
const ALLOW_UNVERIFIED_RUNTIME_DOWNLOADS: bool = true;

/// True when `path` is absolute, has no `..` segments, and lies inside one of `roots`.
fn is_within_roots(path: &Path, roots: &[PathBuf]) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        && roots.iter().any(|root| path.starts_with(root))
}

/// The UI may only open (and create) folders inside the CHAMP data folder or
/// the download cache, so a compromised webview cannot create or reveal
/// arbitrary paths through these commands.
fn ensure_allowed_folder(path: &str) -> Result<PathBuf, String> {
    let base_dir = get_app_data_paths()?.base_dir;
    let roots = [
        base_dir.clone(),
        std::env::temp_dir().join("champ-download"),
    ];
    let path = PathBuf::from(path);
    if is_within_roots(&path, &roots) {
        Ok(path)
    } else {
        Err(format!(
            "CHAMP can only open folders inside {}",
            base_dir.display()
        ))
    }
}

/// Open a folder in the system's file explorer using tauri-plugin-opener.
///
/// The folder is created first when it does not exist yet.
#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    use tauri_plugin_opener::reveal_item_in_dir;

    let path = ensure_allowed_folder(&path)?;
    if !path.exists() {
        fs::create_dir_all(&path).map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    reveal_item_in_dir(&path).map_err(|e| format!("Failed to open folder: {}", e))?;

    Ok(())
}

/// Collect the bin directories of the installed CHAMP runtimes (PHP CLI,
/// MySQL/PostgreSQL clients) so a terminal can use them without setup.
fn runtime_bin_dirs() -> Vec<PathBuf> {
    let Ok(runtimes) = crate::runtime::locator::locate_runtime_binaries() else {
        return Vec::new();
    };

    let mut dirs: Vec<PathBuf> = Vec::new();
    for binary in [&runtimes.php_cgi, &runtimes.mysql, &runtimes.postgresql] {
        if let Some(parent) = binary.parent() {
            if parent.exists() && !dirs.iter().any(|dir| dir == parent) {
                dirs.push(parent.to_path_buf());
            }
        }
    }
    dirs
}

/// Open a terminal in a specific directory with the installed runtimes on PATH.
#[tauri::command]
pub async fn open_project_terminal(path: Option<String>) -> Result<(), String> {
    let target_dir = match path {
        Some(p) => ensure_allowed_folder(&p)?,
        None => get_app_data_paths()?.projects_dir,
    };

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create terminal directory: {}", e))?;
    }

    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = runtime_bin_dirs();
    paths.extend(std::env::split_paths(&current_path));
    let new_path = std::env::join_paths(paths).unwrap_or(current_path);

    spawn_terminal(&target_dir, &new_path)
}

#[cfg(target_os = "windows")]
fn spawn_terminal(target_dir: &Path, path_env: &std::ffi::OsStr) -> Result<(), String> {
    std::process::Command::new("cmd.exe")
        .args(["/c", "start", "cmd.exe"])
        .env("PATH", path_env)
        .current_dir(target_dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open terminal: {}", e))
}

#[cfg(target_os = "macos")]
fn spawn_terminal(target_dir: &Path, path_env: &std::ffi::OsStr) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    // Terminal.app does not inherit our environment, so hand it a tiny script
    // that sets PATH and the working directory before starting the user's shell.
    let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
    let script = format!(
        "#!/bin/sh\ncd {}\nexport PATH={}\nexec \"${{SHELL:-/bin/zsh}}\" -i\n",
        quote(&target_dir.to_string_lossy()),
        quote(&path_env.to_string_lossy()),
    );
    let script_path = std::env::temp_dir().join("champ-terminal.command");
    fs::write(&script_path, script)
        .map_err(|e| format!("Failed to prepare terminal script: {}", e))?;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("Failed to prepare terminal script: {}", e))?;

    std::process::Command::new("open")
        .args(["-a", "Terminal"])
        .arg(&script_path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open terminal: {}", e))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_terminal(target_dir: &Path, path_env: &std::ffi::OsStr) -> Result<(), String> {
    // Most emulators start in the spawning process' working directory, which
    // avoids relying on emulator-specific flags such as --working-directory.
    const TERMINALS: [&str; 6] = [
        "x-terminal-emulator",
        "gnome-terminal",
        "konsole",
        "xfce4-terminal",
        "kitty",
        "xterm",
    ];

    let mut last_error = String::from("no terminal emulator found");
    for terminal in TERMINALS {
        match std::process::Command::new(terminal)
            .env("PATH", path_env)
            .current_dir(target_dir)
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(e) => last_error = format!("{}: {}", terminal, e),
        }
    }
    Err(format!("Failed to open terminal ({})", last_error))
}

/// Data transfer object for application paths
#[derive(Debug, Serialize)]
pub struct AppPathsDto {
    pub base_dir: String,
    pub portable: bool,
    pub runtime_dir: String,
    pub config_dir: String,
    pub mysql_data_dir: String,
    pub postgresql_data_dir: String,
    pub logs_dir: String,
    pub projects_dir: String,
}

/// Data transfer object for installed PHP version information
#[derive(Debug, Serialize)]
pub struct InstalledPhpVersionDto {
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub installed: bool,
    pub active: bool,
    pub eol: bool,
    pub lts: bool,
    pub recommended: bool,
    pub path: Option<String>,
    /// Newest patch release of this branch in the catalog.
    pub latest_version: String,
    /// Installed, but an older patch than `latest_version`.
    pub update_available: bool,
}

/// Data transfer object for system metrics (CPU, memory, network)
#[derive(Clone, Debug, Serialize)]
pub struct SystemMetricsDto {
    pub cpu_usage: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub network_rx_bps: u64,
    pub network_tx_bps: u64,
}

/// Data transfer object for language and sound preferences.
#[derive(Debug, Serialize)]
pub struct LanguageSettingsDto {
    pub language: String,
    pub sound_enabled: bool,
}

/// Minimum interval between system metrics samples to avoid excessive CPU usage
const SYSTEM_METRICS_MIN_SAMPLE_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(SYSTEM_METRICS_MIN_SAMPLE_INTERVAL_MS);

/// System metrics monitor with caching to reduce overhead
struct SystemMetricsMonitor {
    system: System,
    networks: Networks,
    last_network_received_bytes: u64,
    last_network_transmitted_bytes: u64,
    last_sample_time: Instant,
    cached_metrics: Option<SystemMetricsDto>,
}

impl SystemMetricsMonitor {
    fn new() -> Self {
        // Only CPU usage and RAM are shown, so skip processes, disks, etc.
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
                .with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        system.refresh_cpu_usage();
        system.refresh_memory();

        let mut networks = Networks::new_with_refreshed_list();
        networks.refresh(true);
        let (received, transmitted) = aggregate_network_totals(&networks);

        Self {
            system,
            networks,
            last_network_received_bytes: received,
            last_network_transmitted_bytes: transmitted,
            last_sample_time: Instant::now(),
            cached_metrics: None,
        }
    }

    fn collect(&mut self) -> SystemMetricsDto {
        if self.last_sample_time.elapsed() < SYSTEM_METRICS_MIN_SAMPLE_INTERVAL {
            if let Some(metrics) = &self.cached_metrics {
                return metrics.clone();
            }
        }

        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.networks.refresh(true);

        let now = Instant::now();
        let elapsed_seconds = now.duration_since(self.last_sample_time).as_secs_f64();
        let (current_received, current_transmitted) = aggregate_network_totals(&self.networks);

        let received_delta = current_received.saturating_sub(self.last_network_received_bytes);
        let transmitted_delta =
            current_transmitted.saturating_sub(self.last_network_transmitted_bytes);

        let (network_rx_bps, network_tx_bps) = if elapsed_seconds > 0.0 {
            (
                (received_delta as f64 / elapsed_seconds) as u64,
                (transmitted_delta as f64 / elapsed_seconds) as u64,
            )
        } else {
            (0, 0)
        };

        self.last_network_received_bytes = current_received;
        self.last_network_transmitted_bytes = current_transmitted;
        self.last_sample_time = now;

        let metrics = SystemMetricsDto {
            cpu_usage: self.system.global_cpu_usage(),
            memory_used_bytes: self.system.used_memory(),
            memory_total_bytes: self.system.total_memory(),
            network_rx_bps,
            network_tx_bps,
        };
        self.cached_metrics = Some(metrics.clone());
        metrics
    }
}

fn aggregate_network_totals(networks: &Networks) -> (u64, u64) {
    networks
        .iter()
        .fold((0, 0), |(received, transmitted), (_, data)| {
            (
                received.saturating_add(data.total_received()),
                transmitted.saturating_add(data.total_transmitted()),
            )
        })
}

static SYSTEM_METRICS_MONITOR: OnceLock<Mutex<SystemMetricsMonitor>> = OnceLock::new();

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

/// Run a service command, then refresh health and return all statuses.
///
/// When `statuses_on_error` is true the error is returned only after statuses
/// were refreshed, so the frontend's next poll sees the error state.
async fn run_service_command(
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

/// Get app settings
#[tauri::command]
pub async fn get_settings() -> Result<AppSettings, String> {
    Ok(AppSettings::load())
}

/// Save app settings
#[tauri::command]
pub async fn save_settings(mut settings: AppSettings, app: AppHandle) -> Result<(), String> {
    // Language, sound, and the agent connector have dedicated commands that
    // write immediately. The Settings panel holds a snapshot loaded when it
    // opened, so keep the values on disk instead of reverting a change made
    // while the panel was open.
    let stored = AppSettings::load();
    settings.language = stored.language;
    settings.sound_enabled = stored.sound_enabled;
    settings.agent_connector_enabled = stored.agent_connector_enabled;
    settings.agent_connector_read_only = stored.agent_connector_read_only;
    settings.agent_connector_port = stored.agent_connector_port;
    settings.package_selection = settings.package_selection.normalized();

    ensure_selected_database_tool_installed(&settings, &app).await?;

    // Save the settings after any required runtime install succeeds.
    settings.save()?;

    run_with_manager(&app, move |manager| -> Result<(), String> {
        let running_services: Vec<ServiceType> = manager
            .get_all_statuses()
            .iter()
            .filter(|(_, s)| s.state == ServiceState::Running)
            .map(|(ty, _)| *ty)
            .collect();
        let web_stack_was_running = running_services.contains(&ServiceType::Caddy)
            || running_services.contains(&ServiceType::PhpFpm);

        manager.update_ports(&settings);

        if web_stack_was_running {
            manager.stop_stack()?;
            manager.start_all()?;
        } else {
            // Restart any standalone running database service with new port settings.
            for service in running_services {
                let _ = manager.stop(service);
                let _ = manager.start(service);
            }
        }
        Ok(())
    })
    .await?
}

/// Get language and sound preferences.
#[tauri::command]
pub async fn get_language_settings() -> Result<LanguageSettingsDto, String> {
    let settings = AppSettings::load();
    Ok(LanguageSettingsDto {
        language: settings.language,
        sound_enabled: settings.sound_enabled,
    })
}

/// Save the selected UI language.
#[tauri::command]
pub async fn save_language_setting(language: String) -> Result<(), String> {
    if language != "en" && language != "th" {
        return Err(format!("Unsupported language: {}", language));
    }

    let mut settings = AppSettings::load();
    settings.language = language;
    settings.save()
}

/// Save whether UI sound effects are enabled.
#[tauri::command]
pub async fn save_sound_setting(enabled: bool) -> Result<(), String> {
    let mut settings = AppSettings::load();
    settings.sound_enabled = enabled;
    settings.save()
}

/// Emit runtime download progress to the frontend.
fn progress_emitter(app: &AppHandle) -> crate::runtime::downloader::ProgressCallback {
    let app = app.clone();
    Box::new(move |progress| {
        let _ = app.emit("download-progress", &progress);
    })
}

async fn ensure_selected_database_tool_installed(
    settings: &AppSettings,
    app: &AppHandle,
) -> Result<(), String> {
    let tool_id = &settings.package_selection.phpmyadmin;
    let packages = crate::runtime::packages::get_available_packages();
    let package = packages
        .phpmyadmin
        .iter()
        .find(|package| package.id == *tool_id)
        .ok_or_else(|| format!("Unknown database tool: {}", tool_id))?;

    let selection = &settings.package_selection;
    let postgresql = selection.is_postgresql();
    let downloader = RuntimeDownloader::with_packages(selection.clone())
        .allow_unverified_checksums(ALLOW_UNVERIFIED_RUNTIME_DOWNLOADS);
    let runtime_dir = downloader.get_runtime_dir()?;
    if is_database_tool_installed(&runtime_dir, tool_id)
        && is_selected_database_installed(&runtime_dir, postgresql)
    {
        return Ok(());
    }

    eprintln!(
        "{} is selected but not fully installed. Downloading required components now.",
        package.display_name
    );

    // Only what the choice needs: the web stack is already there, and the
    // engine and manager that were not chosen stay out.
    let skip_list = [
        "caddy",
        "php",
        if postgresql { "mysql" } else { "postgresql" },
        if selection.tool_is_adminer() {
            "phpmyadmin"
        } else {
            "adminer"
        },
    ];
    downloader
        .download_all_with_skip(progress_emitter(app), &skip_list)
        .await?;

    if is_database_tool_installed(&runtime_dir, tool_id)
        && is_selected_database_installed(&runtime_dir, postgresql)
    {
        Ok(())
    } else {
        Err(format!(
            "{} was downloaded, but CHAMP could not find all required files in {}",
            package.display_name,
            runtime_dir.display()
        ))
    }
}

fn is_database_tool_installed(runtime_dir: &Path, tool_id: &str) -> bool {
    if tool_id.starts_with("adminer") {
        runtime_dir.join("adminer").join("index.php").exists()
            || runtime_dir.join("adminer.php").exists()
    } else {
        runtime_dir.join("phpmyadmin").join("index.php").exists()
    }
}

fn is_selected_database_installed(runtime_dir: &Path, postgresql: bool) -> bool {
    let (marker, folder, binary) = if postgresql {
        ("postgresql_installed.txt", "postgresql", "postgres")
    } else {
        ("mysql_installed.txt", "mysql", "mysqld")
    };

    runtime_dir.join(marker).exists()
        || [binary.to_string(), format!("{binary}.exe")]
            .iter()
            .any(|name| {
                runtime_dir.join(folder).join("bin").join(name).exists()
                    || runtime_dir.join("bin").join(name).exists()
            })
}

/// Validate settings (check port conflicts, valid paths)
#[tauri::command]
pub async fn validate_settings(settings: AppSettings) -> Result<Vec<String>, Vec<String>> {
    settings.validate()
}

/// Check if specific ports are available
#[tauri::command]
pub async fn check_ports(
    web_port: u16,
    php_port: u16,
    mysql_port: u16,
    postgresql_port: u16,
) -> serde_json::Value {
    use crate::config::is_port_available;

    serde_json::json!({
        "web": { "port": web_port, "available": is_port_available(web_port) },
        "php": { "port": php_port, "available": is_port_available(php_port) },
        "mysql": { "port": mysql_port, "available": is_port_available(mysql_port) },
        "postgresql": { "port": postgresql_port, "available": is_port_available(postgresql_port) }
    })
}

/// Check if runtime binaries are already installed
#[tauri::command]
pub async fn check_runtime_installed() -> Result<bool, String> {
    Ok(RuntimeDownloader::new().is_installed())
}

/// Reset installation (stops every service, then deletes the runtime directory)
#[tauri::command]
pub async fn reset_installation(app: AppHandle) -> Result<String, String> {
    run_with_manager(&app, |manager| {
        let _ = crate::tunnel::stop_https_tunnel();
        manager.stop_all()
    })
    .await??;

    reset_runtime_dir().await
}

/// Stop the tunnel and every service so the installer can replace the app
/// without leaving orphaned Caddy/PHP/database processes behind.
#[tauri::command]
pub async fn prepare_for_update(app: AppHandle) -> Result<(), String> {
    crate::agent::stop();
    run_with_manager(&app, |manager| {
        let _ = crate::tunnel::stop_https_tunnel();
        manager.stop_all()
    })
    .await?
}

pub async fn reset_runtime_dir() -> Result<String, String> {
    let runtime_dir = RuntimeDownloader::new().get_runtime_dir()?;

    if runtime_dir.exists() {
        fs::remove_dir_all(&runtime_dir)
            .map_err(|e| format!("Failed to remove runtime directory: {}", e))?;
    }

    Ok("Installation reset. Run the app again to see first-run wizard.".to_string())
}

/// Get the runtime directory path
#[tauri::command]
pub async fn get_runtime_dir() -> Result<String, String> {
    RuntimeDownloader::new()
        .get_runtime_dir()
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn get_app_paths() -> Result<AppPathsDto, String> {
    let paths = get_app_data_paths()?;
    Ok(AppPathsDto {
        base_dir: paths.base_dir.to_string_lossy().to_string(),
        portable: paths.portable,
        runtime_dir: paths.runtime_dir.to_string_lossy().to_string(),
        config_dir: paths.config_dir.to_string_lossy().to_string(),
        mysql_data_dir: paths.mysql_data_dir.to_string_lossy().to_string(),
        postgresql_data_dir: paths.postgresql_data_dir.to_string_lossy().to_string(),
        logs_dir: paths.logs_dir.to_string_lossy().to_string(),
        projects_dir: paths.projects_dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn get_system_metrics() -> Result<SystemMetricsDto, String> {
    let monitor = SYSTEM_METRICS_MONITOR.get_or_init(|| Mutex::new(SystemMetricsMonitor::new()));
    let mut monitor = monitor
        .lock()
        .map_err(|e| format!("Failed to acquire system metrics lock: {}", e))?;

    Ok(monitor.collect())
}

/// Get the download directory path (where downloaded archives are cached)
#[tauri::command]
pub async fn get_download_dir() -> Result<String, String> {
    let temp_dir = std::env::temp_dir().join("champ-download");
    Ok(temp_dir.to_string_lossy().to_string())
}

/// Get all available runtime packages
#[tauri::command]
pub async fn get_available_packages_cmd() -> Result<PackagesConfig, String> {
    Ok(crate::runtime::packages::get_available_packages())
}

/// Refresh runtime packages from upstream release metadata, then return the active catalog.
#[tauri::command]
pub async fn refresh_runtime_catalog() -> Result<PackagesConfig, String> {
    crate::runtime::packages::refresh_runtime_catalog().await
}

/// Refresh only the PHP releases (fast); the dashboard uses this to offer
/// patch updates for the active PHP.
#[tauri::command]
pub async fn refresh_php_catalog() -> Result<(), String> {
    crate::runtime::packages::refresh_php_catalog().await
}

#[tauri::command]
pub async fn get_runtime_platform() -> String {
    Platform::current().url_key()
}

/// Download and install runtime binaries with custom package selection
#[tauri::command]
pub async fn download_runtime_with_packages(
    package_selection: PackageSelection,
    app: AppHandle,
) -> Result<String, String> {
    RuntimeDownloader::with_packages(package_selection)
        .allow_unverified_checksums(ALLOW_UNVERIFIED_RUNTIME_DOWNLOADS)
        .download_all(progress_emitter(&app))
        .await?;

    Ok("Runtime binaries installed successfully".to_string())
}

/// Update package selection in settings (without downloading)
#[tauri::command]
pub async fn update_package_selection(package_selection: PackageSelection) -> Result<(), String> {
    let mut settings = AppSettings::load();
    settings.package_selection = package_selection.normalized();
    settings.save()
}

#[tauri::command]
pub async fn get_installed_php_versions() -> Result<Vec<InstalledPhpVersionDto>, String> {
    let runtime_dir = RuntimeDownloader::new().get_runtime_dir()?;
    let active_php = AppSettings::load().package_selection.php;
    let packages = crate::runtime::packages::get_available_packages();
    let legacy_version = read_marker_version(&runtime_dir.join("php_installed.txt"));

    Ok(packages
        .php
        .into_iter()
        .map(|package| {
            let package_id = package.id.clone();
            let version_dir = runtime_dir.join("php_versions").join(&package_id);
            let marker = runtime_dir
                .join("php_versions")
                .join(format!("{}_installed.txt", package_id));
            let marker_version = read_marker_version(&marker);
            let installed = marker_version.is_some()
                || legacy_version.as_deref() == Some(package.version.as_str());

            let update_available = marker_version.as_deref().is_some_and(|installed| {
                crate::runtime::php_catalog::is_newer_version(&package.version, installed)
            });

            InstalledPhpVersionDto {
                id: package_id.clone(),
                latest_version: package.version.clone(),
                update_available,
                version: marker_version.unwrap_or(package.version),
                display_name: package.display_name,
                installed,
                active: active_php == package_id,
                eol: package.eol,
                lts: package.lts,
                recommended: package.recommended,
                path: version_dir
                    .exists()
                    .then(|| version_dir.to_string_lossy().to_string()),
            }
        })
        .collect())
}

#[tauri::command]
pub async fn switch_php_version(php_id: String, app: AppHandle) -> Result<ServiceMap, String> {
    let installed_versions = get_installed_php_versions().await?;
    let selected = installed_versions
        .iter()
        .find(|version| version.id == php_id)
        .ok_or_else(|| format!("Unknown PHP version: {}", php_id))?;

    if !selected.installed {
        return Err(format!(
            "{} is not installed yet. Install it before switching.",
            selected.display_name
        ));
    }

    let mut settings = AppSettings::load();
    settings.package_selection.php = php_id;
    settings.save()?;

    run_with_manager(&app, move |manager| -> Result<ServiceMap, String> {
        let php_was_running = manager.status(ServiceType::PhpFpm) == ServiceState::Running;
        let caddy_was_running = manager.status(ServiceType::Caddy) == ServiceState::Running;

        if caddy_was_running {
            manager.stop(ServiceType::Caddy)?;
        }
        if php_was_running {
            manager.stop(ServiceType::PhpFpm)?;
        }

        manager.update_ports(&settings);
        if php_was_running || caddy_was_running {
            manager.initialize()?;
            if php_was_running {
                manager.start(ServiceType::PhpFpm)?;
            }
            if caddy_was_running {
                manager.start(ServiceType::Caddy)?;
            }
        }

        manager.update_health();
        Ok(manager.get_all_statuses())
    })
    .await?
}

#[tauri::command]
/// Install (or update to) the newest patch release of a PHP branch.
///
/// Installing does not change the active PHP; use `switch_php_version`. When
/// the branch being updated is the active one, PHP-FPM and Caddy are stopped
/// while its files are replaced and started again afterwards.
pub async fn download_php_version(php_id: String, app: AppHandle) -> Result<String, String> {
    // Best effort: offline installs fall back to the cached catalog.
    if let Err(error) = crate::runtime::packages::refresh_php_catalog().await {
        eprintln!(
            "PHP catalog refresh failed, using cached catalog: {}",
            error
        );
    }

    let packages = crate::runtime::packages::get_available_packages();
    if !packages.php.iter().any(|package| package.id == php_id) {
        return Err(format!("Unknown PHP version: {}", php_id));
    }

    let mut selection = AppSettings::load().package_selection;
    let is_active = selection.php == php_id;
    selection.php = php_id;

    let (php_was_running, caddy_was_running) = if is_active {
        run_with_manager(&app, |manager| {
            let php = manager.status(ServiceType::PhpFpm) == ServiceState::Running;
            let caddy = manager.status(ServiceType::Caddy) == ServiceState::Running;
            if caddy {
                let _ = manager.stop(ServiceType::Caddy);
            }
            if php {
                let _ = manager.stop(ServiceType::PhpFpm);
            }
            (php, caddy)
        })
        .await?
    } else {
        (false, false)
    };

    let result = RuntimeDownloader::with_packages(selection)
        .allow_unverified_checksums(ALLOW_UNVERIFIED_RUNTIME_DOWNLOADS)
        .download_all_with_skip(
            progress_emitter(&app),
            &["caddy", "mysql", "postgresql", "adminer", "phpmyadmin"],
        )
        .await;

    if php_was_running || caddy_was_running {
        run_with_manager(&app, move |manager| {
            let _ = manager.initialize();
            if php_was_running {
                let _ = manager.start(ServiceType::PhpFpm);
            }
            if caddy_was_running {
                let _ = manager.start(ServiceType::Caddy);
            }
        })
        .await?;
    }

    result?;
    Ok("PHP version installed successfully".to_string())
}

/// Get the selected package IDs from runtime-config.json
#[tauri::command]
pub async fn get_selected_package_ids() -> Result<PackageSelection, String> {
    Ok(crate::runtime::packages::get_selected_package_ids())
}

/// Get the installed runtime versions
#[tauri::command]
pub async fn get_installed_versions() -> Result<std::collections::HashMap<String, String>, String> {
    let runtime_dir = RuntimeDownloader::new().get_runtime_dir()?;

    let mut versions = std::collections::HashMap::new();
    for component in [
        "caddy",
        "php",
        "mysql",
        "postgresql",
        "adminer",
        "phpmyadmin",
        "ngrok",
    ] {
        let marker_file = runtime_dir.join(format!("{}_installed.txt", component));
        if let Some(version) = read_marker_version(&marker_file) {
            versions.insert(component.to_string(), version);
        }
    }

    // Caddy is not package-selectable; fall back to the catalog version.
    versions
        .entry("caddy".to_string())
        .or_insert_with(crate::runtime::packages::selected_caddy_version);

    Ok(versions)
}

/// Check for existing components before download
#[tauri::command]
pub async fn check_existing_components() -> Result<std::collections::HashMap<String, String>, String>
{
    Ok(RuntimeDownloader::new().get_installed_components())
}

/// Download and install runtime binaries with option to skip existing components
#[tauri::command]
pub async fn download_runtime_with_skip(
    package_selection: PackageSelection,
    skip_list: Vec<String>,
    app: AppHandle,
) -> Result<String, String> {
    let skip_refs: Vec<&str> = skip_list.iter().map(String::as_str).collect();

    RuntimeDownloader::with_packages(package_selection)
        .allow_unverified_checksums(ALLOW_UNVERIFIED_RUNTIME_DOWNLOADS)
        .download_all_with_skip(progress_emitter(&app), &skip_refs)
        .await?;

    Ok("Runtime binaries installed successfully".to_string())
}

/// Choose which engine (`mysql` or `postgresql`) fills the stack's database role.
///
/// MySQL and PostgreSQL are interchangeable: the stack runs exactly one of
/// them. The web manager is a separate choice; phpMyAdmin only speaks MySQL,
/// so moving to PostgreSQL also moves a phpMyAdmin stack onto Adminer.
#[tauri::command]
pub async fn switch_database_engine(engine: String, app: AppHandle) -> Result<(), String> {
    if engine != "mysql" && engine != "postgresql" {
        return Err(format!("Unsupported database engine: {}", engine));
    }

    let mut settings = AppSettings::load();
    if settings.package_selection.database_engine == engine {
        return Ok(());
    }
    settings.package_selection.database_engine = engine;
    settings.package_selection = settings.package_selection.normalized();
    apply_database_selection(settings, &app).await
}

/// Choose the web database manager: `phpmyadmin` (MySQL only) or `adminer`
/// (MySQL and PostgreSQL), independently of the engine.
#[tauri::command]
pub async fn switch_database_tool(tool: String, app: AppHandle) -> Result<(), String> {
    if tool != "phpmyadmin" && tool != "adminer" {
        return Err(format!("Unsupported database tool: {}", tool));
    }

    let mut settings = AppSettings::load();
    if tool == "phpmyadmin" && settings.package_selection.is_postgresql() {
        return Err("phpMyAdmin only supports MySQL. Use Adminer with PostgreSQL.".to_string());
    }
    if settings.package_selection.phpmyadmin.starts_with(&tool) {
        return Ok(());
    }
    settings.package_selection.phpmyadmin = tool;
    settings.package_selection = settings.package_selection.normalized();
    apply_database_selection(settings, &app).await
}

/// Install what the new database choice needs, save it, and move the stack
/// onto it (restarting it only if it was running).
async fn apply_database_selection(settings: AppSettings, app: &AppHandle) -> Result<(), String> {
    ensure_selected_database_tool_installed(&settings, app).await?;
    settings.save()?;

    run_with_manager(app, move |manager| -> Result<(), String> {
        let web_stack_was_running = manager.get_all_statuses().iter().any(|(ty, s)| {
            matches!(ty, ServiceType::Caddy | ServiceType::PhpFpm)
                && s.state == ServiceState::Running
        });

        // Stops the previous engine too, so only one database ever runs.
        manager.stop_stack()?;
        manager.update_ports(&settings);
        if web_stack_was_running {
            manager.start_all()?;
        }
        Ok(())
    })
    .await?
}

/// Check system dependencies (libraries required by runtime binaries)
#[tauri::command]
pub async fn check_system_dependencies() -> DependencyCheckResult {
    crate::runtime::deps::check_system_dependencies()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_database_detection_uses_marker_or_binary() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        // `postgresql` is the engine choice, not the manager.
        assert!(!is_selected_database_installed(temp.path(), false));

        fs::create_dir_all(temp.path().join("mysql").join("bin")).unwrap();
        fs::write(temp.path().join("mysql").join("bin").join("mysqld"), "").unwrap();
        assert!(is_selected_database_installed(temp.path(), false));
        assert!(!is_selected_database_installed(temp.path(), true));

        fs::write(temp.path().join("postgresql_installed.txt"), "version=17").unwrap();
        assert!(is_selected_database_installed(temp.path(), true));
    }

    #[test]
    fn folder_scope_only_allows_paths_inside_roots() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let root = temp.path().join("CHAMP");
        let roots = [root.clone()];

        assert!(is_within_roots(&root.join("logs"), &roots));
        assert!(is_within_roots(&root, &roots));
        assert!(!is_within_roots(&temp.path().join("other"), &roots));
        assert!(!is_within_roots(&root.join("..").join("other"), &roots));
        assert!(!is_within_roots(Path::new("relative/logs"), &roots));
        // A sibling whose name merely starts with the root's name is outside.
        assert!(!is_within_roots(&temp.path().join("CHAMP-evil"), &roots));
    }

    #[test]
    fn database_tool_detection_matches_tool_layout() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        assert!(!is_database_tool_installed(temp.path(), "adminer-5.4"));

        fs::create_dir_all(temp.path().join("adminer")).unwrap();
        fs::write(temp.path().join("adminer").join("index.php"), "<?php").unwrap();
        assert!(is_database_tool_installed(temp.path(), "adminer-5.4"));
        assert!(!is_database_tool_installed(temp.path(), "phpmyadmin-5.2"));
    }
}
