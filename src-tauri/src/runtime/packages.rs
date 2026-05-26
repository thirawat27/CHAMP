use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const EMBEDDED_RUNTIME_CONFIG: &str = include_str!("../../runtime-config.json");

/// Available package versions for each component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagesConfig {
    pub php: Vec<PhpPackage>,
    pub mysql: Vec<MySQLPackage>,
    pub postgresql: Vec<PostgreSQLPackage>,
    pub phpmyadmin: Vec<PhpMyAdminPackage>,
}

/// PHP package with version and download URLs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpPackage {
    pub id: String,
    pub version: String,
    pub display_name: String,
    #[serde(rename = "windowsX64")]
    pub windows_x64: String,
    #[serde(rename = "windowsArm64")]
    pub windows_arm64: String,
    #[serde(rename = "linuxX64")]
    pub linux_x64: String,
    #[serde(rename = "linuxArm64")]
    pub linux_arm64: String,
    #[serde(rename = "macOSX64")]
    pub macos_x64: String,
    #[serde(rename = "macOSArm64")]
    pub macos_arm64: String,
    #[serde(default)]
    pub eol: bool,
    #[serde(default)]
    pub lts: bool,
    #[serde(default)]
    pub recommended: bool,
}

/// MySQL package with version and download URLs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySQLPackage {
    pub id: String,
    pub version: String,
    pub display_name: String,
    #[serde(rename = "windowsX64")]
    pub windows_x64: String,
    #[serde(rename = "windowsArm64")]
    pub windows_arm64: String,
    #[serde(rename = "linuxX64")]
    pub linux_x64: String,
    #[serde(rename = "linuxArm64")]
    pub linux_arm64: String,
    #[serde(rename = "macOSX64")]
    pub macos_x64: String,
    #[serde(rename = "macOSArm64")]
    pub macos_arm64: String,
    #[serde(default)]
    pub eol: bool,
    #[serde(default)]
    pub lts: bool,
    #[serde(default)]
    pub recommended: bool,
}

/// PostgreSQL package with version and download URLs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgreSQLPackage {
    pub id: String,
    pub version: String,
    pub display_name: String,
    #[serde(rename = "windowsX64")]
    pub windows_x64: String,
    #[serde(rename = "windowsArm64")]
    pub windows_arm64: String,
    #[serde(rename = "linuxX64")]
    pub linux_x64: String,
    #[serde(rename = "linuxArm64")]
    pub linux_arm64: String,
    #[serde(rename = "macOSX64")]
    pub macos_x64: String,
    #[serde(rename = "macOSArm64")]
    pub macos_arm64: String,
    #[serde(default)]
    pub eol: bool,
    #[serde(default)]
    pub lts: bool,
    #[serde(default)]
    pub recommended: bool,
}

/// phpMyAdmin package with version and download URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpMyAdminPackage {
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub url: String,
    #[serde(default)]
    pub eol: bool,
    #[serde(default)]
    pub lts: bool,
    #[serde(default)]
    pub recommended: bool,
}

/// User's selected package versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSelection {
    pub php: String,
    #[serde(alias = "mariadb")]
    pub mysql: String,
    #[serde(default = "default_postgresql_selection")]
    pub postgresql: String,
    pub phpmyadmin: String,
}

fn default_postgresql_selection() -> String {
    selected_package_ids_from_config(&embedded_default_runtime_config()).postgresql
}

impl Default for PackageSelection {
    fn default() -> Self {
        selected_package_ids_from_config(&embedded_default_runtime_config())
    }
}

/// Runtime configuration loaded from runtime-config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub version: String,
    pub binaries: BinariesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinariesConfig {
    #[serde(rename = "caddy")]
    pub caddy: BinaryConfig,
    #[serde(rename = "php")]
    pub php: BinaryConfig,
    #[serde(rename = "mysql")]
    pub mysql: BinaryConfig,
    #[serde(rename = "postgresql", default = "default_postgresql_binary_config")]
    pub postgresql: BinaryConfig,
    #[serde(rename = "phpmyadmin")]
    pub phpmyadmin: PhpMyAdminConfig,
}

