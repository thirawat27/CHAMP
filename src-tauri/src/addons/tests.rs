use super::catalog::{package_for, system_install_hint, ArchiveFormat};
use super::install::{
    extract_tar_gz, extract_zip, relative_path, swap_in, unpack_into_staging, verify_checksum,
    MARKER_FILE,
};
use super::runtime::{self, port_is_free, LaunchPlan};
use super::*;
use crate::config::AppSettings;
use std::fs::{self, File};
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;

const PLATFORMS: [(&str, &str); 6] = [
    ("windows", "x86_64"),
    ("windows", "aarch64"),
    ("linux", "x86_64"),
    ("linux", "aarch64"),
    ("macos", "x86_64"),
    ("macos", "aarch64"),
];

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    for (name, body) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(body).unwrap();
    }
    zip.finish().unwrap();
}

// --- catalog ---------------------------------------------------------------

#[test]
fn every_download_is_pinned_https_with_a_sha256() {
    for kind in ALL_ADDONS {
        for (os, arch) in PLATFORMS {
            let package = package_for(kind, os, arch);
            assert!(!package.version.is_empty());
            let Some(download) = package.download else {
                continue;
            };
            assert!(
                download.url.starts_with("https://github.com/"),
                "{kind:?} {os}/{arch}: {}",
                download.url
            );
            assert!(
                download.url.contains(package.version),
                "{kind:?} URL must match its pinned version"
            );
            assert_eq!(download.sha256.len(), 64, "{kind:?} {os}/{arch}");
            assert!(download.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(relative_path(download.binary).is_some());
            let zip = download.url.ends_with(".zip");
            assert_eq!(
                zip,
                download.format == ArchiveFormat::Zip,
                "{}",
                download.url
            );
            assert_eq!(os == "windows", download.binary.ends_with(".exe"));
        }
    }
}

#[test]
fn mailpit_downloads_everywhere_and_redis_memcached_only_on_windows() {
    for (os, arch) in PLATFORMS {
        assert!(package_for(AddonKind::Mailpit, os, arch).download.is_some());
        let windows = os == "windows";
        assert_eq!(
            package_for(AddonKind::Redis, os, arch).download.is_some(),
            windows
        );
        assert_eq!(
            package_for(AddonKind::Memcached, os, arch)
                .download
                .is_some(),
            windows
        );
        if !windows {
            assert!(system_install_hint(AddonKind::Redis, os).is_some());
            assert!(system_install_hint(AddonKind::Memcached, os).is_some());
        }
    }
    // Windows on ARM runs the x64 build through emulation.
    assert_eq!(
        package_for(AddonKind::Redis, "windows", "aarch64"),
        package_for(AddonKind::Redis, "windows", "x86_64")
    );
}

#[test]
fn unknown_platforms_have_nothing_to_download() {
    for kind in ALL_ADDONS {
        assert!(package_for(kind, "freebsd", "x86_64").download.is_none());
        assert!(package_for(kind, "windows", "x86").download.is_none());
    }
}

#[test]
fn addon_kind_serializes_as_its_key() {
    for kind in ALL_ADDONS {
        assert_eq!(
            serde_json::to_string(&kind).unwrap(),
            format!("\"{}\"", kind.key())
        );
        let back: AddonKind = serde_json::from_str(&format!("\"{}\"", kind.key())).unwrap();
        assert_eq!(back, kind);
    }
    assert!(serde_json::from_str::<AddonKind>("\"mongodb\"").is_err());
}

// --- install ---------------------------------------------------------------

#[test]
fn relative_path_refuses_escapes() {
    assert!(relative_path("a/b.exe").is_some());
    for bad in ["", "../x", "a/../x", "/abs", "a//b", "a\\b", "C:/x", "./x"] {
        assert!(relative_path(bad).is_none(), "{bad:?} must be refused");
    }
}

#[test]
fn checksum_is_case_insensitive_and_mismatch_is_refused() {
    let hash = "AB".repeat(32);
    assert!(verify_checksum(AddonKind::Redis, &hash.to_lowercase(), &hash).is_ok());
    let error = verify_checksum(AddonKind::Redis, &"00".repeat(32), &hash).unwrap_err();
    assert!(error.contains("Checksum verification failed for Redis"));
    assert!(error.contains("not installed"));
}

#[test]
fn unpack_writes_the_marker_last_and_finds_the_binary() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("a.zip");
    write_zip(
        &archive,
        &[("pkg/bin/server.exe", b"MZ"), ("pkg/README", b"hi")],
    );
    let staging = dir.path().join("staging");

    unpack_into_staging(
        AddonKind::Memcached,
        &archive,
        ArchiveFormat::Zip,
        &staging,
        "pkg/bin/server.exe",
        "1.2.3",
    )
    .unwrap();

    assert!(staging.join("pkg/bin/server.exe").is_file());
    let marker = fs::read_to_string(staging.join(MARKER_FILE)).unwrap();
    assert!(marker.starts_with("version=1.2.3\n"));
}

