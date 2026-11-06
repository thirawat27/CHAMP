//! Checking downloaded archives before they are used.

use super::{
    BinaryComponent, DownloadFinalizeRequest, DownloadedFile, RuntimeDownloader, MERGE_BUFFER_BYTES,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

impl RuntimeDownloader {
    pub(super) fn finalize_downloaded_file(
        &self,
        request: DownloadFinalizeRequest<'_>,
        downloaded_file: DownloadedFile,
    ) -> Result<PathBuf, String> {
        let partial_path = request.partial_path.to_path_buf();
        let result = self.verify_and_publish_download(request, downloaded_file);
        if result.is_err() {
            let _ = fs::remove_file(partial_path);
        }
        result
    }

    pub(super) fn verify_and_publish_download(
        &self,
        request: DownloadFinalizeRequest<'_>,
        downloaded_file: DownloadedFile,
    ) -> Result<PathBuf, String> {
        check_archive_format(
            request.extension,
            &downloaded_file.magic_bytes,
            downloaded_file.downloaded_bytes,
        )?;

        // Checksum verification is fail-closed (S-01): a component/platform with no
        // configured SHA-256 is rejected rather than silently trusted. The only escape
        // hatch is `allow_unverified_checksums`, which every download command currently
        // sets because most upstream packages publish no checksum to pin.
        match self.get_expected_checksum(&request.component) {
            Some(expected_checksum) => {
                let actual_checksum = hex::encode(downloaded_file.hasher.finalize());

                if actual_checksum.to_lowercase() != expected_checksum.to_lowercase() {
                    return Err(format!(
                        "Checksum verification failed for {}.\nExpected: {}\nActual: {}\n\nThe downloaded file may be corrupted or tampered with.",
                        request.component.name(),
                        expected_checksum,
                        actual_checksum
                    ));
                }
                eprintln!(
                    "Checksum verified for {}: {}",
                    request.component.name(),
                    actual_checksum
                );
            }
            None if self.allow_unverified_checksums => {
                eprintln!(
                    "WARNING: no checksum configured for {} on {} — installing UNVERIFIED.",
                    request.component.name(),
                    self.platform.url_key()
                );
            }
            None => {
                return Err(format!(
                    "No SHA-256 checksum is configured for {} on {}. \
                     Installation was refused because the downloaded binary cannot be verified. \
                     Add the expected checksum to runtime-config.json, or explicitly opt in to \
                     installing unverified binaries.",
                    request.component.name(),
                    self.platform.url_key()
                ));
            }
        }

        fs::rename(request.partial_path, request.file_path)
            .map_err(|e| format!("Failed to finalize downloaded file: {}", e))?;

        request.progress_meta.emit(
            100,
            downloaded_file.downloaded_bytes,
            downloaded_file
                .total_bytes
                .max(downloaded_file.downloaded_bytes),
        );

        Ok(request.file_path.to_path_buf())
    }

    /// Get the expected checksum for a component based on current platform
    pub(super) fn get_expected_checksum(&self, component: &BinaryComponent) -> Option<String> {
        use crate::runtime::packages::get_config;

        let config = get_config()?;

        match component {
            BinaryComponent::Php
            | BinaryComponent::MySQL
            | BinaryComponent::PostgreSQL
            | BinaryComponent::Caddy => {
                let version_info = match component {
                    BinaryComponent::Caddy => config.binaries.caddy.versions.iter(),
                    BinaryComponent::Php => config.binaries.php.versions.iter(),
                    BinaryComponent::MySQL => config.binaries.mysql.versions.iter(),
                    BinaryComponent::PostgreSQL => config.binaries.postgresql.versions.iter(),
                    _ => return None,
                };

                // Determine which version ID to look for based on package selection
                let target_id = if let Some(ref selection) = self.package_selection {
                    match component {
                        BinaryComponent::Php => Some(selection.php.as_str()),
                        BinaryComponent::MySQL => Some(selection.mysql.as_str()),
                        BinaryComponent::PostgreSQL => Some(selection.postgresql.as_str()),
                        _ => None,
                    }
                } else {
                    None
                };

                version_info
                    .into_iter()
                    .find(|version| match target_id {
                        Some(id) => version.id == id,
                        None => version.selected,
                    })
                    .and_then(|version| version.checksums.for_platform(self.platform))
            }
            BinaryComponent::PhpMyAdmin => {
                let selected_id = self
                    .package_selection
                    .as_ref()
                    .map(|selection| selection.phpmyadmin.as_str());
                let version = config
                    .binaries
                    .phpmyadmin
                    .versions
                    .iter()
                    .find(|v| selected_id.map(|id| v.id == id).unwrap_or(v.selected))?;
                version.checksum.clone()
            }
        }
    }
}

pub(super) fn check_archive_format(
    extension: &str,
    magic_bytes: &[u8],
    size: u64,
) -> Result<(), String> {
    if extension != "php" && size < 4 {
        return Err(format!(
            "Downloaded file is too small ({} bytes) to be a valid archive",
            size
        ));
    }
    if extension == "zip" && !magic_bytes.starts_with(&[0x50, 0x4B]) {
        return Err("Expected ZIP file but downloaded file doesn't have ZIP magic bytes. URL may have redirected to HTML page.".to_string());
    }
    if (extension == "gz" || extension == "tar.gz") && !magic_bytes.starts_with(&[0x1f, 0x8b]) {
        return Err(
            "Expected gzip file but downloaded file doesn't have gzip magic bytes.".to_string(),
        );
    }
    Ok(())
}

/// Runs the checks of a fresh download on an archive already on disk.
pub(super) fn cached_archive_is_valid(
    archive: &Path,
    extension: &str,
    expected_checksum: Option<&str>,
) -> bool {
    let Ok(mut file) = File::open(archive) else {
        return false;
    };
    let size = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);

    let mut magic_bytes = [0_u8; 4];
    let read = file.read(&mut magic_bytes).unwrap_or(0);
    if check_archive_format(extension, &magic_bytes[..read], size).is_err() {
        return false;
    }

    let Some(expected_checksum) = expected_checksum else {
        return true;
    };
    let mut hasher = Sha256::new();
    hasher.update(&magic_bytes[..read]);
    let mut buffer = vec![0_u8; MERGE_BUFFER_BYTES];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => hasher.update(&buffer[..read]),
            Err(_) => return false,
        }
    }
    hex::encode(hasher.finalize()).eq_ignore_ascii_case(expected_checksum)
}
