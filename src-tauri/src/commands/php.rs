//! Side-by-side PHP versions.

use super::runtime::{downloader_for, progress_emitter};
use crate::config::AppSettings;
use crate::process::runner::run_with_manager;
use crate::process::{ServiceMap, ServiceState, ServiceType};
use crate::runtime::downloader::{read_marker_version, RuntimeDownloader};
use serde::Serialize;
use tauri::AppHandle;

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

#[tauri::command]
pub async fn get_installed_php_versions() -> Result<Vec<InstalledPhpVersionDto>, String> {
    let runtime_dir = RuntimeDownloader::new().get_runtime_dir()?;
    let active_php = AppSettings::load().package_selection.php;
    let packages = crate::runtime::packages::get_available_packages();
    let legacy_version =
        read_marker_version(&crate::runtime::layout::marker_path(&runtime_dir, "php"));

    Ok(packages
        .php
        .into_iter()
        .map(|package| {
            let package_id = package.id.clone();
            let version_dir = crate::runtime::layout::php_version_dir(&runtime_dir, &package_id);
            let marker = crate::runtime::layout::php_version_marker(&runtime_dir, &package_id);
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

    let settings = AppSettings::update(|settings| settings.package_selection.php = php_id)?;

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

    let result = downloader_for(selection)
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
