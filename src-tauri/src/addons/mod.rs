//! Optional services: Redis (cache/queue), Mailpit (catches outgoing mail so
//! nothing is really sent), and Meilisearch (full-text search for Laravel
//! Scout and similar).
//!
//! Add-ons are independent of the core stack: they have their own state and
//! never block or fail a stack start. Enabled add-ons start and stop with the
//! stack, like Laragon's optional services. Everything listens on 127.0.0.1.

mod catalog;
mod install;
mod runtime;

#[cfg(test)]
mod tests;

use crate::config::{AddonSettings, AppSettings};
use crate::process::runner::run_with_manager;
use crate::process::{ServiceState, ServiceType};
use crate::runtime::locator::{get_app_data_paths, AppDataPaths};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AddonKind {
    Redis,
    Mailpit,
    Meilisearch,
}

pub const ALL_ADDONS: [AddonKind; 3] =
    [AddonKind::Redis, AddonKind::Mailpit, AddonKind::Meilisearch];

impl AddonKind {
    pub fn key(self) -> &'static str {
        match self {
            AddonKind::Redis => "redis",
            AddonKind::Mailpit => "mailpit",
            AddonKind::Meilisearch => "meilisearch",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            AddonKind::Redis => "Redis",
            AddonKind::Mailpit => "Mailpit",
            AddonKind::Meilisearch => "Meilisearch",
        }
    }

    fn enabled(self, settings: &AddonSettings) -> bool {
        match self {
            AddonKind::Redis => settings.redis_enabled,
            AddonKind::Mailpit => settings.mailpit_enabled,
            AddonKind::Meilisearch => settings.meilisearch_enabled,
        }
    }

    fn set_enabled(self, settings: &mut AddonSettings, enabled: bool) {
        match self {
            AddonKind::Redis => settings.redis_enabled = enabled,
            AddonKind::Mailpit => settings.mailpit_enabled = enabled,
            AddonKind::Meilisearch => settings.meilisearch_enabled = enabled,
        }
    }

    fn log_name(self) -> String {
        format!("{}.log", self.key())
    }
}

/// Where an add-on binary comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AddonSource {
    /// Downloaded by CHAMP into `runtime/addons/<name>`.
    Champ,
    /// Installed by the system package manager.
    System,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddonStatus {
    pub kind: AddonKind,
    pub name: &'static str,
    /// Version CHAMP downloads for this platform.
    pub version: &'static str,
    pub installed_version: Option<String>,
    pub installed: bool,
    pub source: Option<AddonSource>,
    /// CHAMP can download this add-on for the current platform.
    pub downloadable: bool,
    /// What to run when there is nothing to download (macOS/Linux).
    pub install_hint: Option<&'static str>,
    pub update_available: bool,
    pub installing: bool,
    pub enabled: bool,
    pub state: ServiceState,
    pub port: u16,
    /// Mailpit's web inbox port (a second, separately configured port).
    pub web_port: Option<u16>,
    /// Mailpit's inbox or Meilisearch's dashboard, once it is running.
    pub web_url: Option<String>,
    pub error: Option<String>,
    pub log_path: Option<PathBuf>,
}

/// Add-ons downloading right now; one install per add-on at a time.
static INSTALLING: Mutex<Option<HashSet<AddonKind>>> = Mutex::new(None);

struct InstallGuard(AddonKind);

impl InstallGuard {
    fn acquire(kind: AddonKind) -> Option<Self> {
        let mut installing = INSTALLING.lock().unwrap_or_else(PoisonError::into_inner);
        // `then`, not `then_some`: an eagerly built guard would be dropped here
        // on refusal, deadlocking on this lock and clearing the running install.
        installing
            .get_or_insert_with(HashSet::new)
            .insert(kind)
            .then(|| Self(kind))
    }
}

impl Drop for InstallGuard {
    fn drop(&mut self) {
        let mut installing = INSTALLING.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(set) = installing.as_mut() {
            set.remove(&self.0);
        }
    }
}

fn is_installing(kind: AddonKind) -> bool {
    INSTALLING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .is_some_and(|set| set.contains(&kind))
}

