//! Tauri IPC Commands
//!
//! This module contains all Tauri commands that are exposed to the frontend.

use crate::config::AppSettings;
use crate::process::{ServiceMap, ServiceState, ServiceType};
use crate::runtime::deps::DependencyCheckResult;
use crate::runtime::downloader::{DownloadProgress, RuntimeDownloader};
use crate::runtime::locator::get_app_data_paths;
use crate::runtime::packages::{PackageSelection, PackagesConfig};
use crate::AppState;
use serde::Serialize;
use std::fs;
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

/// Open the user manual in the system's default application using tauri-plugin-opener
///
/// This command locates the MANUAL.html resource file and reveals it in the
/// file manager using tauri-plugin-opener for cross-platform compatibility.
/// Users can then open it with their preferred browser or HTML viewer.
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

// Global state for download progress
static DOWNLOAD_PROGRESS: Mutex<Option<DownloadProgress>> = Mutex::new(None);

#[derive(Debug, Serialize)]
pub struct AppPathsDto {
    pub base_dir: String,
    pub runtime_dir: String,
    pub config_dir: String,
    pub mysql_data_dir: String,
    pub logs_dir: String,
    pub projects_dir: String,
}

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

#[derive(Debug, Serialize)]
pub struct SystemMetricsDto {
    pub cpu_usage: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub network_rx_bps: u64,
    pub network_tx_bps: u64,
}

struct SystemMetricsMonitor {
    system: System,
    networks: Networks,
    last_network_received_bytes: u64,
    last_network_transmitted_bytes: u64,
    last_sample_time: Instant,
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
        }
    }

    fn collect(&mut self) -> SystemMetricsDto {
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

        SystemMetricsDto {
            cpu_usage: self.system.global_cpu_usage(),
            memory_used_bytes: self.system.used_memory(),
            memory_total_bytes: self.system.total_memory(),
            network_rx_bps,
            network_tx_bps,
        }
    }
}

fn aggregate_network_totals(networks: &Networks) -> (u64, u64) {
    networks.iter().fold((0, 0), |(received, transmitted), (_, data)| {
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
#[tauri::command]
pub async fn start_service(
    service: ServiceType,
    state: State<'_, AppState>,
) -> Result<ServiceMap, String> {
    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

    // Initialize if needed - propagate error if this fails
    manager.initialize()?;

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
#[tauri::command]
pub async fn restart_service(
    service: ServiceType,
    state: State<'_, AppState>,
) -> Result<ServiceMap, String> {
    let mut manager = state
        .process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;

    // Initialize if needed - propagate error if this fails
    manager.initialize()?;

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

    manager.stop_all()?;
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

/// Get app settings
#[tauri::command]
pub async fn get_settings() -> Result<crate::config::AppSettings, String> {
    Ok(crate::config::AppSettings::load())
}

/// Save app settings
#[tauri::command]
pub async fn save_settings(
    settings: crate::config::AppSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Save the settings first
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

    // Update ports in the process manager
    manager.update_ports(&settings);

    // Restart any running services with new configuration
    for service in running_services {
        // Stop the service
        let _ = manager.stop(service);
        // Start it again with new port settings
        let _ = manager.start(service);
    }

    Ok(())
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
pub async fn check_ports(web_port: u16, php_port: u16, mysql_port: u16) -> serde_json::Value {
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
pub async fn reset_installation() -> Result<String, String> {
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
        runtime_dir: paths.runtime_dir.to_string_lossy().to_string(),
        config_dir: paths.config_dir.to_string_lossy().to_string(),
        mysql_data_dir: paths.mysql_data_dir.to_string_lossy().to_string(),
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

    manager.stop_all()?;

    Ok("All services stopped".to_string())
}

/// Get all available runtime packages
#[tauri::command]
pub async fn get_available_packages_cmd() -> Result<PackagesConfig, String> {
    Ok(crate::runtime::packages::get_available_packages())
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
    let skip_list = ["caddy", "mysql", "adminer", "phpmyadmin"];

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
    for component in ["caddy", "php", "mysql", "adminer", "phpmyadmin"] {
        let marker_file = runtime_dir.join(format!("{}_installed.txt", component));
        if let Ok(content) = fs::read_to_string(&marker_file) {
            // Parse version from format: "version=1.2.3\ninstalled_at=..."
            for line in content.lines() {
                if let Some(version) = line.strip_prefix("version=") {
                    let key = if component == "phpmyadmin" {
                        "adminer"
                    } else {
                        component
                    };
                    versions.insert(key.to_string(), version.to_string());
                    break;
                }
            }
        }
    }

    // Add Caddy version from default config (not in packages)
    if !versions.contains_key("caddy") {
        versions.insert("caddy".to_string(), "2.11.2".to_string());
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
