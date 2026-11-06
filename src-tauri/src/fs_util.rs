//! Crash-safe file writes shared by settings, catalog, and secret storage.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

const RENAME_ATTEMPTS: usize = 5;
const RENAME_RETRY_DELAY: Duration = Duration::from_millis(50);

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A unique sibling of `path`, so the final rename stays on one filesystem.
fn temp_sibling(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "champ".to_string());
    let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!("{name}.{}.{unique}.tmp", std::process::id()))
}

#[cfg(unix)]
fn create_temp(path: &Path, private: bool) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    let mode = if private { 0o600 } else { 0o644 };
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
}

#[cfg(not(unix))]
fn create_temp(path: &Path, _private: bool) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

/// Antivirus and indexers can briefly hold a just-written file open on Windows.
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    let mut attempt = 1;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(_) if attempt < RENAME_ATTEMPTS => {
                attempt += 1;
                thread::sleep(RENAME_RETRY_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
}

fn write_replacing(path: &Path, contents: &[u8], private: bool) -> io::Result<()> {
    let temp = temp_sibling(path);
    let result = create_temp(&temp, private)
        .and_then(|mut file| {
            file.write_all(contents)?;
            file.sync_all()
        })
        .and_then(|()| rename_with_retry(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Replace `path` so readers and crashes see the old or the new content, never
/// a truncated file.
pub fn atomic_write(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    write_replacing(path, contents.as_ref(), false)
}

/// [`atomic_write`] for secrets: on Unix the file is owner-only from creation,
/// with no window in which the umask could leave it readable by others.
pub fn write_private(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    write_replacing(path, contents.as_ref(), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_content_and_leaves_no_temp_file() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let target = temp.path().join("settings.json");

        atomic_write(&target, "first").unwrap();
        atomic_write(&target, "second").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "second");
        let entries = fs::read_dir(temp.path()).unwrap().count();
        assert_eq!(entries, 1, "temp files must not be left behind");
    }

    #[test]
    fn failed_write_keeps_previous_content_and_cleans_up() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        // Renaming onto a non-empty directory fails on every platform.
        let target = temp.path().join("occupied");
        fs::create_dir_all(target.join("child")).unwrap();

        assert!(atomic_write(&target, "data").is_err());

        assert!(target.join("child").is_dir());
        let entries = fs::read_dir(temp.path()).unwrap().count();
        assert_eq!(entries, 1, "temp file must be removed after a failure");
    }

    #[cfg(unix)]
    #[test]
    fn write_private_creates_owner_only_file() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let target = temp.path().join("token");

        write_private(&target, "secret").unwrap();

        let mode = fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