#[test]
fn unpack_fails_when_the_binary_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("a.zip");
    write_zip(&archive, &[("other.exe", b"MZ")]);
    let staging = dir.path().join("staging");

    let error = unpack_into_staging(
        AddonKind::Redis,
        &archive,
        ArchiveFormat::Zip,
        &staging,
        "redis-server.exe",
        "1",
    )
    .unwrap_err();
    assert!(
        error.contains("not found in the downloaded archive"),
        "{error}"
    );
    assert!(!staging.join(MARKER_FILE).exists());
}

#[test]
fn zip_slip_entries_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil.zip");
    write_zip(&archive, &[("../escaped.txt", b"x")]);
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).unwrap();

    assert!(extract_zip(&archive, &dest).is_err());
    assert!(!dir.path().join("escaped.txt").exists());
}

#[test]
fn a_corrupt_archive_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("broken.zip");
    fs::write(&archive, b"<html>not a zip</html>").unwrap();
    assert!(extract_zip(&archive, dir.path()).is_err());

    let tarball = dir.path().join("broken.tar.gz");
    fs::write(&tarball, b"not gzip").unwrap();
    assert!(extract_tar_gz(&tarball, dir.path()).is_err());
}

#[test]
fn tar_gz_archives_unpack() {
    let dir = tempfile::tempdir().unwrap();
    let tarball = dir.path().join("mailpit.tar.gz");
    {
        let encoder =
            flate2::write::GzEncoder::new(File::create(&tarball).unwrap(), Default::default());
        let mut builder = tar::Builder::new(encoder);
        let body = b"#!/bin/sh\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "mailpit", &body[..])
            .unwrap();
        builder.into_inner().unwrap().finish().unwrap();
    }
    let staging = dir.path().join("staging");
    unpack_into_staging(
        AddonKind::Mailpit,
        &tarball,
        ArchiveFormat::TarGz,
        &staging,
        "mailpit",
        "1.34.0",
    )
    .unwrap();
    assert!(staging.join("mailpit").is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(staging.join("mailpit"))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "the binary must be executable");
    }
}

#[test]
fn swap_in_replaces_an_existing_install() {
    let dir = tempfile::tempdir().unwrap();
    let final_dir = dir.path().join("redis");
    let staging = dir.path().join(".redis-staging");
    fs::create_dir_all(&final_dir).unwrap();
    fs::write(final_dir.join("old.txt"), "old").unwrap();
    fs::create_dir_all(&staging).unwrap();
    fs::write(staging.join("new.txt"), "new").unwrap();

    swap_in(&staging, &final_dir).unwrap();

    assert!(final_dir.join("new.txt").exists());
    assert!(!final_dir.join("old.txt").exists());
    assert!(!staging.exists());
    assert!(!dir.path().join(".redis-old").exists());
}

#[test]
fn swap_in_keeps_the_old_install_when_the_new_one_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let final_dir = dir.path().join("redis");
    fs::create_dir_all(&final_dir).unwrap();
    fs::write(final_dir.join("old.txt"), "old").unwrap();

    assert!(swap_in(&dir.path().join("missing-staging"), &final_dir).is_err());
    assert!(
        final_dir.join("old.txt").exists(),
        "a failed install must not remove the working one"
    );
}

