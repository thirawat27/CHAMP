use crate::runtime::packages::PackageSelection;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const APP_DIR_NAME: &str = "CHAMP";

pub const DEFAULT_PORTS: Ports = Ports {
    web: 8080,
    php: 9000,
    mysql: 3307,
};

#[derive(Debug, Clone, Copy)]
pub struct Ports {
    pub web: u16,
    pub php: u16,
    pub mysql: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub web_port: u16,
    pub php_port: u16,
    pub mysql_port: u16,
    pub project_root: String,
    #[serde(default)]
    pub auto_start_services: bool,
    #[serde(default)]
    pub package_selection: PackageSelection,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            web_port: DEFAULT_PORTS.web,
            php_port: DEFAULT_PORTS.php,
            mysql_port: DEFAULT_PORTS.mysql,
            project_root: dirs::data_local_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
                .join(APP_DIR_NAME)
                .join("projects")
                .to_string_lossy()
                .to_string(),
            auto_start_services: false,
            package_selection: PackageSelection::default(),
        }
    }
}

impl AppSettings {
    /// Get the path to the settings file
    fn settings_path() -> Option<PathBuf> {
        dirs::data_local_dir()
            .or_else(dirs::home_dir)
            .map(|p| p.join(APP_DIR_NAME).join("config").join("settings.json"))
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
            Ok(content) => match serde_json::from_str(&content) {
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

        // Check for port conflicts
        if let Err(e) = std::net::TcpListener::bind(format!("127.0.0.1:{}", self.web_port)) {
            warnings.push(format!("Web port {} may be in use: {}", self.web_port, e));
        }

        if let Err(e) = std::net::TcpListener::bind(format!("127.0.0.1:{}", self.php_port)) {
            warnings.push(format!(
                "PHP-FPM port {} may be in use: {}",
                self.php_port, e
            ));
        }

        if let Err(e) = std::net::TcpListener::bind(format!("127.0.0.1:{}", self.mysql_port)) {
            warnings.push(format!(
                "MySQL port {} may be in use: {}",
                self.mysql_port, e
            ));
        }

        // Check for valid port ranges
        if self.web_port == 0 || self.php_port == 0 || self.mysql_port == 0 {
            errors.push("Port numbers must be greater than 0".to_string());
        }

        if errors.is_empty() {
            Ok(warnings)
        } else {
            Err(errors)
        }
    }
}
