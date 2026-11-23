use crate::runtime::{locator::get_app_data_paths, packages::PackageSelection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

const DEFAULT_AGENT_CONNECTOR_PORT: u16 = 47823;

/// Serializes read-modify-write cycles of `settings.json`.
static SETTINGS_WRITE_LOCK: Mutex<()> = Mutex::new(());

pub const DEFAULT_PORTS: Ports = Ports {
    web: 8080,
    php: 9000,
    mysql: 3306,
    postgresql: 5432,
};

#[derive(Debug, Clone, Copy)]
pub struct Ports {
    pub web: u16,
    pub php: u16,
    pub mysql: u16,
    pub postgresql: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub web_port: u16,
    pub php_port: u16,
    pub mysql_port: u16,
    #[serde(default = "default_postgresql_port")]
    pub postgresql_port: u16,
    /// Port for local HTTPS sites (`https://<name>.localhost:<port>`).
    #[serde(default = "default_https_port")]
    pub https_port: u16,
    #[serde(default)]
    pub auto_start_services: bool,
    /// Serve every project folder at `http://<folder>.localhost:<port>/` too.
    /// Off: projects open only at `http://localhost:<port>/<folder>/`.
    #[serde(default = "default_true")]
    pub project_domains: bool,
    #[serde(default)]
    pub package_selection: PackageSelection,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_true")]
    pub sound_enabled: bool,
    /// Check GitHub Releases for a newer CHAMP once a day on startup.
    #[serde(default = "default_true")]
    pub auto_check_updates: bool,
    /// Local MCP server for AI coding agents (see `agent` module).
    #[serde(default)]
    pub agent_connector_enabled: bool,
    /// Hide and refuse agent tools that start/stop services.
    #[serde(default)]
    pub agent_connector_read_only: bool,
    #[serde(default = "default_agent_connector_port")]
    pub agent_connector_port: u16,
    /// Optional Redis / Mailpit / Meilisearch services (see `addons`). Written by
    /// `configure_addon`; `save_settings` keeps what is on disk.
    #[serde(default)]
    pub addons: AddonSettings,
    /// Step debugging. Written by `set_xdebug`; `save_settings` keeps what is on disk.
    #[serde(default)]
    pub xdebug: XdebugSettings,
    /// Per-project PHP background commands (queue workers, schedulers).
    /// Written by the worker commands; `save_settings` keeps what is on disk.
    #[serde(default)]
    pub workers: Vec<ProjectWorker>,
}

pub const DEFAULT_XDEBUG_CLIENT_PORT: u16 = 9003;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct XdebugSettings {
    pub enabled: bool,
    /// Port the IDE listens on (CHAMP does not bind it).
    pub client_port: u16,
}

impl Default for XdebugSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            client_port: DEFAULT_XDEBUG_CLIENT_PORT,
        }
    }
}

/// A long-running PHP command for one project, run with CHAMP's PHP CLI in
/// the project folder (for example `artisan queue:work`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectWorker {
    /// Lowercase letters, digits, and hyphens.
    pub id: String,
    /// Project folder name.
    pub project: String,
    pub name: String,
    /// Arguments after `php`; the first is a script inside the project
    /// (`artisan`, `bin/console`, `spark`, ...). No shell is involved.
    pub args: Vec<String>,
    /// Start with the stack.
    #[serde(default)]
    pub enabled: bool,
}

/// Which optional services start with the stack, and their ports.
///
/// `services` is keyed by the built-in add-on key (`redis`, `mailpit`, ...);
/// a missing entry or port means "off, default port" (defaults live in
/// `addons::spec`). `custom` holds services the user registered with their
/// own binary.
///
/// Add-ons never move to a fallback port: an app's `.env` names the port, so a
/// busy port is reported instead of silently picking another one.
///
/// Files written before 1.5 used flat keys (`redis_enabled`, `redis_port`,
/// `mailpit_smtp_port`, ...); they are migrated on load and saved in the new
/// shape. Keys of removed add-ons (Memcached) are ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "AddonSettingsFile")]
pub struct AddonSettings {
    pub services: BTreeMap<String, AddonConfig>,
    pub custom: Vec<CustomAddon>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AddonConfig {
    pub enabled: bool,
    /// Port per role (`main`, `smtp`, `web`, ...). Missing roles use the default.
    pub ports: BTreeMap<String, u16>,
}

/// A service the user runs from their own binary. CHAMP starts and stops it
/// and waits for its port, but never downloads, verifies, or updates it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomAddon {
    /// Lowercase letters, digits, and hyphens; never a built-in key.
    pub id: String,
    pub name: String,
    /// Absolute path to the executable.
    pub command: String,
    /// Arguments; `{port}` and `{data_dir}` are replaced at start.
    #[serde(default)]
    pub args: Vec<String>,
    pub port: u16,
    /// The port serves a page in the browser.
    #[serde(default)]
    pub web: bool,
    #[serde(default)]
    pub enabled: bool,
}

