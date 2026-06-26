use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

const EMBEDDED_RUNTIME_CONFIG: &str = include_str!("../../runtime-config.json");

/// Available package versions for each component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagesConfig {
    pub php: Vec<PhpPackage>,
    pub mysql: Vec<MySQLPackage>,
    pub postgresql: Vec<PostgreSQLPackage>,
    pub phpmyadmin: Vec<PhpMyAdminPackage>,
    #[serde(default)]
    pub node: Vec<GenericPackage>,
    #[serde(default)]
    pub python: Vec<GenericPackage>,
    #[serde(default)]
    pub go: Vec<GenericPackage>,
    #[serde(default)]
    pub ruby: Vec<GenericPackage>,
}

/// Generic package with version and download URLs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericPackage {
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
    #[serde(default)]
    pub node: Option<String>,
    #[serde(default)]
    pub python: Option<String>,
    #[serde(default)]
    pub go: Option<String>,
    #[serde(default)]
    pub ruby: Option<String>,
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
    #[serde(rename = "node", default)]
    pub node: Option<BinaryConfig>,
    #[serde(rename = "python", default)]
    pub python: Option<BinaryConfig>,
    #[serde(rename = "go", default)]
    pub go: Option<BinaryConfig>,
    #[serde(rename = "ruby", default)]
    pub ruby: Option<BinaryConfig>,
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
static RUNTIME_CONFIG: OnceLock<RwLock<Option<RuntimeConfig>>> = OnceLock::new();
static TAURI_RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_tauri_resource_dir(path: PathBuf) {
    let _ = TAURI_RESOURCE_DIR.set(path);
}

pub fn runtime_config_search_paths() -> Vec<PathBuf> {
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
                .join("CHAMP")
                .join("config")
                .join("runtime-config.json"),
        );
        paths.push(data_dir.join("CHAMP").join("runtime-config.json"));
        paths.push(data_dir.join("campp").join("runtime-config.json"));
    }

    if let Some(resource_dir) = TAURI_RESOURCE_DIR.get() {
        paths.push(resource_dir.join("runtime-config.json"));
    }

    paths.push(PathBuf::from("runtime-config.json"));
    paths.push(PathBuf::from("src-tauri").join("runtime-config.json"));

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
            paths.push(xdg_data_home.join("CHAMP").join("runtime-config.json"));
            paths.push(xdg_data_home.join("champ").join("runtime-config.json"));
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

#[derive(Debug, Deserialize)]
struct GitHubLatestRelease {
    tag_name: String,
}

