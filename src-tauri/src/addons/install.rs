//! Downloading, verifying, and unpacking an add-on, and finding its binary.

use super::catalog::{AddonDownload, ArchiveFormat};
use super::AddonKind;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The largest add-on (Meilisearch, about 350 MB) is well under this; a bigger
/// body is not the release.
const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;
pub(super) const MARKER_FILE: &str = ".champ-addon";

pub(super) fn addons_runtime_dir(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("addons")
}

pub(super) fn install_dir(runtime_dir: &Path, kind: AddonKind) -> PathBuf {
    addons_runtime_dir(runtime_dir).join(kind.key())
}

/// The version CHAMP installed, read from the marker written last by install.
/// A folder without a marker is an interrupted install and does not count.
pub(super) fn installed_version(runtime_dir: &Path, kind: AddonKind) -> Option<String> {
    let marker = install_dir(runtime_dir, kind).join(MARKER_FILE);
    crate::runtime::downloader::read_marker_version(&marker)
}

/// The CHAMP-installed server binary, when the install is complete.
pub(super) fn installed_binary(
    runtime_dir: &Path,
    kind: AddonKind,
    download: &AddonDownload,
) -> Option<PathBuf> {
    installed_version(runtime_dir, kind)?;
    let binary = install_dir(runtime_dir, kind).join(relative_path(download.binary)?);
    binary.is_file().then_some(binary)
}

/// `a/b/c` as a path, refusing anything that could leave the install folder.
pub(super) fn relative_path(value: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for part in value.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']) {
            return None;
        }
        path.push(part);
    }
    Some(path)
}

/// A server binary installed by the system package manager (macOS/Linux).
///
/// Apps started from Finder or a desktop launcher get a minimal `PATH`, so the
/// usual Homebrew and distro locations are checked too.
pub(super) fn system_binary(kind: AddonKind) -> Option<PathBuf> {
    if cfg!(windows) {
        return None;
    }
    let name = match kind {
        AddonKind::Redis => "redis-server",
        AddonKind::Mailpit => "mailpit",
        AddonKind::Meilisearch => "meilisearch",
    };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    for dir in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/home/linuxbrew/.linuxbrew/bin",
        "/usr/bin",
        "/usr/sbin",
    ] {
        dirs.push(PathBuf::from(dir));
    }
    dirs.into_iter()
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// Download, verify, and unpack `download` into the add-on's install folder.
///
/// The archive is checked against the pinned SHA-256 before anything is
/// unpacked, and the new folder replaces the old one only once it is complete.
pub(super) async fn install(
    kind: AddonKind,
    version: &str,
    download: AddonDownload,
    runtime_dir: &Path,
    progress: impl Fn(u8),
) -> Result<PathBuf, String> {
    let root = addons_runtime_dir(runtime_dir);
    fs::create_dir_all(&root).map_err(|e| format!("Failed to create add-on folder: {}", e))?;

    let archive = root.join(format!(".{}-download.part", kind.key()));
    let downloaded = download_verified(kind, &download, &archive, &progress).await;
    if let Err(error) = downloaded {
        let _ = fs::remove_file(&archive);
        return Err(error);
    }

    let staging = root.join(format!(".{}-staging", kind.key()));
    let final_dir = install_dir(runtime_dir, kind);
    let version = version.to_string();
    let binary = download.binary;
    let format = download.format;
    let archive_for_task = archive.clone();
    let staging_for_task = staging.clone();
    let unpacked = tauri::async_runtime::spawn_blocking(move || {
        unpack_into_staging(
            kind,
            &archive_for_task,
            format,
            &staging_for_task,
            binary,
            &version,
        )
        .and_then(|()| swap_in(&staging_for_task, &final_dir))
    })
    .await
    .map_err(|e| format!("{} install task failed: {}", kind.display_name(), e))?;
    let _ = fs::remove_file(&archive);
    if unpacked.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    unpacked?;
    progress(100);

    install_dir(runtime_dir, kind)
        .join(relative_path(binary).ok_or("Invalid add-on binary path")?)
        .canonicalize()
        .map(|path| dunce::simplified(&path).to_path_buf())
        .map_err(|e| {
            format!(
                "{} was installed but cannot be found: {}",
                kind.display_name(),
                e
            )
        })
}

