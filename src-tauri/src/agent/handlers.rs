//! What each MCP method and tool does.

use super::http::{
    rpc_error, rpc_result, RPC_INVALID_PARAMS, RPC_INVALID_REQUEST, RPC_METHOD_NOT_FOUND,
};
use super::tools::{tool_definitions, WRITE_TOOLS};
use super::{database, Context};
use crate::config::AppSettings;
use crate::process::runner::run_with_manager;
use crate::process::{ServiceMap, ServiceType};
use crate::runtime::locator::get_app_data_paths;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

pub(super) const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] =
    ["2025-06-18", "2025-03-26", "2024-11-05"];

pub(super) const SERVER_INSTRUCTIONS: &str = "CHAMP runs a local web stack: Caddy (web server) -> PHP-FPM -> MySQL or PostgreSQL, with phpMyAdmin or Adminer. Call get_environment before writing app configuration (database host, port, user, URLs, ready-made .env lines). Projects live in the projects folder and are served at http://localhost:<web port>/<folder>/ (list_projects). Use list_databases / create_database instead of asking the user to open phpMyAdmin. When a service is in the error state, call diagnose_service for the cause and fix, then read_logs for details. get_https_preview / start_https_preview give a public HTTPS URL (ngrok) for webhooks or phone testing. Tools that change things are missing when the user turned on read-only mode.";

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
        "capabilities": { "tools": { "listChanged": false } },
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
    }))
}

pub(super) fn list_projects_json() -> Result<Value, String> {
    let paths = get_app_data_paths()?;
    let web_port = AppSettings::load().web_port;
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
            .map(|(name, path)| json!({ "name": name, "path": path, "url": format!("http://localhost:{web_port}/{name}/") }))
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
        _ => rpc_error(id, RPC_METHOD_NOT_FOUND, "Method not found"),
    };
    Some(reply)
}
