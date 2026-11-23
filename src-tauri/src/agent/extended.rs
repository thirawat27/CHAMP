//! MCP tools beyond the core stack: add-ons, caught mail, projects and
//! framework env profiles, database schema, PHP info, Xdebug, and project
//! background workers.
//!
//! Same rules as the core tools: nothing here drops data, runs SQL other
//! than fixed catalog queries, or runs a command the user did not define.

use super::database;
use super::handlers::{active_database, current_statuses, emit_activity};
use crate::config::AppSettings;
use serde_json::{json, Value};
use tauri::AppHandle;

const MAX_MAIL_LIST: u64 = 50;
const MAX_MAIL_TEXT: usize = 20_000;
const MAX_LINKS: usize = 50;
const MAX_ADDON_LOG_LINES: usize = 400;

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    arg_str(args, key).ok_or_else(|| format!("`{key}` is required."))
}

async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|e| format!("Task failed: {e}"))?
}

// --- add-ons -------------------------------------------------------------

pub(super) fn addons_json() -> Value {
    crate::addons::environment_json()
}

pub(super) fn read_addon_log(args: &Value) -> Result<Value, String> {
    let id = required(args, "addon")?.to_string();
    let lines = args["lines"]
        .as_u64()
        .unwrap_or(80)
        .clamp(1, MAX_ADDON_LOG_LINES as u64) as usize;
    let status = crate::addons::statuses()?
        .into_iter()
        .find(|status| status.kind == id)
        .ok_or_else(|| format!("Unknown add-on `{id}`. Call get_addons for the names."))?;
    let Some(path) = status.log_path else {
        return Ok(
            json!({ "addon": id, "lines": [], "note": "No log yet: the add-on has not run." }),
        );
    };
    let all = crate::logs::read_tail_lines(&path).unwrap_or_default();
    let start = all.len().saturating_sub(lines);
    Ok(
        json!({ "addon": id, "file": path, "lines": all[start..], "state": status.state, "error": status.error }),
    )
}

pub(super) async fn addon_write_tool(
    app: &AppHandle,
    name: &str,
    args: &Value,
) -> Result<Value, String> {
    let id = required(args, "addon")?.to_string();
    let outcome = match name {
        "start_addon" => crate::addons::start_addon(id.clone()).await,
        "stop_addon" => crate::addons::stop_addon(id.clone()).await,
        _ => {
            let enabled = args["enabled"]
                .as_bool()
                .ok_or_else(|| "`enabled` must be true or false.".to_string())?;
            crate::addons::configure_addon(
                id.clone(),
                crate::addons::AddonChange {
                    enabled: Some(enabled),
                    ..Default::default()
                },
                app.clone(),
            )
            .await
        }
    };
    emit_activity(app, name, None, Some(&id), &outcome);
    let statuses = outcome?;
    let status = statuses.into_iter().find(|status| status.kind == id);
    Ok(json!({ "addon": status, "php_environment": addons_json()["php_environment"] }))
}

// --- mail ------------------------------------------------------------------

fn mailpit_base() -> Result<String, String> {
    crate::addons::mailpit_api_base().ok_or_else(|| {
        "Mailpit is not running. Call set_addon_enabled with addon \"mailpit\" and enabled true (or start_addon), then send the email again.".to_string()
    })
}

fn address(value: &Value) -> Value {
    json!({ "name": value["Name"], "address": value["Address"] })
}

fn message_summary(message: &Value) -> Value {
    json!({
        "id": message["ID"],
        "from": address(&message["From"]),
        "to": message["To"].as_array().map(|list| list.iter().map(address).collect::<Vec<_>>()),
        "subject": message["Subject"],
        "created": message["Created"],
        "snippet": message["Snippet"],
        "read": message["Read"],
    })
}