#[derive(Debug, Deserialize)]
struct NodeIndexEntry {
    version: String,
    lts: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct GoDownloadEntry {
    version: String,
    stable: bool,
}

/// Refresh the runtime catalog from upstream release metadata.
///
/// Network failures are non-fatal; the current local catalog is returned so the
/// UI can continue to work offline.
pub async fn refresh_runtime_catalog() -> Result<PackagesConfig, String> {
    let mut config = get_config().unwrap_or_else(embedded_default_runtime_config);
    let client = reqwest::Client::builder()
        .user_agent("CHAMP runtime catalog refresher")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let mut changed = false;

    if let Some(version) = fetch_github_latest(&client, "caddyserver", "caddy")
        .await
        .map(|tag| tag.strip_prefix('v').map_or(tag.clone(), str::to_string))
    {
        changed |= replace_versions(&mut config.binaries.caddy.versions, caddy_version(&version));
    }

    if let Some(version) =
        fetch_version_from_text(&client, "https://www.php.net/", "downloads of PHP ").await
    {
        changed |=
            upsert_selected_version(&mut config.binaries.php.versions, php_version(&version));
    }

    if let Some(version) = fetch_version_from_text(
        &client,
        "https://dev.mysql.com/downloads/mysql/?os=3&platform=",
        "MySQL Community Server ",
    )
    .await
    {
        changed |= replace_version_list(
            &mut config.binaries.mysql.versions,
            vec![
                mysql_version(&version),
                selected_version(mysql_version("8.4.10"), false),
            ],
        );
    }

    if let Some(version) = fetch_version_from_text(
        &client,
        "https://www.enterprisedb.com/downloads/postgres-postgresql-downloads",
        "PostgreSQL Version",
    )
    .await
    {
        changed |= replace_version_list(
            &mut config.binaries.postgresql.versions,
            vec![
                postgresql_version(&version),
                selected_version(postgresql_version("17.10"), false),
                selected_version(postgresql_version("16.14"), false),
            ],
        );
    }

    if let Some(version) = fetch_version_from_text(
        &client,
        "https://www.phpmyadmin.net/downloads/",
        "phpMyAdmin ",
    )
    .await
    {
        changed |= upsert_selected_single_url(
            &mut config.binaries.phpmyadmin.versions,
            phpmyadmin_version(&version),
            false,
        );
    }

    if let Some(version) = fetch_version_from_text(
        &client,
        "https://www.adminer.org/version/?current=0&lang=en",
        "",
    )
    .await
    {
        changed |= upsert_selected_single_url(
            &mut config.binaries.phpmyadmin.versions,
            adminer_version(&version),
            false,
        );
    }

    if let Some(versions) = fetch_node_release_lines(&client).await {
        changed |= replace_version_list(
            &mut config
                .binaries
                .node
                .get_or_insert_with(empty_binary_config)
                .versions,
            versions,
        );
    }

    if let Some(version) = fetch_version_from_text(
        &client,
        "https://www.python.org/downloads/",
        "Download Python ",
    )
    .await
    {
        changed |= replace_version_list(
            &mut config
                .binaries
                .python
                .get_or_insert_with(empty_binary_config)
                .versions,
            vec![
                python_version(&version),
                selected_version(python_version("3.13.14"), false),
            ],
        );
    }

    if let Some(versions) = fetch_go_stable_versions(&client).await {
        changed |= replace_version_list(
            &mut config
                .binaries
                .go
                .get_or_insert_with(empty_binary_config)
                .versions,
            versions,
        );
    }

    if let Some(version) = fetch_github_latest(&client, "oneclick", "rubyinstaller2")
        .await
        .and_then(|tag| {
            tag.strip_prefix("RubyInstaller-").and_then(|value| {
                value
                    .rsplit_once('-')
                    .map(|(version, _)| version.to_string())
            })
        })
    {
        changed |= replace_version_list(
            &mut config
                .binaries
                .ruby
                .get_or_insert_with(empty_binary_config)
                .versions,
            vec![
                ruby_version(&version),
                selected_version(ruby_version("3.4.9"), false),
                selected_version(ruby_version("3.3.7"), false),
            ],
        );
    }

    if changed {
        persist_runtime_config_override(&config)?;
        replace_runtime_config(Some(config.clone()));
    }

    Ok(runtime_config_to_packages(&config))
}

fn empty_binary_config() -> BinaryConfig {
    BinaryConfig { versions: vec![] }
}

async fn fetch_github_latest(client: &reqwest::Client, owner: &str, repo: &str) -> Option<String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<GitHubLatestRelease>()
        .await
        .ok()
        .map(|release| release.tag_name)
}

async fn fetch_node_release_lines(client: &reqwest::Client) -> Option<Vec<VersionInfo>> {
    let entries = client
        .get("https://nodejs.org/dist/index.json")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<Vec<NodeIndexEntry>>()
        .await
        .ok()?;

    let mut seen_majors = std::collections::HashSet::new();
    let mut versions = Vec::new();
    let mut selected_lts = false;

    for entry in entries {
        let version = entry.version.trim_start_matches('v').to_string();
        let major = version.split('.').next().unwrap_or_default().to_string();
        if major.is_empty() || !seen_majors.insert(major) {
            continue;
        }

        let is_lts = !entry.lts.is_boolean() || entry.lts.as_bool() != Some(false);
        if is_lts {
            let selected = !selected_lts;
            selected_lts = true;
            versions.push(node_version_with_label(&version, true, selected, "LTS"));
        } else if versions.is_empty() {
            versions.push(node_version_with_label(&version, false, false, "Stable"));
        }

        if versions.len() >= 4 {
            break;
        }
    }

    if versions.is_empty() {
        None
    } else {
        Some(versions)
    }
}

