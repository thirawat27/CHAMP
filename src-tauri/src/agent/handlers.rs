//! What each MCP method and tool does.

use super::http::{
    rpc_error, rpc_result, RPC_INVALID_PARAMS, RPC_INVALID_REQUEST, RPC_METHOD_NOT_FOUND,
};
use super::tools::{tool_definitions, WRITE_TOOLS};
use super::{control, database, extended, resources, Context};
use crate::config::AppSettings;
use crate::process::runner::run_with_manager;
use crate::process::{ServiceMap, ServiceType};
use crate::runtime::locator::get_app_data_paths;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

pub(super) const SUPPORTED_PROTOCOL_VERSIONS: [&str; 4] =
    ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub(super) const SERVER_INSTRUCTIONS: &str = "CHAMP runs a local PHP stack for any framework: Caddy (web server) -> PHP-FPM -> MySQL or PostgreSQL, with phpMyAdmin or Adminer, plus optional add-ons (Redis, Mailpit, Meilisearch, user-registered custom services) and per-project background PHP tasks. \
Each project folder is served at http://localhost:<web port>/<folder>/ and, while project domains are on (the default; see get_environment), also at the root of its own origin, http://<folder>.localhost:<web port>/ (from public/ when it has one). Use the url that get_project returns as the app URL. \
Before configuring a project call get_project: it detects the framework (Laravel, Symfony, WordPress, CodeIgniter, CakePHP, Slim, Yii, Laminas, Drupal, plain PHP) and returns ready configuration lines in that framework's convention; get_env_profiles gives every convention. PHP already receives CHAMP_* environment variables, so plain PHP needs no .env. \
Use create_database, list_tables, and describe_table instead of asking the user to open phpMyAdmin. Mailpit catches all mail: list_mail / read_mail verify emails and give their links. \
When something is in the error state call diagnose_service (core) or read_addon_log (add-ons) before changing anything; for port conflicts call check_ports, then update_settings or set_addon_port. \
Everything the Settings window does is available: get_settings, update_settings (ports, startup), list_php_versions / install_php_version / switch_php_version, switch_database_engine, switch_database_tool, install_addon, set_addon_enabled, set_addon_port, set_xdebug. Custom add-ons, background tasks, and the ngrok authtoken are defined by the user only.get_https_preview / start_https_preview give a public HTTPS URL for webhooks or phone testing. \
Tools that change things are missing when the user turned on read-only mode. Nothing here drops data or runs arbitrary SQL or shell commands.";

pub(super) const READ_ONLY_REFUSAL: &str = "CHAMP's agent connector is in read-only mode, so this tool is disabled. Ask the user to turn off Read-only in CHAMP Settings -> AI agent connector, or ask them to do this step themselves.";

pub(super) const MAX_LOG_LINES: usize = 400;

pub(super) fn negotiate_protocol(params: &Value) -> &'static str {
    let requested = params["protocolVersion"].as_str().unwrap_or_default();
    SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .find(|version| **version == requested)
        .copied()
        .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0])
}

pub(super) fn initialize_result(params: &Value) -> Value {
    json!({
        "protocolVersion": negotiate_protocol(params),
        // JSON responses only (no server stream), so nothing is announced as changing.
        "capabilities": {
            "tools": { "listChanged": false },
            "resources": { "listChanged": false, "subscribe": false },
            "prompts": { "listChanged": false }
        },
        "serverInfo": { "name": "champ", "title": "CHAMP local web stack", "version": env!("CARGO_PKG_VERSION") },
        "instructions": SERVER_INSTRUCTIONS
    })
}

pub(super) fn parse_service(args: &Value) -> Result<ServiceType, String> {
    serde_json::from_value(args["service"].clone())
        .map_err(|_| "`service` must be one of caddy, php-fpm, mysql, postgresql".to_string())
}