#[test]
fn an_install_folder_without_a_marker_does_not_count() {
    let dir = tempfile::tempdir().unwrap();
    let runtime_dir = dir.path();
    let download = package_for(AddonKind::Mailpit, "windows", "x86_64")
        .download
        .unwrap();
    let addon_dir = install::install_dir(runtime_dir, AddonKind::Mailpit);
    fs::create_dir_all(&addon_dir).unwrap();
    fs::write(addon_dir.join("mailpit.exe"), b"MZ").unwrap();

    assert!(install::installed_binary(runtime_dir, AddonKind::Mailpit, &download).is_none());

    fs::write(addon_dir.join(MARKER_FILE), "version=1.34.0\n").unwrap();
    assert_eq!(
        install::installed_binary(runtime_dir, AddonKind::Mailpit, &download),
        Some(addon_dir.join("mailpit.exe"))
    );
    assert_eq!(
        install::installed_version(runtime_dir, AddonKind::Mailpit).as_deref(),
        Some("1.34.0")
    );
}

#[test]
fn only_one_install_per_addon_at_a_time() {
    let first = InstallGuard::acquire(AddonKind::Memcached).expect("first install");
    assert!(InstallGuard::acquire(AddonKind::Memcached).is_none());
    assert!(is_installing(AddonKind::Memcached));
    drop(first);
    assert!(!is_installing(AddonKind::Memcached));
    assert!(InstallGuard::acquire(AddonKind::Memcached).is_some());
}

// --- settings --------------------------------------------------------------

#[test]
fn settings_from_before_addons_load_with_defaults() {
    let old = r#"{"web_port":8080,"php_port":9000,"mysql_port":3306}"#;
    let settings: AppSettings = serde_json::from_str(old).unwrap();
    assert_eq!(settings.addons, AddonSettings::default());

    let partial =
        r#"{"web_port":8080,"php_port":9000,"mysql_port":3306,"addons":{"redis_enabled":true}}"#;
    let settings: AppSettings = serde_json::from_str(partial).unwrap();
    assert!(settings.addons.redis_enabled);
    assert_eq!(settings.addons.redis_port, 6379);
    assert_eq!(settings.addons.mailpit_web_port, 8025);
}

#[test]
fn disabled_addons_never_block_saving_other_settings() {
    let mut settings = AppSettings::default();
    // The web server moved onto Mailpit's default port, but Mailpit is off.
    settings.web_port = settings.addons.mailpit_web_port;
    assert!(settings.validate().is_ok());

    settings.addons.mailpit_enabled = true;
    let errors = settings.validate().unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.contains("Mailpit web") && e.contains("HTTP/Web")),
        "{errors:?}"
    );
}

#[test]
fn apply_change_validates_ports() {
    let mut settings = AppSettings::default();

    let low = AddonChange {
        port: Some(80),
        ..Default::default()
    };
    assert!(apply_change(&mut settings, AddonKind::Redis, &low)
        .unwrap_err()
        .contains("administrator"));

    let web_on_redis = AddonChange {
        web_port: Some(9999),
        ..Default::default()
    };
    assert!(apply_change(&mut settings, AddonKind::Redis, &web_on_redis).is_err());

    let clash = AddonChange {
        enabled: Some(true),
        port: Some(settings.mysql_port.max(1024)),
        ..Default::default()
    };
    // 3306 is above 1024, so this reaches the duplicate check.
    let error = apply_change(&mut settings, AddonKind::Memcached, &clash).unwrap_err();
    assert!(
        error.contains("Memcached") && error.contains("MySQL"),
        "{error}"
    );
    assert!(
        !settings.addons.memcached_enabled,
        "a refused change saves nothing"
    );

    let smtp_equals_web = AddonChange {
        enabled: Some(true),
        port: Some(settings.addons.mailpit_web_port),
        ..Default::default()
    };
    assert!(apply_change(&mut settings, AddonKind::Mailpit, &smtp_equals_web).is_err());

    let ok = AddonChange {
        enabled: Some(true),
        port: Some(6380),
        ..Default::default()
    };
    apply_change(&mut settings, AddonKind::Redis, &ok).unwrap();
    assert!(settings.addons.redis_enabled);
    assert_eq!(settings.addons.redis_port, 6380);
}

// --- PHP / .env integration -------------------------------------------------

