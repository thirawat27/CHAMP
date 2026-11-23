//! Optional services: Redis (cache/queue), Mailpit (catches outgoing mail so
//! nothing is really sent), Meilisearch (full-text search), and custom
//! services the user registers with their own binary.
//!
//! Add-ons are independent of the core stack: they have their own state and
//! never block or fail a stack start. Enabled add-ons start and stop with the
//! stack. Built-ins listen on 127.0.0.1 only.
//!
//! What each built-in is lives in `spec.rs`; this module stays generic.

mod catalog;
mod custom;
mod install;
mod runtime;
pub mod spec;
pub mod workers;

#[cfg(test)]
mod tests;

use crate::config::{AddonSettings, AppSettings, CustomAddon};
use crate::process::runner::run_with_manager;
use crate::process::{ServiceState, ServiceType};
use crate::runtime::locator::{get_app_data_paths, AppDataPaths};
use serde::{Deserialize, Serialize};
use serde_json::json;
use spec::{spec, StopMethod, SPECS};
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
    Centrifugo,
    Rustfs,
}

pub const ALL_ADDONS: [AddonKind; 5] = [
    AddonKind::Redis,
    AddonKind::Mailpit,
    AddonKind::Meilisearch,
    AddonKind::Centrifugo,
    AddonKind::Rustfs,
];

impl AddonKind {
    pub fn key(self) -> &'static str {
        spec(self).key
    }

    pub fn display_name(self) -> &'static str {
        spec(self).name
    }

    pub fn from_key(key: &str) -> Option<Self> {
        SPECS
            .iter()
            .find(|spec| spec.key == key)
            .map(|spec| spec.kind)
    }

    fn log_name(self) -> String {
        format!("{}.log", self.key())
    }
}

/// A built-in add-on or a custom one, resolved from its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddonId {
    Builtin(AddonKind),
    Custom(CustomAddon),
}

impl AddonId {
    pub fn resolve(id: &str, settings: &AddonSettings) -> Result<Self, String> {
        if let Some(kind) = AddonKind::from_key(id) {
            return Ok(Self::Builtin(kind));
        }
        settings
            .custom(id)
            .cloned()
            .map(Self::Custom)
            .ok_or_else(|| format!("Unknown add-on `{id}`."))
    }

    pub fn key(&self) -> &str {
        match self {
            Self::Builtin(kind) => kind.key(),
            Self::Custom(addon) => &addon.id,
        }
    }

    fn enabled(&self, settings: &AddonSettings) -> bool {
        match self {
            Self::Builtin(kind) => settings.enabled(kind.key()),
            Self::Custom(addon) => settings
                .custom(&addon.id)
                .is_some_and(|saved| saved.enabled),
        }
    }

