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

    if let Some(latest) = crate::runtime::php_catalog::fetch_latest_php(&client).await {
        changed |= crate::runtime::php_catalog::merge_php_versions(
            &mut config.binaries.php.versions,
            latest,
        );
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
                selected_version(mysql_version("8.4.13"), false),
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

    // Anchor on the download link: the bare product name also matches the page
    // title, and the next number after it was SVG path data ("6.5").
    if let Some(version) = fetch_version_from_text(
        &client,
        "https://www.phpmyadmin.net/downloads/",
        PHPMYADMIN_VERSION_MARKER,
    )
    .await
    {
        changed |= replace_database_tool(
            &mut config.binaries.phpmyadmin.versions,
            "phpmyadmin",
            phpmyadmin_version(&version),
        );
    }

    // The version page is HTML with CSS in front of the number, so the marker
    // must anchor on the download link; an empty marker picked "1.25" out of
    // `font: 13px/1.25` and produced a dead download URL.
    if let Some(version) = fetch_version_from_text(
        &client,
        "https://www.adminer.org/version/?current=0&lang=en",
        ADMINER_VERSION_MARKER,
    )
    .await
    {
        changed |= replace_database_tool(
            &mut config.binaries.phpmyadmin.versions,
            "adminer",
            adminer_version(&version),
        );
    }

    if changed {
        persist_runtime_config_override(&config)?;
        replace_runtime_config(Some(config.clone()));
    }

    Ok(runtime_config_to_packages(&config))
}

/// Refresh only the PHP section of the catalog (three small JSON requests), so
/// installing or updating PHP always uses the newest patch release.
pub async fn refresh_php_catalog() -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("CHAMP runtime catalog refresher")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let latest = crate::runtime::php_catalog::fetch_latest_php(&client)
        .await
        .ok_or("Could not reach php.net")?;

    let mut config = get_config().unwrap_or_else(embedded_default_runtime_config);
    if crate::runtime::php_catalog::merge_php_versions(&mut config.binaries.php.versions, latest) {
        persist_runtime_config_override(&config)?;
        replace_runtime_config(Some(config));
    }
    Ok(())
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

    let marker_found = marker.is_empty() || text.contains(marker);

    for (start, ch) in haystack.char_indices() {
        if !ch.is_ascii_digit() {
            continue;
        }

        let candidate: String = haystack[start..]
            .chars()
            .take_while(|value| value.is_ascii_digit() || *value == '.')
            .collect();
        let candidate = candidate.trim_end_matches('.').to_string();

        // M-10: only accept a candidate that looks like a real version. If the
        // upstream page wording changed and the first digit run after the marker
        // is not a plausible version (e.g. a year "2024" with no dot, or "0"),
        // keep scanning rather than composing a wrong number into download URLs.
        if is_plausible_version(&candidate) {
            return Some(candidate);
        }
    }

    // M-10: a marker was found but nothing after it parsed as a plausible
    // version. Surface this in logs so a broken scrape is visible instead of
    // silently keeping (or worse, picking a wrong) value. Returning None makes
    // the caller keep its existing local catalog entry, the safe outcome.
    if marker_found {
        eprintln!(
            "runtime catalog refresh: marker {:?} found but no plausible version followed; keeping local catalog entry",
            marker
        );
    }

    None
}

/// M-10: reject implausible version strings scraped from upstream HTML.
///
/// A plausible version has at least one `.`, every dot-separated segment is a
/// non-empty run of ASCII digits, the major segment parses to a `u16` in
/// `1..=99` (rejecting "0" and absurd values), and the whole thing is short.
fn is_plausible_version(candidate: &str) -> bool {
    if candidate.len() > 14 {
        return false;
    }
    if !candidate.contains('.') {
        return false;
    }

    let mut segments = candidate.split('.');

    let major = match segments.next() {
        Some(segment) => segment,
        None => return false,
    };
    if major.is_empty() || !major.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    match major.parse::<u16>() {
        Ok(value) if (1..=99).contains(&value) => {}
        _ => return false,
    }

    // Remaining segments (there is at least one because of the '.' check) must
    // each be a non-empty run of ASCII digits.
    segments.all(|segment| !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit()))
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