#[test]
fn env_lines_cover_only_enabled_addons() {
    let mut settings = AddonSettings::default();
    assert!(env_pairs(&settings).is_empty());
    assert!(dotenv(&settings).is_empty());

    settings.memcached_enabled = true;
    assert!(dotenv(&settings).contains("CACHE_STORE=memcached"));

    settings.redis_enabled = true;
    settings.redis_port = 6390;
    settings.mailpit_enabled = true;
    let dotenv = dotenv(&settings);
    assert!(dotenv.contains("REDIS_PORT=6390"));
    assert!(dotenv.contains("QUEUE_CONNECTION=redis"));
    assert_eq!(dotenv.matches("CACHE_STORE=").count(), 1, "{dotenv}");
    assert!(dotenv.contains("CACHE_STORE=redis"));
    assert!(dotenv.contains("MAIL_PORT=1025"));

    let pairs = env_pairs(&settings);
    assert!(pairs.contains(&("CHAMP_REDIS_PORT", "6390".to_string())));
    assert!(pairs.contains(&("CHAMP_MAILPIT_URL", "http://127.0.0.1:8025/".to_string())));
}

#[test]
fn sendmail_command_is_shell_quoted_and_refuses_quotes() {
    let path =
        Path::new("/Users/me/Library/Application Support/CHAMP/runtime/addons/mailpit/mailpit");
    assert_eq!(
        sendmail_command(path, 1026).unwrap(),
        "'/Users/me/Library/Application Support/CHAMP/runtime/addons/mailpit/mailpit' sendmail -t -i -S 127.0.0.1:1026"
    );
    assert!(sendmail_command(Path::new("/tmp/it's/mailpit"), 1025).is_none());
    assert!(sendmail_command(Path::new("/tmp/a\"b/mailpit"), 1025).is_none());
}

// --- runtime ---------------------------------------------------------------

/// One test owns Memcached's runtime entry; the map is process-wide.
#[test]
fn start_reports_busy_ports_and_spawn_failures_then_stop_clears_them() {
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let busy = listener.local_addr().unwrap().port();
    assert!(!port_is_free(busy));

    let plan = |port: u16| LaunchPlan {
        binary: dir.path().join("does-not-exist").join("memcached.exe"),
        data_dir: dir.path().join("data"),
        log_path: dir.path().join("memcached.log"),
        ports: vec![port],
        owned_binary: false,
    };

    let error = runtime::start(AddonKind::Memcached, plan(busy)).unwrap_err();
    assert!(
        error.contains(&format!("port {busy} is already in use")),
        "{error}"
    );
    let (state, stored, _) = runtime::snapshot(AddonKind::Memcached);
    assert_eq!(state, ServiceState::Error);
    assert_eq!(stored.as_deref(), Some(error.as_str()));

    drop(listener);
    let error = runtime::start(AddonKind::Memcached, plan(busy)).unwrap_err();
    assert!(error.starts_with("Failed to start Memcached"), "{error}");

    // Stop on an add-on that never ran succeeds and clears the error.
    runtime::stop(AddonKind::Memcached).unwrap();
    let (state, stored, _) = runtime::snapshot(AddonKind::Memcached);
    assert_eq!(state, ServiceState::Stopped);
    assert!(stored.is_none());

    // A repeated stop is harmless.
    runtime::stop(AddonKind::Memcached).unwrap();
}

#[test]
fn mailpit_listens_only_on_loopback_with_auth_accepted() {
    let plan = LaunchPlan {
        binary: "mailpit".into(),
        data_dir: std::env::temp_dir(),
        log_path: std::env::temp_dir().join("mailpit.log"),
        ports: vec![1025, 8025],
        owned_binary: true,
    };
    let command = runtime::build_command(AddonKind::Mailpit, &plan);
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect();
    assert!(args.windows(2).any(|w| w == ["--smtp", "127.0.0.1:1025"]));
    assert!(args.windows(2).any(|w| w == ["--listen", "127.0.0.1:8025"]));
    assert!(args.contains(&"--smtp-auth-accept-any".to_string()));

    for kind in [AddonKind::Redis, AddonKind::Memcached] {
        let plan = LaunchPlan {
            ports: vec![7000],
            ..plan_clone(&plan)
        };
        let args: Vec<String> = runtime::build_command(kind, &plan)
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();
        assert!(
            args.contains(&"127.0.0.1".to_string()),
            "{kind:?} must bind loopback"
        );
        assert!(args.contains(&"7000".to_string()));
    }
}

fn plan_clone(plan: &LaunchPlan) -> LaunchPlan {
    LaunchPlan {
        binary: plan.binary.clone(),
        data_dir: plan.data_dir.clone(),
        log_path: plan.log_path.clone(),
        ports: plan.ports.clone(),
        owned_binary: plan.owned_binary,
    }
}

