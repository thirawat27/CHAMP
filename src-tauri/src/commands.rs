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
use serde::{Deserialize, Serialize};
use std::fs;
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

const APP_RELEASE_API_URL: &str = "https://api.github.com/repos/thirawat27/CHAMP/releases/latest";
const RUNTIME_CONFIG_URL: &str =
    "https://raw.githubusercontent.com/thirawat27/CHAMP/main/src-tauri/runtime-config.json";

#[derive(Debug, Serialize)]
pub struct AppUpdateDto {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub available: bool,
    pub asset_name: Option<String>,
    pub download_url: Option<String>,
    pub release_url: Option<String>,
    pub published_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeComponentUpdateDto {
    pub component: String,
    pub display_name: String,
    pub installed_version: Option<String>,
    pub latest_version: String,
    pub update_available: bool,
}

#[derive(Debug, Serialize)]
pub struct AppUpdateDownloadDto {
    pub file_path: String,
    pub asset_name: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    published_at: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

fn marker_version(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    content
        .lines()
        .find_map(|line| line.strip_prefix("version=").map(str::to_string))
}

fn user_runtime_config_path() -> Result<std::path::PathBuf, String> {
    dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .map(|p| p.join("CHAMP").join("config").join("runtime-config.json"))
        .ok_or_else(|| "Cannot determine writable config directory".to_string())
}

fn normalize_version(version: &str) -> Vec<u64> {
    version
        .trim_start_matches('v')
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

fn is_version_newer(latest: &str, current: &str) -> bool {
    let mut latest_parts = normalize_version(latest);
    let mut current_parts = normalize_version(current);
    let max_len = latest_parts.len().max(current_parts.len());
    latest_parts.resize(max_len, 0);
    current_parts.resize(max_len, 0);
    latest_parts > current_parts
}

fn selected_caddy_version() -> Option<String> {
    crate::runtime::packages::get_config().and_then(|config| {
        config
            .binaries
            .caddy
            .versions
            .iter()
            .find(|version| version.selected)
            .or_else(|| config.binaries.caddy.versions.first())
            .map(|version| version.version.clone())
    })
}

fn current_platform_release_asset<'a>(release: &'a GitHubRelease) -> Option<&'a GitHubAsset> {
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    release
        .assets
        .iter()
        .find(|asset| {
            let name = asset.name.to_ascii_lowercase();
            name.contains(platform) && (name.ends_with(".zip") || name.ends_with(".tar.gz"))
        })
        .or_else(|| {
            release.assets.iter().find(|asset| {
                let name = asset.name.to_ascii_lowercase();
                name.contains(platform)
            })
        })
}

async fn fetch_latest_release() -> Result<GitHubRelease, String> {
    reqwest::Client::new()
        .get(APP_RELEASE_API_URL)
        .header("User-Agent", "CHAMP-Updater")
        .send()
        .await
        .map_err(|e| format!("Failed to check GitHub releases: {}", e))?
        .error_for_status()
        .map_err(|e| format!("GitHub release request failed: {}", e))?
        .json::<GitHubRelease>()
        .await
        .map_err(|e| format!("Failed to parse GitHub release: {}", e))
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

#[tauri::command]
pub async fn check_app_update() -> Result<AppUpdateDto, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let release = fetch_latest_release().await?;
    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let asset = current_platform_release_asset(&release);

    Ok(AppUpdateDto {
        available: is_version_newer(&latest_version, &current_version),
        current_version,
        latest_version: Some(latest_version),
        asset_name: asset.map(|asset| asset.name.clone()),
        download_url: asset.map(|asset| asset.browser_download_url.clone()),
        release_url: Some(release.html_url),
        published_at: release.published_at,
        notes: release.body,
    })
}

#[tauri::command]
pub async fn download_app_update(app: tauri::AppHandle) -> Result<AppUpdateDownloadDto, String> {
    let update = check_app_update().await?;
    if !update.available {
        return Err("No newer CHAMP release is available.".to_string());
    }

    let download_url = update
        .download_url
        .clone()
        .ok_or_else(|| "No release asset matches this platform.".to_string())?;
    let asset_name = update
        .asset_name
        .clone()
        .ok_or_else(|| "No release asset name was provided by GitHub.".to_string())?;
    let latest_version = update
        .latest_version
        .clone()
        .unwrap_or_else(|| "latest".to_string());

    let paths = get_app_data_paths()?;
    let updates_dir = paths.base_dir.join("updates");
    fs::create_dir_all(&updates_dir)
        .map_err(|e| format!("Failed to create update directory: {}", e))?;
    let target_path = updates_dir.join(&asset_name);

    let mut response = reqwest::Client::new()
        .get(&download_url)
        .header("User-Agent", "CHAMP-Updater")
        .send()
        .await
        .map_err(|e| format!("Failed to download update: {}", e))?
        .error_for_status()
        .map_err(|e| format!("Update download failed: {}", e))?;

    let total_bytes = response.content_length().unwrap_or(0);
    let mut downloaded_bytes = 0_u64;
    let mut file = fs::File::create(&target_path)
        .map_err(|e| format!("Failed to create update file: {}", e))?;

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Failed while reading update download: {}", e))?
    {
        file.write_all(&chunk)
            .map_err(|e| format!("Failed to write update file: {}", e))?;
        downloaded_bytes = downloaded_bytes.saturating_add(chunk.len() as u64);
        let percent = if total_bytes > 0 {
            ((downloaded_bytes as f64 / total_bytes as f64) * 100.0).round() as u8
        } else {
            0
        };
        let _ = app.emit(
            "app-update-progress",
            serde_json::json!({
                "percent": percent,
                "downloadedBytes": downloaded_bytes,
                "totalBytes": total_bytes,
                "assetName": asset_name,
                "version": latest_version,
            }),
        );
    }

    Ok(AppUpdateDownloadDto {
        file_path: target_path.to_string_lossy().to_string(),
        asset_name,
        version: latest_version,
    })
}

#[tauri::command]
pub async fn refresh_runtime_manifest_from_github() -> Result<PackagesConfig, String> {
    let content = reqwest::Client::new()
        .get(RUNTIME_CONFIG_URL)
        .header("User-Agent", "CHAMP-Updater")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch runtime manifest: {}", e))?
        .error_for_status()
        .map_err(|e| format!("Runtime manifest request failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read runtime manifest: {}", e))?;

    serde_json::from_str::<crate::runtime::packages::RuntimeConfig>(&content)
        .map_err(|e| format!("GitHub runtime manifest is invalid: {}", e))?;

    let path = user_runtime_config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create runtime manifest directory: {}", e))?;
    }
    fs::write(&path, content).map_err(|e| format!("Failed to save runtime manifest: {}", e))?;

