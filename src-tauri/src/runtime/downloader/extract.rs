//! Unpacking archives.

use super::{BinaryComponent, RuntimeDownloader, MERGE_BUFFER_BYTES};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::Path;

impl RuntimeDownloader {
    /// Extract a ZIP archive
    pub(super) fn extract_zip(&self, archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
        let file = BufReader::with_capacity(
            MERGE_BUFFER_BYTES,
            File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?,
        );
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP: {}", e))?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| format!("Failed to get file: {}", e))?;
            let outpath = dest_dir.join(file.enclosed_name().ok_or("Invalid path")?);

            if file.is_dir() {
                fs::create_dir_all(&outpath)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
            } else {
                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create parent directory: {}", e))?;
                }
                let mut outfile = BufWriter::with_capacity(
                    MERGE_BUFFER_BYTES,
                    File::create(&outpath).map_err(|e| format!("Failed to create file: {}", e))?,
                );
                io::copy(&mut file, &mut outfile)
                    .map_err(|e| format!("Failed to write file: {}", e))?;

                // Set executable permission on Unix for binary files
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if is_executable(&outpath.to_string_lossy()) {
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
    pub(super) fn extract_tar_gz(
        &self,
        archive_path: &Path,
        dest_dir: &Path,
    ) -> Result<(), String> {
        use flate2::read::GzDecoder;

        let file = BufReader::with_capacity(
            MERGE_BUFFER_BYTES,
            File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?,
        );
        let decoder = GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        // Unpack the archive, ignoring the first directory level if needed
        archive
            .unpack(dest_dir)
            .map_err(|e| format!("Failed to extract {}: {}", archive_path.display(), e))?;

        // Ensure executable permissions are set for binary files on Unix
        #[cfg(unix)]
        ensure_known_binaries_executable(dest_dir);

        Ok(())
    }

    /// Extract a tar.xz archive
    pub(super) fn extract_tar_xz(
        &self,
        archive_path: &Path,
        dest_dir: &Path,
    ) -> Result<(), String> {
        use xz2::read::XzDecoder;

        let file = BufReader::with_capacity(
            MERGE_BUFFER_BYTES,
            File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?,
        );
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

            ensure_known_binaries_executable(dest_dir);

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
            self.extract_tar_gz(downloaded_path, install_dir)?;
        } else if is_tar_xz || extension == "xz" {
            self.extract_tar_xz(downloaded_path, install_dir)?;
        } else if extension == "zip" {
            self.extract_zip(downloaded_path, install_dir)?;
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

        Ok(())
    }
}

/// The first bytes prove a file is the promised format and not, say, an HTML
/// error page served with a 200.
/// Archives do not always keep execute bits on the binaries CHAMP runs.
#[cfg(unix)]
pub(super) fn ensure_known_binaries_executable(dest_dir: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let binary_paths = [
        dest_dir.join("caddy"),
        dest_dir.join("php-fpm"),
        dest_dir.join("php-cgi"),
        dest_dir.join("buildroot/bin/php-fpm"),
        dest_dir.join("buildroot/bin/php"),
        dest_dir.join("mysql/bin/mysqld"),
        dest_dir.join("bin/mysqld"),
        dest_dir.join("pgsql/bin/postgres"),
        dest_dir.join("postgresql/bin/postgres"),
        dest_dir.join("bin/postgres"),
        dest_dir.join("bin/initdb"),
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

/// Check if a file is executable based on its name
#[cfg(unix)]
pub(super) fn is_executable(name: &str) -> bool {
    name.ends_with("caddy")
        || name.ends_with("php")
        || name.ends_with("php-cgi")
        || name.ends_with("php-fpm")
        || name.ends_with("mysqld")
        || name.ends_with("mysql")
        || name.ends_with("mysqld")
        || name.ends_with("postgres")
        || name.ends_with("initdb")
        || name.ends_with("pg_ctl")
        || name.ends_with("psql")
        || name.contains("bin/")
}