/// Downloads the real archive for this platform: `cargo test -- --ignored`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "downloads Mailpit from GitHub"]
async fn live_mailpit_install_verifies_and_unpacks() {
    let package = super::catalog::current_package(AddonKind::Mailpit);
    let download = package
        .download
        .expect("Mailpit is downloadable everywhere");
    let dir = tempfile::tempdir().unwrap();
    let binary = install::install(
        AddonKind::Mailpit,
        package.version,
        download,
        dir.path(),
        |_| {},
    )
    .await
    .expect("install");
    assert!(binary.is_file());
    assert_eq!(
        install::installed_version(dir.path(), AddonKind::Mailpit).as_deref(),
        Some(package.version)
    );
}

// --- live end-to-end -------------------------------------------------------

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Send `request` and read until `expect` appears (or the server stops talking).
fn talk(port: u16, request: &[u8], expect: &str) -> String {
    use std::io::Read;
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    stream.write_all(request).unwrap();
    let mut reply = String::new();
    let mut buffer = [0_u8; 512];
    while !reply.contains(expect) {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => reply.push_str(&String::from_utf8_lossy(&buffer[..read])),
        }
    }
    reply
}

/// Speak just enough SMTP to deliver one message.
fn send_smtp(port: u16, subject: &str) {
    use std::io::{BufRead, BufReader};
    let stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;
    let mut expect = |code: &str| {
        // Multi-line replies use `250-`; the last line uses `250 `.
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with(code), "expected {code}, got {line:?}");
            if line.as_bytes().get(3) != Some(&b'-') {
                break;
            }
        }
    };
    expect("220");
    for (command, code) in [
        ("EHLO champ.test\r\n", "250"),
        ("MAIL FROM:<app@champ.test>\r\n", "250"),
        ("RCPT TO:<user@example.test>\r\n", "250"),
        ("DATA\r\n", "354"),
    ] {
        writer.write_all(command.as_bytes()).unwrap();
        expect(code);
    }
    writer
        .write_all(format!("Subject: {subject}\r\n\r\nHello from CHAMP.\r\n.\r\n").as_bytes())
        .unwrap();
    expect("250");
    writer.write_all(b"QUIT\r\n").unwrap();
}

