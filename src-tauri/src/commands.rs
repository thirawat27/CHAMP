//! Tauri IPC Commands
//!
//! This module contains all Tauri commands that are exposed to the frontend.
//! Commands are invoked from the React frontend using the `invoke()` function.
//!
//! # Command Categories
//!
//! - **Folder Operations**: `open_folder`, `open_manual`
//! - **Service Control**: `start_service`, `stop_service`, `restart_service`, `start_all_services`, `stop_all_services`, `restart_all_services`
//! - **Service Status**: `get_all_statuses`
//! - **HTTPS Tunnel**: `start_https_tunnel`, `stop_https_tunnel`, `get_https_tunnel_status`
//! - **Settings**: `get_settings`, `save_settings`, `validate_settings`
//! - **Port Checking**: `check_ports`
//! - **Runtime Management**: `check_runtime_installed`, `download_runtime`, `download_runtime_with_packages`, `download_runtime_with_skip`
//! - **Installation**: `reset_installation`, `get_runtime_dir`, `get_install_dir`, `get_app_paths`
//! - **System Metrics**: `get_system_metrics`
//! - **Package Management**: `get_available_packages_cmd`, `get_package_selection`, `update_package_selection`
//! - **PHP Version Management**: `get_installed_php_versions`, `switch_php_version`, `download_php_version`
//! - **Version Info**: `get_installed_versions`, `check_existing_components`
//! - **Dependencies**: `check_system_dependencies`

use crate::config::AppSettings;
use crate::constants::SYSTEM_METRICS_MIN_SAMPLE_INTERVAL_MS;
use crate::process::{ServiceMap, ServiceState, ServiceType};
use crate::runtime::deps::DependencyCheckResult;
use crate::runtime::downloader::Platform;
use crate::runtime::downloader::{DownloadProgress, RuntimeDownloader};
use crate::runtime::locator::get_app_data_paths;
use crate::runtime::packages::{PackageSelection, PackagesConfig};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use sysinfo::{Networks, System};
use tauri::Emitter;
use tauri::State;

/// Open a folder in the system's file explorer using tauri-plugin-opener
///
/// This is a wrapper function that forwards to the plugin for cross-platform compatibility.
/// The plugin handles platform-specific operations internally.
///
/// # Arguments
///
/// * `path` - The folder path to open
///
/// # Returns
///
/// * `Ok(())` - If the folder was opened successfully
/// * `Err(String)` - If the operation failed
///
/// # Examples
///
/// ```rust,ignore
/// open_folder("/path/to/folder".to_string()).await?;
/// ```
#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    use tauri_plugin_opener::reveal_item_in_dir;

    // Ensure folder exists before opening
    let path_obj = std::path::Path::new(&path);
    if !path_obj.exists() {
        fs::create_dir_all(path_obj).map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    // Use tauri-plugin-opener for cross-platform folder opening
    reveal_item_in_dir(&path).map_err(|e| format!("Failed to open folder: {}", e))?;

    Ok(())
}

/// Open a terminal in a specific directory with the PATH injected to include installed runtimes
#[tauri::command]
pub async fn open_project_terminal(path: Option<String>) -> Result<(), String> {
    use std::process::Command;

    let target_dir = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            crate::runtime::locator::get_app_data_paths()
                .map_err(|e| format!("Failed to get app paths: {}", e))?
                .projects_dir
        }
    };

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create terminal directory: {}", e))?;
    }

    // Attempt to locate installed runtimes and collect their bin directories
    let mut runtime_paths = Vec::new();
    if let Ok(runtimes) = crate::runtime::locator::locate_runtime_binaries() {
        if let Some(parent) = runtimes.php_cgi.parent() {
            runtime_paths.push(parent.to_path_buf());
        }
        if let Some(parent) = runtimes.mysql.parent() {
            runtime_paths.push(parent.to_path_buf());
        }
        if let Some(node) = runtimes.node {
            if let Some(parent) = node.parent() {
                runtime_paths.push(parent.to_path_buf());
            }
        }
        if let Some(python) = runtimes.python {
            if let Some(parent) = python.parent() {
                runtime_paths.push(parent.to_path_buf());
                // Python usually needs its Scripts dir too
                runtime_paths.push(parent.join("Scripts"));
            }
        }
        if let Some(go) = runtimes.go {
            if let Some(parent) = go.parent() {
                runtime_paths.push(parent.to_path_buf());
            }
        }
        if let Some(ruby) = runtimes.ruby {
            if let Some(parent) = ruby.parent() {
                runtime_paths.push(parent.to_path_buf());
            }
        }
    }

    // Build the new PATH environment variable
    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = std::env::split_paths(&current_path).collect::<Vec<_>>();
    // Prepend new paths to override system defaults if there's a conflict
    paths.splice(0..0, runtime_paths);
    let new_path = std::env::join_paths(paths).unwrap_or(current_path);

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd.exe")
            .arg("/c")
            .arg("start")
            .arg("cmd.exe")
            .env("PATH", new_path)
            .current_dir(target_dir)
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, open Terminal.app but PATH injection might be tricky depending on how "open" works.
        // A better robust way is launching osascript to open terminal.
        // For now, try launching bash inside Terminal.app using open.
        Command::new("open")
            .arg("-a")
            .arg("Terminal")
            .arg(&target_dir)
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("x-terminal-emulator")
            .arg("--working-directory")
            .arg(&target_dir)
            .env("PATH", new_path)
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    Ok(())
}

