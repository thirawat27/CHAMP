//! AI Agent Connector: a local MCP (Model Context Protocol) server so coding
//! agents such as Claude Code, Cursor, or Codex can inspect and control the
//! CHAMP stack ("what is the MySQL port?", "restart PHP", "why did Caddy fail?").
//!
//! Transport: MCP Streamable HTTP, JSON responses only (no server-initiated
//! stream), on `http://127.0.0.1:<port>/mcp`.
//!
//! Security model:
//! - Off by default and bound to 127.0.0.1 only.
//! - Every request needs `Authorization: Bearer <token>`: 256 random bits kept
//!   in `config/agent-token` and compared in constant time.
//! - Requests with an `Origin` header are refused. Agents never send one, but
//!   browsers always do on cross-site requests, so web pages cannot reach the
//!   server through CSRF or DNS rebinding.
//! - Read-only mode hides and refuses every tool that changes services.
//! - The UI is told about each agent action ("agent-activity" event).

mod database;

use crate::commands::run_with_manager;
use crate::config::AppSettings;
use crate::process::{ServiceMap, ServiceType};
use crate::runtime::locator::get_app_data_paths;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::header::{
    HeaderMap, HeaderValue, ALLOW, AUTHORIZATION, CONTENT_TYPE, ORIGIN, WWW_AUTHENTICATE,
};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::fs;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_LOG_LINES: usize = 400;
const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
const SERVER_INSTRUCTIONS: &str = "CHAMP runs a local web stack: Caddy (web server) -> PHP-FPM -> MySQL or PostgreSQL, with phpMyAdmin or Adminer. Call get_environment before writing app configuration (database host, port, user, URLs, ready-made .env lines). Projects live in the projects folder and are served at http://localhost:<web port>/<folder>/ (list_projects). Use list_databases / create_database instead of asking the user to open phpMyAdmin. When a service is in the error state, call diagnose_service for the cause and fix, then read_logs for details. get_https_preview / start_https_preview give a public HTTPS URL (ngrok) for webhooks or phone testing. Tools that change things are missing when the user turned on read-only mode.";
const READ_ONLY_REFUSAL: &str = "CHAMP's agent connector is in read-only mode, so this tool is disabled. Ask the user to turn off Read-only in CHAMP Settings -> AI agent connector, or ask them to do this step themselves.";

type HttpResponse = Response<Full<Bytes>>;

struct RunningServer {
    cancel: CancellationToken,
    port: u16,
    read_only: bool,
}

struct Context {
    app: AppHandle,
    token: String,
    read_only: bool,
}

static SERVER: Mutex<Option<RunningServer>> = Mutex::new(None);
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Serialize)]
pub struct AgentStatus {
    pub enabled: bool,
    pub running: bool,
    pub read_only: bool,
    pub port: u16,
    pub url: String,
    pub token: Option<String>,
    pub error: Option<String>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn token_path() -> Result<PathBuf, String> {
    Ok(get_app_data_paths()?.config_dir.join("agent-token"))
}

fn generate_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("Failed to generate token: {}", e))?;
    Ok(hex::encode(bytes))
}

fn write_token(token: &str) -> Result<(), String> {
    let path = token_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config folder: {}", e))?;
    }
    fs::write(&path, token).map_err(|e| format!("Failed to save agent token: {}", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn load_or_create_token() -> Result<String, String> {
    let path = token_path()?;
    if let Ok(token) = fs::read_to_string(&path) {
        let token = token.trim().to_string();
        if token.len() == 64 {
            return Ok(token);
        }
    }
    let token = generate_token()?;
    write_token(&token)?;
    Ok(token)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Reject browser-originated and unauthenticated requests.
fn check_headers(headers: &HeaderMap, token: &str) -> Result<(), (StatusCode, &'static str)> {
    if headers.contains_key(ORIGIN) {
        return Err((StatusCode::FORBIDDEN, "Browser requests are not allowed"));
    }
    let presented = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if constant_time_eq(presented.trim().as_bytes(), token.as_bytes()) {
        Ok(())
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            "Missing or invalid bearer token. Copy the setup again from CHAMP Settings -> AI agent connector (the token changes when you create a new one).",
        ))
    }
}

fn text_response(status: StatusCode, message: &str) -> HttpResponse {
    let mut response = Response::new(Full::new(Bytes::from(message.to_string())));
    *response.status_mut() = status;
    response
}

fn json_response(body: &Value) -> HttpResponse {
    let mut response = Response::new(Full::new(Bytes::from(body.to_string())));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn service_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["caddy", "php-fpm", "mysql", "postgresql"],
        "description": "caddy = web server, php-fpm = PHP, mysql / postgresql = database"
    })
}