    fn log_name(&self) -> String {
        match self {
            Self::Builtin(kind) => kind.log_name(),
            Self::Custom(addon) => format!("addon-{}.log", addon.id),
        }
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
    /// A binary the user registered; CHAMP never verifies or updates it.
    Custom,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddonPort {
    pub role: String,
    pub label: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddonStatus {
    /// Built-in key (`redis`) or custom add-on id.
    pub kind: String,
    pub name: String,
    pub category: String,
    /// Registered by the user with their own binary.
    pub custom: bool,
    /// Version CHAMP downloads for this platform (empty for custom add-ons).
    pub version: String,
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
    /// The primary port.
    pub port: u16,
    /// Mailpit's web inbox port (a second, separately configured port).
    pub web_port: Option<u16>,
    /// Every port, the primary one first.
    pub ports: Vec<AddonPort>,
    /// A browser page (Mailpit inbox, Meilisearch preview), once it is running.
    pub web_url: Option<String>,
    pub error: Option<String>,
    pub log_path: Option<PathBuf>,
    /// The saved definition of a custom add-on, for editing.
    pub definition: Option<CustomAddon>,
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

fn data_dir(id: &AddonId, paths: &AppDataPaths) -> PathBuf {
    match id {
        AddonId::Builtin(kind) => paths.base_dir.join("addons").join(kind.key()),
        AddonId::Custom(addon) => paths.base_dir.join("addons").join("custom").join(&addon.id),
    }
}

/// The configured ports of `kind`, in spec order (the primary one first).
pub(super) fn ports_for(kind: AddonKind, settings: &AddonSettings) -> Vec<u16> {
    spec(kind)
        .ports
        .iter()
        .map(|port| settings.port(kind.key(), port.role, port.default))
        .collect()
}

fn id_ports(id: &AddonId, settings: &AddonSettings) -> Vec<u16> {
    match id {
        AddonId::Builtin(kind) => ports_for(*kind, settings),
        AddonId::Custom(addon) => vec![addon.port],
    }
}

/// Ports of the enabled add-ons, labeled for validation messages.
pub fn enabled_ports(settings: &AddonSettings) -> Vec<(String, u16)> {
    let mut ports = Vec::new();
    for spec in SPECS.iter().filter(|spec| settings.enabled(spec.key)) {
        for port in spec.ports {
            ports.push((
                port.label.to_string(),
                settings.port(spec.key, port.role, port.default),
            ));
        }
    }
    for addon in settings.custom.iter().filter(|addon| addon.enabled) {
        ports.push((addon.name.clone(), addon.port));
    }
    ports
}

fn builtin_status(kind: AddonKind, settings: &AddonSettings, paths: &AppDataPaths) -> AddonStatus {
    let spec = spec(kind);
    let package = catalog::current_package(kind);
    let installed_version = install::installed_version(&paths.runtime_dir, kind);
    let resolved = resolve_binary(kind, paths);
    let (state, error, running_ports) = runtime::snapshot(kind.key());
    let configured = ports_for(kind, settings);
    // A running add-on keeps the ports it started on until it restarts.
    let ports = if state.is_running() && running_ports.len() == configured.len() {
        running_ports
    } else {
        configured
    };
    let port_at = |role: &str| {
        spec.ports
            .iter()
            .position(|port| port.role == role)
            .map(|index| ports[index])
    };
    let log_path = paths.logs_dir.join(kind.log_name());
    AddonStatus {
        kind: kind.key().to_string(),
        name: spec.name.to_string(),
        category: spec.category.to_string(),
        custom: false,
        version: package.version.to_string(),
        update_available: installed_version
            .as_deref()
            .is_some_and(|installed| installed != package.version && package.download.is_some()),
        installed_version,
        installed: resolved.is_some(),
        source: resolved.map(|(_, source)| source),
        downloadable: package.download.is_some(),
        install_hint: catalog::system_install_hint(kind, std::env::consts::OS),
        installing: is_installing(kind),
        enabled: settings.enabled(kind.key()),
        state: state.clone(),
        port: ports[0],
        web_port: port_at("web"),
        ports: spec
            .ports
            .iter()
            .zip(&ports)
            .map(|(spec_port, port)| AddonPort {
                role: spec_port.role.to_string(),
                label: spec_port.label.to_string(),
                port: *port,
            })
            .collect(),
        web_url: spec
            .web_role
            .and_then(port_at)
            .filter(|_| state.is_running())
            .map(|port| format!("http://127.0.0.1:{port}/")),
        error,
        log_path: log_path.exists().then_some(log_path),
        definition: None,
    }
}

fn custom_status(addon: &CustomAddon, paths: &AppDataPaths) -> AddonStatus {
    let (state, error, _) = runtime::snapshot(&addon.id);
    let id = AddonId::Custom(addon.clone());
    let log_path = paths.logs_dir.join(id.log_name());
    AddonStatus {
        kind: addon.id.clone(),
        name: addon.name.clone(),
        category: "custom".to_string(),
        custom: true,
        version: String::new(),
        installed_version: None,
        installed: PathBuf::from(&addon.command).is_file(),
        source: Some(AddonSource::Custom),
        downloadable: false,
        install_hint: None,
        update_available: false,
        installing: false,
        enabled: addon.enabled,
        state: state.clone(),
        port: addon.port,
        web_port: None,
        ports: vec![AddonPort {
            role: "main".to_string(),
            label: addon.name.clone(),
            port: addon.port,
        }],
        web_url: (addon.web && state.is_running())
            .then(|| format!("http://127.0.0.1:{}/", addon.port)),
        error,
        log_path: log_path.exists().then_some(log_path),
        definition: Some(addon.clone()),
    }
}

pub fn statuses() -> Result<Vec<AddonStatus>, String> {
    let paths = get_app_data_paths()?;
    let settings = AppSettings::load().addons;
    let mut list: Vec<AddonStatus> = ALL_ADDONS
        .into_iter()
        .map(|kind| builtin_status(kind, &settings, &paths))
        .collect();
    list.extend(
        settings
            .custom
            .iter()
            .map(|addon| custom_status(addon, &paths)),
    );
    Ok(list)
}

fn launch_plan(
    id: &AddonId,
    settings: &AddonSettings,
    paths: &AppDataPaths,
) -> Result<runtime::LaunchPlan, String> {
    let data_dir = data_dir(id, paths);
    let log_path = paths.logs_dir.join(id.log_name());
    let ports = id_ports(id, settings);
    match id {
        AddonId::Builtin(kind) => {
            let kind = *kind;
            let Some((binary, source)) = resolve_binary(kind, paths) else {
                return Err(
                    match catalog::system_install_hint(kind, std::env::consts::OS) {
                        Some(hint) => format!(
                            "{} is not installed. Install it with: {}",
                            kind.display_name(),
                            hint
                        ),
                        None => format!(
                            "{} is not installed yet. Open Add-on services and press Install.",
                            kind.display_name()
                        ),
                    },
                );
            };
            spec::prepare(kind, &ports, &data_dir)?;
            Ok(runtime::LaunchPlan {
                name: kind.display_name().to_string(),
                args: spec::launch_args(kind, &ports, &data_dir),
                binary,
                data_dir,
                log_path,
                ports,
                stop: spec(kind).stop,
                owned_binary: source == AddonSource::Champ,
                env: spec::launch_env(kind)
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect(),
            })
        }
        AddonId::Custom(addon) => {
            custom::check_runnable(addon)?;
            Ok(runtime::LaunchPlan {
                name: addon.name.clone(),
                binary: PathBuf::from(&addon.command),
                args: custom::expand_args(addon, &data_dir),
                data_dir,
                log_path,
                ports,
                stop: StopMethod::Terminate,
                owned_binary: false,
                env: Vec::new(),
            })
        }
    }
}

/// Start one add-on (blocking: waits for its ports).
pub fn start_blocking(id: &str) -> Result<(), String> {
    let paths = get_app_data_paths()?;
    let settings = AppSettings::load().addons;
    let id = AddonId::resolve(id, &settings)?;
    let plan = match launch_plan(&id, &settings, &paths) {
        Ok(plan) => plan,
        Err(message) => {
            runtime::set_error(id.key(), &message);
            return Err(message);
        }
    };
    std::fs::create_dir_all(&paths.logs_dir)
        .map_err(|e| format!("Failed to create logs folder: {}", e))?;
    runtime::start(id.key(), plan)
}

pub fn stop_blocking(id: &str) -> Result<(), String> {
    runtime::stop(id)
}

/// Every add-on id: built-ins first, then custom ones.
fn all_ids(settings: &AddonSettings) -> Vec<AddonId> {
    ALL_ADDONS
        .into_iter()
        .map(AddonId::Builtin)
        .chain(settings.custom.iter().cloned().map(AddonId::Custom))
        .collect()
}

/// Start every enabled add-on, then the project workers that start with the
/// stack. Failures are kept in each status and returned for logging, but never
/// stop the others from starting.
pub fn start_enabled_blocking() -> Vec<String> {
    let settings = AppSettings::load().addons;
    let mut errors: Vec<String> = all_ids(&settings)
        .into_iter()
        .filter(|id| id.enabled(&settings))
        .filter_map(|id| start_blocking(id.key()).err())
        .collect();
    errors.extend(workers::start_enabled_blocking());
    errors
}

/// Stop every add-on and worker, enabled or not (one may have been started by hand).
pub fn stop_all_blocking() {
    let ids: Vec<String> = runtime::lock().keys().cloned().collect();
    for id in ids {
        let _ = runtime::stop(&id);
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
    let endpoints = AddonEndpoints::from_settings(&settings);
    let mail = endpoints.mail?;
    let sendmail_path = get_app_data_paths()
        .ok()
        .and_then(|paths| resolve_binary(AddonKind::Mailpit, &paths))
        .and_then(|(binary, _)| sendmail_command(&binary, mail.smtp_port));
    Some(PhpMailRoute {
        smtp_port: mail.smtp_port,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailEndpoint {
    pub smtp_port: u16,
    pub web_port: u16,
}

/// Where the enabled add-ons listen. Env profiles and agents read this.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AddonEndpoints {
    pub redis: Option<u16>,
    pub mail: Option<MailEndpoint>,
    pub meilisearch: Option<u16>,
    /// Centrifugo's HTTP port (client WebSocket, server API, admin UI).
    pub centrifugo: Option<u16>,
    /// RustFS's S3 API port.
    pub s3: Option<u16>,
    /// `(id, name, port)` of enabled custom add-ons.
    pub custom: Vec<(String, String, u16)>,
}

impl AddonEndpoints {
    pub fn from_settings(settings: &AddonSettings) -> Self {
        let port = |kind: AddonKind, role: &str| {
            let spec = spec(kind);
            spec.ports
                .iter()
                .find(|port| port.role == role)
                .map(|port| settings.port(spec.key, role, port.default))
                .filter(|_| settings.enabled(spec.key))
        };
        Self {
            redis: port(AddonKind::Redis, "main"),
            mail: port(AddonKind::Mailpit, "smtp")
                .zip(port(AddonKind::Mailpit, "web"))
                .map(|(smtp_port, web_port)| MailEndpoint {
                    smtp_port,
                    web_port,
                }),
            meilisearch: port(AddonKind::Meilisearch, "main"),
            centrifugo: port(AddonKind::Centrifugo, "main"),
            s3: port(AddonKind::Rustfs, "main"),
            custom: settings
                .custom
                .iter()
                .filter(|addon| addon.enabled)
                .map(|addon| (addon.id.clone(), addon.name.clone(), addon.port))
                .collect(),
        }
    }
}

/// `KEY=value` pairs for enabled add-ons, set in PHP's environment
/// (`getenv('CHAMP_REDIS_PORT')`) whatever framework the project uses.
pub fn env_pairs(settings: &AddonSettings) -> Vec<(String, String)> {
    let endpoints = AddonEndpoints::from_settings(settings);
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut push = |key: &str, value: String| pairs.push((key.to_string(), value));
    if let Some(port) = endpoints.redis {
        push("CHAMP_REDIS_HOST", "127.0.0.1".to_string());
        push("CHAMP_REDIS_PORT", port.to_string());
        push("CHAMP_REDIS_URL", format!("redis://127.0.0.1:{port}"));
    }
    if let Some(mail) = endpoints.mail {
        push("CHAMP_MAIL_HOST", "127.0.0.1".to_string());
        push("CHAMP_MAIL_PORT", mail.smtp_port.to_string());
        push(
            "CHAMP_MAILPIT_URL",
            format!("http://127.0.0.1:{}/", mail.web_port),
        );
    }
    if let Some(port) = endpoints.meilisearch {
        push("CHAMP_MEILISEARCH_HOST", format!("http://127.0.0.1:{port}"));
    }
    if let Some(port) = endpoints.centrifugo {
        push("CHAMP_CENTRIFUGO_URL", format!("http://127.0.0.1:{port}"));
        push(
            "CHAMP_CENTRIFUGO_API_URL",
            format!("http://127.0.0.1:{port}/api"),
        );
        push(
            "CHAMP_CENTRIFUGO_WS_URL",
            format!("ws://127.0.0.1:{port}/connection/websocket"),
        );
    }
    if let Some(port) = endpoints.s3 {
        push("CHAMP_S3_ENDPOINT", format!("http://127.0.0.1:{port}"));
        push("CHAMP_S3_KEY", spec::S3_ACCESS_KEY.to_string());
        push("CHAMP_S3_SECRET", spec::S3_SECRET_KEY.to_string());
        push("CHAMP_S3_REGION", spec::S3_REGION.to_string());
    }
    for (id, _, port) in &endpoints.custom {
        let key = custom::env_prefix(id);
        push(&format!("{key}_HOST"), "127.0.0.1".to_string());
        push(&format!("{key}_PORT"), port.to_string());
    }
    pairs
}

/// Agent-facing description of the add-ons.
pub fn environment_json() -> serde_json::Value {
    let statuses = statuses().unwrap_or_default();
    json!({
        "services": statuses.iter().map(|status| json!({
            "name": status.kind,
            "display_name": status.name,
            "category": status.category,
            "custom": status.custom,
            "enabled": status.enabled,
            "installed": status.installed,
            "state": status.state,
            "host": "127.0.0.1",
            "port": status.port,
            "ports": status.ports,
            "web_url": status.web_url,
        })).collect::<Vec<_>>(),
        "php_environment": env_pairs(&AppSettings::load().addons)
            .into_iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>(),
        "note": "PHP-FPM already receives the CHAMP_* variables above (getenv or $_SERVER), so plain PHP needs no .env. Framework-specific .env lines are in env_profiles. Mailpit catches every message sent to its SMTP port and never delivers it. Meilisearch runs in development mode without a master key. Custom add-ons are user-managed binaries.",
    })
}

/// Apply a port/enable change to the add-on settings, validated against
/// the whole configuration.
fn apply_change(
    settings: &mut AppSettings,
    id: &AddonId,
    change: &AddonChange,
) -> Result<(), String> {
    let mut next = settings.clone();
    for port in [change.port, change.web_port].into_iter().flatten() {
        if port < 1024 {
            return Err(format!(
                "Port {port} needs administrator rights. Use a port from 1024 to 65535."
            ));
        }
    }
    match id {
        AddonId::Builtin(kind) => {
            let spec = spec(*kind);
            if let Some(enabled) = change.enabled {
                next.addons.set_enabled(spec.key, enabled);
            }
            if let Some(port) = change.port {
                next.addons.set_port(spec.key, spec.ports[0].role, port);
            }
            if let Some(web_port) = change.web_port {
                if !spec.ports.iter().any(|port| port.role == "web") {
                    return Err(format!("{} has no web port", spec.name));
                }
                next.addons.set_port(spec.key, "web", web_port);
            }
        }
        AddonId::Custom(addon) => {
            if change.web_port.is_some() {
                return Err(format!("{} has no web port", addon.name));
            }
            let saved = next
                .addons
                .custom
                .iter_mut()
                .find(|saved| saved.id == addon.id)
                .ok_or_else(|| format!("Unknown add-on `{}`.", addon.id))?;
            if let Some(enabled) = change.enabled {
                saved.enabled = enabled;
            }
            if let Some(port) = change.port {
                saved.port = port;
            }
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

async fn stack_is_running(app: &AppHandle) -> bool {
    run_with_manager(app, |manager| {
        manager.update_health();
        manager.status(ServiceType::Caddy).is_running()
            || manager.status(ServiceType::PhpFpm).is_running()
    })
    .await
    .unwrap_or(false)
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
    let (state, _, _) = runtime::snapshot(kind.key());
    let was_running = state.is_running();
    if was_running {
        tauri::async_runtime::spawn_blocking(move || runtime::stop(kind.key()))
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

    runtime::clear_error(kind.key());
    if was_running {
        let _ = tauri::async_runtime::spawn_blocking(move || start_blocking(kind.key())).await;
    }
    if kind == AddonKind::Mailpit && AppSettings::load().addons.enabled(kind.key()) {
        restart_php_if_running(&app).await;
    }
    get_addons_status().await
}

#[tauri::command]
pub async fn start_addon(kind: String) -> Result<Vec<AddonStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || start_blocking(&kind))
        .await
        .map_err(|e| format!("Add-on start task failed: {}", e))??;
    get_addons_status().await
}

#[tauri::command]
pub async fn stop_addon(kind: String) -> Result<Vec<AddonStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || stop_blocking(&kind))
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
    kind: String,
    change: AddonChange,
    app: AppHandle,
) -> Result<Vec<AddonStatus>, String> {
    let before = AppSettings::load().addons;
    let id = AddonId::resolve(&kind, &before)?;
    let mut validation = Ok(());
    let saved = AppSettings::update(|settings| {
        validation = apply_change(settings, &id, &change);
    })?;
    validation?;
    let after = saved.addons;
    if before == after {
        return get_addons_status().await;
    }
    let id = AddonId::resolve(&kind, &after)?;
    let enabled = id.enabled(&after);
    let ports_changed = id_ports(&id, &before) != id_ports(&id, &after);
    let (state, _, _) = runtime::snapshot(id.key());
    let stack_running = stack_is_running(&app).await;

    let key = id.key().to_string();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if !enabled {
            runtime::stop(&key)?;
            runtime::clear_error(&key);
            return Ok(());
        }
        if state.is_running() && ports_changed {
            runtime::stop(&key)?;
            return start_blocking(&key);
        }
        if !state.is_running() && stack_running {
            return start_blocking(&key);
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

/// Add or replace a custom add-on (user-managed binary). A running copy is
/// restarted so the new command, arguments, or port take effect.
#[tauri::command]
pub async fn save_custom_addon(
    addon: CustomAddon,
    app: AppHandle,
) -> Result<Vec<AddonStatus>, String> {
    let before = AppSettings::load().addons;
    let mut validation = Ok(());
    AppSettings::update(|settings| {
        validation = custom::upsert(settings, addon.clone());
    })?;
    validation?;
    let (state, _, _) = runtime::snapshot(&addon.id);
    let id = addon.id.clone();
    if state.is_running() {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            runtime::stop(&id)?;
            start_blocking(&id)
        })
        .await;
    }
    let after = AppSettings::load().addons;
    if env_pairs(&before) != env_pairs(&after) {
        restart_php_if_running(&app).await;
    }
    get_addons_status().await
}

/// Stop and remove a custom add-on. Its data folder is kept.
#[tauri::command]
pub async fn remove_custom_addon(id: String, app: AppHandle) -> Result<Vec<AddonStatus>, String> {
    let before = AppSettings::load().addons;
    if before.custom(&id).is_none() {
        return Err(format!("Unknown custom add-on `{id}`."));
    }
    let stop_id = id.clone();
    tauri::async_runtime::spawn_blocking(move || runtime::stop(&stop_id))
        .await
        .map_err(|e| format!("Add-on stop task failed: {}", e))??;
    runtime::forget(&id);
    AppSettings::update(|settings| settings.addons.custom.retain(|addon| addon.id != id))?;
    if env_pairs(&before) != env_pairs(&AppSettings::load().addons) {
        restart_php_if_running(&app).await;
    }
    get_addons_status().await
}

/// Number of messages Mailpit is holding, for the inbox badge.
#[tauri::command]
pub async fn get_mailpit_message_count() -> Result<Option<u64>, String> {
    let Some(base) = mailpit_api_base() else {
        return Ok(None);
    };
    let Ok(body) = mailpit_get(&format!("{base}/messages?limit=1")).await else {
        return Ok(None);
    };
    Ok(body.get("total").and_then(serde_json::Value::as_u64))
}

/// `http://127.0.0.1:<web>/api/v1` while Mailpit is running.
pub fn mailpit_api_base() -> Option<String> {
    let (state, _, ports) = runtime::snapshot(AddonKind::Mailpit.key());
    let web_port = ports.get(1).copied().filter(|_| state.is_running())?;
    Some(format!("http://127.0.0.1:{web_port}/api/v1"))
}

pub async fn mailpit_get(url: &str) -> Result<serde_json::Value, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Mailpit did not answer: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Mailpit refused the request: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Mailpit sent an unreadable reply: {e}"))
}
