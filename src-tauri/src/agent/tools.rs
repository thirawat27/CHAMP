//! The MCP tool list.

use super::handlers::MAX_LOG_LINES;
use serde_json::{json, Value};

/// Tools that change something. Hidden and refused in read-only mode.
pub(super) const WRITE_TOOLS: &[&str] = &[
    "start_stack",
    "stop_stack",
    "restart_stack",
    "start_service",
    "stop_service",
    "restart_service",
    "create_database",
    "start_https_preview",
    "stop_https_preview",
    "start_addon",
    "stop_addon",
    "set_addon_enabled",
    "switch_php_version",
    "set_xdebug",
    "start_worker",
    "stop_worker",
    "update_settings",
    "install_php_version",
    "switch_database_engine",
    "switch_database_tool",
    "install_addon",
    "set_addon_port",
];

pub(super) fn service_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["caddy", "php-fpm", "mysql", "postgresql"],
        "description": "caddy = web server, php-fpm = PHP, mysql / postgresql = database"
    })
}

fn addon_schema() -> Value {
    json!({
        "type": "string",
        "description": "Add-on id from get_addons: redis, mailpit, meilisearch, or a custom add-on's id"
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

fn database_name_schema(description: &str) -> Value {
    json!({ "type": "string", "pattern": "^[A-Za-z_][A-Za-z0-9_]{0,62}$", "description": description })
}

pub(super) fn tool_definitions(read_only: bool) -> Vec<Value> {
    let none = json!({});
    let service = json!({ "service": service_schema() });
    let engine = json!({ "engine": engine_schema() });
    let addon = json!({ "addon": addon_schema() });
    let worker =
        json!({ "worker": { "type": "string", "description": "Worker id from list_workers" } });
    let mut tools = vec![
        tool("get_status", "Stack status", "State, port, and last error of every CHAMP service, plus the website, database tool, and HTTPS preview URLs.", none.clone(), &[], true),
        tool("get_environment", "Development environment", "Everything needed to configure an app for CHAMP: database host/port/user/password, PHP version, document root, project URL patterns, add-ons, the CHAMP_* variables PHP already receives, and folders.", none.clone(), &[], true),
        tool("list_projects", "List projects", "Project folders served by CHAMP with their local URLs and detected framework.", none.clone(), &[], true),
        tool(
            "get_project",
            "Inspect a project",
            "One project's framework (Laravel, Symfony, WordPress, CodeIgniter, CakePHP, Slim, Yii, Laminas, Drupal, or plain PHP), web root, required vs running PHP, whether composer install and .env are done, its URL, and the recommended env profile with ready lines.",
            json!({ "name": { "type": "string", "description": "Project folder name from list_projects" } }),
            &["name"],
            true,
        ),
        tool(
            "get_env_profiles",
            "Framework configuration",
            "Ready-to-paste configuration for the database and enabled add-ons in each framework's convention: laravel (.env), symfony (.env.local DSNs), wordpress (wp-config.php), codeigniter (.env), cakephp (config/.env), dsn (generic URLs for Slim/Yii/Laminas/plain PHP), champ (getenv). Give project to use its domain and a database named after it.",
            json!({
                "project": { "type": "string", "description": "Project folder name (optional)" },
                "profile": { "type": "string", "enum": crate::envprofile::PROFILE_IDS, "description": "Only this profile (optional)" },
            }),
            &[],
            true,
        ),
        tool(
            "read_logs",
            "Read service logs",
            "Last lines of a core service's log files. For add-ons use read_addon_log.",
            json!({ "service": service_schema(), "lines": { "type": "integer", "minimum": 1, "maximum": MAX_LOG_LINES, "default": 80 } }),
            &["service"],
            true,
        ),
        tool("diagnose_service", "Diagnose a service", "Run CHAMP Log Doctor on a service: likely cause, fix steps, and the matching log line.", service.clone(), &["service"], true),
        tool("list_databases", "List databases", "User databases on MySQL or PostgreSQL (system databases hidden) plus connection details. The database must be running.", engine.clone(), &[], true),
        tool(
            "list_tables",
            "List tables",
            "Tables in one database (MySQL also gives a row estimate). Read-only catalog query.",
            json!({ "database": database_name_schema("Database name"), "engine": engine_schema() }),
            &["database"],
            true,
        ),
        tool(
            "describe_table",
            "Describe a table",
            "Columns of one table: name, type, nullable, default, and (MySQL) key and extra. Read-only catalog query; use it to write migrations and models that match.",
            json!({
                "database": database_name_schema("Database name"),
                "table": database_name_schema("Table name"),
                "engine": engine_schema(),
            }),
            &["database", "table"],
            true,
        ),
        tool("get_https_preview", "HTTPS Preview status", "Whether the public HTTPS Preview (ngrok) is running, its public URL once ready, and whether an ngrok authtoken is saved.", none.clone(), &[], true),
        tool("get_addons", "Add-on services", "Redis, Mailpit, Meilisearch, and custom add-ons: enabled, installed, state, host, ports, browser URL, and the CHAMP_* variables PHP receives for them.", none.clone(), &[], true),
        tool(
            "read_addon_log",
            "Read an add-on log",
            "Last lines of an add-on's log, with its state and last error.",
            json!({ "addon": addon_schema(), "lines": { "type": "integer", "minimum": 1, "maximum": 400, "default": 80 } }),
            &["addon"],
            true,
        ),
        tool(
            "list_mail",
            "List caught email",
            "Newest messages Mailpit caught (nothing is ever delivered): id, from, to, subject, time, snippet. Use it to check that the app sent a sign-up, reset, or notification email.",
            json!({
                "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10 },
                "search": { "type": "string", "description": "Mailpit search, for example to:user@example.test or subject:\"Reset\"" },
            }),
            &[],
            true,
        ),
        tool(
            "read_mail",
            "Read caught email",
            "One caught message: text body, links (for example a verification link to open), and attachment names. Defaults to the newest message.",
            json!({ "id": { "type": "string", "description": "Message id from list_mail, or latest" } }),
            &[],
            true,
        ),
        tool("get_php_info", "PHP configuration", "Running PHP version, loaded extensions, key ini values (memory_limit, upload sizes, timezone), and Xdebug status.", none.clone(), &[], true),
        tool("list_workers", "Background tasks", "Project background PHP commands the user defined (queue workers, schedulers) with their state.", none.clone(), &[], true),
        tool("get_settings", "CHAMP settings", "Saved ports, startup options, selected PHP/database/tool packages, Xdebug, and counts of custom add-ons and background tasks.", none.clone(), &[], true),
        tool("check_ports", "Check ports", "Every configured port (core services and enabled add-ons): free, used by the running CHAMP service, or taken by another program. Use it before changing ports or when a service cannot start.", none.clone(), &[], true),
        tool("list_php_versions", "PHP versions", "Every PHP branch CHAMP offers: installed or not, active, newest patch, end of life.", none.clone(), &[], true),
        tool("get_system_info", "System info", "CPU, memory, network, OS, CHAMP version, and whether the Log Doctor model is installed.", none.clone(), &[], true),
    ];
    if !read_only {
        tools.extend([
            tool("start_stack", "Start stack", "Start Caddy, PHP-FPM, the selected database, enabled add-ons, and background tasks marked to start with the stack.", none.clone(), &[], false),
            tool("stop_stack", "Stop stack", "Stop every CHAMP service, add-on, background task, and the HTTPS preview.", none.clone(), &[], false),
            tool("restart_stack", "Restart stack", "Restart Caddy, PHP-FPM, the selected database, and enabled add-ons.", none.clone(), &[], false),
            tool("start_service", "Start service", "Start one service.", service.clone(), &["service"], false),
            tool("stop_service", "Stop service", "Stop one service.", service.clone(), &["service"], false),
            tool("restart_service", "Restart service", "Restart one service.", service, &["service"], false),
            tool(
                "create_database",
                "Create database",
                "Create an empty database (utf8mb4 on MySQL) if it does not exist yet, and return its .env lines and connection URL. Nothing is dropped or overwritten.",
                json!({
                    "name": database_name_schema("Database name, for example my_app"),
                    "engine": engine_schema(),
                }),
                &["name"],
                false,
            ),
            tool("start_https_preview", "Start HTTPS Preview", "Start the stack if needed and open a public HTTPS URL (ngrok) to the local website. Needs an ngrok authtoken saved in CHAMP.", none.clone(), &[], false),
            tool("stop_https_preview", "Stop HTTPS Preview", "Close the public HTTPS URL.", none.clone(), &[], false),
            tool("start_addon", "Start add-on", "Start one add-on now (it must be installed). Use set_addon_enabled to keep it starting with the stack.", addon.clone(), &["addon"], false),
            tool("stop_addon", "Stop add-on", "Stop one add-on. Its data is kept.", addon.clone(), &["addon"], false),
            tool(
                "set_addon_enabled",
                "Enable or disable add-on",
                "Turn an add-on on (starts it when the stack runs, and with the stack from now on) or off (stops it). PHP restarts when its environment changes. Data is kept either way.",
                json!({ "addon": addon_schema(), "enabled": { "type": "boolean" } }),
                &["addon", "enabled"],
                false,
            ),
            tool(
                "switch_php_version",
                "Switch PHP version",
                "Switch the stack to another installed PHP version (for example 8.3) and restart PHP. Installing a new version is left to the user.",
                json!({ "version": { "type": "string", "description": "Installed version or branch, for example 8.4 or 8.4.12" } }),
                &["version"],
                false,
            ),
            tool(
                "set_xdebug",
                "Turn Xdebug on or off",
                "Enable step debugging (trigger mode, client port 9003) or disable it, and restart PHP. Needs the Xdebug extension in PHP's ext folder; the error explains how to get it.",
                json!({ "enabled": { "type": "boolean" } }),
                &["enabled"],
                false,
            ),
            tool("start_worker", "Start background task", "Start one of the user's background PHP commands.", worker.clone(), &["worker"], false),
            tool("stop_worker", "Stop background task", "Stop one background PHP command.", worker, &["worker"], false),
            tool(
                "update_settings",
                "Change settings",
                "Change core ports (web, PHP-FPM, MySQL, PostgreSQL, HTTPS) and startup options, validated like the Settings window (ports 1024-65535, no duplicates). A running stack restarts on the new ports.",
                json!({
                    "web_port": { "type": "integer", "minimum": 1024, "maximum": 65535 },
                    "php_port": { "type": "integer", "minimum": 1024, "maximum": 65535 },
                    "mysql_port": { "type": "integer", "minimum": 1024, "maximum": 65535 },
                    "postgresql_port": { "type": "integer", "minimum": 1024, "maximum": 65535 },
                    "https_port": { "type": "integer", "minimum": 1024, "maximum": 65535 },
                    "auto_start_services": { "type": "boolean", "description": "Start the stack when CHAMP opens" },
                    "auto_check_updates": { "type": "boolean", "description": "Check GitHub Releases daily" },
                }),
                &[],
                false,
            ),
            tool(
                "install_php_version",
                "Install PHP version",
                "Download a PHP branch (or its newest patch) from the official feeds, verified. Does not switch; call switch_php_version after.",
                json!({ "version": { "type": "string", "description": "Branch or version from list_php_versions, for example 8.4" } }),
                &["version"],
                false,
            ),
            tool(
                "switch_database_engine",
                "Switch database engine",
                "Make MySQL or PostgreSQL the stack's database (installs it if needed). Data of the other engine is kept.",
                json!({ "engine": { "type": "string", "enum": ["mysql", "postgresql"] } }),
                &["engine"],
                false,
            ),
            tool(
                "switch_database_tool",
                "Switch database tool",
                "Use phpMyAdmin or Adminer at the database tool URL (installs it if needed).",
                json!({ "tool": { "type": "string", "enum": ["phpmyadmin", "adminer"] } }),
                &["tool"],
                false,
            ),
            tool(
                "install_addon",
                "Install add-on",
                "Download and install a built-in add-on (redis, mailpit, meilisearch, centrifugo, rustfs) from its pinned, checksum-verified release, or update it.",
                addon.clone(),
                &["addon"],
                false,
            ),
            tool(
                "set_addon_port",
                "Change add-on port",
                "Move an add-on to another port (web_port: Mailpit's inbox). Validated against every other port; a running add-on restarts.",
                json!({
                    "addon": addon_schema(),
                    "port": { "type": "integer", "minimum": 1024, "maximum": 65535 },
                    "web_port": { "type": "integer", "minimum": 1024, "maximum": 65535 },
                }),
                &["addon"],
                false,
            ),
        ]);
    }
    tools
}