/// Open the user manual in the system's default application using tauri-plugin-opener
///
/// This command locates the MANUAL.html resource file and reveals it in the
/// file manager using tauri-plugin-opener for cross-platform compatibility.
/// Users can then open it with their preferred browser or HTML viewer.
///
/// # Arguments
///
/// * `app` - Tauri application handle
///
/// # Returns
///
/// * `Ok(())` - If the manual was opened successfully
/// * `Err(String)` - If the manual file was not found or could not be opened
#[tauri::command]
pub async fn open_manual(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    use tauri_plugin_opener::reveal_item_in_dir;

    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?;

    let manual_path = resource_dir.join("MANUAL.html");

    // Ensure the manual exists
    if !manual_path.exists() {
        return Err(format!("Manual not found at: {}", manual_path.display()));
    }

    // Use tauri-plugin-opener to reveal the file in the file manager
    // This is cross-platform and lets the user choose how to open it
    reveal_item_in_dir(&manual_path).map_err(|e| format!("Failed to open manual: {}", e))?;

    Ok(())
}

/// Global state for download progress tracking
static DOWNLOAD_PROGRESS: Mutex<Option<DownloadProgress>> = Mutex::new(None);

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

/// Supported project starter templates.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectTemplate {
    Static,
    Php,
    Node,
    Python,
    Go,
    Ruby,
}

