use crate::runtime::{locator::get_app_data_paths, packages::PackageSelection};
use serde::{Deserialize, Serialize};
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
}

pub const DEFAULT_REDIS_PORT: u16 = 6379;
pub const DEFAULT_MAILPIT_SMTP_PORT: u16 = 1025;
pub const DEFAULT_MAILPIT_WEB_PORT: u16 = 8025;
pub const DEFAULT_MEILISEARCH_PORT: u16 = 7700;

/// Which optional services start with the stack, and their ports.
///
/// Add-ons never move to a fallback port: an app's `.env` names the port, so a
/// busy port is reported instead of silently picking another one.
///
/// Settings files from versions that offered Memcached still carry
/// `memcached_enabled` / `memcached_port`; serde ignores those unknown keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AddonSettings {
    pub redis_enabled: bool,
    pub redis_port: u16,
    pub mailpit_enabled: bool,
    pub mailpit_smtp_port: u16,
    pub mailpit_web_port: u16,
    pub meilisearch_enabled: bool,
    pub meilisearch_port: u16,
}

impl Default for AddonSettings {
    fn default() -> Self {
        Self {
            redis_enabled: false,
            redis_port: DEFAULT_REDIS_PORT,
            mailpit_enabled: false,
            mailpit_smtp_port: DEFAULT_MAILPIT_SMTP_PORT,
            mailpit_web_port: DEFAULT_MAILPIT_WEB_PORT,
            meilisearch_enabled: false,
            meilisearch_port: DEFAULT_MEILISEARCH_PORT,
        }
    }
}

impl AddonSettings {
    /// Ports of the enabled add-ons, labeled for validation messages.
    pub fn enabled_ports(&self) -> Vec<(&'static str, u16)> {
        let mut ports = Vec::new();
        if self.redis_enabled {
            ports.push(("Redis", self.redis_port));
        }
        if self.mailpit_enabled {
            ports.push(("Mailpit SMTP", self.mailpit_smtp_port));
            ports.push(("Mailpit web", self.mailpit_web_port));
        }
        if self.meilisearch_enabled {
            ports.push(("Meilisearch", self.meilisearch_port));
        }
        ports
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
            package_selection: PackageSelection::default(),
            language: default_language(),
            sound_enabled: true,
            auto_check_updates: true,
            agent_connector_enabled: false,
            agent_connector_read_only: false,
            agent_connector_port: default_agent_connector_port(),
            addons: AddonSettings::default(),
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
        let mut labeled_ports = vec![
            ("HTTP/Web", self.web_port),
            ("PHP-FPM", self.php_port),
            ("MySQL", self.mysql_port),
            ("PostgreSQL", self.postgresql_port),
            ("HTTPS", self.https_port),
        ];
        if self.agent_connector_enabled {
            labeled_ports.push(("AI agent connector", self.agent_connector_port));
        }
        let addon_ports = self.addons.enabled_ports();
        if addon_ports.iter().any(|(_, port)| *port == 0) {
            errors.push("Add-on port numbers must be greater than 0".to_string());
        }
        labeled_ports.extend(addon_ports);
        // Compare each unique pair once, in a stable, deterministic order.
        for i in 0..labeled_ports.len() {
            for j in (i + 1)..labeled_ports.len() {
                let (label_a, port_a) = labeled_ports[i];
                let (label_b, port_b) = labeled_ports[j];
                if port_a != 0 && port_a == port_b {
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
        settings.addons.redis_port = 0;
        assert!(
            settings.validate().is_ok(),
            "disabled add-ons are not checked"
        );

        settings.addons.redis_enabled = true;
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
