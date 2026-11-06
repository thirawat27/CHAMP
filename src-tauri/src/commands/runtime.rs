//! Installing runtimes and reading the package catalog.

use super::services::stop_tunnel_and_services;
use crate::config::AppSettings;
use crate::process::runner::run_with_manager;
use crate::runtime::deps::DependencyCheckResult;
use crate::runtime::downloader::{read_marker_version, RuntimeDownloader};
use crate::runtime::packages::{PackageSelection, PackagesConfig};
use crate::runtime::platform::Platform;
use std::fs;
use tauri::{AppHandle, Emitter};

/// Installing runtimes without a configured SHA-256 is currently allowed because
/// upstream projects do not publish stable per-platform checksums for every
/// package in `runtime-config.json`. The user is not asked about this. A
/// configured checksum that MISMATCHES still hard-fails regardless of this flag
/// (see `finalize_downloaded_file`).
const ALLOW_UNVERIFIED_RUNTIME_DOWNLOADS: bool = true;

/// Emit runtime download progress to the frontend.
pub(super) fn downloader_for(selection: PackageSelection) -> RuntimeDownloader {
    RuntimeDownloader::with_packages(selection)
        .allow_unverified_checksums(ALLOW_UNVERIFIED_RUNTIME_DOWNLOADS)
}

pub(super) fn progress_emitter(app: &AppHandle) -> crate::runtime::downloader::ProgressCallback {
    let app = app.clone();
    Box::new(move |progress| {
        let _ = app.emit("download-progress", &progress);
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
    stop_tunnel_and_services(&app).await??;

    reset_runtime_dir().await
}

pub async fn reset_runtime_dir() -> Result<String, String> {
    let runtime_dir = RuntimeDownloader::new().get_runtime_dir()?;

    if runtime_dir.exists() {
        fs::remove_dir_all(&runtime_dir)
            .map_err(|e| format!("Failed to remove runtime directory: {}", e))?;
    }

    Ok("Installation reset. Run the app again to see first-run wizard.".to_string())
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
    package_selection.validate_path_ids()?;
    downloader_for(package_selection)
        .download_all(progress_emitter(&app))
        .await?;

    Ok("Runtime binaries installed successfully".to_string())
}

/// Update package selection in settings (without downloading)
#[tauri::command]
pub async fn update_package_selection(
    package_selection: PackageSelection,
    app: AppHandle,
) -> Result<(), String> {
    let selection = package_selection.normalized();
    selection.validate_path_ids()?;
    let settings = AppSettings::update(|settings| settings.package_selection = selection)?;

    // The manager keeps its own copy; without this, the first Start after setup
    // would still run the engine and tool it loaded at launch.
    run_with_manager(&app, move |manager| {
        manager.set_package_selection(settings.package_selection);
    })
    .await
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
        let marker_file = crate::runtime::layout::marker_path(&runtime_dir, component);
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
    package_selection.validate_path_ids()?;
    let skip_refs: Vec<&str> = skip_list.iter().map(String::as_str).collect();

    downloader_for(package_selection)
        .download_all_with_skip(progress_emitter(&app), &skip_refs)
        .await?;

    Ok("Runtime binaries installed successfully".to_string())
}

/// Check system dependencies (libraries required by runtime binaries)
#[tauri::command]
pub async fn check_system_dependencies() -> DependencyCheckResult {
    crate::runtime::deps::check_system_dependencies()
}