/// Data transfer object returned after creating a starter project.
#[derive(Debug, Serialize)]
pub struct ProjectScaffoldResult {
    pub name: String,
    pub template: String,
    pub path: String,
    pub entry_file: String,
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
        let mut system = System::new_all();
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

fn marker_version(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    content
        .lines()
        .find_map(|line| line.strip_prefix("version=").map(str::to_string))
}

/// Start a service
///
/// # Arguments
///
/// * `service` - The service type to start
/// * `state` - Application state containing the process manager
///
/// # Returns
///
/// * `Ok(ServiceMap)` - Updated service statuses
/// * `Err(String)` - If the operation failed
#[tauri::command]
pub async fn start_service(
    service: ServiceType,
    state: State<'_, AppState>,
) -> Result<ServiceMap, String> {
    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

    // Start the service
    let result = manager.start(service);

    // Update health and return statuses regardless of start result
    // This ensures the frontend sees the error state
    manager.update_health();
    let statuses = manager.get_all_statuses();

    // Return error after getting statuses so frontend can see the error state
    result?;
    Ok(statuses)
}

/// Stop a service
///
/// # Arguments
///
/// * `service` - The service type to stop
/// * `state` - Application state containing the process manager
///
/// # Returns
///
/// * `Ok(ServiceMap)` - Updated service statuses
/// * `Err(String)` - If the operation failed
#[tauri::command]
pub async fn stop_service(
    service: ServiceType,
    state: State<'_, AppState>,
) -> Result<ServiceMap, String> {
    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

    // Stop the service
    manager.stop(service)?;

    // Update health and return statuses
    manager.update_health();
    Ok(manager.get_all_statuses())
}

/// Restart a service
///
/// # Arguments
///
/// * `service` - The service type to restart
/// * `state` - Application state containing the process manager
///
/// # Returns
///
/// * `Ok(ServiceMap)` - Updated service statuses
/// * `Err(String)` - If the operation failed
#[tauri::command]
pub async fn restart_service(
    service: ServiceType,
    state: State<'_, AppState>,
) -> Result<ServiceMap, String> {
    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

    // Restart the service
    let result = manager.restart(service);

    // Update health and return statuses regardless of restart result
    manager.update_health();
    let statuses = manager.get_all_statuses();

    // Return error after getting statuses so frontend can see the error state
    result?;
    Ok(statuses)
}

#[tauri::command]
pub async fn start_all_services(state: State<'_, AppState>) -> Result<ServiceMap, String> {
    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

    let result = manager.start_all();
    manager.update_health();
    let statuses = manager.get_all_statuses();
    result?;
    Ok(statuses)
}

#[tauri::command]
pub async fn stop_all_services(state: State<'_, AppState>) -> Result<ServiceMap, String> {
    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

    let _ = crate::tunnel::stop_https_tunnel();
    manager.stop_stack()?;
    manager.update_health();
    Ok(manager.get_all_statuses())
}

#[tauri::command]
pub async fn restart_all_services(state: State<'_, AppState>) -> Result<ServiceMap, String> {
    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

    let result = manager.restart_all();
    manager.update_health();
    let statuses = manager.get_all_statuses();
    result?;
    Ok(statuses)
}

/// Get the status of all services
#[tauri::command]
pub async fn get_all_statuses(state: State<'_, AppState>) -> Result<ServiceMap, String> {
    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

    // Update health before returning statuses
    manager.update_health();
    Ok(manager.get_all_statuses())
}

/// Start a free public HTTPS URL for the local CHAMP web server.
///
/// This uses Cloudflare Quick Tunnel (`*.trycloudflare.com`) and starts the
/// selected CHAMP stack first when the web server is not already running.
#[tauri::command]
pub async fn start_https_tunnel(
    state: State<'_, AppState>,
) -> Result<crate::tunnel::HttpsTunnelStatus, String> {
    let web_port = {
        let mut manager = state
            .process_manager
            .lock()
            .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

        manager.update_health();
        let statuses = manager.get_all_statuses();
        let caddy_running = statuses
            .get(&ServiceType::Caddy)
            .map(|service| service.state == ServiceState::Running)
            .unwrap_or(false);

        if !caddy_running {
            manager.start_all()?;
            manager.update_health();
        }

        manager
            .get_all_statuses()
            .get(&ServiceType::Caddy)
            .map(|service| service.port)
            .unwrap_or(ServiceType::Caddy.default_port())
    };

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

/// Get app settings
#[tauri::command]
pub async fn get_settings() -> Result<crate::config::AppSettings, String> {
    Ok(crate::config::AppSettings::load())
}

/// Save app settings
#[tauri::command]
pub async fn save_settings(
    settings: crate::config::AppSettings,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    ensure_selected_database_tool_installed(&settings, &app).await?;

    // Save the settings after any required runtime install succeeds.
    settings.save()?;

    // Update the ProcessManager with new settings
    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

    // Get the current running services before updating ports
    let running_services: Vec<ServiceType> = manager
        .get_all_statuses()
        .iter()
        .filter(|(_, s)| s.state == ServiceState::Running)
        .map(|(ty, _)| *ty)
        .collect();
    let web_stack_was_running = running_services.contains(&ServiceType::Caddy)
        || running_services.contains(&ServiceType::PhpFpm);

    // Update ports in the process manager
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

async fn ensure_selected_database_tool_installed(
    settings: &AppSettings,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let tool_id = &settings.package_selection.phpmyadmin;
    let packages = crate::runtime::packages::get_available_packages();
    let package = packages
        .phpmyadmin
        .iter()
        .find(|package| package.id == *tool_id)
        .ok_or_else(|| format!("Unknown database tool: {}", tool_id))?;

    let downloader = RuntimeDownloader::with_packages(settings.package_selection.clone());
    let runtime_dir = downloader.get_runtime_dir()?;
    if is_database_tool_installed(&runtime_dir, tool_id)
        && is_selected_database_installed(&runtime_dir, tool_id)
    {
        return Ok(());
    }

    eprintln!(
        "{} is selected but not fully installed. Downloading required components now.",
        package.display_name
    );

    let app_clone = app.clone();
    let skip_list = if tool_id.starts_with("adminer") {
        ["caddy", "php", "mysql", "phpmyadmin"]
    } else {
        ["caddy", "php", "postgresql", "adminer"]
    };
    downloader
        .download_all_with_skip(
            Box::new(move |progress| {
                let _ = app_clone.emit("download-progress", &progress);
                if let Ok(mut p) = DOWNLOAD_PROGRESS.lock() {
                    *p = Some(progress);
                }
            }),
            &skip_list,
        )
        .await?;

    if is_database_tool_installed(&runtime_dir, tool_id)
        && is_selected_database_installed(&runtime_dir, tool_id)
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

fn is_selected_database_installed(runtime_dir: &Path, tool_id: &str) -> bool {
    if tool_id.starts_with("adminer") {
        runtime_dir.join("postgresql_installed.txt").exists()
            || runtime_dir
                .join("postgresql")
                .join("bin")
                .join("postgres.exe")
                .exists()
            || runtime_dir
                .join("postgresql")
                .join("bin")
                .join("postgres")
                .exists()
            || runtime_dir.join("bin").join("postgres.exe").exists()
            || runtime_dir.join("bin").join("postgres").exists()
    } else {
        runtime_dir.join("mysql_installed.txt").exists()
            || runtime_dir
                .join("mysql")
                .join("bin")
                .join("mysqld.exe")
                .exists()
            || runtime_dir
                .join("mysql")
                .join("bin")
                .join("mysqld")
                .exists()
            || runtime_dir.join("bin").join("mysqld.exe").exists()
            || runtime_dir.join("bin").join("mysqld").exists()
    }
}

/// Validate settings (check port conflicts, valid paths)
#[tauri::command]
pub async fn validate_settings(
    settings: crate::config::AppSettings,
) -> Result<Vec<String>, Vec<String>> {
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
        "web": {
            "port": web_port,
            "available": is_port_available(web_port)
        },
        "php": {
            "port": php_port,
            "available": is_port_available(php_port)
        },
        "mysql": {
            "port": mysql_port,
            "available": is_port_available(mysql_port)
        },
        "postgresql": {
            "port": postgresql_port,
            "available": is_port_available(postgresql_port)
        }
    })
}

/// Check if runtime binaries are already installed
#[tauri::command]
pub async fn check_runtime_installed() -> Result<bool, String> {
    let downloader = RuntimeDownloader::new();
    Ok(downloader.is_installed())
}

/// Reset installation (for testing/debug - deletes runtime directory)
#[tauri::command]
pub async fn reset_installation(state: State<'_, AppState>) -> Result<String, String> {
    {
        let mut manager = state
            .process_manager
            .lock()
            .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;
        manager.stop_all()?;
    }

    reset_runtime_dir().await
}

pub async fn reset_runtime_dir() -> Result<String, String> {
    let downloader = RuntimeDownloader::new();
    let runtime_dir = downloader.get_runtime_dir().map_err(|e| e.to_string())?;

    if runtime_dir.exists() {
        fs::remove_dir_all(&runtime_dir)
            .map_err(|e| format!("Failed to remove runtime directory: {}", e))?;
    }

    Ok("Installation reset. Run the app again to see first-run wizard.".to_string())
}

/// Get the runtime directory path
#[tauri::command]
pub async fn get_runtime_dir() -> Result<String, String> {
    let downloader = RuntimeDownloader::new();
    downloader
        .get_runtime_dir()
        .map(|p| p.to_string_lossy().to_string())
}

/// Get the installation directory (where the exe is located)
#[tauri::command]
pub async fn get_install_dir() -> Result<String, String> {
    get_app_data_paths().map(|paths| paths.base_dir.to_string_lossy().to_string())
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
pub async fn create_project_template(
    project_name: String,
    template: ProjectTemplate,
) -> Result<ProjectScaffoldResult, String> {
    let paths = get_app_data_paths()?;
    fs::create_dir_all(&paths.projects_dir)
        .map_err(|e| format!("Failed to create projects directory: {}", e))?;

    create_project_template_in_dir(&paths.projects_dir, &project_name, template)
}

fn create_project_template_in_dir(
    projects_dir: &Path,
    project_name: &str,
    template: ProjectTemplate,
) -> Result<ProjectScaffoldResult, String> {
    let name = validate_project_name(project_name)?;
    let projects_root = projects_dir
        .canonicalize()
        .map_err(|e| format!("Failed to resolve projects directory: {}", e))?;
    let project_dir = projects_root.join(&name);

    if !project_dir.starts_with(&projects_root) {
        return Err("Project path must stay inside the projects directory".to_string());
    }

    if project_dir.exists() {
        let mut entries = fs::read_dir(&project_dir)
            .map_err(|e| format!("Failed to inspect existing project directory: {}", e))?;
        if entries.next().is_some() {
            return Err(format!(
                "Project '{}' already exists and is not empty",
                name
            ));
        }
    } else {
        fs::create_dir(&project_dir)
            .map_err(|e| format!("Failed to create project directory: {}", e))?;
    }

    let files = project_template_files(template, &name);
    for (relative_path, content) in files {
        let file_path = project_dir.join(relative_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create project subdirectory: {}", e))?;
        }
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file_path)
            .and_then(|mut file| file.write_all(content.as_bytes()))
            .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
    }

    let entry_file = project_entry_file(template);
    Ok(ProjectScaffoldResult {
        name,
        template: project_template_id(template).to_string(),
        path: project_dir.to_string_lossy().to_string(),
        entry_file: project_dir.join(entry_file).to_string_lossy().to_string(),
    })
}

fn validate_project_name(project_name: &str) -> Result<String, String> {
    let name = project_name.trim();
    if name.is_empty() {
        return Err("Project name is required".to_string());
    }

    if name == "." || name == ".." {
        return Err("Project name cannot be '.' or '..'".to_string());
    }

    if name.chars().any(|ch| {
        ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    }) {
        return Err(
            "Project name contains characters that are not valid in a folder name".to_string(),
        );
    }

    Ok(name.to_string())
}

fn project_template_id(template: ProjectTemplate) -> &'static str {
    match template {
        ProjectTemplate::Static => "static",
        ProjectTemplate::Php => "php",
        ProjectTemplate::Node => "node",
        ProjectTemplate::Python => "python",
        ProjectTemplate::Go => "go",
        ProjectTemplate::Ruby => "ruby",
    }
}

fn project_entry_file(template: ProjectTemplate) -> &'static str {
    match template {
        ProjectTemplate::Static => "index.html",
        ProjectTemplate::Php => "index.php",
        ProjectTemplate::Node => "README.md",
        ProjectTemplate::Python => "README.md",
        ProjectTemplate::Go => "README.md",
        ProjectTemplate::Ruby => "README.md",
    }
}