fn default_postgresql_binary_config() -> BinaryConfig {
    BinaryConfig {
        versions: vec![default_postgresql_version()],
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryConfig {
    pub versions: Vec<VersionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpMyAdminConfig {
    pub versions: Vec<VersionInfoSingleUrl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    pub version: String,
    pub selected: bool,
    pub display_name: String,
    #[serde(default)]
    pub eol: bool,
    #[serde(default)]
    pub lts: bool,
    #[serde(default)]
    pub checksums: Checksums,
    pub urls: Urls,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Checksums {
    #[serde(rename = "windowsX64", default)]
    pub windows_x64: Option<String>,
    #[serde(rename = "windowsArm64", default)]
    pub windows_arm64: Option<String>,
    #[serde(rename = "linuxX64", default)]
    pub linux_x64: Option<String>,
    #[serde(rename = "linuxArm64", default)]
    pub linux_arm64: Option<String>,
    #[serde(rename = "macOSX64", default)]
    pub macos_x64: Option<String>,
    #[serde(rename = "macOSArm64", default)]
    pub macos_arm64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfoSingleUrl {
    pub id: String,
    pub version: String,
    pub selected: bool,
    pub display_name: String,
    #[serde(default)]
    pub eol: bool,
    #[serde(default)]
    pub lts: bool,
    #[serde(default)]
    pub checksum: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Urls {
    #[serde(rename = "windowsX64")]
    pub windows_x64: Option<String>,
    #[serde(rename = "windowsArm64")]
    pub windows_arm64: Option<String>,
    #[serde(rename = "linuxX64")]
    pub linux_x64: Option<String>,
    #[serde(rename = "linuxArm64")]
    pub linux_arm64: Option<String>,
    #[serde(rename = "macOSX64")]
    pub macos_x64: Option<String>,
    #[serde(rename = "macOSArm64")]
    pub macos_arm64: Option<String>,
}

fn default_postgresql_version() -> VersionInfo {
    embedded_default_runtime_config()
        .binaries
        .postgresql
        .versions
        .into_iter()
        .next()
        .expect("embedded runtime-config.json must define a PostgreSQL package")
}

/// Global runtime config cache
static RUNTIME_CONFIG: OnceLock<Option<RuntimeConfig>> = OnceLock::new();
static TAURI_RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_tauri_resource_dir(path: PathBuf) {
    let _ = TAURI_RESOURCE_DIR.set(path);
}

pub fn runtime_config_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(resource_dir) = TAURI_RESOURCE_DIR.get() {
        paths.push(resource_dir.join("runtime-config.json"));
    }

    paths.push(PathBuf::from("runtime-config.json"));
    paths.push(PathBuf::from("src-tauri").join("runtime-config.json"));

    if let Ok(app_paths) = crate::runtime::locator::get_app_data_paths() {
        paths.push(app_paths.base_dir.join("runtime-config.json"));
        paths.push(app_paths.config_dir.join("runtime-config.json"));
    }

    for env_name in ["CHAMP_DATA_DIR", "CHAMP_PORTABLE_DIR"] {
        if let Some(dir) = std::env::var_os(env_name).map(PathBuf::from) {
            paths.push(dir.join("runtime-config.json"));
            paths.push(dir.join("config").join("runtime-config.json"));
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            add_runtime_config_paths_for_dir(&mut paths, exe_dir);
            if let Some(parent) = exe_dir.parent() {
                add_runtime_config_paths_for_dir(&mut paths, parent);
                if let Some(grandparent) = parent.parent() {
                    add_runtime_config_paths_for_dir(&mut paths, grandparent);
                }
            }
        }
    }

    if let Some(data_dir) = dirs::data_local_dir() {
        paths.push(data_dir.join("CHAMP").join("runtime-config.json"));
        paths.push(
            data_dir
                .join("CHAMP")
                .join("config")
                .join("runtime-config.json"),
        );
        paths.push(data_dir.join("campp").join("runtime-config.json"));
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
            paths.push(
                PathBuf::from(xdg_data_home)
                    .join("CHAMP")
                    .join("runtime-config.json"),
            );
            paths.push(
                PathBuf::from(xdg_data_home)
                    .join("champ")
                    .join("runtime-config.json"),
            );
        }

        if let Ok(xdg_data_dirs) = std::env::var("XDG_DATA_DIRS") {
            for dir in xdg_data_dirs.split(':').filter(|dir| !dir.is_empty()) {
                paths.push(PathBuf::from(dir).join("CHAMP").join("runtime-config.json"));
                paths.push(PathBuf::from(dir).join("champ").join("runtime-config.json"));
            }
        } else {
            paths.push(PathBuf::from("/usr/local/share/CHAMP/runtime-config.json"));
            paths.push(PathBuf::from("/usr/share/CHAMP/runtime-config.json"));
            paths.push(PathBuf::from("/usr/local/share/champ/runtime-config.json"));
            paths.push(PathBuf::from("/usr/share/champ/runtime-config.json"));
        }
    }

    dedupe_paths(paths)
}

fn add_runtime_config_paths_for_dir(paths: &mut Vec<PathBuf>, dir: &Path) {
    paths.push(dir.join("runtime-config.json"));
    paths.push(dir.join("resources").join("runtime-config.json"));
    paths.push(dir.join("share").join("CHAMP").join("runtime-config.json"));
    paths.push(dir.join("share").join("champ").join("runtime-config.json"));
    paths.push(dir.join("lib").join("CHAMP").join("runtime-config.json"));
    paths.push(dir.join("lib").join("champ").join("runtime-config.json"));
    paths.push(
        dir.join("lib")
            .join("share")
            .join("CHAMP")
            .join("runtime-config.json"),
    );
    paths.push(
        dir.join("lib")
            .join("share")
            .join("champ")
            .join("runtime-config.json"),
    );
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut unique = Vec::new();
    for path in paths {
        let key = path.to_string_lossy().to_string();
        if seen.insert(key) {
            unique.push(path);
        }
    }
    unique
}

pub fn read_runtime_config_content() -> Option<(PathBuf, String)> {
    let paths_to_try = runtime_config_search_paths();
    for path in &paths_to_try {
        match fs::read_to_string(path) {
            Ok(content) => {
                eprintln!("Loaded runtime configuration from {}", path.display());
                return Some((path.clone(), content));
            }
            Err(e) if path.exists() => {
                eprintln!(
                    "Found runtime-config.json at {} but could not read it: {}",
                    path.display(),
                    e
                );
            }
            Err(_) => {}
        }
    }

    eprintln!("runtime-config.json not found. Searched these paths:");
    for path in paths_to_try {
        eprintln!("  - {}", path.display());
    }
    None
}

/// Load runtime configuration from file
pub fn load_runtime_config_from_file() -> Option<RuntimeConfig> {
    if let Some((path, content)) = read_runtime_config_content() {
        return serde_json::from_str::<RuntimeConfig>(&content)
            .map_err(|e| {
                eprintln!(
                    "Failed to parse runtime-config.json from {}: {}",
                    path.display(),
                    e
                );
                e
            })
            .ok();
    }

    eprintln!("Using embedded runtime-config.json fallback");
    Some(embedded_default_runtime_config())
}

pub fn embedded_default_runtime_config() -> RuntimeConfig {
    serde_json::from_str(EMBEDDED_RUNTIME_CONFIG)
        .expect("embedded runtime-config.json must be valid")
}

/// Get the platform-appropriate database display name.
fn get_database_display_name(display_name: &str) -> String {
    display_name.replace("MariaDB", "MySQL")
}

/// Get all available packages from config file or defaults
pub fn get_available_packages() -> PackagesConfig {
    let config = RUNTIME_CONFIG.get_or_init(load_runtime_config_from_file);

    if let Some(cfg) = config {
        runtime_config_to_packages(cfg)
    } else {
        eprintln!("Using embedded default package configuration");
        get_default_packages()
    }
}

fn runtime_config_to_packages(cfg: &RuntimeConfig) -> PackagesConfig {
    PackagesConfig {
        php: cfg
            .binaries
            .php
            .versions
            .iter()
            .map(|v| PhpPackage {
                id: v.id.clone(),
                version: v.version.clone(),
                display_name: v.display_name.clone(),
                windows_x64: v.urls.windows_x64.clone().unwrap_or_default(),
                windows_arm64: v.urls.windows_arm64.clone().unwrap_or_default(),
                linux_x64: v.urls.linux_x64.clone().unwrap_or_default(),
                linux_arm64: v.urls.linux_arm64.clone().unwrap_or_default(),
                macos_x64: v.urls.macos_x64.clone().unwrap_or_default(),
                macos_arm64: v.urls.macos_arm64.clone().unwrap_or_default(),
                eol: v.eol,
                lts: v.lts,
                recommended: v.selected,
            })
            .collect(),
        mysql: cfg
            .binaries
            .mysql
            .versions
            .iter()
            .map(|v| MySQLPackage {
                id: v.id.clone(),
                version: v.version.clone(),
                display_name: get_database_display_name(&v.display_name),
                windows_x64: v.urls.windows_x64.clone().unwrap_or_default(),
                windows_arm64: v.urls.windows_arm64.clone().unwrap_or_default(),
                linux_x64: v.urls.linux_x64.clone().unwrap_or_default(),
                linux_arm64: v.urls.linux_arm64.clone().unwrap_or_default(),
                macos_x64: v.urls.macos_x64.clone().unwrap_or_default(),
                macos_arm64: v.urls.macos_arm64.clone().unwrap_or_default(),
                eol: v.eol,
                lts: v.lts,
                recommended: v.selected,
            })
            .collect(),
        postgresql: cfg
            .binaries
            .postgresql
            .versions
            .iter()
            .map(|v| PostgreSQLPackage {
                id: v.id.clone(),
                version: v.version.clone(),
                display_name: v.display_name.clone(),
                windows_x64: v.urls.windows_x64.clone().unwrap_or_default(),
                windows_arm64: v.urls.windows_arm64.clone().unwrap_or_default(),
                linux_x64: v.urls.linux_x64.clone().unwrap_or_default(),
                linux_arm64: v.urls.linux_arm64.clone().unwrap_or_default(),
                macos_x64: v.urls.macos_x64.clone().unwrap_or_default(),
                macos_arm64: v.urls.macos_arm64.clone().unwrap_or_default(),
                eol: v.eol,
                lts: v.lts,
                recommended: v.selected,
            })
            .collect(),
        phpmyadmin: cfg
            .binaries
            .phpmyadmin
            .versions
            .iter()
            .map(|v| PhpMyAdminPackage {
                id: v.id.clone(),
                version: v.version.clone(),
                display_name: v.display_name.clone(),
                url: v.url.clone(),
                eol: v.eol,
                lts: v.lts,
                recommended: v.selected,
            })
            .collect(),
    }
}

/// Get the selected package IDs from config
pub fn get_selected_package_ids() -> PackageSelection {
    let config = RUNTIME_CONFIG.get_or_init(load_runtime_config_from_file);

    if let Some(cfg) = config {
        selected_package_ids_from_config(cfg)
    } else {
        PackageSelection::default()
    }
}

fn selected_package_ids_from_config(cfg: &RuntimeConfig) -> PackageSelection {
    PackageSelection {
        php: selected_version_id(&cfg.binaries.php.versions)
            .expect("runtime-config.json must select a PHP package"),
        mysql: selected_version_id(&cfg.binaries.mysql.versions)
            .expect("runtime-config.json must select a MySQL package"),
        postgresql: selected_version_id(&cfg.binaries.postgresql.versions)
            .expect("runtime-config.json must select a PostgreSQL package"),
        phpmyadmin: cfg
            .binaries
            .phpmyadmin
            .versions
            .iter()
            .find(|v| v.selected)
            .or_else(|| cfg.binaries.phpmyadmin.versions.first())
            .map(|v| v.id.clone())
            .expect("runtime-config.json must define a database tool package"),
    }
}

fn selected_version_id(versions: &[VersionInfo]) -> Option<String> {
    versions
        .iter()
        .find(|v| v.selected)
        .or_else(|| versions.first())
        .map(|v| v.id.clone())
}

/// Get PHP package by ID
pub fn get_php_package(id: &str) -> Option<PhpPackage> {
    get_available_packages()
        .php
        .into_iter()
        .find(|p| p.id == id)
}

/// Get MySQL package by ID
pub fn get_mysql_package(id: &str) -> Option<MySQLPackage> {
    get_available_packages()
        .mysql
        .into_iter()
        .find(|p| p.id == id)
}

/// Get PostgreSQL package by ID
pub fn get_postgresql_package(id: &str) -> Option<PostgreSQLPackage> {
    get_available_packages()
        .postgresql
        .into_iter()
        .find(|p| p.id == id)
}

/// Get phpMyAdmin package by ID
pub fn get_phpmyadmin_package(id: &str) -> Option<PhpMyAdminPackage> {
    get_available_packages()
        .phpmyadmin
        .into_iter()
        .find(|p| p.id == id)
}

/// Reload the runtime configuration (call after modifying the config file)
pub fn reload_runtime_config() {
    let _ = RUNTIME_CONFIG.set(load_runtime_config_from_file());
}

/// Get the runtime configuration
pub fn get_config() -> Option<RuntimeConfig> {
    RUNTIME_CONFIG
        .get_or_init(load_runtime_config_from_file)
        .clone()
}

pub fn selected_caddy_version() -> String {
    let config = get_config().unwrap_or_else(embedded_default_runtime_config);
    config
        .binaries
        .caddy
        .versions
        .iter()
        .find(|version| version.selected)
        .or_else(|| config.binaries.caddy.versions.first())
        .map(|version| version.version.clone())
        .unwrap_or_default()
}

/// Get default packages from the embedded runtime-config.json fallback.
fn get_default_packages() -> PackagesConfig {
    runtime_config_to_packages(&embedded_default_runtime_config())
}
