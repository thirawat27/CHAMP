//! Database helpers for agent tools: list and create databases through the
//! bundled `mysql` / `psql` clients, and connection details agents can paste
//! into an app's `.env`.

use crate::process::{ServiceMap, ServiceState, ServiceType};
use crate::runtime::locator::locate_runtime_binaries;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

const MYSQL_SYSTEM_DATABASES: [&str; 4] =
    ["information_schema", "mysql", "performance_schema", "sys"];
const POSTGRESQL_SYSTEM_DATABASES: [&str; 1] = ["postgres"];

/// `engine` argument, falling back to the database selected in CHAMP.
pub(super) fn parse_engine(args: &Value, active: ServiceType) -> Result<ServiceType, String> {
    match args.get("engine").and_then(Value::as_str) {
        None | Some("") => Ok(active),
        Some("mysql") => Ok(ServiceType::MySQL),
        Some("postgresql") => Ok(ServiceType::PostgreSQL),
        Some(other) => Err(format!(
            "`engine` must be mysql or postgresql (got `{other}`)"
        )),
    }
}

/// Letters, digits, and underscores only, so the name never needs escaping.
pub(super) fn validate_database_name(name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name.len() <= 63
        && name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "Invalid database name `{name}`. Use 1-63 letters, digits, or underscores, starting with a letter or underscore (for example `my_app`)."
        ))
    }
}

fn engine_key(engine: ServiceType) -> &'static str {
    if engine == ServiceType::PostgreSQL {
        "postgresql"
    } else {
        "mysql"
    }
}

/// Connection details for one database, including a Laravel-style `.env` block.
pub(super) fn connection_info(engine: ServiceType, port: u16, database: Option<&str>) -> Value {
    let postgresql = engine == ServiceType::PostgreSQL;
    let user = if postgresql { "postgres" } else { "root" };
    let scheme = if postgresql { "postgresql" } else { "mysql" };
    let name = database.unwrap_or("<database>");
    json!({
        "engine": engine_key(engine),
        "host": "127.0.0.1",
        "port": port,
        "user": user,
        "password": "",
        "database": database,
        "url": format!("{scheme}://{user}@127.0.0.1:{port}/{name}"),
        "dotenv": format!(
            "DB_CONNECTION={}\nDB_HOST=127.0.0.1\nDB_PORT={port}\nDB_DATABASE={name}\nDB_USERNAME={user}\nDB_PASSWORD=",
            if postgresql { "pgsql" } else { "mysql" }
        ),
    })
}

fn sibling_binary(server_binary: &Path, name: &str) -> PathBuf {
    let file = if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    server_binary
        .parent()
        .map(|dir| dir.join(&file))
        .unwrap_or_else(|| PathBuf::from(file))
}

fn client_args(engine: ServiceType, port: u16, sql: &str) -> Vec<String> {
    let port = port.to_string();
    let args: Vec<&str> = if engine == ServiceType::PostgreSQL {
        vec![
            "-h",
            "127.0.0.1",
            "-p",
            &port,
            "-U",
            "postgres",
            "-d",
            "postgres",
            "-w",
            "-At",
            "-c",
            sql,
        ]
    } else {
        vec![
            "--protocol=TCP",
            "-h",
            "127.0.0.1",
            "-P",
            &port,
            "-u",
            "root",
            "--password=",
            "--connect-timeout=3",
            "-N",
            "-B",
            "-e",
            sql,
        ]
    };
    args.into_iter().map(str::to_string).collect()
}

fn run_sql(engine: ServiceType, port: u16, sql: &str) -> Result<String, String> {
    let binaries = locate_runtime_binaries()?;
    let client = if engine == ServiceType::PostgreSQL {
        sibling_binary(&binaries.postgresql, "psql")
    } else {
        sibling_binary(&binaries.mysql, "mysql")
    };
    if !client.exists() {
        return Err(format!(
            "The {} client was not found at {}. Reinstall the database in CHAMP Settings.",
            engine.display_name(),
            client.display()
        ));
    }

    let mut command = Command::new(&client);
    command
        .args(client_args(engine, port, sql))
        .env("PGCONNECT_TIMEOUT", "3");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .map_err(|e| format!("Failed to run {}: {}", client.display(), e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "{} returned an error: {}",
            engine.display_name(),
            stderr.trim()
        ))
    }
}

/// User databases from client output (one name per line), system ones removed.
pub(super) fn parse_database_list(engine: ServiceType, output: &str) -> Vec<String> {
    let system: &[&str] = if engine == ServiceType::PostgreSQL {
        &POSTGRESQL_SYSTEM_DATABASES
    } else {
        &MYSQL_SYSTEM_DATABASES
    };
    let mut names: Vec<String> = output
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty() && !system.contains(name))
        .map(str::to_string)
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names
}