fn project_template_files(template: ProjectTemplate, name: &str) -> Vec<(&'static str, String)> {
    match template {
        ProjectTemplate::Static => vec![
            (
                "index.html",
                format!(
                    r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{name}</title>
    <link rel="stylesheet" href="./styles.css" />
  </head>
  <body>
    <main>
      <h1>{name}</h1>
      <p>Static project served from the CHAMP projects folder.</p>
    </main>
    <script src="./script.js"></script>
  </body>
</html>
"#
                ),
            ),
            (
                "styles.css",
                "body { margin: 0; min-height: 100vh; display: grid; place-items: center; font-family: system-ui, sans-serif; background: #f8fafc; color: #111827; }\nmain { width: min(720px, calc(100vw - 32px)); }\n".to_string(),
            ),
            (
                "script.js",
                "console.log('CHAMP static project ready');\n".to_string(),
            ),
        ],
        ProjectTemplate::Php => vec![(
            "index.php",
            format!(
                r#"<?php
$projectName = "{name}";
?>
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title><?= htmlspecialchars($projectName) ?></title>
  </head>
  <body>
    <h1><?= htmlspecialchars($projectName) ?></h1>
    <p>PHP <?= phpversion() ?> is running through CHAMP.</p>
  </body>
</html>
"#
            ),
        )],
        ProjectTemplate::Node => vec![
            (
                "package.json",
                format!(
                    r#"{{
  "name": "{}",
  "private": true,
  "type": "module",
  "scripts": {{
    "dev": "node src/server.js"
  }}
}}
"#,
                    package_name_slug(name)
                ),
            ),
            (
                "src/server.js",
                "import { createServer } from 'node:http';\n\nconst port = Number(process.env.PORT || 3000);\n\ncreateServer((_req, res) => {\n  res.writeHead(200, { 'content-type': 'text/plain; charset=utf-8' });\n  res.end('Node project ready from CHAMP workspace\\n');\n}).listen(port, () => {\n  console.log(`Node app running at http://localhost:${port}`);\n});\n".to_string(),
            ),
            (
                "README.md",
                "# Node starter\n\nRun `npm run dev` from this folder. CHAMP keeps the project with the rest of your local web workspace.\n".to_string(),
            ),
        ],
        ProjectTemplate::Python => vec![
            (
                "app.py",
                "from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler\n\nPORT = 5000\n\nif __name__ == \"__main__\":\n    server = ThreadingHTTPServer((\"127.0.0.1\", PORT), SimpleHTTPRequestHandler)\n    print(f\"Python app running at http://127.0.0.1:{PORT}\")\n    server.serve_forever()\n".to_string(),
            ),
            (
                "README.md",
                "# Python starter\n\nRun `python app.py` from this folder. CHAMP keeps the project with the rest of your local web workspace.\n".to_string(),
            ),
        ],
        ProjectTemplate::Go => vec![
            (
                "main.go",
                "package main\n\nimport (\n\t\"fmt\"\n\t\"net/http\"\n)\n\nfunc main() {\n\thttp.HandleFunc(\"/\", func(w http.ResponseWriter, r *http.Request) {\n\t\tfmt.Fprintf(w, \"Go project ready from CHAMP workspace\\n\")\n\t})\n\n\tport := \":8080\"\n\tfmt.Printf(\"Go server listening on %s\\n\", port)\n\thttp.ListenAndServe(port, nil)\n}\n".to_string(),
            ),
            (
                "go.mod",
                format!("module {}\n\ngo 1.21\n", package_name_slug(name)),
            ),
            (
                "README.md",
                "# Go starter\n\nRun `go run main.go` from this folder. CHAMP keeps the project with the rest of your local web workspace.\n".to_string(),
            ),
        ],
        ProjectTemplate::Ruby => vec![
            (
                "app.rb",
                "require 'webrick'\n\nserver = WEBrick::HTTPServer.new(Port: 4567)\n\nserver.mount_proc '/' do |req, res|\n  res.body = \"Ruby project ready from CHAMP workspace\\n\"\nend\n\nputs \"Ruby server running at http://localhost:4567\"\nserver.start\n".to_string(),
            ),
            (
                "README.md",
                "# Ruby starter\n\nRun `ruby app.rb` from this folder. CHAMP keeps the project with the rest of your local web workspace.\n".to_string(),
            ),
        ],
    }
}

