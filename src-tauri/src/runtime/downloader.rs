use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use reqwest::Client;

use crate::runtime::locator::get_app_data_paths;
use crate::runtime::packages::{
    get_mysql_package, get_php_package, get_phpmyadmin_package, PackageSelection,
};
use sha2::{Digest, Sha256};

/// Runtime configuration loaded from runtime-config.json (shared with packages.rs)
pub use crate::runtime::packages::{
    BinariesConfig, BinaryConfig, Checksums, PhpMyAdminConfig, RuntimeConfig, Urls, VersionInfo,
};

/// Global runtime config (loaded once)
static RUNTIME_CONFIG: OnceLock<RuntimeConfig> = OnceLock::new();

/// Load runtime configuration from bundled resource or file
pub fn load_runtime_config() -> RuntimeConfig {
    // Try to load from various locations
    let config_content = load_config_content();

    match config_content {
        Some(content) => match serde_json::from_str(&content) {
            Ok(config) => {
                eprintln!("Loaded runtime configuration from file");
                config
            }
            Err(e) => {
                eprintln!(
                    "Failed to parse runtime-config.json: {}. Using defaults.",
                    e
                );
                get_default_config()
            }
        },
        None => {
            eprintln!("runtime-config.json not found. Using default configuration.");
            get_default_config()
        }
    }
}

/// Try to load config content from various locations
fn load_config_content() -> Option<String> {
    // 1. Try current directory (for development)
    if let Ok(content) = fs::read_to_string("runtime-config.json") {
        return Some(content);
    }

    // 2. Try src-tauri directory (for development)
    if let Ok(content) = fs::read_to_string("src-tauri/runtime-config.json") {
        return Some(content);
    }

    // 3. Try alongside the executable (for production)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let config_path = exe_dir.join("runtime-config.json");
            if let Ok(content) = fs::read_to_string(&config_path) {
                return Some(content);
            }
        }
    }

    // 4. Try AppData/resources (Windows)
    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = dirs::data_local_dir() {
            // Check in the app installation directory
            let config_path = local_app_data.join("campp").join("runtime-config.json");
            if let Ok(content) = fs::read_to_string(&config_path) {
                return Some(content);
            }
        }
    }

    None
}

/// Get the global runtime config (loads once, then caches)
fn get_config() -> &'static RuntimeConfig {
    RUNTIME_CONFIG.get_or_init(load_runtime_config)
}

/// Default hardcoded configuration (fallback when config file is not available)
fn get_default_config() -> RuntimeConfig {
    use crate::runtime::packages::VersionInfoSingleUrl;

    RuntimeConfig {
        version: "1.0".to_string(),
        binaries: BinariesConfig {
            caddy: BinaryConfig {
                versions: vec![
                    VersionInfo {
                        id: "caddy-2.11".to_string(),
                        version: "2.11.2".to_string(),
                        selected: true,
                        display_name: "Caddy 2.11.2".to_string(),
                        eol: false,
                        lts: false,
                        checksums: Checksums::default(),
                        urls: Urls {
                            windows_x64: Some("https://github.com/caddyserver/caddy/releases/download/v2.11.2/caddy_2.11.2_windows_amd64.zip".to_string()),
                            windows_arm64: Some("https://github.com/caddyserver/caddy/releases/download/v2.11.2/caddy_2.11.2_windows_arm64.zip".to_string()),
                            linux_x64: Some("https://github.com/caddyserver/caddy/releases/download/v2.11.2/caddy_2.11.2_linux_amd64.tar.gz".to_string()),
                            linux_arm64: Some("https://github.com/caddyserver/caddy/releases/download/v2.11.2/caddy_2.11.2_linux_arm64.tar.gz".to_string()),
                            macos_x64: Some("https://github.com/caddyserver/caddy/releases/download/v2.11.2/caddy_2.11.2_mac_amd64.tar.gz".to_string()),
                            macos_arm64: Some("https://github.com/caddyserver/caddy/releases/download/v2.11.2/caddy_2.11.2_mac_arm64.tar.gz".to_string()),
                        },
                    },
                ],
            },
            php: BinaryConfig {
                versions: vec![
                    VersionInfo {
                        id: "php-8.5".to_string(),
                        version: "8.5.1".to_string(),
                        selected: true,
                        display_name: "PHP 8.5.1".to_string(),
                        eol: false,
                        lts: false,
                        checksums: Checksums::default(),
                        urls: Urls {
                            windows_x64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/php-8.5.1/php-8.5.1-Win32-vs17-x64.zip".to_string()),
                            windows_arm64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/php-8.5.1/php-8.5.1-Win32-vs17-x86.zip".to_string()),
                            linux_x64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/php-8.5.1/php-8.4.18-fpm-linux-x86_64.tar.gz".to_string()),
                            linux_arm64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/php-8.5.1/php-8.4.18-fpm-linux-aarch64.tar.gz".to_string()),
                            macos_x64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/php-8.5.1/php-8.4.18-fpm-macos-x86_64.tar.gz".to_string()),
                            macos_arm64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/php-8.5.1/php-8.4.18-fpm-macos-aarch64.tar.gz".to_string()),
                        },
                    },
                ],
            },
            mysql: BinaryConfig {
                versions: vec![
                    VersionInfo {
                        id: "mysql-8.4".to_string(),
                        version: "8.4.0".to_string(),
                        selected: true,
                        display_name: "MySQL 8.4.0 LTS".to_string(),
                        eol: false,
                        lts: true,
                        checksums: Checksums::default(),
                        urls: Urls {
                            windows_x64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/mysql-8.4.0/mysql-8.4.0-winx64.zip".to_string()),
                            windows_arm64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/mysql-8.4.0/mysql-8.4.0-winx64.zip".to_string()),
                            linux_x64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/mysql-8.4.0/mysql-8.4.0-linux-glibc2.28-x86_64.tar.xz".to_string()),
                            linux_arm64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/mysql-8.4.0/mysql-8.4.0-linux-glibc2.28-aarch64.tar.xz".to_string()),
                            macos_x64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/mysql-8.4.0/mysql-8.4.0-macos14-x86_64.tar.gz".to_string()),
                            macos_arm64: Some("https://github.com/KarnYong/campp-runtime-binaries/releases/download/mysql-8.4.0/mysql-8.4.0-macos14-arm64.tar.gz".to_string()),
                        },
                    },
                ],
            },
            phpmyadmin: PhpMyAdminConfig {
                versions: vec![
                    VersionInfoSingleUrl {
                        id: "adminer-5.4".to_string(),
                        version: "5.4.1".to_string(),
                        selected: true,
                        display_name: "Adminer 5.4.1".to_string(),
                        eol: false,
                        lts: false,
                        checksum: None,
                        url: "https://github.com/vrana/adminer/releases/download/v5.4.1/adminer-5.4.1.php".to_string(),
                    },
                ],
            },
        },
    }
}