async fn mailpit_total(web_port: u16) -> u64 {
    let body: serde_json::Value = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("http://127.0.0.1:{web_port}/api/v1/messages"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    body["total"].as_u64().unwrap()
}

async fn install_for_test(kind: AddonKind, runtime_dir: &Path) -> Option<PathBuf> {
    let package = super::catalog::current_package(kind);
    match package.download {
        Some(download) => Some(
            install::install(kind, package.version, download, runtime_dir, |_| {})
                .await
                .unwrap_or_else(|e| panic!("{kind:?} install failed: {e}")),
        ),
        None => install::system_binary(kind),
    }
}

/// Installs and runs the real servers, like the app does:
/// `cargo test --lib live_addons -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "downloads and runs Redis, Memcached, and Mailpit"]
async fn live_addons_start_serve_and_stop() {
    let dir = tempfile::tempdir().unwrap();
    let runtime_dir = dir.path().join("runtime");
    let logs = dir.path().join("logs");
    fs::create_dir_all(&logs).unwrap();

    let plan = |kind: AddonKind, binary: PathBuf, ports: Vec<u16>| LaunchPlan {
        binary,
        data_dir: dir.path().join("addons").join(kind.key()),
        log_path: logs.join(format!("{}.log", kind.key())),
        ports,
        owned_binary: true,
    };
    let run_blocking =
        |f: Box<dyn FnOnce() -> Result<(), String> + Send>| tokio::task::spawn_blocking(f);

    // Redis: PING, then a write that SHUTDOWN saves to disk.
    if let Some(binary) = install_for_test(AddonKind::Redis, &runtime_dir).await {
        let port = free_port();
        let launch = plan(AddonKind::Redis, binary, vec![port]);
        let data_dir = launch.data_dir.clone();
        run_blocking(Box::new(move || runtime::start(AddonKind::Redis, launch)))
            .await
            .unwrap()
            .unwrap_or_else(|e| panic!("Redis start: {e}"));
        assert_eq!(runtime::snapshot(AddonKind::Redis).0, ServiceState::Running);
        assert!(talk(port, b"PING\r\n", "PONG").contains("+PONG"));
        assert!(talk(port, b"SET champ ok\r\n", "OK").contains("+OK"));
        run_blocking(Box::new(|| runtime::stop(AddonKind::Redis)))
            .await
            .unwrap()
            .unwrap();
        assert!(port_is_free(port), "Redis must release its port");
        assert!(
            data_dir.join("dump.rdb").is_file(),
            "graceful SHUTDOWN must save the data"
        );
        println!("Redis: PING/SET ok, saved on stop");
    } else {
        println!("Redis: skipped (no binary for this system)");
    }

    // Memcached: version and a set/get round trip.
    if let Some(binary) = install_for_test(AddonKind::Memcached, &runtime_dir).await {
        let port = free_port();
        let launch = plan(AddonKind::Memcached, binary, vec![port]);
        run_blocking(Box::new(move || {
            runtime::start(AddonKind::Memcached, launch)
        }))
        .await
        .unwrap()
        .unwrap_or_else(|e| panic!("Memcached start: {e}"));
        assert!(talk(port, b"version\r\n", "\r\n").starts_with("VERSION "));
        assert!(talk(port, b"set champ 0 60 2\r\nok\r\n", "STORED").contains("STORED"));
        assert!(talk(port, b"get champ\r\n", "END").contains("\r\nok\r\n"));
        run_blocking(Box::new(|| runtime::stop(AddonKind::Memcached)))
            .await
            .unwrap()
            .unwrap();
        assert!(port_is_free(port));
        println!("Memcached: version/set/get ok");
    } else {
        println!("Memcached: skipped (no binary for this system)");
    }

    // Mailpit: an SMTP message lands in the inbox API, and PHP's mail()
    // reaches it through the same php.ini lines CHAMP writes.
    let binary = install_for_test(AddonKind::Mailpit, &runtime_dir)
        .await
        .expect("Mailpit is downloadable everywhere");
    let (smtp, web) = (free_port(), free_port());
    let launch = plan(AddonKind::Mailpit, binary.clone(), vec![smtp, web]);
    run_blocking(Box::new(move || runtime::start(AddonKind::Mailpit, launch)))
        .await
        .unwrap()
        .unwrap_or_else(|e| panic!("Mailpit start: {e}"));
    send_smtp(smtp, "CHAMP SMTP test");
    assert_eq!(mailpit_total(web).await, 1);
    println!("Mailpit: SMTP message caught");

    if let Some(php) = installed_php() {
        let ini = dir.path().join("php-mail.ini");
        let route = PhpMailRoute {
            smtp_port: smtp,
            sendmail_path: if cfg!(windows) {
                None
            } else {
                sendmail_command(&binary, smtp)
            },
        };
        fs::write(&ini, format!("[PHP]\n{}", php_mail_ini(&route))).unwrap();
        let output = std::process::Command::new(&php)
            .arg("-n")
            .arg("-c")
            .arg(&ini)
            .arg("-r")
            .arg("exit(mail('user@example.test', 'CHAMP PHP test', 'Hello from mail()') ? 0 : 3);")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "mail() failed: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert_eq!(mailpit_total(web).await, 2);
        println!("Mailpit: PHP mail() caught ({})", php.display());
    } else {
        println!("Mailpit: PHP mail() check skipped (no CHAMP PHP installed)");
    }

    run_blocking(Box::new(|| runtime::stop(AddonKind::Mailpit)))
        .await
        .unwrap()
        .unwrap();
    assert!(port_is_free(smtp) && port_is_free(web));
}

/// The ini lines `php_config::php_mail_lines` writes, for the live test.
fn php_mail_ini(route: &PhpMailRoute) -> String {
    let mut lines = format!(
        "SMTP = 127.0.0.1\nsmtp_port = {}\nsendmail_from = \"champ@localhost\"\n",
        route.smtp_port
    );
    if let Some(sendmail) = &route.sendmail_path {
        lines.push_str(&format!("sendmail_path = \"{sendmail}\"\n"));
    }
    lines
}

fn installed_php() -> Option<PathBuf> {
    let versions = get_app_data_paths().ok()?.runtime_dir.join("php_versions");
    let name = if cfg!(windows) { "php.exe" } else { "php" };
    fs::read_dir(versions)
        .ok()?
        .flatten()
        .map(|entry| entry.path().join(name))
        .find(|path| path.is_file())
}