fn tool(
    name: &str,
    title: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    read_only: bool,
) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
        "annotations": { "readOnlyHint": read_only, "destructiveHint": false, "openWorldHint": false }
    })
}

fn engine_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["mysql", "postgresql"],
        "description": "Database engine. Defaults to the database selected in CHAMP."
    })
}

/// Tools that change the stack; hidden and refused in read-only mode.
const WRITE_TOOLS: [&str; 9] = [
    "start_stack",
    "stop_stack",
    "restart_stack",
    "start_service",
    "stop_service",
    "restart_service",
    "create_database",
    "start_https_preview",
    "stop_https_preview",
];

fn tool_definitions(read_only: bool) -> Vec<Value> {
    let none = json!({});
    let service = json!({ "service": service_schema() });
    let engine = json!({ "engine": engine_schema() });
    let mut tools = vec![
        tool("get_status", "Stack status", "State, port, and last error of every CHAMP service, plus the website, database tool, and HTTPS preview URLs.", none.clone(), &[], true),
        tool("get_environment", "Development environment", "Everything needed to configure an app for CHAMP: database host/port/user/password, PHP version, document root, project URL pattern, folders, and installed versions.", none.clone(), &[], true),
        tool("list_projects", "List projects", "Project folders served by CHAMP with their local URLs.", none.clone(), &[], true),
        tool(
            "read_logs",
            "Read service logs",
            "Last lines of a service's log files.",
            json!({ "service": service_schema(), "lines": { "type": "integer", "minimum": 1, "maximum": MAX_LOG_LINES, "default": 80 } }),
            &["service"],
            true,
        ),
        tool("diagnose_service", "Diagnose a service", "Run CHAMP Log Doctor on a service: likely cause, fix steps, and the matching log line.", service.clone(), &["service"], true),
        tool("list_databases", "List databases", "User databases on MySQL or PostgreSQL (system databases hidden) plus connection details. The database must be running.", engine, &[], true),
        tool("get_https_preview", "HTTPS Preview status", "Whether the public HTTPS Preview (ngrok) is running, its public URL once ready, and whether an ngrok authtoken is saved.", none.clone(), &[], true),
    ];
    if !read_only {
        tools.extend([
            tool(
                "start_stack",
                "Start stack",
                "Start Caddy, PHP-FPM, and the selected database.",
                none.clone(),
                &[],
                false,
            ),
            tool(
                "stop_stack",
                "Stop stack",
                "Stop every CHAMP service and the HTTPS preview.",
                none.clone(),
                &[],
                false,
            ),
            tool(
                "restart_stack",
                "Restart stack",
                "Restart Caddy, PHP-FPM, and the selected database.",
                none.clone(),
                &[],
                false,
            ),
            tool(
                "start_service",
                "Start service",
                "Start one service.",
                service.clone(),
                &["service"],
                false,
            ),
            tool(
                "stop_service",
                "Stop service",
                "Stop one service.",
                service.clone(),
                &["service"],
                false,
            ),
            tool(
                "restart_service",
                "Restart service",
                "Restart one service.",
                service,
                &["service"],
                false,
            ),
            tool(
                "create_database",
                "Create database",
                "Create an empty database (utf8mb4 on MySQL) if it does not exist yet, and return its .env lines and connection URL. Nothing is dropped or overwritten.",
                json!({
                    "name": { "type": "string", "pattern": "^[A-Za-z_][A-Za-z0-9_]{0,62}$", "description": "Database name, for example my_app" },
                    "engine": engine_schema(),
                }),
                &["name"],
                false,
            ),
            tool(
                "start_https_preview",
                "Start HTTPS Preview",
                "Start the stack if needed and open a public HTTPS URL (ngrok) to the local website. Needs an ngrok authtoken saved in CHAMP.",
                none.clone(),
                &[],
                false,
            ),
            tool(
                "stop_https_preview",
                "Stop HTTPS Preview",
                "Close the public HTTPS URL.",
                none,
                &[],
                false,
            ),
        ]);
    }
    tools
}