/// Binary component types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryComponent {
    Caddy,
    Php,
    MySQL,
    PhpMyAdmin,
}

impl BinaryComponent {
    pub fn name(&self) -> &str {
        match self {
            BinaryComponent::Caddy => "Caddy",
            BinaryComponent::Php => "PHP",
            BinaryComponent::MySQL => "MySQL",
            BinaryComponent::PhpMyAdmin => "Adminer",
        }
    }

    pub fn version(&self) -> String {
        let config = get_config();
        match self {
            BinaryComponent::Caddy => config
                .binaries
                .caddy
                .versions
                .iter()
                .find(|v| v.selected)
                .map(|v| v.version.clone())
                .unwrap_or_else(|| {
                    config
                        .binaries
                        .caddy
                        .versions
                        .first()
                        .map(|v| v.version.clone())
                        .unwrap_or_default()
                }),
            BinaryComponent::Php => config
                .binaries
                .php
                .versions
                .iter()
                .find(|v| v.selected)
                .map(|v| v.version.clone())
                .unwrap_or_else(|| {
                    config
                        .binaries
                        .php
                        .versions
                        .first()
                        .map(|v| v.version.clone())
                        .unwrap_or_default()
                }),
            BinaryComponent::MySQL => config
                .binaries
                .mysql
                .versions
                .iter()
                .find(|v| v.selected)
                .map(|v| v.version.clone())
                .unwrap_or_else(|| {
                    config
                        .binaries
                        .mysql
                        .versions
                        .first()
                        .map(|v| v.version.clone())
                        .unwrap_or_default()
                }),
            BinaryComponent::PhpMyAdmin => config
                .binaries
                .phpmyadmin
                .versions
                .iter()
                .find(|v| v.selected)
                .map(|v| v.version.clone())
                .unwrap_or_else(|| {
                    config
                        .binaries
                        .phpmyadmin
                        .versions
                        .first()
                        .map(|v| v.version.clone())
                        .unwrap_or_default()
                }),
        }
    }

    pub fn display_name(&self) -> String {
        format!("{} {}", self.name(), self.version())
    }

    pub fn binary_name(&self) -> &str {
        match self {
            BinaryComponent::Caddy => "caddy",
            BinaryComponent::Php => "php",
            BinaryComponent::MySQL => "mysql",
            BinaryComponent::PhpMyAdmin => "adminer",
        }
    }
}

impl RuntimeDownloader {
    /// Get version for a component based on current package selection
    pub fn get_component_version(&self, component: &BinaryComponent) -> String {
        if let Some(selection) = &self.package_selection {
            match component {
                BinaryComponent::Php => {
                    if let Some(pkg) = get_php_package(&selection.php) {
                        return pkg.version;
                    }
                }
                BinaryComponent::MySQL => {
                    if let Some(pkg) = get_mysql_package(&selection.mysql) {
                        return pkg.version;
                    }
                }
                BinaryComponent::PhpMyAdmin => {
                    if let Some(pkg) = get_phpmyadmin_package(&selection.phpmyadmin) {
                        return pkg.version;
                    }
                }
                BinaryComponent::Caddy => {
                    // Caddy uses default version
                }
            }
        }

        // Fall back to default config
        component.version()
    }
}

/// Platform information
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    WindowsX64,
    WindowsArm64,
    MacOSX64,
    MacOSArm64,
    LinuxX64,
    LinuxArm64,
}