/// cdn.mysql.com keeps only the current patch release under `/Downloads/` and
/// moves superseded builds to `/archives/mysql-<family>/`.
pub fn mysql_archive_url(url: &str) -> Option<String> {
    let (family, file) = url
        .strip_prefix("https://cdn.mysql.com/Downloads/MySQL-")?
        .split_once('/')?;
    Some(format!(
        "https://cdn.mysql.com/archives/mysql-{family}/{file}"
    ))
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

/// Text right before the version number in phpMyAdmin's download links.
const PHPMYADMIN_VERSION_MARKER: &str = "files.phpmyadmin.net/phpMyAdmin/";

/// Text right before the version number on adminer.org's version page.
const ADMINER_VERSION_MARKER: &str = "#download\" target=\"_blank\">";

/// Keep exactly one release of a web database manager (`adminer` or
/// `phpmyadmin`): it only ever offers the latest download, so older entries
/// (and ones an earlier bad refresh wrote) point at dead URLs.
fn replace_database_tool(
    versions: &mut Vec<VersionInfoSingleUrl>,
    family: &str,
    next: VersionInfoSingleUrl,
) -> bool {
    let unchanged = {
        let mut current = versions.iter().filter(|v| v.id.starts_with(family));
        matches!(
            (current.next(), current.next()),
            (Some(only), None) if only.id == next.id && only.version == next.version && only.url == next.url
        )
    };
    if unchanged {
        return false;
    }

    let was_selected = versions
        .iter()
        .any(|version| version.id.starts_with(family) && version.selected);
    versions.retain(|version| !version.id.starts_with(family));
    versions.push(VersionInfoSingleUrl {
        selected: was_selected,
        ..next
    });
    true
}

/// Map a saved web-database-manager id onto the catalog.
///
/// The id carries the release (`adminer-5.4`), so it goes stale when the
/// catalog moves on. The family (`adminer` / `phpmyadmin`) is what decides the
/// database engine, so fall back to the same family's current entry.
pub fn resolve_database_tool_id(id: &str) -> String {
    resolve_tool_id_in(id, &get_available_packages().phpmyadmin)
}

fn resolve_tool_id_in(id: &str, tools: &[PhpMyAdminPackage]) -> String {
    if tools.iter().any(|tool| tool.id == id) {
        return id.to_string();
    }
    let family = if id.starts_with("adminer") {
        "adminer"
    } else {
        "phpmyadmin"
    };
    tools
        .iter()
        .find(|tool| tool.id.starts_with(family))
        .map_or_else(|| id.to_string(), |tool| tool.id.clone())
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
mod tests {
    use super::*;

    #[test]
    fn mysql_archive_url_maps_downloads_to_archives() {
        assert_eq!(
            mysql_archive_url("https://cdn.mysql.com/Downloads/MySQL-9.7/mysql-9.7.1-winx64.zip")
                .as_deref(),
            Some("https://cdn.mysql.com/archives/mysql-9.7/mysql-9.7.1-winx64.zip")
        );
        assert_eq!(
            mysql_archive_url("https://cdn.mysql.com/archives/mysql-9.7/mysql-9.7.1-winx64.zip"),
            None
        );
    }

    #[test]
    fn version_after_marker_accepts_plausible() {
        assert_eq!(
            version_after_marker("downloads of PHP 8.4.3 now", "downloads of PHP "),
            Some("8.4.3".to_string())
        );
    }

    #[test]
    fn version_after_marker_rejects_implausible() {
        // A lone "0" after the marker (no dot) must not be accepted.
        assert_eq!(version_after_marker("release 0 build", "release "), None);

        // A bare year "2024" (no dot) must not be accepted.
        assert_eq!(
            version_after_marker("copyright 2024 vendor", "copyright "),
            None
        );

        // Marker present but followed by non-version garbage.
        assert_eq!(
            version_after_marker("version: none here", "version: "),
            None
        );
    }

    #[test]
    fn adminer_version_ignores_digits_in_page_css() {
        let page = "<style>body { font: 13px/1.25 Verdana; }</style>\
                    <a href=\"../#download\" target=\"_blank\">6.2.0</a>";
        assert_eq!(
            version_after_marker(page, ADMINER_VERSION_MARKER),
            Some("6.2.0".to_string())
        );
        // Page wording changed: no marker means no version, never a CSS number.
        assert_eq!(
            version_after_marker("<style>font: 13px/1.25</style>", ADMINER_VERSION_MARKER),
            None
        );
    }

    #[test]
    fn phpmyadmin_version_ignores_svg_numbers() {
        let page = "<title>Downloads · phpMyAdmin</title><path d=\"M11 6.5a.5.5\"/>\
                    <a href=\"https://files.phpmyadmin.net/phpMyAdmin/5.2.3/phpMyAdmin-5.2.3.zip\">";
        assert_eq!(
            version_after_marker(page, PHPMYADMIN_VERSION_MARKER),
            Some("5.2.3".to_string())
        );
    }

    #[test]
    fn replace_database_tool_keeps_a_single_current_release() {
        let mut versions = vec![
            phpmyadmin_version("5.2.3"),
            adminer_version("5.4.2"),
            adminer_version("1.25"),
        ];

        assert!(replace_database_tool(
            &mut versions,
            "adminer",
            adminer_version("6.2.0")
        ));
        let adminers: Vec<_> = versions
            .iter()
            .filter(|v| v.id.starts_with("adminer"))
            .collect();
        assert_eq!(adminers.len(), 1);
        assert_eq!(adminers[0].version, "6.2.0");
        assert!(versions.iter().any(|v| v.id == "phpmyadmin-5.2"));

        // Same release again is a no-op.
        assert!(!replace_database_tool(
            &mut versions,
            "adminer",
            adminer_version("6.2.0")
        ));
    }

    #[test]
    fn stale_database_tool_id_resolves_to_current_release() {
        let tool = |id: &str| PhpMyAdminPackage {
            id: id.to_string(),
            version: String::new(),
            display_name: String::new(),
            url: String::new(),
            eol: false,
            lts: false,
            recommended: false,
        };
        let catalog = [tool("phpmyadmin-5.2"), tool("adminer-6.2")];

        assert_eq!(resolve_tool_id_in("adminer-5.4", &catalog), "adminer-6.2");
        assert_eq!(resolve_tool_id_in("adminer-6.2", &catalog), "adminer-6.2");
        assert_eq!(
            resolve_tool_id_in("phpmyadmin-4.9", &catalog),
            "phpmyadmin-5.2"
        );
        // No catalog match: the id is left alone.
        assert_eq!(resolve_tool_id_in("adminer-5.4", &[]), "adminer-5.4");
    }

    #[test]
    fn is_plausible_version_basic_cases() {
        // Valid versions.
        assert!(is_plausible_version("8.4.3"));
        assert!(is_plausible_version("17.10"));
        assert!(is_plausible_version("1.0"));

        // No dot.
        assert!(!is_plausible_version("2024"));
        assert!(!is_plausible_version("8"));

        // Zero / out-of-range major.
        assert!(!is_plausible_version("0.1"));
        assert!(!is_plausible_version("100.1"));

        // Empty segment.
        assert!(!is_plausible_version("8..3"));
        assert!(!is_plausible_version("8."));

        // Too long.
        assert!(!is_plausible_version("1.2345678901234"));

        // Non-digit segment.
        assert!(!is_plausible_version("8.4a"));
    }
}