/// Accepts both the current shape and the flat pre-1.5 keys.
#[derive(Deserialize, Default)]
#[serde(default)]
struct AddonSettingsFile {
    services: BTreeMap<String, AddonConfig>,
    custom: Vec<CustomAddon>,
    redis_enabled: Option<bool>,
    redis_port: Option<u16>,
    mailpit_enabled: Option<bool>,
    mailpit_smtp_port: Option<u16>,
    mailpit_web_port: Option<u16>,
    meilisearch_enabled: Option<bool>,
    meilisearch_port: Option<u16>,
}

impl From<AddonSettingsFile> for AddonSettings {
    fn from(file: AddonSettingsFile) -> Self {
        let mut services = file.services;
        let legacy = [
            ("redis", file.redis_enabled, vec![("main", file.redis_port)]),
            (
                "mailpit",
                file.mailpit_enabled,
                vec![
                    ("smtp", file.mailpit_smtp_port),
                    ("web", file.mailpit_web_port),
                ],
            ),
            (
                "meilisearch",
                file.meilisearch_enabled,
                vec![("main", file.meilisearch_port)],
            ),
        ];
        for (key, enabled, ports) in legacy {
            let has_legacy = enabled.is_some() || ports.iter().any(|(_, port)| port.is_some());
            // A file in the new shape wins over stray flat keys.
            if !has_legacy || services.contains_key(key) {
                continue;
            }
            services.insert(
                key.to_string(),
                AddonConfig {
                    enabled: enabled.unwrap_or(false),
                    ports: ports
                        .into_iter()
                        .filter_map(|(role, port)| port.map(|port| (role.to_string(), port)))
                        .collect(),
                },
            );
        }
        Self {
            services,
            custom: file.custom,
        }
    }
}

impl AddonSettings {
    pub fn enabled(&self, key: &str) -> bool {
        self.services.get(key).is_some_and(|config| config.enabled)
    }

    pub fn set_enabled(&mut self, key: &str, enabled: bool) {
        self.services.entry(key.to_string()).or_default().enabled = enabled;
    }

    /// The saved port for `role`, or `default` when none is saved.
    pub fn port(&self, key: &str, role: &str, default: u16) -> u16 {
        self.services
            .get(key)
            .and_then(|config| config.ports.get(role).copied())
            .unwrap_or(default)
    }

    pub fn set_port(&mut self, key: &str, role: &str, port: u16) {
        self.services
            .entry(key.to_string())
            .or_default()
            .ports
            .insert(role.to_string(), port);
    }

    pub fn custom(&self, id: &str) -> Option<&CustomAddon> {
        self.custom.iter().find(|addon| addon.id == id)
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            web_port: DEFAULT_PORTS.web,
            php_port: DEFAULT_PORTS.php,
            mysql_port: DEFAULT_PORTS.mysql,
            postgresql_port: DEFAULT_PORTS.postgresql,
            https_port: default_https_port(),
            auto_start_services: false,
            project_domains: true,
            package_selection: PackageSelection::default(),
            language: default_language(),
            sound_enabled: true,
            auto_check_updates: true,
            agent_connector_enabled: false,
            agent_connector_read_only: false,
            agent_connector_port: default_agent_connector_port(),
            addons: AddonSettings::default(),
            xdebug: XdebugSettings::default(),
            workers: Vec::new(),
        }
    }
}

impl AppSettings {
    /// Get the path to the settings file
    fn settings_path() -> Option<PathBuf> {
        get_app_data_paths()
            .map(|paths| paths.config_dir.join("settings.json"))
            .ok()
    }