impl Platform {
    /// Detect the current platform
    #[allow(unreachable_code)]
    pub fn current() -> Self {
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        return Platform::WindowsX64;

        #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
        return Platform::WindowsArm64;

        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        return Platform::MacOSX64;

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        return Platform::MacOSArm64;

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        return Platform::LinuxX64;

        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        return Platform::LinuxArm64;

        // Default to Linux x64 for unknown platforms
        Platform::LinuxX64
    }

    /// Get the URL key for config lookup (matches JSON keys)
    pub fn url_key(&self) -> String {
        match self {
            Platform::WindowsX64 => "windowsX64",
            Platform::WindowsArm64 => "windowsArm64",
            Platform::LinuxX64 => "linuxX64",
            Platform::LinuxArm64 => "linuxArm64",
            Platform::MacOSX64 => "macOSX64",
            Platform::MacOSArm64 => "macOSArm64",
        }
        .to_string()
    }
}

/// Download progress information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStep {
    Downloading,
    Extracting,
    Installing,
    Complete,
    Error(String),
}

pub type ProgressCallback = Box<dyn Fn(DownloadProgress) + Send + Sync>;

/// Runtime binary downloader
pub struct RuntimeDownloader {
    platform: Platform,
    client: Client,
    package_selection: Option<PackageSelection>,
}

impl RuntimeDownloader {
    /// Create a new runtime downloader
    pub fn new() -> Self {
        Self {
            platform: Platform::current(),
            client: Client::new(),
            package_selection: None,
        }
    }

    /// Create a new runtime downloader with custom package selection
    pub fn with_packages(package_selection: PackageSelection) -> Self {
        Self {
            platform: Platform::current(),
            client: Client::new(),
            package_selection: Some(package_selection),
        }
    }

    /// Get the URL for a binary component from config
    fn get_binary_url(&self, component: BinaryComponent) -> String {
        // Use selected packages if available, otherwise fall back to default config
        if let Some(selection) = &self.package_selection {
            match component {
                BinaryComponent::Php => {
                    if let Some(pkg) = get_php_package(&selection.php) {
                        return match self.platform {
                            Platform::WindowsX64 => pkg.windows_x64,
                            Platform::WindowsArm64 => pkg.windows_arm64,
                            Platform::MacOSX64 => pkg.macos_x64,
                            Platform::MacOSArm64 => pkg.macos_arm64,
                            Platform::LinuxX64 => pkg.linux_x64,
                            Platform::LinuxArm64 => pkg.linux_arm64,
                        };
                    }
                }
                BinaryComponent::MySQL => {
                    if let Some(pkg) = get_mysql_package(&selection.mysql) {
                        return match self.platform {
                            Platform::WindowsX64 => pkg.windows_x64,
                            Platform::WindowsArm64 => pkg.windows_arm64,
                            Platform::MacOSX64 => pkg.macos_x64,
                            Platform::MacOSArm64 => pkg.macos_arm64,
                            Platform::LinuxX64 => pkg.linux_x64,
                            Platform::LinuxArm64 => pkg.linux_arm64,
                        };
                    }
                }
                BinaryComponent::PhpMyAdmin => {
                    if let Some(pkg) = get_phpmyadmin_package(&selection.phpmyadmin) {
                        return pkg.url;
                    }
                }
                BinaryComponent::Caddy => {
                    // Caddy doesn't have package selection, use default
                }
            }
        }

        // Fall back to default config
        let config = get_config();

        match component {
            BinaryComponent::Caddy => {
                let version_info = config
                    .binaries
                    .caddy
                    .versions
                    .iter()
                    .find(|v| v.selected)
                    .or_else(|| config.binaries.caddy.versions.first())
                    .unwrap();
                match self.platform {
                    Platform::WindowsX64 => {
                        version_info.urls.windows_x64.clone().unwrap_or_default()
                    }
                    Platform::WindowsArm64 => {
                        version_info.urls.windows_arm64.clone().unwrap_or_default()
                    }
                    Platform::MacOSX64 => version_info.urls.macos_x64.clone().unwrap_or_default(),
                    Platform::MacOSArm64 => {
                        version_info.urls.macos_arm64.clone().unwrap_or_default()
                    }
                    Platform::LinuxX64 => version_info.urls.linux_x64.clone().unwrap_or_default(),
                    Platform::LinuxArm64 => {
                        version_info.urls.linux_arm64.clone().unwrap_or_default()
                    }
                }
            }
            BinaryComponent::Php => {
                let version_info = config
                    .binaries
                    .php
                    .versions
                    .iter()
                    .find(|v| v.selected)
                    .or_else(|| config.binaries.php.versions.first())
                    .unwrap();
                match self.platform {
                    Platform::WindowsX64 => {
                        version_info.urls.windows_x64.clone().unwrap_or_default()
                    }
                    Platform::WindowsArm64 => {
                        version_info.urls.windows_arm64.clone().unwrap_or_default()
                    }
                    Platform::MacOSX64 => version_info.urls.macos_x64.clone().unwrap_or_default(),
                    Platform::MacOSArm64 => {
                        version_info.urls.macos_arm64.clone().unwrap_or_default()
                    }
                    Platform::LinuxX64 => version_info.urls.linux_x64.clone().unwrap_or_default(),
                    Platform::LinuxArm64 => {
                        version_info.urls.linux_arm64.clone().unwrap_or_default()
                    }
                }
            }
            BinaryComponent::MySQL => {
                let version_info = config
                    .binaries
                    .mysql
                    .versions
                    .iter()
                    .find(|v| v.selected)
                    .or_else(|| config.binaries.mysql.versions.first())
                    .unwrap();
                match self.platform {
                    Platform::WindowsX64 => {
                        version_info.urls.windows_x64.clone().unwrap_or_default()
                    }
                    Platform::WindowsArm64 => {
                        version_info.urls.windows_arm64.clone().unwrap_or_default()
                    }
                    Platform::MacOSX64 => version_info.urls.macos_x64.clone().unwrap_or_default(),
                    Platform::MacOSArm64 => {
                        version_info.urls.macos_arm64.clone().unwrap_or_default()
                    }
                    Platform::LinuxX64 => version_info.urls.linux_x64.clone().unwrap_or_default(),
                    Platform::LinuxArm64 => {
                        version_info.urls.linux_arm64.clone().unwrap_or_default()
                    }
                }
            }
            BinaryComponent::PhpMyAdmin => {
                let version_info = config
                    .binaries
                    .phpmyadmin
                    .versions
                    .iter()
                    .find(|v| v.selected)
                    .or_else(|| config.binaries.phpmyadmin.versions.first())
                    .unwrap();
                version_info.url.clone()
            }
        }
    }