pub(super) fn status_json(statuses: &ServiceMap) -> Value {
    let settings = AppSettings::load();
    let adminer = settings.package_selection.tool_is_adminer();
    let postgresql = settings.package_selection.is_postgresql();
    let web_port = statuses
        .get(&ServiceType::Caddy)
        .map(|service| service.port)
        .unwrap_or(settings.web_port);
    let services: Vec<Value> = [
        ServiceType::Caddy,
        ServiceType::PhpFpm,
        ServiceType::MySQL,
        ServiceType::PostgreSQL,
    ]
    .iter()
    .filter_map(|service| statuses.get(service))
    .map(|info| {
        json!({
            "service": info.service_type.key(),
            "name": info.service_type.display_name(),
            "state": info.state,
            "port": info.port,
            "error": info.error_message,
        })
    })
    .collect();

    json!({
        "services": services,
        "active_database": if postgresql { "postgresql" } else { "mysql" },
        "web_url": format!("http://localhost:{web_port}/"),
        "database_tool": {
            "name": if adminer { "Adminer" } else { "phpMyAdmin" },
            "url": format!("http://localhost:{web_port}/{}", if adminer { "adminer" } else { "phpmyadmin" }),
        },
        "https_preview": crate::tunnel::get_https_tunnel_status().ok(),
    })
}

pub(super) async fn current_statuses(app: &AppHandle) -> Result<ServiceMap, String> {
    run_with_manager(app, |manager| {
        manager.update_health();
        manager.get_all_statuses()
    })
    .await
}

pub(super) async fn environment_json(app: &AppHandle) -> Result<Value, String> {
    let statuses = current_statuses(app).await?;
    let settings = AppSettings::load();
    let paths = get_app_data_paths()?;
    let port = |service: ServiceType, fallback: u16| {
        statuses
            .get(&service)
            .map(|info| info.port)
            .unwrap_or(fallback)
    };
    let web_port = port(ServiceType::Caddy, settings.web_port);
    let mysql_port = port(ServiceType::MySQL, settings.mysql_port);
    let postgresql_port = port(ServiceType::PostgreSQL, settings.postgresql_port);
    let dotenv = |engine, port| database::connection_info(engine, port, None)["dotenv"].clone();
    let versions = crate::commands::runtime::get_installed_versions()
        .await
        .unwrap_or_default();

    Ok(json!({
        "web": {
            "url": format!("http://localhost:{web_port}/"),
            "document_root": paths.projects_dir,
            "project_url_pattern": format!("http://localhost:{web_port}/<project-folder>/"),
            "project_domains_enabled": settings.project_domains,
            "project_domain_pattern": settings.project_domains.then(|| format!("http://<project-folder>.localhost:{web_port}/")),
            "project_domain_note": if settings.project_domains {
                "Each folder whose name is a DNS label is also served at the root of its own origin, from public/ when the folder has public/index.php or public/index.html (Laravel, Symfony). Set APP_URL to this URL. Browsers and curl resolve *.localhost themselves; server-side code on Windows may not."
            } else {
                "Project domains are off (Settings -> General), so projects open only at the path URL; set APP_URL to it. Ask the user to turn domains on if the app needs to run at the root of its own origin."
            },
            "server": "Caddy with .htaccess rewrite compatibility",
        },
        "php": {
            "selected_package": settings.package_selection.php,
            "version": versions.get("php"),
            "fpm_address": format!("127.0.0.1:{}", port(ServiceType::PhpFpm, settings.php_port)),
        },
        "databases": {
            "mysql": {
                "host": "127.0.0.1",
                "port": mysql_port,
                "user": "root",
                "password": "",
                "version": versions.get("mysql"),
                "dotenv": dotenv(ServiceType::MySQL, mysql_port),
            },
            "postgresql": {
                "host": "127.0.0.1",
                "port": postgresql_port,
                "user": "postgres",
                "password": "",
                "auth": "trust (local connections only)",
                "version": versions.get("postgresql"),
                "dotenv": dotenv(ServiceType::PostgreSQL, postgresql_port),
            },
            "active": if settings.package_selection.is_postgresql() { "postgresql" } else { "mysql" },
            "note": "Use host 127.0.0.1 rather than localhost so drivers connect over TCP.",
        },
        "folders": {
            "projects": paths.projects_dir,
            "logs": paths.logs_dir,
            "config": paths.config_dir,
            "runtime": paths.runtime_dir,
        },
        "installed_versions": versions,
        "addons": crate::addons::environment_json(),
        "xdebug": crate::xdebug::status(),
        "env_profiles_hint": "Call get_project <name> or get_env_profiles for ready configuration in Laravel, Symfony, WordPress, CodeIgniter, CakePHP, generic DSN, or CHAMP getenv form.",
    }))
}