/// The binary to run: CHAMP's own install first, then a system one.
fn resolve_binary(kind: AddonKind, paths: &AppDataPaths) -> Option<(PathBuf, AddonSource)> {
    let package = catalog::current_package(kind);
    package
        .download
        .and_then(|download| install::installed_binary(&paths.runtime_dir, kind, &download))
        .map(|binary| (binary, AddonSource::Champ))
        .or_else(|| install::system_binary(kind).map(|binary| (binary, AddonSource::System)))
}

fn data_dir(kind: AddonKind, paths: &AppDataPaths) -> PathBuf {
    paths.base_dir.join("addons").join(kind.key())
}

fn status_for(kind: AddonKind, settings: &AddonSettings, paths: &AppDataPaths) -> AddonStatus {
    let package = catalog::current_package(kind);
    let installed_version = install::installed_version(&paths.runtime_dir, kind);
    let resolved = resolve_binary(kind, paths);
    let (state, error, running_ports) = runtime::snapshot(kind);
    let configured = runtime::ports_for(kind, settings);
    // A running add-on keeps the ports it started on until it restarts.
    let ports = if state.is_running() && !running_ports.is_empty() {
        running_ports
    } else {
        configured
    };
    let web_port = (kind == AddonKind::Mailpit).then(|| ports[1]);
    // Meilisearch serves its dashboard on its API port.
    let browser_port = match kind {
        AddonKind::Mailpit => web_port,
        AddonKind::Meilisearch => Some(ports[0]),
        AddonKind::Redis => None,
    };
    let log_path = paths.logs_dir.join(kind.log_name());
    AddonStatus {
        kind,
        name: kind.display_name(),
        version: package.version,
        update_available: installed_version
            .as_deref()
            .is_some_and(|installed| installed != package.version && package.download.is_some()),
        installed_version,
        installed: resolved.is_some(),
        source: resolved.map(|(_, source)| source),
        downloadable: package.download.is_some(),
        install_hint: catalog::system_install_hint(kind, std::env::consts::OS),
        installing: is_installing(kind),
        enabled: kind.enabled(settings),
        state: state.clone(),
        port: ports[0],
        web_port,
        web_url: browser_port
            .filter(|_| state.is_running())
            .map(|port| format!("http://127.0.0.1:{port}/")),
        error,
        log_path: log_path.exists().then_some(log_path),
    }
}

pub fn statuses() -> Result<Vec<AddonStatus>, String> {
    let paths = get_app_data_paths()?;
    let settings = AppSettings::load().addons;
    Ok(ALL_ADDONS
        .into_iter()
        .map(|kind| status_for(kind, &settings, &paths))
        .collect())
}

/// Start one add-on (blocking: waits for its ports).
pub fn start_blocking(kind: AddonKind) -> Result<(), String> {
    let paths = get_app_data_paths()?;
    let settings = AppSettings::load().addons;
    let Some((binary, source)) = resolve_binary(kind, &paths) else {
        let message = match catalog::system_install_hint(kind, std::env::consts::OS) {
            Some(hint) => format!(
                "{} is not installed. Install it with: {}",
                kind.display_name(),
                hint
            ),
            None => format!(
                "{} is not installed yet. Open Add-on services and press Install.",
                kind.display_name()
            ),
        };
        runtime::set_error(kind, &message);
        return Err(message);
    };
    std::fs::create_dir_all(&paths.logs_dir)
        .map_err(|e| format!("Failed to create logs folder: {}", e))?;
    runtime::start(
        kind,
        runtime::LaunchPlan {
            binary,
            data_dir: data_dir(kind, &paths),
            log_path: paths.logs_dir.join(kind.log_name()),
            ports: runtime::ports_for(kind, &settings),
            owned_binary: source == AddonSource::Champ,
        },
    )
}

pub fn stop_blocking(kind: AddonKind) -> Result<(), String> {
    runtime::stop(kind)
}

/// Start every enabled add-on. Failures are kept in each add-on's status and
/// returned for logging, but never stop the others from starting.
pub fn start_enabled_blocking() -> Vec<String> {
    let settings = AppSettings::load().addons;
    ALL_ADDONS
        .into_iter()
        .filter(|kind| kind.enabled(&settings))
        .filter_map(|kind| start_blocking(kind).err())
        .collect()
}