pub(super) async fn list_mail(args: &Value) -> Result<Value, String> {
    let base = mailpit_base()?;
    let limit = args["limit"]
        .as_u64()
        .unwrap_or(10)
        .clamp(1, MAX_MAIL_LIST)
        .to_string();
    let url = match arg_str(args, "search") {
        Some(query) => reqwest::Url::parse_with_params(
            &format!("{base}/search"),
            &[("query", query), ("limit", limit.as_str())],
        ),
        None => reqwest::Url::parse_with_params(
            &format!("{base}/messages"),
            &[("limit", limit.as_str())],
        ),
    }
    .map_err(|e| e.to_string())?;
    let body = crate::addons::mailpit_get(url.as_str()).await?;
    Ok(json!({
        "total": body["total"],
        "messages": body["messages"].as_array().map(|list| list.iter().map(message_summary).collect::<Vec<_>>()).unwrap_or_default(),
    }))
}

/// Mailpit ids are short URL-safe strings; `latest` is the newest message.
fn valid_mail_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

/// `http(s)://` links in text or HTML, deduplicated, HTML entities decoded.
pub(super) fn extract_links(texts: &[&str]) -> Vec<String> {
    let mut links: Vec<String> = Vec::new();
    for text in texts {
        let mut rest = *text;
        while let Some(start) = rest.find("http") {
            let candidate = &rest[start..];
            if !(candidate.starts_with("http://") || candidate.starts_with("https://")) {
                rest = &rest[start + 4..];
                continue;
            }
            let end = candidate
                .find(|ch: char| {
                    ch.is_whitespace() || matches!(ch, '"' | '\'' | '<' | '>' | ')' | ']')
                })
                .unwrap_or(candidate.len());
            let link = candidate[..end]
                .trim_end_matches(['.', ',', ';'])
                .replace("&amp;", "&");
            if link.len() > "https://".len() && !links.contains(&link) {
                links.push(link);
            }
            if links.len() >= MAX_LINKS {
                return links;
            }
            rest = &candidate[end..];
        }
    }
    links
}

pub(super) async fn read_mail(args: &Value) -> Result<Value, String> {
    let base = mailpit_base()?;
    let id = arg_str(args, "id").unwrap_or("latest");
    if !valid_mail_id(id) {
        return Err(format!(
            "Invalid message id `{id}`. Use an id from list_mail, or latest."
        ));
    }
    let message = crate::addons::mailpit_get(&format!("{base}/message/{id}")).await?;
    let text = message["Text"].as_str().unwrap_or_default();
    let html = message["HTML"].as_str().unwrap_or_default();
    let mut body: String = text.chars().take(MAX_MAIL_TEXT).collect();
    let truncated = text.chars().count() > MAX_MAIL_TEXT;
    if truncated {
        body.push_str("\n[truncated]");
    }
    Ok(json!({
        "id": message["ID"],
        "from": address(&message["From"]),
        "to": message["To"].as_array().map(|list| list.iter().map(address).collect::<Vec<_>>()),
        "subject": message["Subject"],
        "date": message["Date"],
        "text": body,
        "has_html": !html.is_empty(),
        "links": extract_links(&[text, html]),
        "attachments": message["Attachments"].as_array().map(|list| list.iter().map(|file| json!({ "name": file["FileName"], "type": file["ContentType"], "size": file["Size"] })).collect::<Vec<_>>()),
    }))
}

// --- projects and env profiles ---------------------------------------------

async fn installed_php_version() -> Option<String> {
    crate::commands::runtime::get_installed_versions()
        .await
        .ok()
        .and_then(|versions| versions.get("php").cloned())
}

