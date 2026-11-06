//! Where things live inside the CHAMP runtime folder.

use std::path::{Path, PathBuf};

const PHP_VERSIONS_DIR: &str = "php_versions";

/// The marker that records an installed component and its version.
pub fn marker_path(runtime_dir: &Path, component: &str) -> PathBuf {
    runtime_dir.join(format!("{component}_installed.txt"))
}

/// The folder that holds every side-by-side PHP install.
pub fn php_versions_dir(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(PHP_VERSIONS_DIR)
}

pub fn php_version_dir(runtime_dir: &Path, php_id: &str) -> PathBuf {
    php_versions_dir(runtime_dir).join(php_id)
}

/// The marker of one side-by-side PHP install.
pub fn php_version_marker(runtime_dir: &Path, php_id: &str) -> PathBuf {
    marker_path(&php_versions_dir(runtime_dir), php_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_keeps_the_on_disk_names() {
        let runtime = Path::new("runtime");
        assert_eq!(
            marker_path(runtime, "caddy"),
            runtime.join("caddy_installed.txt")
        );
        assert_eq!(
            php_version_dir(runtime, "php-8.5"),
            runtime.join("php_versions").join("php-8.5")
        );
        assert_eq!(
            php_version_marker(runtime, "php-8.5"),
            runtime.join("php_versions").join("php-8.5_installed.txt")
        );
    }
}