    /// Extract file extension from URL
    fn get_extension_from_url(url: &str) -> String {
        // Get the filename from the URL
        if let Some(filename) = url.split('/').last() {
            // Check for .tar.gz first (compound extension)
            if filename.ends_with(".tar.gz") {
                return "tar.gz".to_string();
            }
            // Otherwise get the extension after the last dot
            if let Some(ext) = filename.split('.').last() {
                return ext.to_string();
            }
        }
        // Default to zip if we can't determine
        "zip".to_string()
    }

    /// Download a single binary component
    async fn download_component(
        &self,
        component: BinaryComponent,
        dest_dir: &Path,
        progress_cb: &ProgressCallback,
        _current: u8,
        total: u8,
    ) -> Result<PathBuf, String> {
        let url = self.get_binary_url(component);
        let extension = Self::get_extension_from_url(&url);

        eprintln!("[DEBUG] Platform: {:?}", self.platform);
        eprintln!("[DEBUG] Downloading {} from: {}", component.name(), url);
        eprintln!("[DEBUG] Full URL ({} chars): {}", url.len(), url);

        // Set platform-appropriate User-Agent
        let user_agent = match self.platform {
            Platform::LinuxX64 | Platform::LinuxArm64 => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            Platform::MacOSX64 | Platform::MacOSArm64 => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            Platform::WindowsX64 | Platform::WindowsArm64 => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        };

        // MySQL downloads require specific headers to bypass their gateway
        let mut request = self.client.get(&url).header("User-Agent", user_agent);

        // Add Referer header for MySQL downloads (required by dev.mysql.com gateway)
        if component == BinaryComponent::MySQL && url.contains("dev.mysql.com") {
            request = request
                .header("Referer", "https://dev.mysql.com/downloads/mysql/")
                .header(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                );
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("Failed to fetch {}: {}", component.name(), e))?;

        // Check status code
        let status = response.status();
        if !status.is_success() {
            return Err(format!(
                "HTTP error {}: Failed to download {}\nURL: {}",
                status.as_u16(),
                component.name(),
                url
            ));
        }

        // Get final URL after redirects
        let final_url = response.url().clone();
        if final_url.as_str() != url {
            eprintln!("Redirected: {} -> {}", url, final_url);
        }

        // Check content type
        if let Some(content_type) = response.headers().get("content-type") {
            if let Ok(ct) = content_type.to_str() {
                if ct.contains("text/html") {
                    return Err(format!(
                        "Server returned HTML instead of binary. URL may be incorrect: {}",
                        final_url
                    ));
                }
            }
        }

        let total_bytes = response.content_length().unwrap_or(0);
        let version = self.get_component_version(&component);
        let file_path = dest_dir.join(format!(
            "{}-{}.{}",
            component.binary_name(),
            version,
            extension
        ));

        // Create parent directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let mut file =
            File::create(&file_path).map_err(|e| format!("Failed to create file: {}", e))?;

        // Download using bytes() for simplicity
        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to download bytes: {}", e))?;

        // Verify the file is valid by checking magic bytes
        if extension != "php" && bytes.len() < 4 {
            return Err(format!(
                "Downloaded file is too small ({} bytes) to be a valid archive",
                bytes.len()
            ));
        }

        // Check if it's a ZIP file (starts with PK)
        let is_zip = bytes.len() >= 2 && bytes[0] == 0x50 && bytes[1] == 0x4B;
        // Check if it's gzip (starts with 0x1f 0x8b)
        let is_gzip = bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b;

        if extension == "zip" && !is_zip {
            return Err(format!(
                "Expected ZIP file but downloaded file doesn't have ZIP magic bytes. URL may have redirected to HTML page."
            ));
        }

        if (extension == "gz" || extension == "tar.gz") && !is_gzip {
            return Err(format!(
                "Expected gzip file but downloaded file doesn't have gzip magic bytes."
            ));
        }

        let downloaded_bytes = bytes.len() as u64;
        file.write_all(&bytes)
            .map_err(|e| format!("Failed to write to file: {}", e))?;

        // Verify checksum if available
        if let Some(expected_checksum) = self.get_expected_checksum(&component, &url) {
            let actual_checksum = self
                .calculate_checksum_from_bytes(&bytes)
                .map_err(|e| format!("Failed to calculate checksum: {}", e))?;

            if actual_checksum.to_lowercase() != expected_checksum.to_lowercase() {
                return Err(format!(
                    "Checksum verification failed for {}.\nExpected: {}\nActual: {}\n\nThe downloaded file may be corrupted or tampered with.",
                    component.name(),
                    expected_checksum,
                    actual_checksum
                ));
            }
            eprintln!(
                "Checksum verified for {}: {}",
                component.name(),
                actual_checksum
            );
        }

        let percent = if total_bytes > 0 {
            ((downloaded_bytes as f64 / total_bytes as f64) * 100.0) as u8
        } else {
            100
        };

        progress_cb(DownloadProgress {
            step: DownloadStep::Downloading,
            percent,
            current_component: component.name().to_string(),
            component_display: component.display_name(),
            version: self.get_component_version(&component),
            total_components: total,
            downloaded_bytes,
            total_bytes,
        });

        Ok(file_path)
    }

