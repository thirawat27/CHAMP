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

use crate::config::AppSettings;
use handlers::SUPPORTED_PROTOCOL_VERSIONS;
use http::{accept_loop, bind};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::AppHandle;
use token::{generate_token, load_or_create_token, token_path, write_token};
use tokio_util::sync::CancellationToken;

mod handlers;
mod http;
mod token;
mod tools;

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

const SELF_TEST_TIMEOUT: Duration = Duration::from_secs(5);

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
    let settings = AppSettings::load();
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

    AppSettings::update(|settings| {
        settings.agent_connector_enabled = enabled;
        settings.agent_connector_read_only = read_only;
        settings.agent_connector_port = port;
    })?;
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
        .timeout(SELF_TEST_TIMEOUT)
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
mod tests;
