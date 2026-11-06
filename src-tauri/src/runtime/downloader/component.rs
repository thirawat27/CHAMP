//! What the current package selection means for each component.

use super::{get_config, selected_or_first, BinaryComponent, RuntimeDownloader};
use crate::runtime::packages::{
    get_mysql_package, get_php_package, get_phpmyadmin_package, get_postgresql_package, Urls,
    VersionInfo,
};
use crate::runtime::platform::Platform;

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
                BinaryComponent::PostgreSQL => {
                    if let Some(pkg) = get_postgresql_package(&selection.postgresql) {
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

    pub(super) fn component_name(&self, component: BinaryComponent) -> String {
        if component == BinaryComponent::PhpMyAdmin {
            return self
                .package_selection
                .as_ref()
                .and_then(|selection| get_phpmyadmin_package(&selection.phpmyadmin))
                .map(|package| {
                    package
                        .display_name
                        .split_whitespace()
                        .next()
                        .unwrap_or(package.display_name.as_str())
                        .to_string()
                })
                .unwrap_or_else(|| {
                    if crate::runtime::packages::is_adminer_id(&self.selected_database_tool_id()) {
                        "Adminer".to_string()
                    } else {
                        "phpMyAdmin".to_string()
                    }
                });
        }

        component.name().to_string()
    }

    pub(super) fn component_display_name(&self, component: BinaryComponent) -> String {
        if component == BinaryComponent::PhpMyAdmin {
            return self.component_name(component);
        }

        component.display_name()
    }

    /// Get the URL for a binary component from config
    pub(super) fn get_binary_url(&self, component: BinaryComponent) -> String {
        // Use selected packages if available, otherwise fall back to default config
        if let Some(selection) = &self.package_selection {
            match component {
                BinaryComponent::Php => {
                    if let Some(pkg) = get_php_package(&selection.php) {
                        return pkg.url_for(self.platform);
                    }
                }
                BinaryComponent::MySQL => {
                    if let Some(pkg) = get_mysql_package(&selection.mysql) {
                        return pkg.url_for(self.platform);
                    }
                }
                BinaryComponent::PostgreSQL => {
                    if let Some(pkg) = get_postgresql_package(&selection.postgresql) {
                        return pkg.url_for(self.platform);
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
        let binaries = &config.binaries;

        match component {
            BinaryComponent::Caddy => self.selected_version_url(&binaries.caddy.versions),
            BinaryComponent::Php => self.selected_version_url(&binaries.php.versions),
            BinaryComponent::MySQL => self.selected_version_url(&binaries.mysql.versions),
            BinaryComponent::PostgreSQL => self.selected_version_url(&binaries.postgresql.versions),
            BinaryComponent::PhpMyAdmin => {
                let versions = &binaries.phpmyadmin.versions;
                selected_or_first(versions, |v| v.selected)
                    .map(|v| v.url.clone())
                    .unwrap_or_default()
            }
        }
    }

    /// Empty when no versions are configured, which `download_component` reports.
    pub(super) fn selected_version_url(&self, versions: &[VersionInfo]) -> String {
        selected_or_first(versions, |v| v.selected)
            .map(|v| self.url_for_platform(&v.urls))
            .unwrap_or_default()
    }

    /// Map a `Urls` struct to the URL string for the current platform, or an
    /// empty string when the platform has no configured URL. Collapses the
    /// platform-selection match that was repeated across every fallback config
    /// arm of `get_binary_url`. (M-02)
    pub(super) fn url_for_platform(&self, urls: &Urls) -> String {
        match self.platform {
            Platform::WindowsX64 => urls.windows_x64.clone(),
            Platform::WindowsArm64 => urls.windows_arm64.clone(),
            Platform::MacOSX64 => urls.macos_x64.clone(),
            Platform::MacOSArm64 => urls.macos_arm64.clone(),
            Platform::LinuxX64 => urls.linux_x64.clone(),
            Platform::LinuxArm64 => urls.linux_arm64.clone(),
        }
        .unwrap_or_default()
    }

    pub(super) fn selected_php_id(&self) -> String {
        self.package_selection
            .as_ref()
            .map(|selection| selection.php.clone())
            .unwrap_or_else(|| crate::config::AppSettings::load().package_selection.php)
    }

    pub(super) fn selected_database_tool_id(&self) -> String {
        self.package_selection
            .as_ref()
            .map(|selection| selection.phpmyadmin.clone())
            .unwrap_or_else(|| {
                crate::config::AppSettings::load()
                    .package_selection
                    .phpmyadmin
            })
    }

    /// The database engine follows the explicit engine choice, not the manager:
    /// Adminer works with either MySQL or PostgreSQL.
    pub(super) fn selected_database_component(&self) -> BinaryComponent {
        let postgresql = match &self.package_selection {
            Some(selection) => selection.is_postgresql(),
            None => crate::config::AppSettings::load()
                .package_selection
                .is_postgresql(),
        };
        if postgresql {
            BinaryComponent::PostgreSQL
        } else {
            BinaryComponent::MySQL
        }
    }

    pub(super) fn selected_stack_components(&self) -> Vec<BinaryComponent> {
        vec![
            BinaryComponent::Caddy,
            BinaryComponent::Php,
            self.selected_database_component(),
            BinaryComponent::PhpMyAdmin,
        ]
    }

    pub(super) fn marker_name(&self, component: BinaryComponent) -> &'static str {
        if component == BinaryComponent::PhpMyAdmin
            && crate::runtime::packages::is_phpmyadmin_id(&self.selected_database_tool_id())
        {
            "phpmyadmin"
        } else {
            component.binary_name()
        }
    }
}
