//! MCP tools for the rest of the app: settings and ports, PHP installs,
//! database engine and tool, add-on installs and ports, and system info.
//!
//! Every change goes through the same command the UI uses, so validation,
//! restarts, and saved-settings rules are identical. Deliberately not exposed:
//! creating custom add-ons or background tasks (the user decided agents may
//! only start and stop them), the ngrok authtoken (a credential), the agent
//! connector's own settings, and anything that deletes data.

use super::handlers::{current_statuses, emit_activity, status_json};
use crate::config::AppSettings;
use serde_json::{json, Value};
use tauri::AppHandle;

fn port_arg(args: &Value, key: &str) -> Result<Option<u16>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let port = value
                .as_u64()
                .filter(|port| (1024..=65535).contains(port))
                .ok_or_else(|| format!("`{key}` must be a port from 1024 to 65535."))?;
            Ok(Some(port as u16))
        }
    }
}

fn bool_arg(args: &Value, key: &str) -> Result<Option<bool>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("`{key}` must be true or false.")),
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("`{key}` is required."))
}

// --- read ------------------------------------------------------------------

pub(super) fn settings_json() -> Value {
    let settings = AppSettings::load();
    json!({
        "ports": {
            "web": settings.web_port,
            "php_fpm": settings.php_port,
            "mysql": settings.mysql_port,
            "postgresql": settings.postgresql_port,
            "https": settings.https_port,
        },
        "auto_start_services": settings.auto_start_services,
        "auto_check_updates": settings.auto_check_updates,
        "project_domains": settings.project_domains,
        "packages": settings.package_selection,
        "language": settings.language,
        "xdebug": settings.xdebug,
        "workers": settings.workers.len(),
        "custom_addons": settings.addons.custom.len(),
        "note": "Change these with update_settings, switch_php_version, switch_database_engine, switch_database_tool, set_addon_enabled, set_addon_port, or set_xdebug.",
    })
}

/// Each configured port: free, used by the running CHAMP service, or taken
/// by another program.
pub(super) async fn check_ports(app: &AppHandle) -> Result<Value, String> {
    let settings = AppSettings::load();
    let statuses = current_statuses(app).await?;
    let own = |port: u16| {
        statuses
            .values()
            .any(|info| info.port == port && info.state.is_running())
    };
    let mut ports: Vec<(String, u16)> = vec![
        ("web".to_string(), settings.web_port),
        ("php-fpm".to_string(), settings.php_port),
        ("mysql".to_string(), settings.mysql_port),
        ("postgresql".to_string(), settings.postgresql_port),
        ("https".to_string(), settings.https_port),
    ];
    ports.extend(crate::addons::enabled_ports(&settings.addons));
    let rows: Vec<Value> = ports
        .into_iter()
        .map(|(label, port)| {
            let state = if crate::config::is_port_available(port) {
                "free"
            } else if own(port) {
                "used by CHAMP"
            } else {
                "taken by another program"
            };
            json!({ "service": label, "port": port, "state": state })
        })
        .collect();
    let conflicts = rows
        .iter()
        .filter(|row| row["state"] == "taken by another program")
        .count();
    Ok(json!({
        "ports": rows,
        "conflicts": conflicts,
        "hint": if conflicts > 0 { "Move the conflicting service with update_settings (core) or set_addon_port (add-ons), or stop the other program." } else { "No conflicts." },
    }))
}

pub(super) async fn list_php_versions() -> Result<Value, String> {
    let versions = crate::commands::php::get_installed_php_versions().await?;
    Ok(json!({
        "versions": versions,
        "note": "install_php_version downloads one (or its newest patch); switch_php_version makes an installed one active.",
    }))
}

pub(super) async fn system_info() -> Result<Value, String> {
    let metrics = crate::commands::services::get_system_metrics().await?;
    Ok(json!({
        "metrics": metrics,
        "log_doctor": crate::doctor::get_log_doctor_status().await,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "champ_version": env!("CARGO_PKG_VERSION"),
    }))
}

// --- write -----------------------------------------------------------------

/// Ports and the two startup switches. Saved through `save_settings`, which
/// keeps add-on, Xdebug, worker, and connector settings and restarts a
/// running stack on its new ports.
pub(super) async fn update_settings(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let mut settings = AppSettings::load();
    let before = settings.clone();
    for (key, field) in [
        ("web_port", &mut settings.web_port),
        ("php_port", &mut settings.php_port),
        ("mysql_port", &mut settings.mysql_port),
        ("postgresql_port", &mut settings.postgresql_port),
        ("https_port", &mut settings.https_port),
    ] {
        if let Some(port) = port_arg(args, key)? {
            *field = port;
        }
    }
    if let Some(value) = bool_arg(args, "auto_start_services")? {
        settings.auto_start_services = value;
    }
    if let Some(value) = bool_arg(args, "auto_check_updates")? {
        settings.auto_check_updates = value;
    }
    if let Some(value) = bool_arg(args, "project_domains")? {
        settings.project_domains = value;
    }
    let changed = serde_json::to_value(&settings).ok() != serde_json::to_value(&before).ok();
    if !changed {
        return Ok(json!({ "changed": false, "settings": settings_json() }));
    }
    let warnings = settings
        .validate()
        .map_err(|errors| format!("Not saved: {}", errors.join("; ")))?;
    let outcome = crate::commands::settings::save_settings(settings, app.clone()).await;
    emit_activity(app, "update_settings", None, Some("Settings"), &outcome);
    outcome?;
    Ok(json!({
        "changed": true,
        "warnings": warnings,
        "settings": settings_json(),
        "status": status_json(&current_statuses(app).await?),
        "note": "A running stack was restarted on the new ports. Update APP_URL and DB_PORT in projects that name the old ones (get_env_profiles has the new lines).",
    }))
}

