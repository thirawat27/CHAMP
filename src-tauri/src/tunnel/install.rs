//! Finding or downloading the ngrok agent.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

pub(super) const NGROK_COMPONENT: &str = "ngrok";

/// ngrok publishes only a rolling "v3-stable" download channel.
pub(super) const NGROK_VERSION: &str = "v3-stable";

pub(super) const NGROK_DOWNLOAD_BASE: &str = "https://bin.equinox.io/c/bNyj1mQVY4c";

pub(super) async fn ensure_ngrok_installed(runtime_dir: &Path) -> Result<PathBuf, String> {
    if let Some(path) = detect_ngrok(runtime_dir) {
        return Ok(path);
    }

    let install_dir = runtime_dir.join(NGROK_COMPONENT);
    fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Failed to create ngrok directory: {}", e))?;

    let target = install_dir.join(ngrok_executable_name());
    let download = ngrok_download()?;
    let temp = install_dir.join(download.temp_file_name);

    let bytes = reqwest::Client::builder()
        .user_agent("CHAMP HTTPS tunnel installer")
        .connect_timeout(Duration::from_secs(20))
        // A stalled download must not leave the Start button busy forever.
        .read_timeout(Duration::from_secs(60))
        .https_only(true)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?
        .get(download.url.as_str())
        .send()
        .await
        .map_err(|e| format!("Failed to download ngrok: {}", e))?
        .error_for_status()
        .map_err(|e| format!("Failed to download ngrok: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Failed to read ngrok download: {}", e))?;

    // TODO(S-02): the stable channel is unversioned, so no SHA-256 can be pinned.
    // Move to a versioned ngrok artifact with published checksums and verify here.
    eprintln!("WARNING: no checksum available for ngrok {NGROK_VERSION}; installing UNVERIFIED.");

    fs::write(&temp, bytes).map_err(|e| format!("Failed to save ngrok: {}", e))?;
    // Extract beside the target and rename: detection only checks that the file
    // exists, so a partial binary there would never be downloaded again.
    let partial = target.with_file_name(format!("{}.part", ngrok_executable_name()));
    let extracted = match download.archive {
        NgrokArchive::Zip => extract_ngrok_zip(&temp, &partial),
        NgrokArchive::Tgz => extract_ngrok_tgz(&temp, &partial),
    };
    let _ = fs::remove_file(&temp);
    let installed = extracted
        .and_then(|()| make_executable(&partial))
        .and_then(|()| {
            fs::rename(&partial, &target).map_err(|e| format!("Failed to install ngrok: {}", e))
        });
    if installed.is_err() {
        let _ = fs::remove_file(&partial);
    }
    installed?;

    fs::write(
        crate::runtime::layout::marker_path(runtime_dir, "ngrok"),
        format!(
            "version={NGROK_VERSION}\ninstalled_at={:?}\n",
            SystemTime::now()
        ),
    )
    .map_err(|e| format!("Failed to write ngrok marker: {}", e))?;

    Ok(target)
}

pub(super) fn runtime_ngrok_paths(runtime_dir: &Path) -> [PathBuf; 3] {
    let executable = ngrok_executable_name();
    [
        runtime_dir.join(NGROK_COMPONENT).join(executable),
        runtime_dir.join(executable),
        runtime_dir.join("bin").join(executable),
    ]
}

pub(super) fn detect_ngrok(runtime_dir: &Path) -> Option<PathBuf> {
    runtime_ngrok_paths(runtime_dir)
        .into_iter()
        .find(|path| path.exists())
        .or_else(system_ngrok)
}

pub(super) fn system_ngrok() -> Option<PathBuf> {
    let mut command = Command::new("ngrok");
    command
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::process::command::hide_console(&mut command);
    let output = command.output().ok()?;
    output.status.success().then(|| PathBuf::from("ngrok"))
}

pub(super) enum NgrokArchive {
    Zip,
    Tgz,
}

pub(super) struct NgrokDownload {
    url: String,
    temp_file_name: &'static str,
    archive: NgrokArchive,
}

pub(super) fn ngrok_download() -> Result<NgrokDownload, String> {
    let (platform, archive) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => ("windows-amd64", NgrokArchive::Zip),
        ("windows", "aarch64") => ("windows-arm64", NgrokArchive::Zip),
        ("linux", "x86_64") => ("linux-amd64", NgrokArchive::Tgz),
        ("linux", "aarch64") => ("linux-arm64", NgrokArchive::Tgz),
        ("macos", "x86_64") => ("darwin-amd64", NgrokArchive::Zip),
        ("macos", "aarch64") => ("darwin-arm64", NgrokArchive::Zip),
        (os, arch) => {
            return Err(format!(
                "Automatic HTTPS tunnel runtime is not available for {os}/{arch}"
            ))
        }
    };
    let (extension, temp_file_name) = match archive {
        NgrokArchive::Zip => ("zip", "ngrok-download.zip"),
        NgrokArchive::Tgz => ("tgz", "ngrok-download.tgz"),
    };
    Ok(NgrokDownload {
        url: format!("{NGROK_DOWNLOAD_BASE}/ngrok-{NGROK_VERSION}-{platform}.{extension}"),
        temp_file_name,
        archive,
    })
}

pub(super) fn ngrok_executable_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ngrok.exe"
    } else {
        "ngrok"
    }
}

pub(super) fn extract_ngrok_zip(archive_path: &Path, target: &Path) -> Result<(), String> {
    let file =
        File::open(archive_path).map_err(|e| format!("Failed to open ngrok archive: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ngrok archive: {}", e))?;
    let mut entry = archive
        .by_name(ngrok_executable_name())
        .map_err(|_| "ngrok binary was not found in the downloaded archive".to_string())?;
    let mut output =
        File::create(target).map_err(|e| format!("Failed to create ngrok binary: {}", e))?;
    std::io::copy(&mut entry, &mut output)
        .map_err(|e| format!("Failed to extract ngrok: {}", e))?;
    Ok(())
}

pub(super) fn extract_ngrok_tgz(archive_path: &Path, target: &Path) -> Result<(), String> {
    let file =
        File::open(archive_path).map_err(|e| format!("Failed to open ngrok archive: {}", e))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to inspect ngrok archive: {}", e))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read ngrok archive: {}", e))?;
        let path = entry
            .path()
            .map_err(|e| format!("Failed to read ngrok archive path: {}", e))?;
        if path.file_name().and_then(|name| name.to_str()) == Some("ngrok") {
            let mut output = File::create(target)
                .map_err(|e| format!("Failed to create ngrok binary: {}", e))?;
            std::io::copy(&mut entry, &mut output)
                .map_err(|e| format!("Failed to extract ngrok: {}", e))?;
            return Ok(());
        }
    }

    Err("ngrok binary was not found in the downloaded archive".to_string())
}

pub(super) fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|e| format!("Failed to read ngrok permissions: {}", e))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .map_err(|e| format!("Failed to mark ngrok executable: {}", e))?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}
