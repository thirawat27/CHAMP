//! Unpacking downloaded archives (runtimes and add-ons) so that no entry can
//! land outside its destination folder.

use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

const BUFFER_BYTES: usize = 1024 * 1024;

fn open(archive_path: &Path) -> Result<BufReader<File>, String> {
    let file = File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
    Ok(BufReader::with_capacity(BUFFER_BYTES, file))
}

/// Extract a ZIP archive into `dest`.
///
/// `enclosed_name` rejects absolute paths and `..`, so an unsafe entry fails
/// the extraction instead of writing outside `dest`. On Unix a file becomes
/// executable when the archive says so or when `force_executable` (given the
/// entry's path inside the archive) does, since ZIPs made on Windows carry no
/// execute bits.
pub(crate) fn extract_zip(
    archive_path: &Path,
    dest: &Path,
    force_executable: impl Fn(&Path) -> bool,
) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(open(archive_path)?)
        .map_err(|e| format!("Failed to read ZIP: {}", e))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("Failed to read ZIP entry: {}", e))?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(format!(
                "Unsafe path in archive: {}",
                entry.name().unwrap_or_default()
            ));
        };
        let out = dest.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| format!("Failed to create folder: {}", e))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create folder: {}", e))?;
        }
        let file = File::create(&out).map_err(|e| format!("Failed to create file: {}", e))?;
        let mut target = BufWriter::with_capacity(BUFFER_BYTES, file);
        // Flushed explicitly: dropping a BufWriter hides a failed final write
        // (a full disk) and leaves a truncated file behind.
        io::copy(&mut entry, &mut target)
            .and_then(|_| target.flush())
            .map_err(|e| format!("Failed to unpack file: {}", e))?;

        let archived_executable = entry.unix_mode().is_some_and(|mode| mode & 0o111 != 0);
        if archived_executable || force_executable(&relative) {
            make_executable(&out)?;
        }
    }
    Ok(())
}

/// Extract a `.tar.gz` archive into `dest`.
pub(crate) fn extract_tar_gz(archive_path: &Path, dest: &Path) -> Result<(), String> {
    unpack_tar(
        flate2::read::GzDecoder::new(open(archive_path)?),
        archive_path,
        dest,
    )
}

/// Extract a `.tar.xz` archive into `dest`.
pub(crate) fn extract_tar_xz(archive_path: &Path, dest: &Path) -> Result<(), String> {
    unpack_tar(
        xz2::read::XzDecoder::new(open(archive_path)?),
        archive_path,
        dest,
    )
}

fn unpack_tar(reader: impl Read, archive_path: &Path, dest: &Path) -> Result<(), String> {
    // `unpack` refuses entries that would escape `dest` and keeps file modes.
    tar::Archive::new(reader)
        .unpack(dest)
        .map_err(|e| format!("Failed to extract {}: {}", archive_path.display(), e))
}

/// Add execute permission (`a+rx`, owner write) on Unix; nothing elsewhere.
pub(crate) fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|e| format!("Failed to read permissions: {}", e))?
            .permissions();
        permissions.set_mode(permissions.mode() | 0o755);
        fs::set_permissions(path, permissions)
            .map_err(|e| format!("Failed to mark {} executable: {}", path.display(), e))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_zip(path: &Path, entries: &[(&str, &[u8], u32)]) {
        let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, data, mode) in entries {
            let options = zip::write::SimpleFileOptions::default().unix_permissions(*mode);
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn zip_entries_unpack_inside_dest() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("pkg.zip");
        write_zip(&archive, &[("app/index.php", b"<?php", 0o644)]);
        let dest = dir.path().join("dest");

        extract_zip(&archive, &dest, |_| false).unwrap();
        assert_eq!(
            fs::read_to_string(dest.join("app/index.php")).unwrap(),
            "<?php"
        );
    }

    #[cfg(unix)]
    #[test]
    fn zip_execute_bits_come_from_the_archive_or_the_caller() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("pkg.zip");
        write_zip(
            &archive,
            &[
                ("tool", b"#!", 0o755),
                ("forced", b"#!", 0o644),
                ("page.php", b"<?php", 0o644),
            ],
        );
        let dest = dir.path().join("dest");
        extract_zip(&archive, &dest, |path| path == Path::new("forced")).unwrap();

        let executable =
            |name: &str| fs::metadata(dest.join(name)).unwrap().permissions().mode() & 0o111 != 0;
        assert!(executable("tool"));
        assert!(executable("forced"));
        assert!(!executable("page.php"));
    }
}