async fn fetch_go_stable_versions(client: &reqwest::Client) -> Option<Vec<VersionInfo>> {
    let entries = client
        .get("https://go.dev/dl/?mode=json")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<Vec<GoDownloadEntry>>()
        .await
        .ok()?;

    let versions: Vec<_> = entries
        .into_iter()
        .filter(|entry| entry.stable)
        .take(2)
        .enumerate()
        .map(|(index, entry)| {
            selected_version(
                go_version(entry.version.trim_start_matches("go")),
                index == 0,
            )
        })
        .collect();

    if versions.is_empty() {
        None
    } else {
        Some(versions)
    }
}

async fn fetch_version_from_text(
    client: &reqwest::Client,
    url: &str,
    marker: &str,
) -> Option<String> {
    let text = client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()?;
    version_after_marker(&text, marker)
}

fn version_after_marker(text: &str, marker: &str) -> Option<String> {
    let haystack = if marker.is_empty() {
        text
    } else {
        let index = text.find(marker)? + marker.len();
        &text[index..]
    };

    for (start, ch) in haystack.char_indices() {
        if !ch.is_ascii_digit() {
            continue;
        }

        let candidate: String = haystack[start..]
            .chars()
            .take_while(|value| value.is_ascii_digit() || *value == '.')
            .collect();
        let candidate = candidate.trim_end_matches('.').to_string();
        if candidate.matches('.').count() >= 1 {
            return Some(candidate);
        }
    }

    None
}