pub(super) async fn install_php(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let wanted = required_str(args, "version")?.to_string();
    let versions = crate::commands::php::get_installed_php_versions().await?;
    let selected = versions
        .iter()
        .find(|version| version.id == wanted || version.version == wanted)
        .or_else(|| {
            versions.iter().find(|version| {
                version.id == format!("php-{wanted}")
                    || version.version.starts_with(&format!("{wanted}."))
            })
        })
        .ok_or_else(|| {
            format!(
                "No PHP {wanted} in the catalog. Available: {}.",
                versions
                    .iter()
                    .map(|version| version.latest_version.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    if selected.installed && !selected.update_available {
        return Ok(
            json!({ "installed": selected.id, "version": selected.version, "changed": false, "note": "Already installed and up to date. Call switch_php_version to use it." }),
        );
    }
    let id = selected.id.clone();
    let outcome = crate::commands::php::download_php_version(id.clone(), app.clone()).await;
    emit_activity(app, "install_php_version", None, Some(&id), &outcome);
    let message = outcome?;
    Ok(
        json!({ "installed": id, "changed": true, "message": message, "note": "Installed, not switched: call switch_php_version to use it." }),
    )
}

pub(super) async fn switch_database(
    app: &AppHandle,
    name: &str,
    args: &Value,
) -> Result<Value, String> {
    let (key, label) = if name == "switch_database_engine" {
        ("engine", "database engine")
    } else {
        ("tool", "database tool")
    };
    let value = required_str(args, key)?.to_string();
    let outcome = if name == "switch_database_engine" {
        crate::commands::database::switch_database_engine(value.clone(), app.clone()).await
    } else {
        crate::commands::database::switch_database_tool(value.clone(), app.clone()).await
    };
    emit_activity(app, name, None, Some(&value), &outcome);
    outcome.map_err(|error| format!("Could not switch the {label}: {error}"))?;
    Ok(json!({ "switched_to": value, "status": status_json(&current_statuses(app).await?) }))
}

pub(super) async fn install_addon(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let key = required_str(args, "addon")?.to_string();
    let kind = crate::addons::AddonKind::from_key(&key).ok_or_else(|| {
        format!("`{key}` is not a built-in add-on. Custom add-ons are installed by the user.")
    })?;
    let outcome = crate::addons::install_addon(kind, app.clone()).await;
    emit_activity(app, "install_addon", None, Some(&key), &outcome);
    let statuses = outcome?;
    Ok(json!({
        "addon": statuses.into_iter().find(|status| status.kind == key),
        "note": "Installed from a pinned, checksum-verified download. Call set_addon_enabled to run it with the stack.",
    }))
}

pub(super) async fn set_addon_port(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let key = required_str(args, "addon")?.to_string();
    let change = crate::addons::AddonChange {
        enabled: None,
        port: port_arg(args, "port")?,
        web_port: port_arg(args, "web_port")?,
    };
    if change.port.is_none() && change.web_port.is_none() {
        return Err("Give `port` and/or `web_port`.".to_string());
    }
    let outcome = crate::addons::configure_addon(key.clone(), change, app.clone()).await;
    emit_activity(app, "set_addon_port", None, Some(&key), &outcome);
    let statuses = outcome?;
    Ok(json!({
        "addon": statuses.into_iter().find(|status| status.kind == key),
        "note": "A running add-on restarted on the new port, and PHP's CHAMP_* variables were updated. Update project .env files that name the old port.",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_must_be_unprivileged_and_booleans_strict() {
        assert_eq!(port_arg(&json!({ "p": 8081 }), "p").unwrap(), Some(8081));
        assert_eq!(port_arg(&json!({}), "p").unwrap(), None);
        for bad in [
            json!({ "p": 80 }),
            json!({ "p": 70000 }),
            json!({ "p": "8080" }),
        ] {
            assert!(port_arg(&bad, "p").is_err(), "{bad}");
        }
        assert_eq!(bool_arg(&json!({ "b": true }), "b").unwrap(), Some(true));
        assert!(bool_arg(&json!({ "b": "yes" }), "b").is_err());
    }
}