pub(super) fn list_projects_json() -> Result<Value, String> {
    let paths = get_app_data_paths()?;
    let settings = AppSettings::load();
    let web_port = settings.web_port;
    let domains_enabled = settings.project_domains;
    let mut projects: Vec<(String, PathBuf)> = fs::read_dir(&paths.projects_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().to_string();
                    (!name.starts_with('.')).then(|| (name, entry.path()))
                })
                .collect()
        })
        .unwrap_or_default();
    projects.sort_by_key(|(name, _)| name.to_lowercase());

    Ok(json!({
        "projects_folder": paths.projects_dir,
        "projects": projects
            .into_iter()
            .map(|(name, path)| {
                let domain_url =
                    crate::projects::enabled_domain_url(&name, web_port, domains_enabled);
                let info = crate::projects::inspect(&path);
                json!({
                    "name": name,
                    "path": path,
                    "url": format!("http://localhost:{web_port}/{name}/"),
                    // The project at the root of its own origin (served from
                    // public/ when it has one); null when the name is not a DNS label.
                    "domain_url": domain_url,
                    "framework": info.framework,
                    "web_root": info.web_root,
                })
            })
            .collect::<Vec<_>>(),
    }))
}

pub(super) fn read_logs_json(service: ServiceType, lines: usize) -> Result<Value, String> {
    let paths = get_app_data_paths()?;
    let files: Vec<Value> = crate::doctor::service_log_files(service, &paths.logs_dir)
        .into_iter()
        .filter_map(|path| {
            let all = crate::logs::read_tail_lines(&path)?;
            let start = all.len().saturating_sub(lines);
            Some(json!({ "file": path, "lines": all[start..] }))
        })
        .collect();
    Ok(json!({ "service": service.key(), "files": files }))
}

pub(super) async fn diagnose_json(
    app: &AppHandle,
    service: ServiceType,
) -> Result<(Value, String), String> {
    let statuses = current_statuses(app).await?;
    let error_message = statuses
        .get(&service)
        .and_then(|info| info.error_message.clone());
    let diagnosis = tauri::async_runtime::spawn_blocking(move || {
        crate::doctor::diagnose(service, error_message.as_deref())
    })
    .await
    .map_err(|e| format!("Log Doctor task failed: {}", e))??;

    let mut summary = format!("Log Doctor for {}:\n", service.display_name());
    if diagnosis.matches.is_empty() {
        summary.push_str("No known cause found in the recent log lines.\n");
    }
    for found in &diagnosis.matches {
        summary.push_str(&format!(
            "\n- {} ({:.0}% confidence)\n  Cause: {}\n  Evidence: {}\n  Fix:\n",
            found.title.en,
            found.confidence * 100.0,
            found.cause.en,
            found.evidence
        ));
        for (index, step) in found.steps.en.iter().enumerate() {
            summary.push_str(&format!("    {}. {}\n", index + 1, step));
        }
    }
    let value = serde_json::to_value(&diagnosis).map_err(|e| e.to_string())?;
    Ok((value, summary))
}

pub(super) async fn run_write_tool(
    app: &AppHandle,
    name: &str,
    args: &Value,
) -> Result<Value, String> {
    let service = match name {
        "start_service" | "stop_service" | "restart_service" => Some(parse_service(args)?),
        _ => None,
    };
    let action = name.to_string();
    // The stack tools cover the enabled add-ons too, like the dashboard buttons.
    if matches!(name, "stop_stack" | "restart_stack") {
        crate::addons::stop_all().await;
    }
    let result = run_with_manager(app, move |manager| {
        let result = match (action.as_str(), service) {
            ("start_stack", _) => manager.start_all(),
            ("stop_stack", _) => {
                let _ = crate::tunnel::stop_https_tunnel();
                manager.stop_stack()
            }
            ("restart_stack", _) => manager.restart_all(),
            ("start_service", Some(service)) => manager.start(service),
            ("stop_service", Some(service)) => manager.stop(service),
            ("restart_service", Some(service)) => manager.restart(service),
            _ => Err("Unknown tool".to_string()),
        };
        manager.update_health();
        (result, manager.get_all_statuses())
    })
    .await?;

    if matches!(name, "start_stack" | "restart_stack") {
        crate::addons::start_enabled(app).await;
    }
    let (outcome, statuses) = result;
    emit_activity(app, name, service, None, &outcome);
    outcome?;
    Ok(status_json(&statuses))
}