fn package_name_slug(name: &str) -> String {
    let slug = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        "champ-node-project".to_string()
    } else {
        slug
    }
}

#[tauri::command]
pub async fn get_system_metrics() -> Result<SystemMetricsDto, String> {
    let monitor = SYSTEM_METRICS_MONITOR.get_or_init(|| Mutex::new(SystemMetricsMonitor::new()));
    let mut monitor = monitor
        .lock()
        .map_err(|e| format!("Failed to acquire system metrics lock: {}", e))?;

    Ok(monitor.collect())
}

/// Get the download directory path (where ZIP files are stored)
#[tauri::command]
pub async fn get_download_dir() -> Result<String, String> {
    let temp_dir = std::env::temp_dir().join("campp-download");
    Ok(temp_dir.to_string_lossy().to_string())
}

/// Download and install runtime binaries
#[tauri::command]
pub async fn download_runtime(app: tauri::AppHandle) -> Result<String, String> {
    let downloader = RuntimeDownloader::new();
    let app_clone = app.clone();

    // Emit progress updates via Tauri events
    downloader
        .download_all(Box::new(move |progress| {
            let _ = app_clone.emit("download-progress", &progress);

            // Store latest progress
            if let Ok(mut p) = DOWNLOAD_PROGRESS.lock() {
                *p = Some(progress);
            }
        }))
        .await?;

    Ok("Runtime binaries installed successfully".to_string())
}

