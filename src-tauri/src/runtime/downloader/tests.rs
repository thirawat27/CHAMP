use super::install::*;
use super::verify::*;
use super::*;
use sha2::Digest;
use std::fs::File;
use std::io::Write;

fn write_zip(path: &Path, entries: &[(&str, &str)]) {
    let file = File::create(path).expect("failed to create zip");
    let mut zip = zip::ZipWriter::new(file);
    for (name, content) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .expect("failed to start zip entry");
        zip.write_all(content.as_bytes())
            .expect("failed to write zip entry");
    }
    zip.finish().expect("failed to finish zip");
}

/// The name `detect_php_binary_in_dir` looks for on this platform.
fn php_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "php-cgi.exe"
    } else {
        "php-fpm"
    }
}

fn php_downloader(php_id: &str) -> RuntimeDownloader {
    RuntimeDownloader::with_packages(PackageSelection {
        php: php_id.to_string(),
        ..Default::default()
    })
}

#[test]
fn php_update_swaps_in_the_new_version_and_removes_old_files() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let runtime = temp.path().join("runtime");
    let install_dir = runtime.join("php_versions").join("php-8.5");
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join(php_binary_name()), "old").unwrap();
    fs::write(install_dir.join("removed-in-new-patch.dll"), "old").unwrap();

    let archive = temp.path().join("php.zip");
    write_zip(&archive, &[(php_binary_name(), "new")]);

    php_downloader("php-8.5")
        .install_downloaded_component(BinaryComponent::Php, &archive, &runtime)
        .expect("install failed");

    assert_eq!(
        fs::read_to_string(install_dir.join(php_binary_name())).unwrap(),
        "new"
    );
    assert!(!install_dir.join("removed-in-new-patch.dll").exists());
    assert!(!runtime.join("php_versions").join("php-8.5.new").exists());
    assert!(!runtime.join("php_versions").join("php-8.5.old").exists());
    assert!(runtime
        .join("php_versions")
        .join("php-8.5_installed.txt")
        .exists());
}

#[test]
fn broken_php_archive_keeps_the_working_version() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let runtime = temp.path().join("runtime");
    let install_dir = runtime.join("php_versions").join("php-8.5");
    fs::create_dir_all(&install_dir).unwrap();
    fs::write(install_dir.join(php_binary_name()), "old").unwrap();

    let archive = temp.path().join("php.zip");
    write_zip(&archive, &[("readme.txt", "no php in here")]);

    let result = php_downloader("php-8.5").install_downloaded_component(
        BinaryComponent::Php,
        &archive,
        &runtime,
    );

    assert!(result.is_err());
    assert_eq!(
        fs::read_to_string(install_dir.join(php_binary_name())).unwrap(),
        "old"
    );
    assert!(!runtime.join("php_versions").join("php-8.5.new").exists());
}

#[test]
fn php_install_restores_a_backup_left_by_a_crash() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let runtime = temp.path().join("runtime");
    let backup = runtime.join("php_versions").join("php-8.5.old");
    fs::create_dir_all(&backup).unwrap();
    fs::write(backup.join(php_binary_name()), "old").unwrap();

    let archive = temp.path().join("php.zip");
    write_zip(&archive, &[("readme.txt", "broken")]);

    let result = php_downloader("php-8.5").install_downloaded_component(
        BinaryComponent::Php,
        &archive,
        &runtime,
    );

    assert!(result.is_err());
    let install_dir = runtime.join("php_versions").join("php-8.5");
    assert_eq!(
        fs::read_to_string(install_dir.join(php_binary_name())).unwrap(),
        "old"
    );
}

#[test]
fn php_install_refuses_an_id_that_leaves_the_versions_folder() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let runtime = temp.path().join("runtime");
    let victim = temp.path().join("victim");
    fs::create_dir_all(&victim).unwrap();
    fs::create_dir_all(runtime.join("php_versions")).unwrap();
    let archive = temp.path().join("php.zip");
    write_zip(&archive, &[(php_binary_name(), "x")]);

    let result = php_downloader("../../victim").install_downloaded_component(
        BinaryComponent::Php,
        &archive,
        &runtime,
    );

    assert!(result.unwrap_err().contains("Invalid PHP version id"));
    assert!(victim.exists());
}

#[test]
fn interrupted_component_is_not_reported_as_installed() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let runtime = temp.path();
    fs::create_dir_all(runtime.join("mysql").join("bin")).unwrap();
    fs::write(runtime.join("mysql").join("bin").join("mysqld.exe"), "").unwrap();
    fs::write(runtime.join("mysql_installed.txt"), "version=9").unwrap();
    assert!(is_selected_database_installed(runtime, false));

    fs::write(install_sentinel_path(runtime, "mysql"), "").unwrap();
    assert!(!is_selected_database_installed(runtime, false));

    fs::create_dir_all(runtime.join("adminer")).unwrap();
    fs::write(runtime.join("adminer").join("index.php"), "<?php").unwrap();
    assert!(is_database_tool_installed(runtime, "adminer-5.4"));
    fs::write(install_sentinel_path(runtime, "adminer"), "").unwrap();
    assert!(!is_database_tool_installed(runtime, "adminer-5.4"));
}

#[test]
fn failed_component_extraction_leaves_the_sentinel_and_no_marker() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let runtime = temp.path().join("runtime");
    fs::create_dir_all(&runtime).unwrap();
    let archive = temp.path().join("caddy.zip");
    fs::write(&archive, "this is not a zip file").unwrap();

    let result = RuntimeDownloader::new().install_downloaded_component(
        BinaryComponent::Caddy,
        &archive,
        &runtime,
    );

    assert!(result.is_err());
    assert!(install_was_interrupted(&runtime, "caddy"));
    assert!(!runtime.join("caddy_installed.txt").exists());
}