fn persist_runtime_config_override(config: &RuntimeConfig) -> Result<(), String> {
    let app_paths = crate::runtime::locator::get_app_data_paths()
        .map_err(|e| format!("Failed to get app data paths: {}", e))?;
    fs::create_dir_all(&app_paths.config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;
    let target = app_paths.config_dir.join("runtime-config.json");
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize runtime config: {}", e))?;
    fs::write(&target, format!("{content}\n"))
        .map_err(|e| format!("Failed to write {}: {}", target.display(), e))
}

fn replace_versions(versions: &mut Vec<VersionInfo>, next: VersionInfo) -> bool {
    if versions.len() == 1 && versions[0].id == next.id && versions[0].version == next.version {
        versions[0].selected = true;
        return false;
    }
    *versions = vec![next];
    true
}

fn replace_version_list(versions: &mut Vec<VersionInfo>, next: Vec<VersionInfo>) -> bool {
    let changed = versions.len() != next.len()
        || versions.iter().zip(&next).any(|(current, updated)| {
            current.id != updated.id
                || current.version != updated.version
                || current.selected != updated.selected
                || current.display_name != updated.display_name
                || current.eol != updated.eol
                || current.lts != updated.lts
        });

    if changed {
        *versions = next;
    }

    changed
}

fn selected_version(mut version: VersionInfo, selected: bool) -> VersionInfo {
    version.selected = selected;
    version
}

fn upsert_selected_version(versions: &mut Vec<VersionInfo>, next: VersionInfo) -> bool {
    let mut changed = false;
    for version in versions.iter_mut() {
        version.selected = false;
    }

    if let Some(existing) = versions.iter_mut().find(|version| version.id == next.id) {
        if existing.version != next.version || existing.display_name != next.display_name {
            changed = true;
        }
        *existing = next;
    } else {
        versions.insert(0, next);
        changed = true;
    }

    versions.sort_by(|a, b| b.version.cmp(&a.version));
    changed
}

fn upsert_selected_single_url(
    versions: &mut Vec<VersionInfoSingleUrl>,
    next: VersionInfoSingleUrl,
    select: bool,
) -> bool {
    let mut changed = false;
    if select {
        for version in versions.iter_mut() {
            version.selected = false;
        }
    }

    if let Some(existing) = versions.iter_mut().find(|version| version.id == next.id) {
        if existing.version != next.version || existing.url != next.url {
            changed = true;
        }
        *existing = VersionInfoSingleUrl {
            selected: select || existing.selected,
            ..next
        };
    } else {
        versions.push(VersionInfoSingleUrl {
            selected: select,
            ..next
        });
        changed = true;
    }

    changed
}

fn version_major_minor(version: &str) -> String {
    let mut parts = version.split('.');
    match (parts.next(), parts.next()) {
        (Some(major), Some(minor)) => format!("{major}.{minor}"),
        _ => version.to_string(),
    }
}

fn caddy_version(version: &str) -> VersionInfo {
    VersionInfo {
        id: format!("caddy-{}", version_major_minor(version)),
        version: version.to_string(),
        selected: true,
        display_name: format!("Caddy {version}"),
        eol: false,
        lts: true,
        checksums: Checksums::default(),
        urls: Urls {
            windows_x64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_windows_amd64.zip")),
            windows_arm64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_windows_arm64.zip")),
            linux_x64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_linux_amd64.tar.gz")),
            linux_arm64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_linux_arm64.tar.gz")),
            macos_x64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_mac_amd64.tar.gz")),
            macos_arm64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_mac_arm64.tar.gz")),
        },
    }
}

fn php_version(version: &str) -> VersionInfo {
    let family = version_major_minor(version);
    VersionInfo {
        id: format!("php-{family}"),
        version: version.to_string(),
        selected: true,
        display_name: format!("PHP {family}"),
        eol: false,
        lts: false,
        checksums: Checksums::default(),
        urls: Urls {
            windows_x64: Some(format!("https://windows.php.net/downloads/releases/php-{version}-nts-Win32-vs17-x64.zip")),
            windows_arm64: None,
            linux_x64: Some(format!("https://dl.static-php.dev/static-php-cli/bulk/php-{version}-fpm-linux-x86_64.tar.gz")),
            linux_arm64: Some(format!("https://dl.static-php.dev/static-php-cli/bulk/php-{version}-fpm-linux-aarch64.tar.gz")),
            macos_x64: Some(format!("https://dl.static-php.dev/static-php-cli/bulk/php-{version}-fpm-macos-x86_64.tar.gz")),
            macos_arm64: Some(format!("https://dl.static-php.dev/static-php-cli/bulk/php-{version}-fpm-macos-aarch64.tar.gz")),
        },
    }
}

fn mysql_version(version: &str) -> VersionInfo {
    let family = version_major_minor(version);
    VersionInfo {
        id: format!("mysql-{family}"),
        version: version.to_string(),
        selected: true,
        display_name: format!("MySQL {version}"),
        eol: false,
        lts: false,
        checksums: Checksums::default(),
        urls: Urls {
            windows_x64: Some(format!("https://cdn.mysql.com/Downloads/MySQL-{family}/mysql-{version}-winx64.zip")),
            windows_arm64: None,
            linux_x64: Some(format!("https://cdn.mysql.com/Downloads/MySQL-{family}/mysql-{version}-linux-glibc2.28-x86_64.tar.xz")),
            linux_arm64: Some(format!("https://cdn.mysql.com/Downloads/MySQL-{family}/mysql-{version}-linux-glibc2.28-aarch64.tar.xz")),
            macos_x64: Some(format!("https://cdn.mysql.com/Downloads/MySQL-{family}/mysql-{version}-macos15-x86_64.tar.gz")),
            macos_arm64: Some(format!("https://cdn.mysql.com/Downloads/MySQL-{family}/mysql-{version}-macos15-arm64.tar.gz")),
        },
    }
}

fn postgresql_version(version: &str) -> VersionInfo {
    let major = version.split('.').next().unwrap_or(version);
    VersionInfo {
        id: format!("postgresql-{major}"),
        version: version.to_string(),
        selected: true,
        display_name: format!("PostgreSQL {version}"),
        eol: false,
        lts: false,
        checksums: Checksums::default(),
        urls: Urls {
            windows_x64: Some(format!("https://get.enterprisedb.com/postgresql/postgresql-{version}-1-windows-x64-binaries.zip")),
            windows_arm64: None,
            linux_x64: Some(format!("https://downloads.percona.com/downloads/postgresql-distribution-{major}/{version}/binary/tarball/percona-postgresql-{version}-ssl3-linux-x86_64.tar.gz")),
            linux_arm64: Some(format!("https://downloads.percona.com/downloads/postgresql-distribution-{major}/{version}/binary/tarball/percona-postgresql-{version}-ssl3-linux-aarch64.tar.gz")),
            macos_x64: Some(format!("https://get.enterprisedb.com/postgresql/postgresql-{version}-1-osx-binaries.zip")),
            macos_arm64: Some(format!("https://get.enterprisedb.com/postgresql/postgresql-{version}-1-osx-binaries.zip")),
        },
    }
}

fn phpmyadmin_version(version: &str) -> VersionInfoSingleUrl {
    VersionInfoSingleUrl {
        id: format!("phpmyadmin-{}", version_major_minor(version)),
        version: version.to_string(),
        selected: true,
        display_name: format!("phpMyAdmin {version}"),
        eol: false,
        lts: false,
        checksum: None,
        url: format!("https://files.phpmyadmin.net/phpMyAdmin/{version}/phpMyAdmin-{version}-all-languages.zip"),
    }
}

fn adminer_version(version: &str) -> VersionInfoSingleUrl {
    VersionInfoSingleUrl {
        id: format!("adminer-{}", version_major_minor(version)),
        version: version.to_string(),
        selected: false,
        display_name: format!("Adminer {version}"),
        eol: false,
        lts: false,
        checksum: None,
        url: format!("https://www.adminer.org/static/download/{version}/adminer-{version}.php"),
    }
}

fn node_version_with_label(version: &str, lts: bool, selected: bool, label: &str) -> VersionInfo {
    VersionInfo {
        id: format!("node-{}", version.split('.').next().unwrap_or(version)),
        version: version.to_string(),
        selected,
        display_name: format!("Node.js {version} ({label})"),
        eol: false,
        lts,
        checksums: Checksums::default(),
        urls: Urls {
            windows_x64: Some(format!(
                "https://nodejs.org/dist/v{version}/node-v{version}-win-x64.zip"
            )),
            windows_arm64: Some(format!(
                "https://nodejs.org/dist/v{version}/node-v{version}-win-arm64.zip"
            )),
            linux_x64: Some(format!(
                "https://nodejs.org/dist/v{version}/node-v{version}-linux-x64.tar.xz"
            )),
            linux_arm64: Some(format!(
                "https://nodejs.org/dist/v{version}/node-v{version}-linux-arm64.tar.xz"
            )),
            macos_x64: Some(format!(
                "https://nodejs.org/dist/v{version}/node-v{version}-darwin-x64.tar.gz"
            )),
            macos_arm64: Some(format!(
                "https://nodejs.org/dist/v{version}/node-v{version}-darwin-arm64.tar.gz"
            )),
        },
    }
}

fn python_version(version: &str) -> VersionInfo {
    let build_tag = "20260623";
    VersionInfo {
        id: format!("python-{}", version_major_minor(version)),
        version: version.to_string(),
        selected: true,
        display_name: format!("Python {version}"),
        eol: false,
        lts: false,
        checksums: Checksums::default(),
        urls: Urls {
            windows_x64: Some(format!("https://www.python.org/ftp/python/{version}/python-{version}-embed-amd64.zip")),
            windows_arm64: Some(format!("https://www.python.org/ftp/python/{version}/python-{version}-embed-arm64.zip")),
            linux_x64: Some(format!("https://github.com/astral-sh/python-build-standalone/releases/download/{build_tag}/cpython-{version}+{build_tag}-x86_64-unknown-linux-gnu-install_only.tar.gz")),
            linux_arm64: Some(format!("https://github.com/astral-sh/python-build-standalone/releases/download/{build_tag}/cpython-{version}+{build_tag}-aarch64-unknown-linux-gnu-install_only.tar.gz")),
            macos_x64: Some(format!("https://github.com/astral-sh/python-build-standalone/releases/download/{build_tag}/cpython-{version}+{build_tag}-x86_64-apple-darwin-install_only.tar.gz")),
            macos_arm64: Some(format!("https://github.com/astral-sh/python-build-standalone/releases/download/{build_tag}/cpython-{version}+{build_tag}-aarch64-apple-darwin-install_only.tar.gz")),
        },
    }
}

fn go_version(version: &str) -> VersionInfo {
    VersionInfo {
        id: format!("go-{}", version_major_minor(version)),
        version: version.to_string(),
        selected: true,
        display_name: format!("Go {version}"),
        eol: false,
        lts: false,
        checksums: Checksums::default(),
        urls: Urls {
            windows_x64: Some(format!("https://go.dev/dl/go{version}.windows-amd64.zip")),
            windows_arm64: Some(format!("https://go.dev/dl/go{version}.windows-arm64.zip")),
            linux_x64: Some(format!("https://go.dev/dl/go{version}.linux-amd64.tar.gz")),
            linux_arm64: Some(format!("https://go.dev/dl/go{version}.linux-arm64.tar.gz")),
            macos_x64: Some(format!("https://go.dev/dl/go{version}.darwin-amd64.tar.gz")),
            macos_arm64: Some(format!("https://go.dev/dl/go{version}.darwin-arm64.tar.gz")),
        },
    }
}

fn ruby_version(version: &str) -> VersionInfo {
    let family = version_major_minor(version);
    VersionInfo {
        id: format!("ruby-{family}"),
        version: version.to_string(),
        selected: true,
        display_name: format!("Ruby {version}"),
        eol: false,
        lts: false,
        checksums: Checksums::default(),
        urls: Urls {
            windows_x64: Some(format!("https://github.com/oneclick/rubyinstaller2/releases/download/RubyInstaller-{version}-1/rubyinstaller-{version}-1-x64.7z")),
            windows_arm64: None,
            linux_x64: Some(format!("https://cache.ruby-lang.org/pub/ruby/{family}/ruby-{version}.tar.xz")),
            linux_arm64: Some(format!("https://cache.ruby-lang.org/pub/ruby/{family}/ruby-{version}.tar.xz")),
            macos_x64: Some(format!("https://cache.ruby-lang.org/pub/ruby/{family}/ruby-{version}.tar.xz")),
            macos_arm64: Some(format!("https://cache.ruby-lang.org/pub/ruby/{family}/ruby-{version}.tar.xz")),
        },
    }
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
        node: cfg
            .binaries
            .node
            .as_ref()
            .map(|b| b.versions.iter().map(version_info_to_generic).collect())
            .unwrap_or_default(),
        python: cfg
            .binaries
            .python
            .as_ref()
            .map(|b| b.versions.iter().map(version_info_to_generic).collect())
            .unwrap_or_default(),
        go: cfg
            .binaries
            .go
            .as_ref()
            .map(|b| b.versions.iter().map(version_info_to_generic).collect())
            .unwrap_or_default(),
        ruby: cfg
            .binaries
            .ruby
            .as_ref()
            .map(|b| b.versions.iter().map(version_info_to_generic).collect())
            .unwrap_or_default(),
    }
}

fn version_info_to_generic(v: &VersionInfo) -> GenericPackage {
    GenericPackage {
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
        node: None,
        python: None,
        go: None,
        ruby: None,
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

/// Get Node.js package by ID
pub fn get_node_package(id: &str) -> Option<GenericPackage> {
    get_available_packages()
        .node
        .into_iter()
        .find(|p| p.id == id)
}

/// Get Python package by ID
pub fn get_python_package(id: &str) -> Option<GenericPackage> {
    get_available_packages()
        .python
        .into_iter()
        .find(|p| p.id == id)
}

/// Get Go package by ID
pub fn get_go_package(id: &str) -> Option<GenericPackage> {
    get_available_packages().go.into_iter().find(|p| p.id == id)
}

/// Get Ruby package by ID
pub fn get_ruby_package(id: &str) -> Option<GenericPackage> {
    get_available_packages()
        .ruby
        .into_iter()
        .find(|p| p.id == id)
}

/// Reload the runtime configuration (call after modifying the config file)
pub fn reload_runtime_config() {
    replace_runtime_config(load_runtime_config_from_file());
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
