use crate::runtime::locator::APP_DIR_NAME;
use crate::runtime::platform::Platform;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

mod catalog;

pub use catalog::{mysql_archive_url, refresh_php_catalog, refresh_runtime_catalog};

const EMBEDDED_RUNTIME_CONFIG: &str = include_str!("../../../runtime-config.json");

/// Available package versions for each component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagesConfig {
    pub php: Vec<PhpPackage>,
    pub mysql: Vec<MySQLPackage>,
    pub postgresql: Vec<PostgreSQLPackage>,
    pub phpmyadmin: Vec<PhpMyAdminPackage>,
}

/// A package with one download URL per platform (PHP, MySQL, PostgreSQL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformPackage {
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

impl PlatformPackage {
    /// The download URL for `platform`; empty when it has no build.
    pub fn url_for(self, platform: Platform) -> String {
        match platform {
            Platform::WindowsX64 => self.windows_x64,
            Platform::WindowsArm64 => self.windows_arm64,
            Platform::MacOSX64 => self.macos_x64,
            Platform::MacOSArm64 => self.macos_arm64,
            Platform::LinuxX64 => self.linux_x64,
            Platform::LinuxArm64 => self.linux_arm64,
        }
    }
}

pub type PhpPackage = PlatformPackage;
pub type MySQLPackage = PlatformPackage;
pub type PostgreSQLPackage = PlatformPackage;

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
    /// The web database manager (`phpmyadmin-*` or `adminer-*`).
    pub phpmyadmin: String,
    /// Which engine fills the stack's database role: `mysql` or `postgresql`.
    /// Chosen separately from the manager; empty in settings saved before that.
    #[serde(default)]
    pub database_engine: String,
}

/// Whether a database tool package id is Adminer, as opposed to phpMyAdmin.
pub fn is_adminer_id(id: &str) -> bool {
    id.starts_with("adminer")
}

/// Whether a database tool package id is phpMyAdmin, as opposed to Adminer.
pub fn is_phpmyadmin_id(id: &str) -> bool {
    id.starts_with("phpmyadmin")
}

impl PackageSelection {
    pub fn is_postgresql(&self) -> bool {
        self.database_engine == "postgresql"
    }

    pub fn tool_is_adminer(&self) -> bool {
        is_adminer_id(&self.phpmyadmin)
    }

    /// The PHP id names an install folder, and its old contents get deleted when
    /// that branch is updated, so it must never be able to point anywhere else.
    pub fn validate_path_ids(&self) -> Result<(), String> {
        if is_safe_path_segment(&self.php) {
            Ok(())
        } else {
            Err(format!("Invalid PHP version id: {:?}", self.php))
        }
    }

    /// Fill and reconcile the database choice.
    ///
    /// Older settings tied the engine to the manager (Adminer meant
    /// PostgreSQL), so an empty engine is inferred from it. phpMyAdmin only
    /// speaks MySQL, so a PostgreSQL stack always ends up on Adminer. Saved
    /// manager ids go stale as releases change, so they resolve to the catalog.
    pub fn normalized(mut self) -> Self {
        if self.database_engine != "mysql" && self.database_engine != "postgresql" {
            self.database_engine = if self.tool_is_adminer() {
                "postgresql"
            } else {
                "mysql"
            }
            .to_string();
        }
        if self.is_postgresql() && !self.tool_is_adminer() {
            self.phpmyadmin = "adminer".to_string();
        }
        self.phpmyadmin = resolve_database_tool_id(&self.phpmyadmin);
        self
    }
}