/// Port of a running database, or an error that tells the agent what to do.
pub(super) fn running_port(statuses: &ServiceMap, engine: ServiceType) -> Result<u16, String> {
    let info = statuses
        .get(&engine)
        .ok_or_else(|| format!("{} is not installed in CHAMP.", engine.display_name()))?;
    if info.state == ServiceState::Running {
        Ok(info.port)
    } else {
        Err(format!(
            "{name} is not running. Call start_service with service \"{key}\" first (or ask the user to start it in CHAMP).",
            name = engine.display_name(),
            key = engine_key(engine)
        ))
    }
}

fn list(engine: ServiceType, port: u16) -> Result<Vec<String>, String> {
    let sql = if engine == ServiceType::PostgreSQL {
        "SELECT datname FROM pg_database WHERE NOT datistemplate ORDER BY datname"
    } else {
        "SHOW DATABASES"
    };
    Ok(parse_database_list(engine, &run_sql(engine, port, sql)?))
}

pub(super) fn list_databases(engine: ServiceType, port: u16) -> Result<Value, String> {
    Ok(json!({
        "engine": engine_key(engine),
        "databases": list(engine, port)?,
        "connection": connection_info(engine, port, None),
    }))
}

pub(super) fn create_database(engine: ServiceType, port: u16, name: &str) -> Result<Value, String> {
    validate_database_name(name)?;
    let existed = list(engine, port)?.iter().any(|db| db == name);
    if !existed {
        let sql = if engine == ServiceType::PostgreSQL {
            format!("CREATE DATABASE \"{name}\"")
        } else {
            format!("CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci")
        };
        run_sql(engine, port, &sql)?;
    }
    Ok(json!({
        "engine": engine_key(engine),
        "database": name,
        "created": !existed,
        "message": if existed { "Database already existed; nothing changed." } else { "Database created." },
        "connection": connection_info(engine, port, Some(name)),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_names_are_restricted_to_safe_identifiers() {
        for name in ["my_app", "_tmp", "shop2", "A"] {
            assert!(validate_database_name(name).is_ok(), "{name}");
        }
        for name in [
            "",
            "2shop",
            "my-app",
            "a b",
            "x`; DROP",
            "db\"",
            &"a".repeat(64),
        ] {
            assert!(validate_database_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn engine_defaults_to_the_active_database() {
        assert_eq!(
            parse_engine(&json!({}), ServiceType::PostgreSQL).unwrap(),
            ServiceType::PostgreSQL
        );
        assert_eq!(
            parse_engine(&json!({ "engine": "mysql" }), ServiceType::PostgreSQL).unwrap(),
            ServiceType::MySQL
        );
        assert!(parse_engine(&json!({ "engine": "oracle" }), ServiceType::MySQL).is_err());
    }

    #[test]
    fn system_databases_are_hidden() {
        let mysql = parse_database_list(
            ServiceType::MySQL,
            "information_schema\nshop\nmysql\nperformance_schema\nsys\nBlog\n",
        );
        assert_eq!(mysql, vec!["Blog", "shop"]);
        let postgresql = parse_database_list(ServiceType::PostgreSQL, "postgres\r\napp\r\n");
        assert_eq!(postgresql, vec!["app"]);
    }

    #[test]
    fn connection_info_has_dotenv_and_url() {
        let mysql = connection_info(ServiceType::MySQL, 3307, Some("shop"));
        assert_eq!(mysql["url"], "mysql://root@127.0.0.1:3307/shop");
        assert!(mysql["dotenv"]
            .as_str()
            .unwrap()
            .contains("DB_CONNECTION=mysql\nDB_HOST=127.0.0.1\nDB_PORT=3307\nDB_DATABASE=shop"));
        let postgresql = connection_info(ServiceType::PostgreSQL, 5432, None);
        assert_eq!(postgresql["user"], "postgres");
        assert!(postgresql["dotenv"]
            .as_str()
            .unwrap()
            .contains("DB_CONNECTION=pgsql"));
    }

    #[test]
    fn client_args_use_tcp_and_the_service_port() {
        let mysql = client_args(ServiceType::MySQL, 3307, "SHOW DATABASES");
        assert!(mysql.contains(&"--protocol=TCP".to_string()));
        assert!(mysql.contains(&"3307".to_string()));
        assert_eq!(mysql.last().unwrap(), "SHOW DATABASES");
        let postgresql = client_args(ServiceType::PostgreSQL, 5433, "SELECT 1");
        assert!(postgresql.contains(&"5433".to_string()));
        assert!(postgresql.contains(&"-w".to_string()));
    }

    #[test]
    fn stopped_database_explains_how_to_start_it() {
        let mut statuses = ServiceMap::new();
        statuses.insert(
            ServiceType::MySQL,
            crate::process::ServiceInfo::new(ServiceType::MySQL),
        );
        let error = running_port(&statuses, ServiceType::MySQL).unwrap_err();
        assert!(error.contains("start_service"), "{error}");
    }
}