/// Stop every add-on, enabled or not (one may have been started by hand).
pub fn stop_all_blocking() {
    for kind in ALL_ADDONS {
        let _ = runtime::stop(kind);
    }
}

pub async fn start_enabled(app: &AppHandle) {
    let errors = tauri::async_runtime::spawn_blocking(start_enabled_blocking)
        .await
        .unwrap_or_default();
    if !errors.is_empty() {
        let _ = app.emit("addon-errors", errors);
    }
}

pub async fn stop_all() {
    let _ = tauri::async_runtime::spawn_blocking(stop_all_blocking).await;
}

/// How PHP's `mail()` reaches Mailpit, when Mailpit is enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhpMailRoute {
    pub smtp_port: u16,
    /// For PHP on macOS/Linux, which hands mail to a sendmail program.
    pub sendmail_path: Option<String>,
}

pub fn php_mail_route() -> Option<PhpMailRoute> {
    let settings = AppSettings::load().addons;
    if !settings.mailpit_enabled {
        return None;
    }
    let sendmail_path = get_app_data_paths()
        .ok()
        .and_then(|paths| resolve_binary(AddonKind::Mailpit, &paths))
        .and_then(|(binary, _)| sendmail_command(&binary, settings.mailpit_smtp_port));
    Some(PhpMailRoute {
        smtp_port: settings.mailpit_smtp_port,
        sendmail_path,
    })
}

/// `'<mailpit>' sendmail -t -i -S 127.0.0.1:<port>`, quoted for `/bin/sh`
/// (PHP runs sendmail_path through the shell). `None` for paths that cannot
/// be quoted safely.
pub(crate) fn sendmail_command(binary: &std::path::Path, smtp_port: u16) -> Option<String> {
    let path = binary.to_str()?;
    if path.contains(['\'', '"', '\n', '\r']) {
        return None;
    }
    Some(format!("'{path}' sendmail -t -i -S 127.0.0.1:{smtp_port}"))
}

/// `KEY=value` lines for enabled add-ons, for PHP's environment and agents.
pub fn env_pairs(settings: &AddonSettings) -> Vec<(&'static str, String)> {
    let mut pairs = Vec::new();
    if settings.redis_enabled {
        pairs.push(("CHAMP_REDIS_HOST", "127.0.0.1".to_string()));
        pairs.push(("CHAMP_REDIS_PORT", settings.redis_port.to_string()));
    }
    if settings.mailpit_enabled {
        pairs.push(("CHAMP_MAIL_HOST", "127.0.0.1".to_string()));
        pairs.push(("CHAMP_MAIL_PORT", settings.mailpit_smtp_port.to_string()));
        pairs.push((
            "CHAMP_MAILPIT_URL",
            format!("http://127.0.0.1:{}/", settings.mailpit_web_port),
        ));
    }
    if settings.meilisearch_enabled {
        pairs.push((
            "CHAMP_MEILISEARCH_HOST",
            format!("http://127.0.0.1:{}", settings.meilisearch_port),
        ));
    }
    pairs
}

/// A Laravel-style `.env` block for the enabled add-ons.
pub fn dotenv(settings: &AddonSettings) -> String {
    let mut lines: Vec<String> = Vec::new();
    if settings.redis_enabled {
        lines.extend([
            "REDIS_CLIENT=predis".to_string(),
            "REDIS_HOST=127.0.0.1".to_string(),
            "REDIS_PASSWORD=null".to_string(),
            format!("REDIS_PORT={}", settings.redis_port),
            "CACHE_STORE=redis".to_string(),
            "QUEUE_CONNECTION=redis".to_string(),
        ]);
    }
    if settings.mailpit_enabled {
        lines.extend([
            "MAIL_MAILER=smtp".to_string(),
            "MAIL_HOST=127.0.0.1".to_string(),
            format!("MAIL_PORT={}", settings.mailpit_smtp_port),
            "MAIL_USERNAME=null".to_string(),
            "MAIL_PASSWORD=null".to_string(),
            "MAIL_ENCRYPTION=null".to_string(),
        ]);
    }
    if settings.meilisearch_enabled {
        lines.extend([
            "SCOUT_DRIVER=meilisearch".to_string(),
            format!(
                "MEILISEARCH_HOST=http://127.0.0.1:{}",
                settings.meilisearch_port
            ),
            // Development mode runs without a master key.
            "MEILISEARCH_KEY=".to_string(),
        ]);
    }
    lines.join("\n")
}