pub(super) async fn get_project(args: &Value) -> Result<Value, String> {
    let name = required(args, "name")?.to_string();
    let lookup = name.clone();
    let info = blocking(move || crate::projects::detect(&lookup)).await?;
    let php_version = installed_php_version().await;
    let php_ok = match (&info.php_requirement, &php_version) {
        (Some(constraint), Some(version)) => crate::projects::php_satisfies(constraint, version),
        _ => None,
    };
    let profile_name = name.clone();
    let reply =
        blocking(move || crate::envprofile::profiles_for_project(Some(&profile_name))).await?;
    let recommended = reply
        .profiles
        .iter()
        .find(|profile| profile.id == reply.recommended)
        .cloned();
    let web_port = AppSettings::load().web_port;
    let mut advice: Vec<String> = Vec::new();
    if info.has_composer && !info.has_vendor {
        advice.push("Dependencies are not installed: run `composer install` in the project folder (CHAMP's terminal, Ctrl+T, has PHP on PATH).".to_string());
    }
    if info.has_env_example && !info.has_env {
        advice.push(
            "No .env yet: copy .env.example to .env, then apply the recommended env profile."
                .to_string(),
        );
    }
    if php_ok == Some(false) {
        advice.push(format!(
            "composer.json needs PHP {} but CHAMP runs {}: call switch_php_version.",
            info.php_requirement.as_deref().unwrap_or_default(),
            php_version.as_deref().unwrap_or("an unknown version")
        ));
    }
    Ok(json!({
        "project": info,
        "url": crate::projects::domain_url(&name, web_port).unwrap_or_else(|| format!("http://localhost:{web_port}/{name}/")),
        "php": { "running": php_version, "required": info.php_requirement, "satisfied": php_ok },
        "suggested_database": crate::envprofile::database_name_for(&name),
        "recommended_profile": recommended,
        "advice": advice,
    }))
}

pub(super) async fn get_env_profiles(args: &Value) -> Result<Value, String> {
    let project = arg_str(args, "project").map(str::to_string);
    let wanted = arg_str(args, "profile").map(str::to_string);
    let reply =
        blocking(move || crate::envprofile::profiles_for_project(project.as_deref())).await?;
    let mut value = serde_json::to_value(&reply).map_err(|e| e.to_string())?;
    if let Some(wanted) = wanted {
        let profile = reply
            .profiles
            .iter()
            .find(|profile| profile.id == wanted)
            .ok_or_else(|| {
                format!(
                    "Unknown profile `{wanted}`. Use one of: {}.",
                    crate::envprofile::PROFILE_IDS.join(", ")
                )
            })?;
        value["profiles"] = json!([profile]);
    }
    Ok(value)
}

// --- database schema -------------------------------------------------------

pub(super) async fn schema_tool(
    app: &AppHandle,
    name: &str,
    args: &Value,
) -> Result<Value, String> {
    let engine = database::parse_engine(args, active_database())?;
    let port = database::running_port(&current_statuses(app).await?, engine)?;
    let db = required(args, "database")?.to_string();
    let table = arg_str(args, "table").map(str::to_string);
    let describe = name == "describe_table";
    blocking(move || {
        if describe {
            let table = table.ok_or_else(|| "`table` is required.".to_string())?;
            database::describe_table(engine, port, &db, &table)
        } else {
            database::list_tables(engine, port, &db)
        }
    })
    .await
}

// --- PHP -------------------------------------------------------------------

/// A fixed script: versions, extensions, and the ini values apps trip over.
const PHP_INFO_SCRIPT: &str = "echo json_encode(['version' => PHP_VERSION, 'thread_safe' => (bool) PHP_ZTS, 'extensions' => get_loaded_extensions(), 'zend_extensions' => get_loaded_extensions(true), 'ini' => ['memory_limit' => ini_get('memory_limit'), 'upload_max_filesize' => ini_get('upload_max_filesize'), 'post_max_size' => ini_get('post_max_size'), 'max_input_vars' => ini_get('max_input_vars'), 'display_errors' => ini_get('display_errors'), 'date.timezone' => ini_get('date.timezone'), 'xdebug.mode' => ini_get('xdebug.mode')]]);";