/// True when `value` is a single plain file or folder name. Catalog ids and
/// versions come from upstream feeds and a user-editable file, and end up in paths.
pub fn is_safe_path_segment(value: &str) -> bool {
    !value.is_empty()
        && !value.ends_with('.')
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
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

impl Checksums {
    pub fn for_platform(&self, platform: Platform) -> Option<String> {
        match platform {
            Platform::WindowsX64 => self.windows_x64.clone(),
            Platform::WindowsArm64 => self.windows_arm64.clone(),
            Platform::LinuxX64 => self.linux_x64.clone(),
            Platform::LinuxArm64 => self.linux_arm64.clone(),
            Platform::MacOSX64 => self.macos_x64.clone(),
            Platform::MacOSArm64 => self.macos_arm64.clone(),
        }
    }
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
static RUNTIME_CONFIG: OnceLock<RwLock<Option<RuntimeConfig>>> = OnceLock::new();
static TAURI_RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_tauri_resource_dir(path: PathBuf) {
    let _ = TAURI_RESOURCE_DIR.set(path);
}

fn runtime_config_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(app_paths) = crate::runtime::locator::get_app_data_paths() {
        paths.push(app_paths.config_dir.join("runtime-config.json"));
        paths.push(app_paths.base_dir.join("runtime-config.json"));
    }

    for env_name in ["CHAMP_DATA_DIR", "CHAMP_PORTABLE_DIR"] {
        if let Some(dir) = std::env::var_os(env_name).map(PathBuf::from) {
            paths.push(dir.join("config").join("runtime-config.json"));
            paths.push(dir.join("runtime-config.json"));
        }
    }

    if let Some(data_dir) = dirs::data_local_dir() {
        paths.push(
            data_dir
                .join(APP_DIR_NAME)
                .join("config")
                .join("runtime-config.json"),
        );
        paths.push(data_dir.join(APP_DIR_NAME).join("runtime-config.json"));
        // Deliberate legacy migration path: old "campp" installs stored their
        // runtime-config.json here. Kept so existing users keep working; not a typo.
        paths.push(data_dir.join("campp").join("runtime-config.json"));
    }

    if let Some(resource_dir) = TAURI_RESOURCE_DIR.get() {
        paths.push(resource_dir.join("runtime-config.json"));
    }

    // Dev convenience only: reading config relative to the current working
    // directory is unsafe in a shipped app (attacker-writable CWD could inject
    // download URLs). Restricted to debug builds. (S-05)
    #[cfg(debug_assertions)]
    {
        paths.push(PathBuf::from("runtime-config.json"));
        paths.push(PathBuf::from("src-tauri").join("runtime-config.json"));
    }

    // Dev convenience only, like the working-directory paths above. A portable
    // build often runs from a folder anyone can drop files into (Downloads), and
    // this file decides which binaries get downloaded and executed. Shipped
    // builds rely on the data folder, the bundled resource, and the embedded copy.
    #[cfg(debug_assertions)]
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

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
            let xdg_data_home = PathBuf::from(xdg_data_home);
            paths.push(xdg_data_home.join(APP_DIR_NAME).join("runtime-config.json"));
            paths.push(xdg_data_home.join("champ").join("runtime-config.json"));
        }

        if let Ok(xdg_data_dirs) = std::env::var("XDG_DATA_DIRS") {
            for dir in xdg_data_dirs.split(':').filter(|dir| !dir.is_empty()) {
                paths.push(
                    PathBuf::from(dir)
                        .join(APP_DIR_NAME)
                        .join("runtime-config.json"),
                );
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

#[cfg(debug_assertions)]
fn add_runtime_config_paths_for_dir(paths: &mut Vec<PathBuf>, dir: &std::path::Path) {
    paths.push(dir.join("runtime-config.json"));
    paths.push(dir.join("resources").join("runtime-config.json"));
    paths.push(
        dir.join("share")
            .join(APP_DIR_NAME)
            .join("runtime-config.json"),
    );
    paths.push(dir.join("share").join("champ").join("runtime-config.json"));
    paths.push(
        dir.join("lib")
            .join(APP_DIR_NAME)
            .join("runtime-config.json"),
    );
    paths.push(dir.join("lib").join("champ").join("runtime-config.json"));
    paths.push(
        dir.join("lib")
            .join("share")
            .join(APP_DIR_NAME)
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

fn read_runtime_config_content() -> Option<(PathBuf, String)> {
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
fn load_runtime_config_from_file() -> Option<RuntimeConfig> {
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
            .ok()
            .map(|config| {
                if config_has_every_package(&config) {
                    config
                } else {
                    eprintln!(
                        "{} has an empty version list; using the embedded runtime catalog",
                        path.display()
                    );
                    embedded_default_runtime_config()
                }
            });
    }

    eprintln!("Using embedded runtime-config.json fallback");
    Some(embedded_default_runtime_config())
}

/// The rest of the code relies on every component having at least one version.
/// The file is user-editable, and `"versions": []` parses fine.
fn config_has_every_package(config: &RuntimeConfig) -> bool {
    let binaries = &config.binaries;
    !binaries.caddy.versions.is_empty()
        && !binaries.php.versions.is_empty()
        && !binaries.mysql.versions.is_empty()
        && !binaries.postgresql.versions.is_empty()
        && !binaries.phpmyadmin.versions.is_empty()
}

fn replace_runtime_config(config: Option<RuntimeConfig>) {
    let cache = RUNTIME_CONFIG.get_or_init(|| RwLock::new(None));
    if let Ok(mut guard) = cache.write() {
        *guard = config;
    }
}

pub fn embedded_default_runtime_config() -> RuntimeConfig {
    serde_json::from_str(EMBEDDED_RUNTIME_CONFIG)
        .expect("embedded runtime-config.json must be valid")
}

/// Map a saved web-database-manager id onto the catalog.
///
/// The id carries the release (`adminer-5.4`), so it goes stale when the
/// catalog moves on. The family (`adminer` / `phpmyadmin`) is what decides the
/// database engine, so fall back to the same family's current entry.
fn resolve_database_tool_id(id: &str) -> String {
    resolve_tool_id_in(id, &get_available_packages().phpmyadmin)
}

fn resolve_tool_id_in(id: &str, tools: &[PhpMyAdminPackage]) -> String {
    if tools.iter().any(|tool| tool.id == id) {
        return id.to_string();
    }
    let family = if is_adminer_id(id) {
        "adminer"
    } else {
        "phpmyadmin"
    };
    tools
        .iter()
        .find(|tool| tool.id.starts_with(family))
        .map_or_else(|| id.to_string(), |tool| tool.id.clone())
}

/// Get the platform-appropriate database display name.
fn get_database_display_name(display_name: &str) -> String {
    display_name.replace("MariaDB", "MySQL")
}

/// Get all available packages from config file or defaults
pub fn get_available_packages() -> PackagesConfig {
    let config = get_config();

    if let Some(cfg) = config {
        runtime_config_to_packages(&cfg)
    } else {
        eprintln!("Using embedded default package configuration");
        get_default_packages()
    }
}

fn platform_package(v: &VersionInfo) -> PlatformPackage {
    PlatformPackage {
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
    }
}

fn runtime_config_to_packages(cfg: &RuntimeConfig) -> PackagesConfig {
    PackagesConfig {
        php: cfg
            .binaries
            .php
            .versions
            .iter()
            .map(platform_package)
            .collect(),
        mysql: cfg
            .binaries
            .mysql
            .versions
            .iter()
            .map(|v| PlatformPackage {
                display_name: get_database_display_name(&v.display_name),
                ..platform_package(v)
            })
            .collect(),
        postgresql: cfg
            .binaries
            .postgresql
            .versions
            .iter()
            .map(platform_package)
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
    let config = get_config();

    if let Some(cfg) = config {
        selected_package_ids_from_config(&cfg)
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
        database_engine: "mysql".to_string(),
    }
}

fn selected_version_id(versions: &[VersionInfo]) -> Option<String> {
    versions
        .iter()
        .find(|v| v.selected)
        .or_else(|| versions.first())
        .map(|v| v.id.clone())
}

fn package_by_id(packages: Vec<PlatformPackage>, id: &str) -> Option<PlatformPackage> {
    packages.into_iter().find(|p| p.id == id)
}

/// Get PHP package by ID
pub fn get_php_package(id: &str) -> Option<PhpPackage> {
    package_by_id(get_available_packages().php, id)
}

/// Get MySQL package by ID
pub fn get_mysql_package(id: &str) -> Option<MySQLPackage> {
    package_by_id(get_available_packages().mysql, id)
}

/// Get PostgreSQL package by ID
pub fn get_postgresql_package(id: &str) -> Option<PostgreSQLPackage> {
    package_by_id(get_available_packages().postgresql, id)
}

/// Get phpMyAdmin package by ID
pub fn get_phpmyadmin_package(id: &str) -> Option<PhpMyAdminPackage> {
    get_available_packages()
        .phpmyadmin
        .into_iter()
        .find(|p| p.id == id)
}

/// Get the runtime configuration
pub fn get_config() -> Option<RuntimeConfig> {
    let cache = RUNTIME_CONFIG.get_or_init(|| RwLock::new(load_runtime_config_from_file()));
    cache.read().ok().and_then(|guard| guard.clone())
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

#[cfg(test)]
mod tests;