/// Tell the dashboard about an agent action. `target` names non-service targets.
pub(super) fn emit_activity<T>(
    app: &AppHandle,
    tool: &str,
    service: Option<ServiceType>,
    target: Option<&str>,
    outcome: &Result<T, String>,
) {
    let _ = app.emit(
        "agent-activity",
        json!({ "tool": tool, "service": service.map(|service| service.key()), "target": target, "ok": outcome.is_ok(), "error": outcome.as_ref().err() }),
    );
}

pub(super) fn active_database() -> ServiceType {
    if AppSettings::load().package_selection.is_postgresql() {
        ServiceType::PostgreSQL
    } else {
        ServiceType::MySQL
    }
}

pub(super) async fn database_tool(
    app: &AppHandle,
    name: &str,
    args: &Value,
) -> Result<Value, String> {
    let engine = database::parse_engine(args, active_database())?;
    let port = database::running_port(&current_statuses(app).await?, engine)?;
    let create = name == "create_database";
    let db_name = args["name"].as_str().unwrap_or_default().trim().to_string();
    if create {
        database::validate_database_name(&db_name)?;
    }
    let task_name = db_name.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        if create {
            database::create_database(engine, port, &task_name)
        } else {
            database::list_databases(engine, port)
        }
    })
    .await
    .map_err(|e| format!("Database task failed: {}", e))?;
    if create {
        emit_activity(app, name, Some(engine), Some(&db_name), &outcome);
    }
    outcome
}

pub(super) async fn https_preview_tool(app: &AppHandle, name: &str) -> Result<Value, String> {
    let outcome = match name {
        "start_https_preview" => crate::commands::services::start_https_tunnel(app.clone()).await,
        "stop_https_preview" => crate::tunnel::stop_https_tunnel(),
        _ => crate::tunnel::get_https_tunnel_status(),
    };
    if name != "get_https_preview" {
        emit_activity(app, name, None, Some("HTTPS Preview"), &outcome);
    }
    let status = outcome?;
    let mut value = serde_json::to_value(&status).map_err(|e| e.to_string())?;
    if !status.authtoken_configured {
        value["hint"] = json!("No ngrok authtoken is saved. Ask the user to paste their free ngrok authtoken in CHAMP's HTTPS Preview panel.");
    }
    Ok(value)
}

