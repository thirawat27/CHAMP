//! The MCP tool list.

use super::handlers::MAX_LOG_LINES;
use serde_json::{json, Value};

pub(super) const WRITE_TOOLS: [&str; 9] = [
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

pub(super) fn service_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["caddy", "php-fpm", "mysql", "postgresql"],
        "description": "caddy = web server, php-fpm = PHP, mysql / postgresql = database"
    })
}

pub(super) fn tool(
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

pub(super) fn engine_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["mysql", "postgresql"],
        "description": "Database engine. Defaults to the database selected in CHAMP."
    })
}

pub(super) fn tool_definitions(read_only: bool) -> Vec<Value> {
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
