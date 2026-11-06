//! Switching the database engine and web database manager.

use super::runtime::{downloader_for, progress_emitter};
use crate::config::AppSettings;
use crate::process::runner::run_with_manager;
use crate::process::{ServiceState, ServiceType};
use crate::runtime::downloader::{is_database_tool_installed, is_selected_database_installed};
use crate::runtime::packages::PackageSelection;
use tauri::AppHandle;

pub(super) async fn ensure_selected_database_tool_installed(
    selection: &PackageSelection,
    app: &AppHandle,
) -> Result<(), String> {
    let tool_id = &selection.phpmyadmin;
    let packages = crate::runtime::packages::get_available_packages();
    let package = packages
        .phpmyadmin
        .iter()
        .find(|package| package.id == *tool_id)
        .ok_or_else(|| format!("Unknown database tool: {}", tool_id))?;

    let postgresql = selection.is_postgresql();
    let downloader = downloader_for(selection.clone());
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

    let mut selection = AppSettings::load().package_selection;
    if selection.database_engine == engine {
        return Ok(());
    }
    selection.database_engine = engine;
    apply_database_selection(selection.normalized(), &app).await
}

/// Choose the web database manager: `phpmyadmin` (MySQL only) or `adminer`
/// (MySQL and PostgreSQL), independently of the engine.
#[tauri::command]
pub async fn switch_database_tool(tool: String, app: AppHandle) -> Result<(), String> {
    if tool != "phpmyadmin" && tool != "adminer" {
        return Err(format!("Unsupported database tool: {}", tool));
    }

    let mut selection = AppSettings::load().package_selection;
    if tool == "phpmyadmin" && selection.is_postgresql() {
        return Err("phpMyAdmin only supports MySQL. Use Adminer with PostgreSQL.".to_string());
    }
    if selection.phpmyadmin.starts_with(&tool) {
        return Ok(());
    }
    selection.phpmyadmin = tool;
    apply_database_selection(selection.normalized(), &app).await
}

/// Install what the new database choice needs, save it, and move the stack
/// onto it (restarting it only if it was running).
pub(super) async fn apply_database_selection(
    selection: PackageSelection,
    app: &AppHandle,
) -> Result<(), String> {
    ensure_selected_database_tool_installed(&selection, app).await?;
    let settings = AppSettings::update(|settings| settings.package_selection = selection)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
    fn database_tool_detection_matches_tool_layout() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        assert!(!is_database_tool_installed(temp.path(), "adminer-5.4"));

        fs::create_dir_all(temp.path().join("adminer")).unwrap();
        fs::write(temp.path().join("adminer").join("index.php"), "<?php").unwrap();
        assert!(is_database_tool_installed(temp.path(), "adminer-5.4"));
        assert!(!is_database_tool_installed(temp.path(), "phpmyadmin-5.2"));
    }
}
