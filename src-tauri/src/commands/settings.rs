//! Saved settings and port checks.

use super::database::ensure_selected_database_tool_installed;
use crate::config::AppSettings;
use crate::process::runner::run_with_manager;
use crate::process::{ServiceState, ServiceType};
use serde::Serialize;
use tauri::AppHandle;

/// Data transfer object for language and sound preferences.
#[derive(Debug, Serialize)]
pub struct LanguageSettingsDto {
    pub language: String,
    pub sound_enabled: bool,
}

/// Get app settings
#[tauri::command]
pub async fn get_settings() -> Result<AppSettings, String> {
    Ok(AppSettings::load())
}

/// Save app settings
#[tauri::command]
pub async fn save_settings(mut settings: AppSettings, app: AppHandle) -> Result<(), String> {
    settings.package_selection = settings.package_selection.normalized();
    settings.package_selection.validate_path_ids()?;

    ensure_selected_database_tool_installed(&settings.package_selection, &app).await?;

    // Save the settings after any required runtime install succeeds. Language,
    // sound, and the agent connector have dedicated commands that write
    // immediately. The Settings panel holds a snapshot loaded when it opened, and
    // the install above can take minutes, so keep the values on disk right now
    // instead of reverting a change made in the meantime.
    let settings = AppSettings::update(|stored| {
        settings.language = stored.language.clone();
        settings.sound_enabled = stored.sound_enabled;
        settings.agent_connector_enabled = stored.agent_connector_enabled;
        settings.agent_connector_read_only = stored.agent_connector_read_only;
        settings.agent_connector_port = stored.agent_connector_port;
        *stored = settings;
    })?;

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

    AppSettings::update(|settings| settings.language = language).map(|_| ())
}

/// Save whether UI sound effects are enabled.
#[tauri::command]
pub async fn save_sound_setting(enabled: bool) -> Result<(), String> {
    AppSettings::update(|settings| settings.sound_enabled = enabled).map(|_| ())
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
