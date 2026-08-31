use crate::runtime::{locator::get_app_data_paths, packages::PackageSelection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const APP_DIR_NAME: &str = "CHAMP";

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
    pub project_root: String,
    #[serde(default)]
    pub auto_start_services: bool,
    #[serde(default)]
    pub package_selection: PackageSelection,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_sound_enabled")]
    pub sound_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            web_port: DEFAULT_PORTS.web,
            php_port: DEFAULT_PORTS.php,
            mysql_port: DEFAULT_PORTS.mysql,
            postgresql_port: DEFAULT_PORTS.postgresql,
            project_root: default_project_root().to_string_lossy().to_string(),
            auto_start_services: false,
            package_selection: PackageSelection::default(),
            language: default_language(),
            sound_enabled: default_sound_enabled(),
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
                Ok(settings) => settings,
                Err(e) => {
                    eprintln!("Failed to parse settings file: {}, using defaults", e);
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
    pub fn save(&self) -> Result<(), String> {
        let path = Self::settings_path()
            .ok_or_else(|| "Cannot determine settings file path".to_string())?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        fs::write(&path, content).map_err(|e| format!("Failed to write settings file: {}", e))?;

        Ok(())
    }

    /// Validate settings (check for port conflicts, valid paths, etc.)
    pub fn validate(&self) -> Result<Vec<String>, Vec<String>> {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        // Check if project root exists
        let project_path = PathBuf::from(&self.project_root);
        if !project_path.exists() {
            warnings.push(format!(
                "Project root '{}' does not exist. It will be created when services start.",
                self.project_root
            ));
        }

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
        {
            errors.push("Port numbers must be greater than 0".to_string());
        }

        // Check for duplicate ports across services. Zero ports are handled by the
        // ">0" check above, so ignore them here to avoid duplicate/confusing errors.
        let labeled_ports = [
            ("HTTP/Web", self.web_port),
            ("PHP-FPM", self.php_port),
            ("MySQL", self.mysql_port),
            ("PostgreSQL", self.postgresql_port),
        ];
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

fn default_language() -> String {
    "en".to_string()
}

fn default_sound_enabled() -> bool {
    true
}

fn default_project_root() -> PathBuf {
    get_app_data_paths()
        .map(|paths| paths.projects_dir)
        .unwrap_or_else(|_| {
            dirs::data_local_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
                .join(APP_DIR_NAME)
                .join("projects")
        })
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
    fn validate_allows_distinct_ports() {
        // Default ports are all distinct (web 8080, php 9000, mysql 3306, postgresql 5432).
        let settings = AppSettings::default();
        assert!(
            settings.validate().is_ok(),
            "expected distinct default ports to validate successfully"
        );
    }
}
