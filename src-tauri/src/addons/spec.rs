//! What each built-in add-on is: its ports, its browser page, and how it is
//! launched and stopped. Everything generic (settings, status, validation,
//! environment) reads this table instead of matching on the add-on.
//!
//! The pinned downloads live in `catalog.rs`; the processes in `runtime.rs`.

use super::AddonKind;
use std::ffi::OsString;
use std::path::Path;

/// One TCP port an add-on listens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortSpec {
    /// Key in the settings file (`ports.<role>`).
    pub role: &'static str,
    /// Label for validation messages ("Mailpit SMTP").
    pub label: &'static str,
    pub default: u16,
}

/// How a running add-on is asked to exit before it is killed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopMethod {
    /// Send Redis `SHUTDOWN` on the primary port, so it saves its data.
    RedisShutdown,
    /// SIGTERM on Unix; a kill on Windows (nothing gentler exists there).
    Terminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddonSpec {
    pub kind: AddonKind,
    pub key: &'static str,
    pub name: &'static str,
    /// What the add-on is for: cache, mail, search, storage, realtime, queue.
    pub category: &'static str,
    /// The primary port first.
    pub ports: &'static [PortSpec],
    /// Role of the port that serves a page in the browser, if any.
    pub web_role: Option<&'static str>,
    pub stop: StopMethod,
    /// Executable name looked up on `PATH` when CHAMP has no download.
    pub system_binary: &'static str,
}

pub const SPECS: [AddonSpec; 5] = [
    AddonSpec {
        kind: AddonKind::Redis,
        key: "redis",
        name: "Redis",
        category: "cache",
        ports: &[PortSpec {
            role: "main",
            label: "Redis",
            default: 6379,
        }],
        web_role: None,
        stop: StopMethod::RedisShutdown,
        system_binary: "redis-server",
    },
    AddonSpec {
        kind: AddonKind::Mailpit,
        key: "mailpit",
        name: "Mailpit",
        category: "mail",
        ports: &[
            PortSpec {
                role: "smtp",
                label: "Mailpit SMTP",
                default: 1025,
            },
            PortSpec {
                role: "web",
                label: "Mailpit web",
                default: 8025,
            },
        ],
        web_role: Some("web"),
        stop: StopMethod::Terminate,
        system_binary: "mailpit",
    },
    AddonSpec {
        kind: AddonKind::Meilisearch,
        key: "meilisearch",
        name: "Meilisearch",
        category: "search",
        ports: &[PortSpec {
            role: "main",
            label: "Meilisearch",
            default: 7700,
        }],
        // The search preview dashboard is served on the API port.
        web_role: Some("main"),
        stop: StopMethod::Terminate,
        system_binary: "meilisearch",
    },
    AddonSpec {
        kind: AddonKind::Centrifugo,
        key: "centrifugo",
        name: "Centrifugo",
        category: "realtime",
        ports: &[PortSpec {
            role: "main",
            label: "Centrifugo",
            default: 8000,
        }],
        // The admin web UI is served at the root of the same port.
        web_role: Some("main"),
        stop: StopMethod::Terminate,
        system_binary: "centrifugo",
    },
    AddonSpec {
        kind: AddonKind::Rustfs,
        key: "rustfs",
        name: "RustFS (S3)",
        category: "storage",
        ports: &[
            PortSpec {
                role: "main",
                label: "RustFS S3 API",
                default: 9100,
            },
            PortSpec {
                role: "console",
                label: "RustFS console",
                default: 9101,
            },
        ],
        web_role: Some("console"),
        stop: StopMethod::Terminate,
        system_binary: "rustfs",
    },
];

/// Local-only S3 credentials for RustFS (it listens on 127.0.0.1 only).
pub const S3_ACCESS_KEY: &str = "rustfsadmin";
pub const S3_SECRET_KEY: &str = "rustfsadmin";
pub const S3_REGION: &str = "us-east-1";

/// Centrifugo development config: no tokens, no API key, open admin UI.
/// Safe only because it listens on 127.0.0.1.
pub fn centrifugo_config(port: u16) -> String {
    serde_json::json!({
        "http_server": { "address": "127.0.0.1", "port": port },
        "client": { "insecure": true, "allowed_origins": ["*"] },
        "http_api": { "insecure": true },
        "admin": { "enabled": true, "insecure": true },
    })
    .to_string()
}

/// Files an add-on needs before it starts (Centrifugo's config).
pub fn prepare(kind: AddonKind, ports: &[u16], data_dir: &Path) -> Result<(), String> {
    if kind == AddonKind::Centrifugo {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("Failed to create Centrifugo data folder: {e}"))?;
        std::fs::write(data_dir.join("config.json"), centrifugo_config(ports[0]))
            .map_err(|e| format!("Failed to write Centrifugo config: {e}"))?;
    }
    Ok(())
}

/// Environment variables for the add-on process.
pub fn launch_env(kind: AddonKind) -> Vec<(&'static str, &'static str)> {
    match kind {
        // No update check against version.rustfs.com on every start.
        AddonKind::Rustfs => vec![("RUSTFS_CHECK_UPDATE", "false")],
        _ => Vec::new(),
    }
}

pub fn spec(kind: AddonKind) -> &'static AddonSpec {
    SPECS
        .iter()
        .find(|spec| spec.kind == kind)
        .expect("every AddonKind has a spec")
}

/// Command-line arguments for `kind`, listening on `ports` (in spec order).
/// Every built-in binds 127.0.0.1 only.
pub fn launch_args(kind: AddonKind, ports: &[u16], data_dir: &Path) -> Vec<OsString> {
    let strings = |items: &[&str]| items.iter().map(OsString::from).collect::<Vec<_>>();
    match kind {
        AddonKind::Redis => strings(&[
            "--port",
            &ports[0].to_string(),
            "--bind",
            "127.0.0.1",
            "--protected-mode",
            "yes",
            "--daemonize",
            "no",
            // Relative to the working directory (the data folder); the Windows
            // build runs on an msys2 runtime that handles relative paths best.
            "--dir",
            "./",
        ]),
        AddonKind::Mailpit => {
            let mut args = strings(&[
                "--smtp",
                &format!("127.0.0.1:{}", ports[0]),
                "--listen",
                &format!("127.0.0.1:{}", ports[1]),
                "--database",
            ]);
            args.push(data_dir.join("mailpit.db").into_os_string());
            // Apps configured with a username/password must still deliver.
            args.extend(strings(&[
                "--smtp-auth-accept-any",
                "--smtp-auth-allow-insecure",
                "--disable-version-check",
            ]));
            args
        }
        AddonKind::Meilisearch => {
            // Development mode: no master key, and the search preview is
            // served on the same port. Dumps and snapshots default to folders
            // under the working directory (the data folder).
            let mut args = strings(&["--http-addr", &format!("127.0.0.1:{}", ports[0])]);
            args.push("--db-path".into());
            args.push(data_dir.join("data.ms").into_os_string());
            args.extend(strings(&["--env", "development", "--no-analytics"]));
            args
        }
        AddonKind::Centrifugo => {
            let mut args = strings(&["-c"]);
            args.push(data_dir.join("config.json").into_os_string());
            args
        }
        AddonKind::Rustfs => {
            let mut args = strings(&[
                "server",
                "--address",
                &format!("127.0.0.1:{}", ports[0]),
                "--console-address",
                &format!("127.0.0.1:{}", ports[1]),
                "--access-key",
                S3_ACCESS_KEY,
                "--secret-key",
                S3_SECRET_KEY,
            ]);
            args.push(data_dir.join("data").into_os_string());
            args
        }
    }
}