    /// Calculate SHA256 checksum from bytes
    fn calculate_checksum_from_bytes(&self, bytes: &[u8]) -> Result<String, String> {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Ok(hex::encode(hasher.finalize()))
    }

    /// Get the expected checksum for a component based on current platform
    fn get_expected_checksum(&self, component: &BinaryComponent, _url: &str) -> Option<String> {
        use crate::runtime::packages::get_config;

        let config = get_config()?;
        let platform_key = self.platform.url_key();

        match component {
            BinaryComponent::Php | BinaryComponent::MySQL | BinaryComponent::Caddy => {
                let version_info = match component {
                    BinaryComponent::Caddy => config.binaries.caddy.versions.iter(),
                    BinaryComponent::Php => config.binaries.php.versions.iter(),
                    BinaryComponent::MySQL => config.binaries.mysql.versions.iter(),
                    _ => return None,
                };

                // Determine which version ID to look for based on package selection
                let target_id = if let Some(ref selection) = self.package_selection {
                    match component {
                        BinaryComponent::Php => Some(selection.php.as_str()),
                        BinaryComponent::MySQL => Some(selection.mysql.as_str()),
                        _ => None,
                    }
                } else {
                    None
                };

                for version in version_info {
                    if target_id.is_some() && version.id == target_id.unwrap() {
                        // Use the package selection
                        return match platform_key.as_str() {
                            "windowsX64" => version.checksums.windows_x64.clone(),
                            "windowsArm64" => version.checksums.windows_arm64.clone(),
                            "linuxX64" => version.checksums.linux_x64.clone(),
                            "linuxArm64" => version.checksums.linux_arm64.clone(),
                            "macOSX64" => version.checksums.macos_x64.clone(),
                            "macOSArm64" => version.checksums.macos_arm64.clone(),
                            _ => None,
                        };
                    } else if target_id.is_none() && version.selected {
                        // Fall back to selected flag
                        return match platform_key.as_str() {
                            "windowsX64" => version.checksums.windows_x64.clone(),
                            "windowsArm64" => version.checksums.windows_arm64.clone(),
                            "linuxX64" => version.checksums.linux_x64.clone(),
                            "linuxArm64" => version.checksums.linux_arm64.clone(),
                            "macOSX64" => version.checksums.macos_x64.clone(),
                            "macOSArm64" => version.checksums.macos_arm64.clone(),
                            _ => None,
                        };
                    }
                }
                None
            }
            BinaryComponent::PhpMyAdmin => {
                let version = config
                    .binaries
                    .phpmyadmin
                    .versions
                    .iter()
                    .find(|v| v.selected)?;
                version.checksum.clone()
            }
        }
    }

    /// Extract a ZIP archive
    fn extract_zip(&self, archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
        let file =
            File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP: {}", e))?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| format!("Failed to get file: {}", e))?;
            let outpath = dest_dir.join(file.enclosed_name().ok_or("Invalid path")?);

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
            } else {
                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create parent directory: {}", e))?;
                }
                let mut outfile =
                    File::create(&outpath).map_err(|e| format!("Failed to create file: {}", e))?;
                io::copy(&mut file, &mut outfile)
                    .map_err(|e| format!("Failed to write file: {}", e))?;