async fn download_verified(
    kind: AddonKind,
    download: &AddonDownload,
    archive: &Path,
    progress: &impl Fn(u8),
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "CHAMP-Addon-Installer/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/thirawat27/CHAMP)"
        ))
        .connect_timeout(Duration::from_secs(20))
        // A stalled transfer must not leave the Install button busy forever.
        .read_timeout(Duration::from_secs(60))
        .https_only(true)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let mut response = client
        .get(download.url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("Failed to download {}: {}", kind.display_name(), e))?;

    let is_html = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/html"));
    if is_html {
        return Err(format!(
            "The {} download returned a web page instead of an archive.",
            kind.display_name()
        ));
    }
    let total = response.content_length().unwrap_or(0);
    if total > MAX_DOWNLOAD_BYTES {
        return Err(format!(
            "The {} download is unexpectedly large ({} bytes).",
            kind.display_name(),
            total
        ));
    }

    let mut file =
        File::create(archive).map_err(|e| format!("Failed to save the download: {}", e))?;
    let mut hasher = Sha256::new();
    let mut received: u64 = 0;
    let mut last_percent = 0_u8;
    progress(0);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Failed to download {}: {}", kind.display_name(), e))?
    {
        received += chunk.len() as u64;
        if received > MAX_DOWNLOAD_BYTES {
            return Err(format!(
                "The {} download is larger than expected; stopped.",
                kind.display_name()
            ));
        }
        hasher.update(&chunk);
        file.write_all(&chunk)
            .map_err(|e| format!("Failed to save the download: {}", e))?;
        // Leave the last few percent for verification and unpacking. An
        // unknown length (0) shows no percentage instead of dividing by zero.
        if let Some(percent) = (received * 95).checked_div(total) {
            let percent = percent.min(95) as u8;
            if percent != last_percent {
                last_percent = percent;
                progress(percent);
            }
        }
    }
    file.flush()
        .map_err(|e| format!("Failed to save the download: {}", e))?;
    drop(file);

    let actual = hex::encode(hasher.finalize());
    verify_checksum(kind, &actual, download.sha256)
}

pub(super) fn verify_checksum(kind: AddonKind, actual: &str, expected: &str) -> Result<(), String> {
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "Checksum verification failed for {}.\nExpected: {}\nActual: {}\n\nThe download may be corrupted or tampered with, so it was not installed.",
            kind.display_name(),
            expected,
            actual
        ))
    }
}

pub(super) fn unpack_into_staging(
    kind: AddonKind,
    archive: &Path,
    format: ArchiveFormat,
    staging: &Path,
    binary: &str,
    version: &str,
) -> Result<(), String> {
    if staging.exists() {
        fs::remove_dir_all(staging).map_err(|e| {
            format!(
                "Failed to clear an old {} install: {}",
                kind.display_name(),
                e
            )
        })?;
    }
    fs::create_dir_all(staging).map_err(|e| format!("Failed to create install folder: {}", e))?;
    let binary_path = staging.join(relative_path(binary).ok_or("Invalid add-on binary path")?);
    match format {
        ArchiveFormat::Zip => extract_zip(archive, staging)?,
        ArchiveFormat::TarGz => extract_tar_gz(archive, staging)?,
        ArchiveFormat::Binary => {
            if let Some(parent) = binary_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create folder: {}", e))?;
            }
            fs::copy(archive, &binary_path)
                .map_err(|e| format!("Failed to install {}: {}", kind.display_name(), e))?;
        }
    }

    if !binary_path.is_file() {
        return Err(format!(
            "{} was not found in the downloaded archive ({}).",
            kind.display_name(),
            binary
        ));
    }
    make_executable(&binary_path)?;

    // Written last: a folder without it is an interrupted install.
    fs::write(
        staging.join(MARKER_FILE),
        format!(
            "version={}\ninstalled_at={:?}\n",
            version,
            SystemTime::now()
        ),
    )
    .map_err(|e| format!("Failed to write the {} marker: {}", kind.display_name(), e))
}

/// Replace `final_dir` with `staging`, keeping the old folder until the new one
/// is in place so a failure never leaves nothing behind.
pub(super) fn swap_in(staging: &Path, final_dir: &Path) -> Result<(), String> {
    let backup = final_dir.with_file_name(format!(
        ".{}-old",
        final_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("addon")
    ));
    if backup.exists() {
        let _ = fs::remove_dir_all(&backup);
    }
    let had_previous = final_dir.exists();
    if had_previous {
        fs::rename(final_dir, &backup).map_err(|e| {
            format!(
                "Failed to replace {} (stop the service and try again): {}",
                final_dir.display(),
                e
            )
        })?;
    }
    if let Err(error) = fs::rename(staging, final_dir) {
        if had_previous {
            let _ = fs::rename(&backup, final_dir);
        }
        return Err(format!(
            "Failed to install {}: {}",
            final_dir.display(),
            error
        ));
    }
    if had_previous {
        let _ = fs::remove_dir_all(&backup);
    }
    Ok(())
}

pub(super) fn extract_zip(archive_path: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| format!("Failed to read ZIP: {}", e))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("Failed to read ZIP entry: {}", e))?;
        // `enclosed_name` rejects absolute paths and `..`, so nothing lands
        // outside the install folder.
        let Some(relative) = entry.enclosed_name() else {
            return Err(format!(
                "Unsafe path in archive: {}",
                entry.name().unwrap_or_default()
            ));
        };
        let out = dest.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| format!("Failed to create folder: {}", e))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create folder: {}", e))?;
        }
        let mut target = File::create(&out).map_err(|e| format!("Failed to create file: {}", e))?;
        io::copy(&mut entry, &mut target).map_err(|e| format!("Failed to unpack file: {}", e))?;
    }
    Ok(())
}

pub(super) fn extract_tar_gz(archive_path: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(BufReader::new(file)));
    // `unpack` refuses entries that would escape `dest`.
    archive
        .unpack(dest)
        .map_err(|e| format!("Failed to unpack archive: {}", e))
}

fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|e| format!("Failed to read permissions: {}", e))?
            .permissions();
        permissions.set_mode(permissions.mode() | 0o755);
        fs::set_permissions(path, permissions)
            .map_err(|e| format!("Failed to mark the binary executable: {}", e))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