/// Agent-facing description of the add-ons.
pub fn environment_json() -> serde_json::Value {
    let settings = AppSettings::load().addons;
    let statuses = statuses().unwrap_or_default();
    json!({
        "services": statuses.iter().map(|status| json!({
            "name": status.kind.key(),
            "enabled": status.enabled,
            "installed": status.installed,
            "state": status.state,
            "host": "127.0.0.1",
            "port": status.port,
            "web_url": status.web_port.map(|port| format!("http://127.0.0.1:{port}/")),
        })).collect::<Vec<_>>(),
        "dotenv": dotenv(&settings),
        "note": "Mailpit catches every message sent to its SMTP port and never delivers it. Laravel needs the predis/predis package for REDIS_CLIENT=predis, or the phpredis extension. Laravel Scout with Meilisearch needs laravel/scout and meilisearch/meilisearch-php http-interop/http-factory-guzzle; Meilisearch runs in development mode without a master key.",
    })
}

/// Apply a port/enable change to the add-on settings, validated against
/// the whole configuration.
fn apply_change(
    settings: &mut AppSettings,
    kind: AddonKind,
    change: &AddonChange,
) -> Result<(), String> {
    let mut next = settings.clone();
    if let Some(enabled) = change.enabled {
        kind.set_enabled(&mut next.addons, enabled);
    }
    if let Some(port) = change.port {
        match kind {
            AddonKind::Redis => next.addons.redis_port = port,
            AddonKind::Mailpit => next.addons.mailpit_smtp_port = port,
            AddonKind::Meilisearch => next.addons.meilisearch_port = port,
        }
    }
    if let Some(web_port) = change.web_port {
        if kind != AddonKind::Mailpit {
            return Err(format!("{} has no web port", kind.display_name()));
        }
        next.addons.mailpit_web_port = web_port;
    }
    for port in [change.port, change.web_port].into_iter().flatten() {
        if port < 1024 {
            return Err(format!(
                "Port {port} needs administrator rights. Use a port from 1024 to 65535."
            ));
        }
    }
    next.validate().map_err(|errors| errors.join("; "))?;
    *settings = next;
    Ok(())
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct AddonChange {
    pub enabled: Option<bool>,
    pub port: Option<u16>,
    pub web_port: Option<u16>,
}

/// PHP reads the mail route and add-on environment at start, so restart it
/// when they change. A stopped PHP picks them up on its next start.
async fn restart_php_if_running(app: &AppHandle) {
    let _ = run_with_manager(app, |manager| {
        manager.update_health();
        if manager.status(ServiceType::PhpFpm).is_running() {
            let _ = manager.restart(ServiceType::PhpFpm);
        }
    })
    .await;
}

fn emit_progress(app: &AppHandle, kind: AddonKind, percent: u8) {
    let _ = app.emit(
        "addon-install-progress",
        json!({ "kind": kind.key(), "percent": percent }),
    );
}

// ---------------------------------------------------------------------------
// IPC commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_addons_status() -> Result<Vec<AddonStatus>, String> {
    tauri::async_runtime::spawn_blocking(statuses)
        .await
        .map_err(|e| format!("Add-on status task failed: {}", e))?
}