    /// Load settings from file, or return defaults if file doesn't exist
    pub fn load() -> Self {
        let path = match Self::settings_path() {
            Some(p) => p,
            None => return Self::default(),
        };

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<Self>(&content) {
                Ok(mut settings) => {
                    settings.package_selection = settings.package_selection.normalized();
                    settings
                }
                Err(e) => {
                    eprintln!("Failed to parse settings file: {}, using defaults", e);
                    // The next save replaces this file, so keep what was in it.
                    let _ = fs::copy(&path, path.with_extension("json.corrupt"));
                    Self::default()
                }
            },
            Err(e) => {
                eprintln!("Failed to read settings file: {}, using defaults", e);
                Self::default()
            }
        }
    }

    /// Save settings to file
    fn save(&self) -> Result<(), String> {
        let path = Self::settings_path()
            .ok_or_else(|| "Cannot determine settings file path".to_string())?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        crate::fs_util::atomic_write(&path, content)
            .map_err(|e| format!("Failed to write settings file: {}", e))?;

        Ok(())
    }

    /// Change settings based on what is on disk right now, under a lock.
    ///
    /// Use this for every write. A `load()` followed later by `save()` silently
    /// reverts anything another command saved in between, such as a language
    /// change made during a long runtime download.
    pub fn update(change: impl FnOnce(&mut Self)) -> Result<Self, String> {
        let _guard = SETTINGS_WRITE_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut settings = Self::load();
        change(&mut settings);
        settings.save()?;
        Ok(settings)
    }

    /// Validate settings (check for port conflicts, valid paths, etc.)
    pub fn validate(&self) -> Result<Vec<String>, Vec<String>> {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        // Check for port conflicts using the shared availability primitive.
        if !crate::config::is_port_available(self.web_port) {
            warnings.push(format!("Web port {} may already be in use.", self.web_port));
        }

        if !crate::config::is_port_available(self.php_port) {
            warnings.push(format!(
                "PHP-FPM port {} may already be in use.",
                self.php_port
            ));
        }

        if !crate::config::is_port_available(self.mysql_port) {
            warnings.push(format!(
                "MySQL port {} may already be in use.",
                self.mysql_port
            ));
        }

        if !crate::config::is_port_available(self.postgresql_port) {
            warnings.push(format!(
                "PostgreSQL port {} may already be in use.",
                self.postgresql_port
            ));
        }

        // Check for valid port ranges
        if self.web_port == 0
            || self.php_port == 0
            || self.mysql_port == 0
            || self.postgresql_port == 0
            || self.https_port == 0
        {
            errors.push("Port numbers must be greater than 0".to_string());
        }

        // Check for duplicate ports across services. Zero ports are handled by the
        // ">0" check above, so ignore them here to avoid duplicate/confusing errors.
        let mut labeled_ports: Vec<(String, u16)> = [
            ("HTTP/Web", self.web_port),
            ("PHP-FPM", self.php_port),
            ("MySQL", self.mysql_port),
            ("PostgreSQL", self.postgresql_port),
            ("HTTPS", self.https_port),
        ]
        .into_iter()
        .map(|(label, port)| (label.to_string(), port))
        .collect();
        if self.agent_connector_enabled {
            labeled_ports.push(("AI agent connector".to_string(), self.agent_connector_port));
        }
        let addon_ports = crate::addons::enabled_ports(&self.addons);
        if addon_ports.iter().any(|(_, port)| *port == 0) {
            errors.push("Add-on port numbers must be greater than 0".to_string());
        }
        labeled_ports.extend(addon_ports);
        // Compare each unique pair once, in a stable, deterministic order.
        for i in 0..labeled_ports.len() {
            for j in (i + 1)..labeled_ports.len() {
                let (label_a, port_a) = &labeled_ports[i];
                let (label_b, port_b) = &labeled_ports[j];
                if *port_a != 0 && port_a == port_b {
                    errors.push(format!(
                        "Ports must be unique: {} and {} are both set to {}",
                        label_a, label_b, port_a
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(warnings)
        } else {
            Err(errors)
        }
    }
}

fn default_postgresql_port() -> u16 {
    DEFAULT_PORTS.postgresql
}

fn default_https_port() -> u16 {
    8443
}

fn default_language() -> String {
    "en".to_string()
}

fn default_true() -> bool {
    true
}

fn default_agent_connector_port() -> u16 {
    DEFAULT_AGENT_CONNECTOR_PORT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_duplicate_ports() {
        let mut settings = AppSettings::default();
        settings.mysql_port = settings.web_port;

        let result = settings.validate();
        assert!(result.is_err(), "expected duplicate ports to be rejected");

        let errors = result.unwrap_err();
        let port = settings.web_port;
        assert!(
            errors.iter().any(|e| {
                e.contains("HTTP/Web") && e.contains("MySQL") && e.contains(&port.to_string())
            }),
            "expected an error mentioning both services and the port, got: {:?}",
            errors
        );
    }

    #[test]
    fn validate_checks_the_agent_connector_port_only_when_enabled() {
        let mut settings = AppSettings::default();
        settings.agent_connector_port = settings.mysql_port;
        assert!(
            settings.validate().is_ok(),
            "a disabled connector must not block saving"
        );

        settings.agent_connector_enabled = true;
        let errors = settings.validate().unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("AI agent connector") && e.contains("MySQL")),
            "{errors:?}"
        );
    }

    #[test]
    fn validate_rejects_a_zero_port_for_an_enabled_addon() {
        let mut settings = AppSettings::default();
        settings.addons.set_port("redis", "main", 0);
        assert!(
            settings.validate().is_ok(),
            "disabled add-ons are not checked"
        );

        settings.addons.set_enabled("redis", true);
        let errors = settings.validate().unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("Add-on port")),
            "{errors:?}"
        );
    }

    #[test]
    fn validate_allows_distinct_ports() {
        // Default ports are all distinct (web 8080, php 9000, mysql 3306, postgresql 5432).
        let settings = AppSettings::default();
        assert!(
            settings.validate().is_ok(),
            "expected distinct default ports to validate successfully"
        );
    }
}