                // Set executable permission on Unix for binary files
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if is_executable(&file.name()) {
                        let mut perms = fs::metadata(&outpath)
                            .map_err(|e| format!("Failed to get metadata: {}", e))?
                            .permissions();
                        perms.set_mode(0o755);
                        fs::set_permissions(&outpath, perms)
                            .map_err(|e| format!("Failed to set permissions: {}", e))?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Extract a tar.gz archive
    fn extract_tar_gz(&self, archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
        use flate2::read::GzDecoder;

        let file =
            File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
        let decoder = GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        // Unpack the archive, ignoring the first directory level if needed
        archive
            .unpack(dest_dir)
            .map_err(|e| format!("Failed to extract {}: {}", archive_path.display(), e))?;

        // Ensure executable permissions are set for binary files on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            // Known binary paths that need execute permission
            let binary_paths = [
                dest_dir.join("caddy"),
                dest_dir.join("php-fpm"),
                dest_dir.join("php-cgi"),
                dest_dir.join("buildroot/bin/php-fpm"),
                dest_dir.join("buildroot/bin/php"),
                dest_dir.join("mysql/bin/mysqld"),
                dest_dir.join("bin/mysqld"),
            ];

            for path in &binary_paths {
                if path.exists() {
                    if let Ok(metadata) = fs::metadata(path) {
                        let mut perms = metadata.permissions();
                        let mode = perms.mode();
                        // Add execute permission if not already set
                        if mode & 0o111 == 0 {
                            perms.set_mode(mode | 0o755);
                            let _ = fs::set_permissions(path, perms);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Extract a tar.xz archive
    fn extract_tar_xz(&self, archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
        use xz2::read::XzDecoder;

        let file =
            File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
        let decoder = XzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        // Unpack the archive, ignoring the first directory level if needed
        archive
            .unpack(dest_dir)
            .map_err(|e| format!("Failed to extract {}: {}", archive_path.display(), e))?;

        // Ensure executable permissions are set for binary files on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            // Known binary paths that need execute permission
            let binary_paths = [
                dest_dir.join("caddy"),
                dest_dir.join("php-fpm"),
                dest_dir.join("php-cgi"),
                dest_dir.join("buildroot/bin/php-fpm"),
                dest_dir.join("buildroot/bin/php"),
                dest_dir.join("mysql/bin/mysqld"),
                dest_dir.join("bin/mysqld"),
            ];

            for path in &binary_paths {
                if path.exists() {
                    if let Ok(metadata) = fs::metadata(path) {
                        let mut perms = metadata.permissions();
                        let mode = perms.mode();
                        // Set execute bits if not already set
                        if mode & 0o111 == 0 {
                            perms.set_mode(mode | 0o755);
                            let _ = fs::set_permissions(path, perms);
                        }
                    }
                }
            }

            // Recursively set executable permission for all files in bin directories
            if let Ok(entries) = fs::read_dir(dest_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                            if dir_name == "bin" || dir_name == "sbin" {
                                if let Ok(bin_entries) = fs::read_dir(&path) {
                                    for bin_entry in bin_entries.flatten() {
                                        let bin_path = bin_entry.path();
                                        if bin_path.is_file() {
                                            if let Ok(metadata) = fs::metadata(&bin_path) {
                                                let mut perms = metadata.permissions();
                                                perms.set_mode(0o755);
                                                let _ = fs::set_permissions(&bin_path, perms);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Download and install all runtime binaries
    pub async fn download_all(
        &self,
        progress_cb: ProgressCallback,
    ) -> Result<Vec<PathBuf>, String> {
        let components = [
            BinaryComponent::Caddy,
            BinaryComponent::Php,
            BinaryComponent::MySQL,
            BinaryComponent::PhpMyAdmin,
        ];
        let total = components.len() as u8;

        // Create temp directory for downloads
        let temp_dir = std::env::temp_dir().join("campp-download");
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp directory: {}", e))?;

        let mut downloaded_files = Vec::new();

        for (i, component) in components.iter().enumerate() {
            let current = (i + 1) as u8;

            // Download
            let downloaded_path = self
                .download_component(*component, &temp_dir, &progress_cb, current, total)
                .await?;

            // Verify checksum (TODO: add expected checksums)
            // For now, skip checksum verification since we'd need to pre-calculate them
            // In production, download from a trusted source with known checksums
            progress_cb(DownloadProgress {
                step: DownloadStep::Extracting,
                percent: 0,
                current_component: component.name().to_string(),
                component_display: component.display_name(),
                version: self.get_component_version(&component),
                total_components: total,
                downloaded_bytes: 0,
                total_bytes: 0,
            });

            let runtime_dir = self.get_runtime_dir()?;
            fs::create_dir_all(&runtime_dir)
                .map_err(|e| format!("Failed to create runtime directory: {}", e))?;

            self.install_downloaded_component(*component, &downloaded_path, &runtime_dir)?;

            downloaded_files.push(downloaded_path);
        }

        // Create all application directories (config, logs, mysql/data, projects)
        if let Ok(app_paths) = get_app_data_paths() {
            if let Err(e) = app_paths.ensure_directories() {
                eprintln!("Warning: Failed to create app directories: {}", e);
            }
        }

        progress_cb(DownloadProgress {
            step: DownloadStep::Complete,
            percent: 100,
            current_component: "All".to_string(),
            component_display: "All Components".to_string(),
            version: String::new(),
            total_components: total,
            downloaded_bytes: 0,
            total_bytes: 0,
        });

        // Keep temp files for user to access if needed
        // Uncomment to cleanup: let _ = fs::remove_dir_all(temp_dir);

        Ok(downloaded_files)
    }

    /// Download and install runtime binaries with option to skip existing components
    pub async fn download_all_with_skip(
        &self,
        progress_cb: ProgressCallback,
        skip_list: &[&str], // Component names to skip (e.g., ["php", "mysql"])
    ) -> Result<Vec<PathBuf>, String> {
        let components = [
            BinaryComponent::Caddy,
            BinaryComponent::Php,
            BinaryComponent::MySQL,
            BinaryComponent::PhpMyAdmin,
        ];
        let total = components.len() as u8;

        // Create temp directory for downloads
        let temp_dir = std::env::temp_dir().join("campp-download");
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp directory: {}", e))?;

        let mut downloaded_files = Vec::new();

        for (i, component) in components.iter().enumerate() {
            let component_name = component.binary_name();

            // Skip if component is in skip list. PHP is version-aware: only skip it
            // when the requested active PHP version is already present.
            if skip_list.contains(&component_name)
                && (*component != BinaryComponent::Php
                    || self.is_selected_php_installed(&self.get_runtime_dir()?))
            {
                eprintln!("Skipping {} (already installed)", component.name());
                continue;
            }

            let current = (i + 1) as u8;

            // Download
            let downloaded_path = self
                .download_component(*component, &temp_dir, &progress_cb, current, total)
                .await?;

            // Verify checksum (TODO: add expected checksums)
            // For now, skip checksum verification since we'd need to pre-calculate them
            // In production, download from a trusted source with known checksums
            progress_cb(DownloadProgress {
                step: DownloadStep::Extracting,
                percent: 0,
                current_component: component.name().to_string(),
                component_display: component.display_name(),
                version: self.get_component_version(&component),
                total_components: total,
                downloaded_bytes: 0,
                total_bytes: 0,
            });

            let runtime_dir = self.get_runtime_dir()?;
            fs::create_dir_all(&runtime_dir)
                .map_err(|e| format!("Failed to create runtime directory: {}", e))?;

            self.install_downloaded_component(*component, &downloaded_path, &runtime_dir)?;

            downloaded_files.push(downloaded_path);
        }

        // Create all application directories (config, logs, mysql/data, projects)
        if let Ok(app_paths) = get_app_data_paths() {
            if let Err(e) = app_paths.ensure_directories() {
                eprintln!("Warning: Failed to create app directories: {}", e);
            }
        }

        progress_cb(DownloadProgress {
            step: DownloadStep::Complete,
            percent: 100,
            current_component: "All".to_string(),
            component_display: "All Components".to_string(),
            version: String::new(),
            total_components: total,
            downloaded_bytes: 0,
            total_bytes: 0,
        });

        // Keep temp files for user to access if needed
        // Uncomment to cleanup: let _ = fs::remove_dir_all(temp_dir);

        Ok(downloaded_files)
    }

    /// Get the runtime directory
    pub fn get_runtime_dir(&self) -> Result<PathBuf, String> {
        Ok(get_app_data_paths()?.runtime_dir)
    }

    fn selected_php_id(&self) -> String {
        self.package_selection
            .as_ref()
            .map(|selection| selection.php.clone())
            .unwrap_or_else(|| crate::config::AppSettings::load().package_selection.php)
    }

    fn install_dir_for_component(&self, component: BinaryComponent, runtime_dir: &Path) -> PathBuf {
        if component == BinaryComponent::Php {
            runtime_dir
                .join("php_versions")
                .join(self.selected_php_id())
        } else {
            runtime_dir.to_path_buf()
        }
    }

    fn is_selected_php_installed(&self, runtime_dir: &Path) -> bool {
        let php_id = self.selected_php_id();
        let selected_version = get_php_package(&php_id).map(|package| package.version);
        let legacy_version = fs::read_to_string(runtime_dir.join("php_installed.txt"))
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find_map(|line| line.strip_prefix("version=").map(str::to_string))
            });

        runtime_dir
            .join("php_versions")
            .join(format!("{}_installed.txt", php_id))
            .exists()
            || selected_version
                .zip(legacy_version)
                .map(|(selected, legacy)| selected == legacy)
                .unwrap_or(false)
    }

    fn install_downloaded_component(
        &self,
        component: BinaryComponent,
        downloaded_path: &Path,
        runtime_dir: &Path,
    ) -> Result<(), String> {
        let install_dir = self.install_dir_for_component(component, runtime_dir);
        fs::create_dir_all(&install_dir)
            .map_err(|e| format!("Failed to create runtime directory: {}", e))?;

        let extension = downloaded_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let is_tar_gz = downloaded_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".tar.gz"))
            .unwrap_or(false);

        let is_tar_xz = downloaded_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".tar.xz"))
            .unwrap_or(false);

        if is_tar_gz || extension == "gz" {
            self.extract_tar_gz(downloaded_path, &install_dir)?;
        } else if is_tar_xz || extension == "xz" {
            self.extract_tar_xz(downloaded_path, &install_dir)?;
        } else if extension == "zip" {
            self.extract_zip(downloaded_path, &install_dir)?;
        } else if component == BinaryComponent::PhpMyAdmin && extension == "php" {
            let adminer_dir = runtime_dir.join("adminer");
            fs::create_dir_all(&adminer_dir)
                .map_err(|e| format!("Failed to create Adminer runtime directory: {}", e))?;
            fs::copy(downloaded_path, adminer_dir.join("index.php"))
                .map_err(|e| format!("Failed to install Adminer: {}", e))?;
        } else if extension.is_empty() {
            let binary_name = downloaded_path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("Invalid binary name")?;

            let dest_path = install_dir.join(binary_name);
            fs::copy(downloaded_path, &dest_path)
                .map_err(|e| format!("Failed to copy binary: {}", e))?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&dest_path)
                    .map_err(|e| format!("Failed to get metadata: {}", e))?
                    .permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&dest_path, perms)
                    .map_err(|e| format!("Failed to set permissions: {}", e))?;
            }
        } else {
            return Err(format!("Unsupported archive format: {}", extension));
        }

        self.write_installed_marker(component, runtime_dir)
    }

    fn write_installed_marker(
        &self,
        component: BinaryComponent,
        runtime_dir: &Path,
    ) -> Result<(), String> {
        let version = self.get_component_version(&component);
        let marker_content = format!(
            "version={}\ninstalled_at={:?}",
            version,
            std::time::SystemTime::now()
        );

        if component == BinaryComponent::Php {
            let php_id = self.selected_php_id();
            let versions_dir = runtime_dir.join("php_versions");
            fs::create_dir_all(&versions_dir)
                .map_err(|e| format!("Failed to create PHP versions directory: {}", e))?;
            fs::write(
                versions_dir.join(format!("{}_installed.txt", php_id)),
                format!("id={}\n{}", php_id, marker_content),
            )
            .map_err(|e| format!("Failed to create PHP version marker file: {}", e))?;
        }

        let marker_file = runtime_dir.join(format!("{}_installed.txt", component.binary_name()));
        fs::write(&marker_file, marker_content)
            .map_err(|e| format!("Failed to create marker file: {}", e))
    }

    /// Check if runtime binaries are already installed
    pub fn is_installed(&self) -> bool {
        let runtime_dir = match self.get_runtime_dir() {
            Ok(dir) => dir,
            Err(_) => return false,
        };

        // Check for marker files created during simulation or actual binaries
        let caddy_marker = runtime_dir.join("caddy_installed.txt");
        let php_marker = runtime_dir.join("php_installed.txt");
        let mysql_marker = runtime_dir.join("mysql_installed.txt");
        let adminer_marker = runtime_dir.join("adminer_installed.txt");
        let legacy_phpmyadmin_marker = runtime_dir.join("phpmyadmin_installed.txt");

        // Also check for actual binaries (for production use)
        let caddy_exe = runtime_dir.join("caddy").join("caddy.exe");
        let php_exe = runtime_dir.join("php").join("php.exe");
        let mysql_exe = runtime_dir.join("mysql").join("bin").join("mysqld.exe");

        caddy_marker.exists()
            || php_marker.exists()
            || mysql_marker.exists()
            || adminer_marker.exists()
            || legacy_phpmyadmin_marker.exists()
            || caddy_exe.exists()
            || php_exe.exists()
            || mysql_exe.exists()
    }

    /// Check which components are already installed with their versions
    pub fn get_installed_components(&self) -> std::collections::HashMap<String, String> {
        let mut installed = std::collections::HashMap::new();
        let runtime_dir = match self.get_runtime_dir() {
            Ok(dir) => dir,
            Err(_) => return installed,
        };

        for component in ["caddy", "php", "mysql", "adminer", "phpmyadmin"] {
            let marker_file = runtime_dir.join(format!("{}_installed.txt", component));
            if let Ok(content) = fs::read_to_string(&marker_file) {
                // Parse version from format: "version=1.2.3\ninstalled_at=..."
                for line in content.lines() {
                    if let Some(version) = line.strip_prefix("version=") {
                        let key = if component == "phpmyadmin" {
                            "adminer"
                        } else {
                            component
                        };
                        installed.insert(key.to_string(), version.to_string());
                        break;
                    }
                }
            }
        }

        // Add Caddy version from default config (not in packages)
        if !installed.contains_key("caddy") {
            installed.insert("caddy".to_string(), "2.11.2".to_string());
        }

        installed
    }
}

impl Default for RuntimeDownloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a file is executable based on its name
#[cfg(unix)]
fn is_executable(name: &str) -> bool {
    name.ends_with("caddy")
        || name.ends_with("php")
        || name.ends_with("php-cgi")
        || name.ends_with("php-fpm")
        || name.ends_with("mysqld")
        || name.ends_with("mysql")
        || name.ends_with("mysqld")
        || name.contains("bin/")
}