/// Stop all running services (for cleanup on app exit)
#[tauri::command]
pub async fn cleanup_all_services(state: State<'_, AppState>) -> Result<String, String> {
    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

    let _ = crate::tunnel::stop_https_tunnel();
    manager.stop_all()?;

    Ok("All services stopped".to_string())
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

#[tauri::command]
pub async fn get_runtime_platform() -> String {
    Platform::current().url_key()
}

/// Download and install runtime binaries with custom package selection
#[tauri::command]
pub async fn download_runtime_with_packages(
    package_selection: PackageSelection,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let downloader = RuntimeDownloader::with_packages(package_selection);
    let app_clone = app.clone();

    // Emit progress updates via Tauri events
    downloader
        .download_all(Box::new(move |progress| {
            let _ = app_clone.emit("download-progress", &progress);

            // Store latest progress
            if let Ok(mut p) = DOWNLOAD_PROGRESS.lock() {
                *p = Some(progress);
            }
        }))
        .await?;

    Ok("Runtime binaries installed successfully".to_string())
}

/// Get the current package selection from settings
#[tauri::command]
pub async fn get_package_selection() -> Result<PackageSelection, String> {
    let settings = AppSettings::load();
    Ok(settings.package_selection)
}

/// Update package selection in settings (without downloading)
#[tauri::command]
pub async fn update_package_selection(package_selection: PackageSelection) -> Result<(), String> {
    let mut settings = AppSettings::load();
    settings.package_selection = package_selection;
    settings.save()?;
    Ok(())
}

#[tauri::command]
pub async fn get_installed_php_versions() -> Result<Vec<InstalledPhpVersionDto>, String> {
    let runtime_dir = RuntimeDownloader::new().get_runtime_dir()?;
    let settings = AppSettings::load();
    let active_php = settings.package_selection.php;
    let packages = crate::runtime::packages::get_available_packages();
    let legacy_version = marker_version(&runtime_dir.join("php_installed.txt"));

    Ok(packages
        .php
        .into_iter()
        .map(|package| {
            let package_id = package.id.clone();
            let version_dir = runtime_dir.join("php_versions").join(&package_id);
            let marker = runtime_dir
                .join("php_versions")
                .join(format!("{}_installed.txt", package_id));
            let marker_version = marker_version(&marker);
            let installed = marker_version.is_some()
                || legacy_version
                    .as_ref()
                    .map(|version| version == &package.version)
                    .unwrap_or(false);

            InstalledPhpVersionDto {
                id: package_id.clone(),
                version: marker_version.unwrap_or(package.version),
                display_name: package.display_name,
                installed,
                active: active_php == package_id,
                eol: package.eol,
                lts: package.lts,
                recommended: package.recommended,
                path: if version_dir.exists() {
                    Some(version_dir.to_string_lossy().to_string())
                } else {
                    None
                },
            }
        })
        .collect())
}

#[tauri::command]
pub async fn switch_php_version(
    php_id: String,
    state: State<'_, AppState>,
) -> Result<ServiceMap, String> {
    let packages = crate::runtime::packages::get_available_packages();
    if !packages.php.iter().any(|package| package.id == php_id) {
        return Err(format!("Unknown PHP version: {}", php_id));
    }

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

    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;
    let statuses = manager.get_all_statuses();
    let php_was_running = statuses
        .get(&ServiceType::PhpFpm)
        .map(|service| service.state == ServiceState::Running)
        .unwrap_or(false);
    let caddy_was_running = statuses
        .get(&ServiceType::Caddy)
        .map(|service| service.state == ServiceState::Running)
        .unwrap_or(false);

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
}

#[tauri::command]
pub async fn download_php_version(php_id: String, app: tauri::AppHandle) -> Result<String, String> {
    let packages = crate::runtime::packages::get_available_packages();
    if !packages.php.iter().any(|package| package.id == php_id) {
        return Err(format!("Unknown PHP version: {}", php_id));
    }

    let mut settings = AppSettings::load();
    settings.package_selection.php = php_id;
    settings.save()?;

    let downloader = RuntimeDownloader::with_packages(settings.package_selection);
    let app_clone = app.clone();
    let skip_list = ["caddy", "mysql", "postgresql", "adminer", "phpmyadmin"];

    downloader
        .download_all_with_skip(
            Box::new(move |progress| {
                let _ = app_clone.emit("download-progress", &progress);
                if let Ok(mut p) = DOWNLOAD_PROGRESS.lock() {
                    *p = Some(progress);
                }
            }),
            &skip_list,
        )
        .await?;

    Ok("PHP version installed successfully".to_string())
}

/// Get the selected package IDs from runtime-config.json
#[tauri::command]
pub async fn get_selected_package_ids() -> Result<PackageSelection, String> {
    Ok(crate::runtime::packages::get_selected_package_ids())
}

/// Reload the runtime configuration from runtime-config.json
#[tauri::command]
pub async fn reload_runtime_config() -> Result<String, String> {
    crate::runtime::packages::reload_runtime_config();
    Ok("Runtime configuration reloaded successfully".to_string())
}

/// Get the installed runtime versions
#[tauri::command]
pub async fn get_installed_versions() -> Result<std::collections::HashMap<String, String>, String> {
    let downloader = RuntimeDownloader::new();
    let runtime_dir = downloader.get_runtime_dir()?;

    let mut versions = std::collections::HashMap::new();

    // Read version from marker files
    for component in [
        "caddy",
        "php",
        "mysql",
        "postgresql",
        "adminer",
        "phpmyadmin",
        "node",
        "python",
        "go",
        "ruby",
        "cloudflared",
    ] {
        let marker_file = runtime_dir.join(format!("{}_installed.txt", component));
        if let Ok(content) = fs::read_to_string(&marker_file) {
            // Parse version from format: "version=1.2.3\ninstalled_at=..."
            for line in content.lines() {
                if let Some(version) = line.strip_prefix("version=") {
                    versions.insert(component.to_string(), version.to_string());
                    break;
                }
            }
        }
    }

    // Add Caddy version from default config (not in packages)
    if !versions.contains_key("caddy") {
        versions.insert(
            "caddy".to_string(),
            crate::runtime::packages::selected_caddy_version(),
        );
    }

    Ok(versions)
}

/// Check for existing components before download
#[tauri::command]
pub async fn check_existing_components() -> Result<std::collections::HashMap<String, String>, String>
{
    let downloader = RuntimeDownloader::new();
    Ok(downloader.get_installed_components())
}

/// Download and install runtime binaries with option to skip existing components
#[tauri::command]
pub async fn download_runtime_with_skip(
    package_selection: PackageSelection,
    skip_list: Vec<String>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let downloader = RuntimeDownloader::with_packages(package_selection);
    let app_clone = app.clone();

    // Convert Vec<String> to Vec<&str> for the skip_list
    let skip_refs: Vec<&str> = skip_list.iter().map(|s| s.as_str()).collect();

    // Emit progress updates via Tauri events
    downloader
        .download_all_with_skip(
            Box::new(move |progress| {
                let _ = app_clone.emit("download-progress", &progress);

                // Store latest progress
                if let Ok(mut p) = DOWNLOAD_PROGRESS.lock() {
                    *p = Some(progress);
                }
            }),
            &skip_refs,
        )
        .await?;

    Ok("Runtime binaries installed successfully".to_string())
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
    fn validate_project_name_rejects_path_traversal() {
        assert!(validate_project_name("..").is_err());
        assert!(validate_project_name("../outside").is_err());
        assert!(validate_project_name("nested\\outside").is_err());
    }

    #[test]
    fn package_name_slug_keeps_npm_safe_ascii() {
        assert_eq!(package_name_slug("My Node App"), "my-node-app");
        assert_eq!(package_name_slug("โปรเจกต์"), "champ-node-project");
    }

    #[test]
    fn create_static_project_writes_expected_files() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");

        let result = create_project_template_in_dir(temp.path(), "demo", ProjectTemplate::Static)
            .expect("failed to create project");

        assert_eq!(result.name, "demo");
        assert!(temp.path().join("demo").join("index.html").exists());
        assert!(temp.path().join("demo").join("styles.css").exists());
        assert!(temp.path().join("demo").join("script.js").exists());
    }

    #[test]
    fn create_project_does_not_overwrite_non_empty_directory() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let project = temp.path().join("demo");
        std::fs::create_dir(&project).expect("failed to create project dir");
        std::fs::write(project.join("keep.txt"), "keep").expect("failed to write sentinel");

        let result = create_project_template_in_dir(temp.path(), "demo", ProjectTemplate::Php);

        assert!(result.is_err());
        assert_eq!(
            std::fs::read_to_string(project.join("keep.txt")).expect("failed to read sentinel"),
            "keep"
        );
    }
}