#[tauri::command]
pub async fn install_addon(kind: AddonKind, app: AppHandle) -> Result<Vec<AddonStatus>, String> {
    let package = catalog::current_package(kind);
    let Some(download) = package.download else {
        return Err(
            match catalog::system_install_hint(kind, std::env::consts::OS) {
                Some(hint) => format!(
                    "CHAMP cannot download {} for this system. Install it with: {}",
                    kind.display_name(),
                    hint
                ),
                None => format!("{} is not available for this system.", kind.display_name()),
            },
        );
    };
    let Some(_guard) = InstallGuard::acquire(kind) else {
        return Err(format!(
            "{} is already being installed.",
            kind.display_name()
        ));
    };
    let paths = get_app_data_paths()?;

    // A running copy locks its files on Windows; reinstalling replaces them.
    let (state, _, _) = runtime::snapshot(kind);
    let was_running = state.is_running();
    if was_running {
        tauri::async_runtime::spawn_blocking(move || runtime::stop(kind))
            .await
            .map_err(|e| format!("Add-on stop task failed: {}", e))??;
    }

    let progress_app = app.clone();
    install::install(
        kind,
        package.version,
        download,
        &paths.runtime_dir,
        move |percent| emit_progress(&progress_app, kind, percent),
    )
    .await?;

    runtime::clear_error(kind);
    if was_running {
        let _ = tauri::async_runtime::spawn_blocking(move || start_blocking(kind)).await;
    }
    if kind == AddonKind::Mailpit && AppSettings::load().addons.mailpit_enabled {
        restart_php_if_running(&app).await;
    }
    get_addons_status().await
}

#[tauri::command]
pub async fn start_addon(kind: AddonKind) -> Result<Vec<AddonStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || start_blocking(kind))
        .await
        .map_err(|e| format!("Add-on start task failed: {}", e))??;
    get_addons_status().await
}

#[tauri::command]
pub async fn stop_addon(kind: AddonKind) -> Result<Vec<AddonStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || stop_blocking(kind))
        .await
        .map_err(|e| format!("Add-on stop task failed: {}", e))??;
    get_addons_status().await
}

/// Enable/disable an add-on or change its ports.
///
/// Enabling starts it when the stack is running (as Start would have);
/// disabling stops it. A port change restarts a running add-on. PHP restarts
/// when the mail route or the add-on environment changed.
#[tauri::command]
pub async fn configure_addon(
    kind: AddonKind,
    change: AddonChange,
    app: AppHandle,
) -> Result<Vec<AddonStatus>, String> {
    let before = AppSettings::load().addons;
    let mut validation = Ok(());
    let saved = AppSettings::update(|settings| {
        validation = apply_change(settings, kind, &change);
    })?;
    validation?;
    let after = saved.addons;
    if before == after {
        return get_addons_status().await;
    }

    let enabled = kind.enabled(&after);
    let ports_changed = runtime::ports_for(kind, &before) != runtime::ports_for(kind, &after);
    let (state, _, _) = runtime::snapshot(kind);
    let stack_running = run_with_manager(&app, |manager| {
        manager.update_health();
        manager.status(ServiceType::Caddy).is_running()
            || manager.status(ServiceType::PhpFpm).is_running()
    })
    .await
    .unwrap_or(false);

    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if !enabled {
            runtime::stop(kind)?;
            runtime::clear_error(kind);
            return Ok(());
        }
        if state.is_running() && ports_changed {
            runtime::stop(kind)?;
            return start_blocking(kind);
        }
        if !state.is_running() && stack_running {
            return start_blocking(kind);
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("Add-on task failed: {}", e))?;

    if env_pairs(&before) != env_pairs(&after) {
        restart_php_if_running(&app).await;
    }
    result?;
    get_addons_status().await
}

/// Number of messages Mailpit is holding, for the inbox badge.
#[tauri::command]
pub async fn get_mailpit_message_count() -> Result<Option<u64>, String> {
    let (state, _, ports) = runtime::snapshot(AddonKind::Mailpit);
    let Some(web_port) = ports.get(1).copied().filter(|_| state.is_running()) else {
        return Ok(None);
    };
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?
        .get(format!(
            "http://127.0.0.1:{web_port}/api/v1/messages?limit=1"
        ))
        .send()
        .await;
    let Ok(response) = response else {
        return Ok(None);
    };
    let body: serde_json::Value = response.json().await.unwrap_or_default();
    Ok(body.get("total").and_then(serde_json::Value::as_u64))
}