#[test]
fn successful_component_install_clears_the_sentinel() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let runtime = temp.path().join("runtime");
    fs::create_dir_all(&runtime).unwrap();
    let archive = temp.path().join("caddy.zip");
    write_zip(&archive, &[("caddy.exe", "binary")]);

    RuntimeDownloader::new()
        .install_downloaded_component(BinaryComponent::Caddy, &archive, &runtime)
        .expect("install failed");

    assert!(!install_was_interrupted(&runtime, "caddy"));
    assert!(runtime.join("caddy_installed.txt").exists());
}

#[test]
fn stack_counts_as_installed_only_when_every_component_is_there() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let runtime = temp.path();
    let mut selection = PackageSelection {
        php: "php-8.5".to_string(),
        phpmyadmin: "phpmyadmin-5.2".to_string(),
        database_engine: "mysql".to_string(),
        ..Default::default()
    };

    fs::write(runtime.join("caddy_installed.txt"), "version=2").unwrap();
    assert!(
        !is_stack_installed(runtime, &selection),
        "a failed first run must bring the wizard back"
    );

    fs::create_dir_all(runtime.join("php_versions")).unwrap();
    fs::write(
        runtime.join("php_versions").join("php-8.5_installed.txt"),
        "version=8.5",
    )
    .unwrap();
    fs::write(runtime.join("mysql_installed.txt"), "version=9").unwrap();
    assert!(!is_stack_installed(runtime, &selection));

    fs::create_dir_all(runtime.join("phpmyadmin")).unwrap();
    fs::write(runtime.join("phpmyadmin").join("index.php"), "<?php").unwrap();
    assert!(is_stack_installed(runtime, &selection));

    selection.database_engine = "postgresql".to_string();
    assert!(!is_stack_installed(runtime, &selection));
}

#[test]
fn cached_archive_must_match_its_format_and_checksum() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let archive = temp.path().join("caddy-2.zip");
    let content = b"PK\x03\x04 pretend zip content";
    fs::write(&archive, content).unwrap();
    let checksum = hex::encode(Sha256::digest(content));

    assert!(cached_archive_is_valid(&archive, "zip", None));
    assert!(cached_archive_is_valid(&archive, "zip", Some(&checksum)));
    assert!(cached_archive_is_valid(
        &archive,
        "zip",
        Some(&checksum.to_uppercase())
    ));
    assert!(!cached_archive_is_valid(&archive, "zip", Some("deadbeef")));
    assert!(!cached_archive_is_valid(&archive, "tar.gz", None));

    fs::write(&archive, "<html>error page</html>").unwrap();
    assert!(!cached_archive_is_valid(&archive, "zip", None));
}

#[test]
fn archive_format_check_rejects_short_and_mislabeled_files() {
    assert!(check_archive_format("zip", b"PK\x03\x04", 100).is_ok());
    assert!(check_archive_format("tar.gz", &[0x1f, 0x8b, 0, 0], 100).is_ok());
    assert!(check_archive_format("php", b"<?", 2).is_ok());
    assert!(check_archive_format("zip", b"PK", 2).is_err());
    assert!(check_archive_format("zip", b"<htm", 100).is_err());
    assert!(check_archive_format("gz", b"PK\x03\x04", 100).is_err());
}

#[test]
fn extension_comes_from_the_url_path() {
    let extension = RuntimeDownloader::get_extension_from_url;
    assert_eq!(extension("https://x.test/caddy_2.8_linux.tar.gz"), "tar.gz");
    assert_eq!(extension("https://x.test/php.zip?mirror=1#top"), "zip");
    assert_eq!(extension("https://x.test/adminer.php"), "php");
}

#[test]
fn content_range_total_is_the_part_after_the_slash() {
    let total = RuntimeDownloader::parse_content_range_total;
    assert_eq!(total("bytes 0-0/1234"), Some(1234));
    assert_eq!(total("bytes 0-0/*"), None);
}

#[test]
fn download_percent_is_clamped_and_handles_unknown_totals() {
    let percent = RuntimeDownloader::download_percent;
    assert_eq!(percent(5, 0), 0);
    assert_eq!(percent(1, 4), 25);
    assert_eq!(percent(10, 4), 100);
}

#[test]
fn marker_version_reads_the_version_line() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let marker = temp.path().join("caddy_installed.txt");
    fs::write(&marker, "installed\nversion=2.8.4\n").unwrap();

    assert_eq!(read_marker_version(&marker).as_deref(), Some("2.8.4"));
    assert_eq!(read_marker_version(&temp.path().join("missing.txt")), None);
}

#[test]
fn tar_gz_extraction_writes_entries_under_the_destination() {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let archive_path = temp.path().join("runtime.tar.gz");
    let encoder = flate2::write::GzEncoder::new(
        File::create(&archive_path).unwrap(),
        flate2::Compression::fast(),
    );
    let mut builder = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    header.set_size(5);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, "bin/tool", "hello".as_bytes())
        .unwrap();
    builder.into_inner().unwrap().finish().unwrap();

    let dest = temp.path().join("out");
    fs::create_dir_all(&dest).unwrap();
    RuntimeDownloader::new()
        .extract_tar_gz(&archive_path, &dest)
        .expect("extraction failed");

    assert_eq!(fs::read_to_string(dest.join("bin/tool")).unwrap(), "hello");
}