pub(super) async fn php_info() -> Result<Value, String> {
    blocking(|| {
        let paths = crate::runtime::locator::locate_runtime_binaries()?;
        let php = crate::runtime::locator::sibling_binary(&paths.php_cgi, "php");
        if !php.is_file() {
            return Err(format!(
                "CHAMP's PHP command line was not found at {}.",
                php.display()
            ));
        }
        let mut command = std::process::Command::new(&php);
        if paths.php_ini.is_file() {
            command.arg("-c").arg(&paths.php_ini);
        }
        command.arg("-r").arg(PHP_INFO_SCRIPT);
        crate::process::command::hide_console(&mut command);
        let output = command
            .output()
            .map_err(|e| format!("Failed to run PHP: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Startup warnings may precede the JSON; it is the last line.
        let json_line = stdout
            .lines()
            .rev()
            .find(|line| line.starts_with('{'))
            .unwrap_or_default();
        let mut info: Value = serde_json::from_str(json_line).map_err(|_| {
            format!(
                "PHP did not report its configuration: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )
        })?;
        info["php_ini"] = json!(paths.php_ini);
        info["xdebug"] = serde_json::to_value(crate::xdebug::status()).unwrap_or_default();
        info["note"] = json!(
            "Values are from the PHP command line with CHAMP's php.ini, which PHP-FPM also uses."
        );
        Ok(info)
    })
    .await
}

pub(super) async fn switch_php(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let wanted = required(args, "version")?.to_string();
    let versions = crate::commands::php::get_installed_php_versions().await?;
    let installed: Vec<_> = versions
        .iter()
        .filter(|version| version.installed)
        .collect();
    let selected = installed
        .iter()
        .find(|version| version.id == wanted || version.version == wanted)
        .or_else(|| {
            installed
                .iter()
                .find(|version| version.version.starts_with(&format!("{wanted}.")))
        })
        .ok_or_else(|| {
            format!(
                "PHP {wanted} is not installed. Installed: {}. Ask the user to install it in CHAMP Settings -> PHP.",
                installed.iter().map(|version| version.version.as_str()).collect::<Vec<_>>().join(", ")
            )
        })?;
    let id = selected.id.clone();
    let outcome = crate::commands::php::switch_php_version(id.clone(), app.clone()).await;
    emit_activity(app, "switch_php_version", None, Some(&id), &outcome);
    outcome?;
    Ok(json!({ "switched_to": id, "version": selected.version }))
}

pub(super) async fn set_xdebug(app: &AppHandle, args: &Value) -> Result<Value, String> {
    let enabled = args["enabled"]
        .as_bool()
        .ok_or_else(|| "`enabled` must be true or false.".to_string())?;
    let outcome = crate::xdebug::set_enabled(app, enabled).await;
    emit_activity(app, "set_xdebug", None, Some("Xdebug"), &outcome);
    let status = outcome?;
    Ok(
        json!({ "xdebug": status, "how_to_debug": "Listen on the client port in the IDE, then add XDEBUG_TRIGGER=1 to the URL (or use the Xdebug browser extension)." }),
    )
}

// --- workers ---------------------------------------------------------------

pub(super) async fn list_workers() -> Result<Value, String> {
    let workers = blocking(crate::addons::workers::statuses).await?;
    Ok(
        json!({ "workers": workers, "note": "Background tasks are defined by the user in CHAMP's Add-on services window; agents can start and stop them but not create them." }),
    )
}

pub(super) async fn worker_write_tool(
    app: &AppHandle,
    name: &str,
    args: &Value,
) -> Result<Value, String> {
    let id = required(args, "worker")?.to_string();
    let outcome = if name == "start_worker" {
        crate::addons::workers::start_worker(id.clone()).await
    } else {
        crate::addons::workers::stop_worker(id.clone()).await
    };
    emit_activity(app, name, None, Some(&id), &outcome);
    Ok(json!({ "workers": outcome? }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_come_from_text_and_html_once() {
        let links = extract_links(&[
            "Verify: https://shop.localhost:8080/verify?id=1&sig=a.\nhttp",
            r#"<a href="https://shop.localhost:8080/verify?id=1&amp;sig=a">x</a> <img src='http://cdn.test/a.png'>"#,
        ]);
        assert_eq!(
            links,
            vec![
                "https://shop.localhost:8080/verify?id=1&sig=a".to_string(),
                "http://cdn.test/a.png".to_string(),
            ]
        );
    }

    #[test]
    fn mail_ids_are_url_safe() {
        assert!(valid_mail_id("latest"));
        assert!(valid_mail_id("aB3-x_9"));
        assert!(!valid_mail_id("../messages"));
        assert!(!valid_mail_id(""));
    }
}