/// Execute a tool and wrap it as an MCP `CallToolResult`.
pub(super) async fn call_tool(
    ctx: &Context,
    name: &str,
    args: &Value,
) -> Result<Value, (i64, String)> {
    let known = tool_definitions(false)
        .iter()
        .any(|tool| tool["name"].as_str() == Some(name));
    if !known {
        return Err((
            RPC_INVALID_PARAMS,
            format!("Unknown tool: {name}. Call tools/list for the available CHAMP tools."),
        ));
    }
    if ctx.read_only && WRITE_TOOLS.contains(&name) {
        return Ok(tool_error(READ_ONLY_REFUSAL));
    }

    let outcome: Result<(Value, Option<String>), String> = match name {
        "get_status" => current_statuses(&ctx.app)
            .await
            .map(|s| (status_json(&s), None)),
        "get_environment" => environment_json(&ctx.app).await.map(|v| (v, None)),
        "list_projects" => list_projects_json().map(|v| (v, None)),
        "read_logs" => parse_service(args).and_then(|service| {
            let lines = args["lines"]
                .as_u64()
                .unwrap_or(80)
                .clamp(1, MAX_LOG_LINES as u64) as usize;
            read_logs_json(service, lines).map(|v| (v, None))
        }),
        "diagnose_service" => match parse_service(args) {
            Ok(service) => diagnose_json(&ctx.app, service)
                .await
                .map(|(v, text)| (v, Some(text))),
            Err(error) => Err(error),
        },
        "list_databases" | "create_database" => {
            database_tool(&ctx.app, name, args).await.map(|v| (v, None))
        }
        "get_https_preview" | "start_https_preview" | "stop_https_preview" => {
            https_preview_tool(&ctx.app, name).await.map(|v| (v, None))
        }
        "get_project" => extended::get_project(args).await.map(|v| (v, None)),
        "get_env_profiles" => extended::get_env_profiles(args).await.map(|v| (v, None)),
        "list_tables" | "describe_table" => extended::schema_tool(&ctx.app, name, args)
            .await
            .map(|v| (v, None)),
        "get_addons" => Ok((extended::addons_json(), None)),
        "read_addon_log" => extended::read_addon_log(args).map(|v| (v, None)),
        "list_mail" => extended::list_mail(args).await.map(|v| (v, None)),
        "read_mail" => extended::read_mail(args).await.map(|v| (v, None)),
        "get_php_info" => extended::php_info().await.map(|v| (v, None)),
        "list_workers" => extended::list_workers().await.map(|v| (v, None)),
        "start_addon" | "stop_addon" | "set_addon_enabled" => {
            extended::addon_write_tool(&ctx.app, name, args)
                .await
                .map(|v| (v, None))
        }
        "switch_php_version" => extended::switch_php(&ctx.app, args)
            .await
            .map(|v| (v, None)),
        "set_xdebug" => extended::set_xdebug(&ctx.app, args)
            .await
            .map(|v| (v, None)),
        "get_settings" => Ok((control::settings_json(), None)),
        "check_ports" => control::check_ports(&ctx.app).await.map(|v| (v, None)),
        "list_php_versions" => control::list_php_versions().await.map(|v| (v, None)),
        "get_system_info" => control::system_info().await.map(|v| (v, None)),
        "update_settings" => control::update_settings(&ctx.app, args)
            .await
            .map(|v| (v, None)),
        "install_php_version" => control::install_php(&ctx.app, args)
            .await
            .map(|v| (v, None)),
        "switch_database_engine" | "switch_database_tool" => {
            control::switch_database(&ctx.app, name, args)
                .await
                .map(|v| (v, None))
        }
        "install_addon" => control::install_addon(&ctx.app, args)
            .await
            .map(|v| (v, None)),
        "set_addon_port" => control::set_addon_port(&ctx.app, args)
            .await
            .map(|v| (v, None)),
        "start_worker" | "stop_worker" => extended::worker_write_tool(&ctx.app, name, args)
            .await
            .map(|v| (v, None)),
        _ => run_write_tool(&ctx.app, name, args)
            .await
            .map(|v| (v, None)),
    };

    Ok(match outcome {
        Ok((structured, text)) => {
            let text = text
                .unwrap_or_else(|| serde_json::to_string_pretty(&structured).unwrap_or_default());
            json!({ "content": [{ "type": "text", "text": text }], "structuredContent": structured, "isError": false })
        }
        Err(error) => tool_error(&error),
    })
}

pub(super) fn tool_error(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

/// Handle one JSON-RPC message. `None` means no response body (notification).
pub(super) async fn respond(ctx: &Context, message: &Value) -> Option<Value> {
    if message.is_array() {
        return Some(rpc_error(
            Value::Null,
            RPC_INVALID_REQUEST,
            "Batch requests are not supported",
        ));
    }
    let id = message.get("id").cloned()?;
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let reply = match message["method"].as_str().unwrap_or_default() {
        "initialize" => rpc_result(id, initialize_result(&params)),
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(id, json!({ "tools": tool_definitions(ctx.read_only) })),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or_default();
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match call_tool(ctx, name, &args).await {
                Ok(result) => rpc_result(id, result),
                Err((code, message)) => rpc_error(id, code, &message),
            }
        }
        "resources/list" => rpc_result(id, resources::list_resources()),
        "resources/templates/list" => rpc_result(id, resources::list_templates()),
        "resources/read" => {
            let uri = params["uri"].as_str().unwrap_or_default();
            match resources::read_resource(ctx, uri).await {
                Ok(result) => rpc_result(id, result),
                Err(message) => rpc_error(id, RPC_INVALID_PARAMS, &message),
            }
        }
        "prompts/list" => rpc_result(id, resources::list_prompts()),
        "prompts/get" => {
            let name = params["name"].as_str().unwrap_or_default();
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match resources::get_prompt(name, &args) {
                Ok(result) => rpc_result(id, result),
                Err(message) => rpc_error(id, RPC_INVALID_PARAMS, &message),
            }
        }
        _ => rpc_error(id, RPC_METHOD_NOT_FOUND, "Method not found"),
    };
    Some(reply)
}