fn negotiate_protocol(params: &Value) -> &'static str {
    let requested = params["protocolVersion"].as_str().unwrap_or_default();
    SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .find(|version| **version == requested)
        .copied()
        .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0])
}

fn initialize_result(params: &Value) -> Value {
    json!({
        "protocolVersion": negotiate_protocol(params),
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "champ", "title": "CHAMP local web stack", "version": env!("CARGO_PKG_VERSION") },
        "instructions": SERVER_INSTRUCTIONS
    })
}

fn service_key(service: ServiceType) -> &'static str {
    match service {
        ServiceType::Caddy => "caddy",
        ServiceType::PhpFpm => "php-fpm",
        ServiceType::MySQL => "mysql",
        ServiceType::PostgreSQL => "postgresql",
    }
}

fn parse_service(args: &Value) -> Result<ServiceType, String> {
    serde_json::from_value(args["service"].clone())
        .map_err(|_| "`service` must be one of caddy, php-fpm, mysql, postgresql".to_string())
}

fn status_json(statuses: &ServiceMap) -> Value {
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
            "service": service_key(info.service_type),
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

async fn current_statuses(app: &AppHandle) -> Result<ServiceMap, String> {
    run_with_manager(app, |manager| {
        manager.update_health();
        manager.get_all_statuses()
    })
    .await
}

async fn environment_json(app: &AppHandle) -> Result<Value, String> {
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
    let versions = crate::commands::get_installed_versions()
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

fn list_projects_json() -> Result<Value, String> {
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

fn read_logs_json(service: ServiceType, lines: usize) -> Result<Value, String> {
    let paths = get_app_data_paths()?;
    let files: Vec<Value> = crate::doctor::service_log_files(service, &paths.logs_dir)
        .into_iter()
        .filter_map(|path| {
            let all = crate::doctor::read_tail_lines(&path)?;
            let start = all.len().saturating_sub(lines);
            Some(json!({ "file": path, "lines": all[start..] }))
        })
        .collect();
    Ok(json!({ "service": service_key(service), "files": files }))
}

async fn diagnose_json(app: &AppHandle, service: ServiceType) -> Result<(Value, String), String> {
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

async fn run_write_tool(app: &AppHandle, name: &str, args: &Value) -> Result<Value, String> {
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
fn emit_activity<T>(
    app: &AppHandle,
    tool: &str,
    service: Option<ServiceType>,
    target: Option<&str>,
    outcome: &Result<T, String>,
) {
    let _ = app.emit(
        "agent-activity",
        json!({ "tool": tool, "service": service.map(service_key), "target": target, "ok": outcome.is_ok(), "error": outcome.as_ref().err() }),
    );
}

fn active_database() -> ServiceType {
    if AppSettings::load().package_selection.is_postgresql() {
        ServiceType::PostgreSQL
    } else {
        ServiceType::MySQL
    }
}

async fn database_tool(app: &AppHandle, name: &str, args: &Value) -> Result<Value, String> {
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

async fn https_preview_tool(app: &AppHandle, name: &str) -> Result<Value, String> {
    let outcome = match name {
        "start_https_preview" => crate::commands::start_https_tunnel(app.clone()).await,
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
async fn call_tool(ctx: &Context, name: &str, args: &Value) -> Result<Value, (i64, String)> {
    let known = tool_definitions(false)
        .iter()
        .any(|tool| tool["name"].as_str() == Some(name));
    if !known {
        return Err((
            -32602,
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

fn tool_error(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

/// Handle one JSON-RPC message. `None` means no response body (notification).
async fn respond(ctx: &Context, message: &Value) -> Option<Value> {
    if message.is_array() {
        return Some(rpc_error(
            Value::Null,
            -32600,
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
        _ => rpc_error(id, -32601, "Method not found"),
    };
    Some(reply)
}

async fn handle(request: Request<Incoming>, ctx: Arc<Context>) -> HttpResponse {
    if request.uri().path() != "/mcp" {
        return text_response(
            StatusCode::NOT_FOUND,
            "Not found. The CHAMP MCP endpoint is /mcp",
        );
    }
    if let Err((status, message)) = check_headers(request.headers(), &ctx.token) {
        let mut response = text_response(status, message);
        if status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                WWW_AUTHENTICATE,
                HeaderValue::from_static("Bearer realm=\"CHAMP\""),
            );
        }
        return response;
    }
    if request.method() != Method::POST {
        let mut response = text_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "Use POST (MCP Streamable HTTP, JSON responses only)",
        );
        response
            .headers_mut()
            .insert(ALLOW, HeaderValue::from_static("POST"));
        return response;
    }

    let body = match Limited::new(request.into_body(), MAX_BODY_BYTES)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return text_response(StatusCode::PAYLOAD_TOO_LARGE, "Request too large"),
    };
    let message: Value = match serde_json::from_slice(&body) {
        Ok(message) => message,
        Err(_) => return json_response(&rpc_error(Value::Null, -32700, "Parse error")),
    };

    match respond(&ctx, &message).await {
        Some(reply) => json_response(&reply),
        None => text_response(StatusCode::ACCEPTED, ""),
    }
}

async fn accept_loop(listener: TcpListener, ctx: Arc<Context>, cancel: CancellationToken) {
    loop {
        let stream = tokio::select! {
            _ = cancel.cancelled() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => stream,
                Err(_) => continue,
            },
        };

        let ctx = Arc::clone(&ctx);
        let cancel = cancel.clone();
        tauri::async_runtime::spawn(async move {
            let service = service_fn(move |request| {
                let ctx = Arc::clone(&ctx);
                async move { Ok::<_, Infallible>(handle(request, ctx).await) }
            });
            let connection = http1::Builder::new().serve_connection(TokioIo::new(stream), service);
            tokio::select! {
                _ = connection => {}
                _ = cancel.cancelled() => {}
            }
        });
    }
}

async fn bind(port: u16) -> Result<TcpListener, String> {
    // A server that was just cancelled may still hold the port for a moment.
    let mut last_error = None;
    for _ in 0..10 {
        match TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
            Ok(listener) => return Ok(listener),
            Err(error) => last_error = Some(error),
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(format!(
        "Port {} is not available ({}). Another program may be using it; choose a different port.",
        port,
        last_error.map(|e| e.to_string()).unwrap_or_default()
    ))
}

/// Stop the connector if it is running.
pub fn stop() {
    if let Some(server) = lock(&SERVER).take() {
        server.cancel.cancel();
    }
}

/// Start or stop the connector to match the saved settings.
pub async fn apply(app: &AppHandle) -> AgentStatus {
    stop();
    *lock(&LAST_ERROR) = None;
    let settings = AppSettings::load();

    if settings.agent_connector_enabled {
        let started = async {
            let token = load_or_create_token()?;
            let listener = bind(settings.agent_connector_port).await?;
            let cancel = CancellationToken::new();
            *lock(&SERVER) = Some(RunningServer {
                cancel: cancel.clone(),
                port: settings.agent_connector_port,
                read_only: settings.agent_connector_read_only,
            });
            let ctx = Arc::new(Context {
                app: app.clone(),
                token,
                read_only: settings.agent_connector_read_only,
            });
            tauri::async_runtime::spawn(accept_loop(listener, ctx, cancel));
            Ok::<(), String>(())
        }
        .await;
        if let Err(error) = started {
            *lock(&LAST_ERROR) = Some(error);
        }
    }

    status()
}

fn status() -> AgentStatus {
    let settings = AppSettings::load();
    let server = lock(&SERVER);
    let port = server
        .as_ref()
        .map(|server| server.port)
        .unwrap_or(settings.agent_connector_port);
    AgentStatus {
        enabled: settings.agent_connector_enabled,
        running: server.is_some(),
        read_only: server
            .as_ref()
            .map(|server| server.read_only)
            .unwrap_or(settings.agent_connector_read_only),
        port,
        url: format!("http://127.0.0.1:{port}/mcp"),
        token: settings
            .agent_connector_enabled
            .then(|| fs::read_to_string(token_path().ok()?).ok())
            .flatten()
            .map(|token| token.trim().to_string()),
        error: lock(&LAST_ERROR).clone(),
    }
}

#[tauri::command]
pub async fn get_agent_connector_status() -> AgentStatus {
    status()
}

#[tauri::command]
pub async fn configure_agent_connector(
    enabled: bool,
    read_only: bool,
    port: u16,
    app: AppHandle,
) -> Result<AgentStatus, String> {
    let mut settings = AppSettings::load();
    if port < 1024 {
        return Err("Use a port between 1024 and 65535".to_string());
    }
    if [
        settings.web_port,
        settings.php_port,
        settings.mysql_port,
        settings.postgresql_port,
    ]
    .contains(&port)
    {
        return Err(format!("Port {port} is already used by a CHAMP service"));
    }

    settings.agent_connector_enabled = enabled;
    settings.agent_connector_read_only = read_only;
    settings.agent_connector_port = port;
    settings.save()?;
    Ok(apply(&app).await)
}

#[derive(Debug, Serialize)]
pub struct AgentTestResult {
    pub ok: bool,
    pub tools: Vec<String>,
    pub message: String,
}

/// Connect to the running server exactly like an agent would (initialize +
/// tools/list over HTTP with the bearer token) so the user can confirm setup.
#[tauri::command]
pub async fn test_agent_connector() -> AgentTestResult {
    match probe(&status()).await {
        Ok(tools) => AgentTestResult {
            ok: true,
            message: format!("{} tools available", tools.len()),
            tools,
        },
        Err(message) => AgentTestResult {
            ok: false,
            tools: Vec::new(),
            message,
        },
    }
}

async fn probe(status: &AgentStatus) -> Result<Vec<String>, String> {
    if !status.enabled {
        return Err("The agent connector is turned off.".to_string());
    }
    if !status.running {
        return Err(status
            .error
            .clone()
            .unwrap_or_else(|| "The agent connector is not running.".to_string()));
    }
    let token = status.token.clone().unwrap_or_default();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    let call = |id: u64, method: &str, params: Value| {
        client
            .post(&status.url)
            .bearer_auth(&token)
            .header("Accept", "application/json, text/event-stream")
            .json(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .send()
    };

    let response = call(
        1,
        "initialize",
        json!({ "protocolVersion": SUPPORTED_PROTOCOL_VERSIONS[0], "capabilities": {}, "clientInfo": { "name": "champ-self-test", "version": env!("CARGO_PKG_VERSION") } }),
    )
    .await
    .map_err(|e| format!("Could not reach {}: {}", status.url, e))?;
    if !response.status().is_success() {
        let code = response.status();
        return Err(format!(
            "Server answered {}: {}",
            code,
            response.text().await.unwrap_or_default()
        ));
    }

    let listed: Value = call(2, "tools/list", json!({}))
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| format!("Unexpected tools/list reply: {}", e))?;
    Ok(listed["result"]["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| tool["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

/// Replace the token; agents configured with the old one stop working.
#[tauri::command]
pub async fn regenerate_agent_token(app: AppHandle) -> Result<AgentStatus, String> {
    write_token(&generate_token()?)?;
    Ok(apply(&app).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn headers(pairs: &[(hyper::header::HeaderName, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(name.clone(), HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn requests_need_the_exact_bearer_token() {
        let bearer = format!("Bearer {TOKEN}");
        assert!(check_headers(&headers(&[(AUTHORIZATION, &bearer)]), TOKEN).is_ok());
        assert_eq!(
            check_headers(&headers(&[]), TOKEN).unwrap_err().0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            check_headers(&headers(&[(AUTHORIZATION, "Bearer wrong")]), TOKEN)
                .unwrap_err()
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            check_headers(&headers(&[(AUTHORIZATION, TOKEN)]), TOKEN)
                .unwrap_err()
                .0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn browser_requests_are_refused_even_with_token() {
        let bearer = format!("Bearer {TOKEN}");
        let result = check_headers(
            &headers(&[(AUTHORIZATION, &bearer), (ORIGIN, "https://evil.example")]),
            TOKEN,
        );
        assert_eq!(result.unwrap_err().0, StatusCode::FORBIDDEN);
    }

    #[test]
    fn read_only_mode_hides_write_tools() {
        let names = |read_only| {
            tool_definitions(read_only)
                .iter()
                .map(|tool| tool["name"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        let full = names(false);
        let limited = names(true);
        assert!(WRITE_TOOLS
            .iter()
            .all(|name| full.iter().any(|t| t == name)));
        assert!(WRITE_TOOLS
            .iter()
            .all(|name| !limited.iter().any(|t| t == name)));
        assert!(limited.iter().any(|t| t == "diagnose_service"));
    }

    #[test]
    fn protocol_version_is_negotiated() {
        assert_eq!(
            negotiate_protocol(&json!({ "protocolVersion": "2025-03-26" })),
            "2025-03-26"
        );
        assert_eq!(
            negotiate_protocol(&json!({ "protocolVersion": "1999-01-01" })),
            "2025-06-18"
        );
        assert_eq!(initialize_result(&json!({}))["serverInfo"]["name"], "champ");
    }

    #[test]
    fn generated_tokens_are_random_hex() {
        let a = generate_token().unwrap();
        let b = generate_token().unwrap();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn every_tool_hidden_in_read_only_mode_is_a_write_tool() {
        let limited: Vec<Value> = tool_definitions(true);
        for tool in tool_definitions(false) {
            let name = tool["name"].as_str().unwrap();
            let visible = limited.iter().any(|t| t["name"] == name);
            assert_eq!(!visible, WRITE_TOOLS.contains(&name), "{name}");
            assert_eq!(tool["annotations"]["readOnlyHint"], visible, "{name}");
        }
        for name in ["list_databases", "get_https_preview", "get_environment"] {
            assert!(limited.iter().any(|t| t["name"] == name), "{name}");
        }
    }

    #[test]
    fn self_test_explains_why_it_cannot_connect() {
        let mut status = AgentStatus {
            enabled: false,
            running: false,
            read_only: false,
            port: 47823,
            url: "http://127.0.0.1:47823/mcp".to_string(),
            token: None,
            error: None,
        };
        let off = tauri::async_runtime::block_on(probe(&status)).unwrap_err();
        assert!(off.contains("turned off"), "{off}");
        status.enabled = true;
        status.error = Some("Port 47823 is not available".to_string());
        let failed = tauri::async_runtime::block_on(probe(&status)).unwrap_err();
        assert!(failed.contains("47823"), "{failed}");
    }

    #[test]
    fn tool_schemas_are_objects() {
        for tool in tool_definitions(false) {
            assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
        }
    }
}