    crate::runtime::packages::reload_runtime_config();
    Ok(crate::runtime::packages::get_available_packages())
}

#[tauri::command]
pub async fn check_runtime_updates() -> Result<Vec<RuntimeComponentUpdateDto>, String> {
    let settings = AppSettings::load();
    let installed = get_installed_versions().await?;
    let packages = crate::runtime::packages::get_available_packages();

    let selected_php = crate::runtime::packages::get_php_package(&settings.package_selection.php)
        .or_else(|| packages.php.first().cloned());
    let selected_mysql =
        crate::runtime::packages::get_mysql_package(&settings.package_selection.mysql)
            .or_else(|| packages.mysql.first().cloned());
    let selected_tool =
        crate::runtime::packages::get_phpmyadmin_package(&settings.package_selection.phpmyadmin)
            .or_else(|| packages.phpmyadmin.first().cloned());

    let mut updates = Vec::new();
    if let Some(version) = selected_caddy_version() {
        let installed_version = installed.get("caddy").cloned();
        updates.push(RuntimeComponentUpdateDto {
            component: "caddy".to_string(),
            display_name: "Caddy".to_string(),
            update_available: installed_version
                .as_ref()
                .map(|installed| installed != &version)
                .unwrap_or(true),
            installed_version,
            latest_version: version,
        });
    }

    if let Some(package) = selected_php {
        let installed_version = installed.get("php").cloned();
        updates.push(RuntimeComponentUpdateDto {
            component: "php".to_string(),
            display_name: package.display_name,
            update_available: installed_version
                .as_ref()
                .map(|installed| installed != &package.version)
                .unwrap_or(true),
            installed_version,
            latest_version: package.version,
        });
    }

    if let Some(package) = selected_mysql {
        let installed_version = installed.get("mysql").cloned();
        updates.push(RuntimeComponentUpdateDto {
            component: "mysql".to_string(),
            display_name: package.display_name,
            update_available: installed_version
                .as_ref()
                .map(|installed| installed != &package.version)
                .unwrap_or(true),
            installed_version,
            latest_version: package.version,
        });
    }

    if let Some(package) = selected_tool {
        let component = if package.id.starts_with("adminer") {
            "adminer"
        } else {
            "phpmyadmin"
        };
        let installed_version = installed.get(component).cloned();
        updates.push(RuntimeComponentUpdateDto {
            component: component.to_string(),
            display_name: package.display_name,
            update_available: installed_version
                .as_ref()
                .map(|installed| installed != &package.version)
                .unwrap_or(true),
            installed_version,
            latest_version: package.version,
        });
    }

    Ok(updates)
}

#[tauri::command]
pub async fn update_runtime_components(app: tauri::AppHandle) -> Result<String, String> {
    let settings = AppSettings::load();
    let updates = check_runtime_updates().await?;
    let skip_list: Vec<String> = updates
        .iter()
        .filter(|update| !update.update_available)
        .map(|update| update.component.clone())
        .collect();

    let downloader = RuntimeDownloader::with_packages(settings.package_selection);
    let app_clone = app.clone();
    let skip_refs: Vec<&str> = skip_list.iter().map(|item| item.as_str()).collect();

    downloader
        .download_all_with_skip(
            Box::new(move |progress| {
                let _ = app_clone.emit("download-progress", &progress);
                if let Ok(mut p) = DOWNLOAD_PROGRESS.lock() {
                    *p = Some(progress);
                }
            }),
            &skip_refs,
        )
        .await?;

    Ok("Runtime components are up to date.".to_string())
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
    let settings = AppSettings::load();

    let mut versions = std::collections::HashMap::new();

    // Read version from marker files
    for component in ["caddy", "php", "mysql", "adminer", "phpmyadmin"] {
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

    let active_php_marker = runtime_dir
        .join("php_versions")
        .join(format!("{}_installed.txt", settings.package_selection.php));
    if let Some(version) = marker_version(&active_php_marker) {
        versions.insert("php".to_string(), version);
    }

    // Add Caddy version from default config (not in packages)
    if !versions.contains_key("caddy") {
        versions.insert(
            "caddy".to_string(),
            selected_caddy_version().unwrap_or_else(|| "2.11.2".to_string()),
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
