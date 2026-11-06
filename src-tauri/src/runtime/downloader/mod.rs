use crate::runtime::packages::{PackageSelection, RuntimeConfig, VersionInfo};
use reqwest::Client;
use sha2::Sha256;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

mod component;
mod extract;
mod fetch;
mod install;
mod verify;

pub use crate::runtime::platform::Platform;
pub use install::{is_database_tool_installed, is_selected_database_installed};

const MERGE_BUFFER_BYTES: usize = 1024 * 1024;
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TOTAL_TIMEOUT: Duration = Duration::from_secs(900);

/// Get the shared runtime config from packages.rs so catalog refreshes affect downloads.
fn get_config() -> RuntimeConfig {
    crate::runtime::packages::get_config().unwrap_or_else(get_default_config)
}

/// Default hardcoded configuration (fallback when config file is not available)
fn get_default_config() -> RuntimeConfig {
    crate::runtime::packages::embedded_default_runtime_config()
}

/// The entry marked selected, falling back to the first one.
fn selected_or_first<T>(versions: &[T], selected: impl Fn(&T) -> bool) -> Option<&T> {
    versions.iter().find(|v| selected(v)).or(versions.first())
}

/// The selected version string, falling back to the first configured version,
/// or an empty string when no versions are configured. (M-02)
fn selected_or_first_version(versions: &[VersionInfo]) -> String {
    selected_or_first(versions, |v| v.selected)
        .map(|v| v.version.clone())
        .unwrap_or_default()
}

/// Binary component types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryComponent {
    Caddy,
    Php,
    MySQL,
    PostgreSQL,
    PhpMyAdmin,
}

impl BinaryComponent {
    pub fn name(&self) -> &str {
        match self {
            BinaryComponent::Caddy => "Caddy",
            BinaryComponent::Php => "PHP",
            BinaryComponent::MySQL => "MySQL",
            BinaryComponent::PostgreSQL => "PostgreSQL",
            BinaryComponent::PhpMyAdmin => "phpMyAdmin",
        }
    }

    pub fn version(&self) -> String {
        let config = get_config();
        match self {
            BinaryComponent::Caddy => selected_or_first_version(&config.binaries.caddy.versions),
            BinaryComponent::Php => selected_or_first_version(&config.binaries.php.versions),
            BinaryComponent::MySQL => selected_or_first_version(&config.binaries.mysql.versions),
            BinaryComponent::PostgreSQL => {
                selected_or_first_version(&config.binaries.postgresql.versions)
            }
            BinaryComponent::PhpMyAdmin => {
                selected_or_first(&config.binaries.phpmyadmin.versions, |v| v.selected)
                    .map(|v| v.version.clone())
                    .unwrap_or_default()
            }
        }
    }

    pub fn display_name(&self) -> String {
        format!("{} {}", self.name(), self.version())
    }

    pub fn binary_name(&self) -> &'static str {
        match self {
            BinaryComponent::Caddy => "caddy",
            BinaryComponent::Php => "php",
            BinaryComponent::MySQL => "mysql",
            BinaryComponent::PostgreSQL => "postgresql",
            BinaryComponent::PhpMyAdmin => "adminer",
        }
    }
}

/// Download progress information
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub step: DownloadStep,
    pub percent: u8,
    pub current_component: String,
    pub component_display: String,
    pub version: String,
    pub total_components: u8,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

/// Download step
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStep {
    Downloading,
    Extracting,
    Complete,
}

pub type ProgressCallback = Box<dyn Fn(DownloadProgress) + Send + Sync>;

struct DownloadedFile {
    downloaded_bytes: u64,
    total_bytes: u64,
    magic_bytes: Vec<u8>,
    hasher: Sha256,
}

struct SegmentDownloadResult {
    index: usize,
    path: PathBuf,
    bytes: u64,
}

struct DownloadProgressMeta<'a> {
    progress_cb: &'a ProgressCallback,
    component_name: String,
    component_display: String,
    current: u8,
    total: u8,
    version: &'a str,
}

impl DownloadProgressMeta<'_> {
    fn emit(&self, percent: u8, downloaded_bytes: u64, total_bytes: u64) {
        (self.progress_cb)(DownloadProgress {
            step: DownloadStep::Downloading,
            percent,
            current_component: format!("{} {}/{}", self.component_name, self.current, self.total),
            component_display: self.component_display.clone(),
            version: self.version.to_string(),
            total_components: self.total,
            downloaded_bytes,
            total_bytes,
        });
    }
}

#[derive(Clone, Copy)]
struct DownloadFinalizeRequest<'a> {
    component: BinaryComponent,
    extension: &'a str,
    partial_path: &'a Path,
    file_path: &'a Path,
    progress_meta: &'a DownloadProgressMeta<'a>,
}

struct DownloadMetadata {
    total_bytes: u64,
    accepts_ranges: bool,
}

/// Runtime binary downloader
#[derive(Clone)]
pub struct RuntimeDownloader {
    platform: Platform,
    client: Client,
    package_selection: Option<PackageSelection>,
    /// When `false` (default, fail-closed) a download whose component/platform
    /// has no configured SHA-256 checksum is rejected. `true` installs it on TLS
    /// alone; the UI does not ask the user about this. (S-01)
    allow_unverified_checksums: bool,
}

impl RuntimeDownloader {
    fn build_client() -> Client {
        Client::builder()
            .connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
            .timeout(DOWNLOAD_TOTAL_TIMEOUT)
            .tcp_nodelay(true)
            // Many runtimes have no checksum, so TLS is their only protection;
            // a redirect must not be able to downgrade it.
            .https_only(true)
            .build()
            .unwrap_or_else(|_| Client::new())
    }

    /// Create a new runtime downloader
    pub fn new() -> Self {
        Self {
            platform: Platform::current(),
            client: Self::build_client(),
            package_selection: None,
            allow_unverified_checksums: false,
        }
    }

    /// Create a new runtime downloader with custom package selection
    pub fn with_packages(package_selection: PackageSelection) -> Self {
        Self {
            platform: Platform::current(),
            client: Self::build_client(),
            package_selection: Some(package_selection.normalized()),
            allow_unverified_checksums: false,
        }
    }

    /// Opt in to installing binaries that have no configured SHA-256 checksum.
    /// This bypasses the fail-closed guard in `finalize_downloaded_file`. (S-01)
    pub fn allow_unverified_checksums(mut self, allow: bool) -> Self {
        self.allow_unverified_checksums = allow;
        self
    }
}

impl Default for RuntimeDownloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Read `version=...` from an install marker file (`version=1.2.3\ninstalled_at=...`).
pub fn read_marker_version(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("version=").map(str::to_string))
}

#[cfg(test)]
mod tests;
