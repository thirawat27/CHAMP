//! Unpacking a downloaded runtime into its install folder.

use super::{BinaryComponent, RuntimeDownloader};
use crate::archive;
use std::fs;
use std::path::Path;

/// File names of the binaries CHAMP runs from a runtime archive.
const RUNTIME_BINARY_NAMES: &[&str] = &[
    "caddy", "php", "php-cgi", "php-fpm", "mysqld", "mysql", "postgres", "initdb", "pg_ctl", "psql",
];

/// Where those binaries sit in the tarballs CHAMP installs.
#[cfg(unix)]
const KNOWN_BINARY_PATHS: &[&str] = &[
    "caddy",
    "php-fpm",
    "php-cgi",
    "buildroot/bin/php-fpm",
    "buildroot/bin/php",
    "mysql/bin/mysqld",
    "bin/mysqld",
    "pgsql/bin/postgres",
    "postgresql/bin/postgres",
    "bin/postgres",
    "bin/initdb",
];

impl RuntimeDownloader {
    /// Unpack a downloaded runtime into `install_dir`.
    pub(super) fn extract_component(
        &self,
        component: BinaryComponent,
        downloaded_path: &Path,
        install_dir: &Path,
        runtime_dir: &Path,
    ) -> Result<(), String> {
        fs::create_dir_all(install_dir)
            .map_err(|e| format!("Failed to create runtime directory: {}", e))?;

        let file_name = downloaded_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let extension = downloaded_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if file_name.ends_with(".tar.gz") || extension == "gz" {
            archive::extract_tar_gz(downloaded_path, install_dir)?;
            mark_runtime_binaries_executable(install_dir);
        } else if file_name.ends_with(".tar.xz") || extension == "xz" {
            archive::extract_tar_xz(downloaded_path, install_dir)?;
            mark_runtime_binaries_executable(install_dir);
        } else if extension == "zip" {
            archive::extract_zip(downloaded_path, install_dir, is_runtime_binary)?;
            if component == BinaryComponent::PhpMyAdmin {
                self.normalize_phpmyadmin_install(runtime_dir)?;
            }
        } else if component == BinaryComponent::PhpMyAdmin && extension == "php" {
            let adminer_dir = runtime_dir.join("adminer");
            fs::create_dir_all(&adminer_dir)
                .map_err(|e| format!("Failed to create Adminer runtime directory: {}", e))?;
            fs::copy(downloaded_path, adminer_dir.join("index.php"))
                .map_err(|e| format!("Failed to install Adminer: {}", e))?;
        } else if extension.is_empty() {
            if file_name.is_empty() {
                return Err("Invalid binary name".to_string());
            }
            let dest_path = install_dir.join(file_name);
            fs::copy(downloaded_path, &dest_path)
                .map_err(|e| format!("Failed to copy binary: {}", e))?;
            archive::make_executable(&dest_path)?;
        } else {
            return Err(format!("Unsupported archive format: {}", extension));
        }

        Ok(())
    }
}

/// True for a ZIP entry (its path inside the archive) that CHAMP may run: a
/// known runtime binary by its exact file name, or anything in a `bin`/`sbin`
/// folder. Matching whole names keeps `index.php` and friends non-executable.
pub(super) fn is_runtime_binary(relative: &Path) -> bool {
    let named = relative
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| RUNTIME_BINARY_NAMES.contains(&name));
    let in_bin_dir = relative.parent().is_some_and(|dir| {
        dir.components()
            .any(|part| matches!(part.as_os_str().to_str(), Some("bin" | "sbin")))
    });
    named || in_bin_dir
}

/// Tarballs keep their modes, but not every build ships its binaries with
/// execute bits: mark the known ones and everything in a top-level `bin` or
/// `sbin` folder. Best effort, like the start that would report a failure.
fn mark_runtime_binaries_executable(dest_dir: &Path) {
    #[cfg(unix)]
    {
        for path in KNOWN_BINARY_PATHS.iter().map(|path| dest_dir.join(path)) {
            if path.is_file() {
                let _ = archive::make_executable(&path);
            }
        }
        for dir in ["bin", "sbin"] {
            let Ok(entries) = fs::read_dir(dest_dir.join(dir)) else {
                continue;
            };
            for path in entries.flatten().map(|entry| entry.path()) {
                if path.is_file() {
                    let _ = archive::make_executable(&path);
                }
            }
        }
    }
    #[cfg(not(unix))]
    let _ = dest_dir;
}
