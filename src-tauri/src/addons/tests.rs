use super::catalog::{package_for, system_install_hint, ArchiveFormat};
use super::install::{relative_path, swap_in, unpack_into_staging, verify_checksum, MARKER_FILE};
use crate::archive::extract_tar_gz;

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    crate::archive::extract_zip(archive, dest, |_| false)
}
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
            let archive = download.url.ends_with(".zip") || download.url.ends_with(".tar.gz");
            assert_eq!(
                archive,
                download.format != ArchiveFormat::Binary,
                "{}",
                download.url
            );
            assert_eq!(os == "windows", download.binary.ends_with(".exe"));
        }
    }
}

#[test]
fn mailpit_and_meilisearch_download_everywhere_and_redis_only_on_windows() {
    for (os, arch) in PLATFORMS {
        assert!(package_for(AddonKind::Mailpit, os, arch).download.is_some());
        assert!(package_for(AddonKind::Meilisearch, os, arch)
            .download
            .is_some());
        let windows = os == "windows";
        assert_eq!(
            package_for(AddonKind::Redis, os, arch).download.is_some(),
            windows
        );
        if !windows {
            assert!(system_install_hint(AddonKind::Redis, os).is_some());
        }
    }
    // Windows on ARM runs the x64 build through emulation.
    for kind in [AddonKind::Redis, AddonKind::Meilisearch] {
        assert_eq!(
            package_for(kind, "windows", "aarch64"),
            package_for(kind, "windows", "x86_64")
        );
    }
}

#[test]
fn a_bare_executable_download_is_installed_as_the_binary() {
    let dir = tempfile::tempdir().unwrap();
    let download = dir.path().join("meilisearch.part");
    fs::write(&download, b"fake server").unwrap();
    let staging = dir.path().join("staging");

    unpack_into_staging(
        AddonKind::Meilisearch,
        &download,
        ArchiveFormat::Binary,
        &staging,
        "meilisearch.exe",
        "1.56.1",
    )
    .unwrap();

    assert_eq!(
        fs::read(staging.join("meilisearch.exe")).unwrap(),
        b"fake server"
    );
    assert!(staging.join(MARKER_FILE).is_file());
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
        AddonKind::Redis,
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
    // The only non-ignored test that takes an install guard, so it owns them.
    let first = InstallGuard::acquire(AddonKind::Redis).expect("first install");
    assert!(InstallGuard::acquire(AddonKind::Redis).is_none());
    assert!(is_installing(AddonKind::Redis));
    drop(first);
    assert!(!is_installing(AddonKind::Redis));
    assert!(InstallGuard::acquire(AddonKind::Redis).is_some());
}

// --- spec ------------------------------------------------------------------

#[test]
fn every_kind_has_one_spec_with_unique_ports() {
    for kind in ALL_ADDONS {
        let spec = spec::spec(kind);
        assert_eq!(spec.kind, kind);
        assert_eq!(AddonKind::from_key(spec.key), Some(kind));
        assert!(!spec.ports.is_empty());
        if let Some(role) = spec.web_role {
            assert!(spec.ports.iter().any(|port| port.role == role), "{kind:?}");
        }
    }
    let defaults: Vec<u16> = SPECS
        .iter()
        .flat_map(|spec| spec.ports.iter().map(|port| port.default))
        .collect();
    let unique: HashSet<u16> = defaults.iter().copied().collect();
    assert_eq!(unique.len(), defaults.len(), "default ports must not clash");
}

// --- settings --------------------------------------------------------------

#[test]
fn settings_from_before_addons_load_with_defaults() {
    let old = r#"{"web_port":8080,"php_port":9000,"mysql_port":3306}"#;
    let settings: AppSettings = serde_json::from_str(old).unwrap();
    assert_eq!(settings.addons, AddonSettings::default());
    assert_eq!(
        ports_for(AddonKind::Mailpit, &settings.addons),
        vec![1025, 8025]
    );
}

#[test]
fn flat_pre_1_5_settings_migrate_and_save_in_the_new_shape() {
    let old = r#"{"web_port":8080,"php_port":9000,"mysql_port":3306,"addons":{
        "redis_enabled":true,"redis_port":6390,
        "mailpit_enabled":false,"mailpit_smtp_port":1026,"mailpit_web_port":8026,
        "meilisearch_enabled":true}}"#;
    let settings: AppSettings = serde_json::from_str(old).unwrap();
    let addons = &settings.addons;
    assert!(addons.enabled("redis"));
    assert_eq!(ports_for(AddonKind::Redis, addons), vec![6390]);
    assert!(!addons.enabled("mailpit"));
    assert_eq!(ports_for(AddonKind::Mailpit, addons), vec![1026, 8026]);
    assert!(addons.enabled("meilisearch"));
    assert_eq!(ports_for(AddonKind::Meilisearch, addons), vec![7700]);

    let saved = serde_json::to_string(&settings).unwrap();
    assert!(!saved.contains("redis_port"), "{saved}");
    let reloaded: AppSettings = serde_json::from_str(&saved).unwrap();
    assert_eq!(
        reloaded.addons, settings.addons,
        "round trip must keep every port"
    );
}

#[test]
fn the_new_shape_wins_over_stray_flat_keys() {
    let mixed = r#"{"web_port":8080,"php_port":9000,"mysql_port":3306,"addons":{
        "redis_port":6390,"services":{"redis":{"enabled":true,"ports":{"main":6400}}}}}"#;
    let settings: AppSettings = serde_json::from_str(mixed).unwrap();
    assert_eq!(ports_for(AddonKind::Redis, &settings.addons), vec![6400]);
}

#[test]
fn settings_that_still_name_memcached_load() {
    // Written by versions that offered Memcached; the keys are now ignored.
    let old = r#"{"web_port":8080,"php_port":9000,"mysql_port":3306,"addons":{"redis_enabled":true,"memcached_enabled":true,"memcached_port":11211}}"#;
    let settings: AppSettings = serde_json::from_str(old).unwrap();
    assert!(settings.addons.enabled("redis"));
    let saved = serde_json::to_string(&settings).unwrap();
    assert!(!saved.contains("memcached"), "{saved}");
}

#[test]
fn disabled_addons_never_block_saving_other_settings() {
    // The web server moved onto Mailpit's default port, but Mailpit is off.
    let mut settings = AppSettings {
        web_port: 8025,
        ..Default::default()
    };
    assert!(settings.validate().is_ok());

    settings.addons.set_enabled("mailpit", true);
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
    let redis = AddonId::Builtin(AddonKind::Redis);
    let mailpit = AddonId::Builtin(AddonKind::Mailpit);

    let low = AddonChange {
        port: Some(80),
        ..Default::default()
    };
    assert!(apply_change(&mut settings, &redis, &low)
        .unwrap_err()
        .contains("administrator"));

    let web_on_redis = AddonChange {
        web_port: Some(9999),
        ..Default::default()
    };
    assert!(apply_change(&mut settings, &redis, &web_on_redis).is_err());

    let clash = AddonChange {
        enabled: Some(true),
        port: Some(settings.mysql_port.max(1024)),
        ..Default::default()
    };
    // 3306 is above 1024, so this reaches the duplicate check.
    let error = apply_change(&mut settings, &redis, &clash).unwrap_err();
    assert!(
        error.contains("Redis") && error.contains("MySQL"),
        "{error}"
    );
    assert!(
        !settings.addons.enabled("redis"),
        "a refused change saves nothing"
    );

    let smtp_equals_web = AddonChange {
        enabled: Some(true),
        port: Some(8025),
        ..Default::default()
    };
    assert!(apply_change(&mut settings, &mailpit, &smtp_equals_web).is_err());

    let ok = AddonChange {
        enabled: Some(true),
        port: Some(6380),
        ..Default::default()
    };
    apply_change(&mut settings, &redis, &ok).unwrap();
    assert!(settings.addons.enabled("redis"));
    assert_eq!(ports_for(AddonKind::Redis, &settings.addons), vec![6380]);
}

// --- custom add-ons ----------------------------------------------------------

fn custom_addon(command: &Path) -> CustomAddon {
    CustomAddon {
        id: "valkey".to_string(),
        name: "Valkey".to_string(),
        command: command.to_string_lossy().to_string(),
        args: vec![
            "--port".to_string(),
            "{port}".to_string(),
            "--dir".to_string(),
            "{data_dir}".to_string(),
        ],
        port: 6500,
        web: false,
        enabled: true,
    }
}

#[test]
fn custom_addons_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("valkey-server.exe");
    fs::write(&binary, b"MZ").unwrap();
    let good = custom_addon(&binary);
    assert!(custom::validate(&good).is_ok());

    for (bad, why) in [
        (
            CustomAddon {
                id: "Valkey".into(),
                ..good.clone()
            },
            "uppercase id",
        ),
        (
            CustomAddon {
                id: "redis".into(),
                ..good.clone()
            },
            "built-in id",
        ),
        (
            CustomAddon {
                id: "a-".into(),
                ..good.clone()
            },
            "trailing hyphen",
        ),
        (
            CustomAddon {
                name: " ".into(),
                ..good.clone()
            },
            "empty name",
        ),
        (
            CustomAddon {
                port: 80,
                ..good.clone()
            },
            "privileged port",
        ),
        (
            CustomAddon {
                command: "valkey-server".into(),
                ..good.clone()
            },
            "relative command",
        ),
        (
            CustomAddon {
                command: dir.path().join("missing.exe").to_string_lossy().to_string(),
                ..good.clone()
            },
            "missing binary",
        ),
    ] {
        assert!(custom::validate(&bad).is_err(), "{why} must be refused");
    }
}

#[test]
fn custom_addons_join_validation_env_and_args() {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("valkey-server.exe");
    fs::write(&binary, b"MZ").unwrap();
    let mut settings = AppSettings::default();

    let clash = CustomAddon {
        port: settings.mysql_port,
        ..custom_addon(&binary)
    };
    assert!(custom::upsert(&mut settings, clash)
        .unwrap_err()
        .contains("MySQL"));
    assert!(
        settings.addons.custom.is_empty(),
        "a refused add-on is not saved"
    );

    custom::upsert(&mut settings, custom_addon(&binary)).unwrap();
    // Saving the same id again replaces it.
    custom::upsert(
        &mut settings,
        CustomAddon {
            port: 6501,
            ..custom_addon(&binary)
        },
    )
    .unwrap();
    assert_eq!(settings.addons.custom.len(), 1);
    assert_eq!(settings.addons.custom[0].port, 6501);

    let pairs = env_pairs(&settings.addons);
    assert!(pairs.contains(&("CHAMP_VALKEY_PORT".to_string(), "6501".to_string())));
    assert!(enabled_ports(&settings.addons).contains(&("Valkey".to_string(), 6501)));

    let data = dir.path().join("data");
    let args = custom::expand_args(&settings.addons.custom[0], &data);
    assert_eq!(args[1], "6501");
    assert_eq!(args[3], data.as_os_str());
    assert_eq!(custom::env_prefix("my-s3"), "CHAMP_MY_S3");

    let id = AddonId::resolve("valkey", &settings.addons).unwrap();
    assert!(matches!(id, AddonId::Custom(_)));
    assert!(AddonId::resolve("nope", &settings.addons).is_err());
}

// --- PHP environment ----------------------------------------------------------

#[test]
fn env_pairs_cover_only_enabled_addons() {
    let mut settings = AddonSettings::default();
    assert!(env_pairs(&settings).is_empty());

    settings.set_enabled("redis", true);
    settings.set_port("redis", "main", 6390);
    settings.set_enabled("mailpit", true);
    let pairs = env_pairs(&settings);
    let has = |key: &str, value: &str| pairs.contains(&(key.to_string(), value.to_string()));
    assert!(has("CHAMP_REDIS_PORT", "6390"));
    assert!(has("CHAMP_REDIS_URL", "redis://127.0.0.1:6390"));
    assert!(has("CHAMP_MAILPIT_URL", "http://127.0.0.1:8025/"));
    assert!(!pairs.iter().any(|(key, _)| key.contains("MEILISEARCH")));

    settings.set_enabled("meilisearch", true);
    settings.set_port("meilisearch", "main", 7710);
    assert!(env_pairs(&settings).contains(&(
        "CHAMP_MEILISEARCH_HOST".to_string(),
        "http://127.0.0.1:7710".to_string()
    )));
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

/// One test owns Redis's runtime entry; the map is process-wide. The live
/// test also uses Redis but only runs on its own with `--ignored`.
#[test]
fn start_reports_busy_ports_and_spawn_failures_then_stop_clears_them() {
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let busy = listener.local_addr().unwrap().port();
    assert!(!port_is_free(busy));

    let plan = |port: u16| LaunchPlan {
        name: "Redis".to_string(),
        binary: dir.path().join("does-not-exist").join("redis-server.exe"),
        args: Vec::new(),
        data_dir: dir.path().join("data"),
        log_path: dir.path().join("redis.log"),
        ports: vec![port],
        stop: StopMethod::RedisShutdown,
        owned_binary: false,
        env: Vec::new(),
    };

    let error = runtime::start("redis", plan(busy)).unwrap_err();
    assert!(
        error.contains(&format!("port {busy} is already in use")),
        "{error}"
    );
    let (state, stored, _) = runtime::snapshot("redis");
    assert_eq!(state, ServiceState::Error);
    assert_eq!(stored.as_deref(), Some(error.as_str()));

    drop(listener);
    let error = runtime::start("redis", plan(busy)).unwrap_err();
    assert!(error.starts_with("Failed to start Redis"), "{error}");

    // Stop on an add-on that never ran succeeds and clears the error.
    runtime::stop("redis").unwrap();
    let (state, stored, _) = runtime::snapshot("redis");
    assert_eq!(state, ServiceState::Stopped);
    assert!(stored.is_none());

    // A repeated stop is harmless.
    runtime::stop("redis").unwrap();
}

fn launch_strings(kind: AddonKind, ports: &[u16]) -> Vec<String> {
    spec::launch_args(kind, ports, &std::env::temp_dir())
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect()
}

#[test]
fn centrifugo_and_rustfs_listen_only_on_loopback() {
    let config: serde_json::Value = serde_json::from_str(&spec::centrifugo_config(8010)).unwrap();
    assert_eq!(config["http_server"]["address"], "127.0.0.1");
    assert_eq!(config["http_server"]["port"], 8010);
    assert_eq!(config["admin"]["enabled"], true);

    let dir = tempfile::tempdir().unwrap();
    spec::prepare(AddonKind::Centrifugo, &[8010], dir.path()).unwrap();
    assert!(dir.path().join("config.json").is_file());
    let args = launch_strings(AddonKind::Centrifugo, &[8010]);
    assert_eq!(args[0], "-c");

    let args = launch_strings(AddonKind::Rustfs, &[9100, 9101]);
    assert_eq!(args[0], "server");
    assert!(args
        .windows(2)
        .any(|w| w == ["--address", "127.0.0.1:9100"]));
    assert!(args
        .windows(2)
        .any(|w| w == ["--console-address", "127.0.0.1:9101"]));
    assert!(spec::launch_env(AddonKind::Rustfs).contains(&("RUSTFS_CHECK_UPDATE", "false")));

    let mut settings = AddonSettings::default();
    settings.set_enabled("rustfs", true);
    settings.set_enabled("centrifugo", true);
    let pairs = env_pairs(&settings);
    assert!(pairs.contains(&(
        "CHAMP_S3_ENDPOINT".to_string(),
        "http://127.0.0.1:9100".to_string()
    )));
    assert!(pairs.contains(&(
        "CHAMP_CENTRIFUGO_API_URL".to_string(),
        "http://127.0.0.1:8000/api".to_string()
    )));
}

#[test]
fn builtins_listen_only_on_loopback() {
    let args = launch_strings(AddonKind::Mailpit, &[1025, 8025]);
    assert!(args.windows(2).any(|w| w == ["--smtp", "127.0.0.1:1025"]));
    assert!(args.windows(2).any(|w| w == ["--listen", "127.0.0.1:8025"]));
    assert!(args.contains(&"--smtp-auth-accept-any".to_string()));

    let args = launch_strings(AddonKind::Redis, &[7000]);
    assert!(
        args.contains(&"127.0.0.1".to_string()),
        "Redis must bind loopback"
    );
    assert!(args.contains(&"7000".to_string()));

    let args = launch_strings(AddonKind::Meilisearch, &[7000]);
    assert!(args
        .windows(2)
        .any(|w| w == ["--http-addr", "127.0.0.1:7000"]));
    assert!(args.contains(&"--no-analytics".to_string()));
    assert!(args.windows(2).any(|w| w == ["--env", "development"]));
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
#[ignore = "downloads and runs Redis and Mailpit"]
async fn live_addons_start_serve_and_stop() {
    let dir = tempfile::tempdir().unwrap();
    let runtime_dir = dir.path().join("runtime");
    let logs = dir.path().join("logs");
    fs::create_dir_all(&logs).unwrap();

    let plan = |kind: AddonKind, binary: PathBuf, ports: Vec<u16>| {
        let data_dir = dir.path().join("addons").join(kind.key());
        LaunchPlan {
            name: kind.display_name().to_string(),
            binary,
            args: spec::launch_args(kind, &ports, &data_dir),
            data_dir,
            log_path: logs.join(format!("{}.log", kind.key())),
            ports,
            stop: spec::spec(kind).stop,
            owned_binary: true,
            env: Vec::new(),
        }
    };
    let run_blocking =
        |f: Box<dyn FnOnce() -> Result<(), String> + Send>| tokio::task::spawn_blocking(f);

    // Redis: PING, then a write that SHUTDOWN saves to disk.
    if let Some(binary) = install_for_test(AddonKind::Redis, &runtime_dir).await {
        let port = free_port();
        let launch = plan(AddonKind::Redis, binary, vec![port]);
        let data_dir = launch.data_dir.clone();
        run_blocking(Box::new(move || runtime::start("redis", launch)))
            .await
            .unwrap()
            .unwrap_or_else(|e| panic!("Redis start: {e}"));
        assert_eq!(runtime::snapshot("redis").0, ServiceState::Running);
        assert!(talk(port, b"PING\r\n", "PONG").contains("+PONG"));
        assert!(talk(port, b"SET champ ok\r\n", "OK").contains("+OK"));
        run_blocking(Box::new(|| runtime::stop("redis")))
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

    // Mailpit: an SMTP message lands in the inbox API, and PHP's mail()
    // reaches it through the same php.ini lines CHAMP writes.
    let binary = install_for_test(AddonKind::Mailpit, &runtime_dir)
        .await
        .expect("Mailpit is downloadable everywhere");
    let (smtp, web) = (free_port(), free_port());
    let launch = plan(AddonKind::Mailpit, binary.clone(), vec![smtp, web]);
    run_blocking(Box::new(move || runtime::start("mailpit", launch)))
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

    run_blocking(Box::new(|| runtime::stop("mailpit")))
        .await
        .unwrap()
        .unwrap();
    assert!(port_is_free(smtp) && port_is_free(web));
}

/// Downloads the real Meilisearch (about 350 MB), indexes a document, and
/// finds it: `cargo test --lib live_meilisearch -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "downloads and runs Meilisearch"]
async fn live_meilisearch_indexes_and_searches() {
    let dir = tempfile::tempdir().unwrap();
    let runtime_dir = dir.path().join("runtime");
    let binary = install_for_test(AddonKind::Meilisearch, &runtime_dir)
        .await
        .expect("Meilisearch is downloadable everywhere");
    let port = free_port();
    let data_dir = dir.path().join("addons").join("meilisearch");
    let launch = LaunchPlan {
        name: "Meilisearch".to_string(),
        binary,
        args: spec::launch_args(AddonKind::Meilisearch, &[port], &data_dir),
        data_dir,
        log_path: dir.path().join("meilisearch.log"),
        ports: vec![port],
        stop: StopMethod::Terminate,
        owned_binary: true,
        env: Vec::new(),
    };
    tokio::task::spawn_blocking(move || runtime::start("meilisearch", launch))
        .await
        .unwrap()
        .unwrap_or_else(|e| panic!("Meilisearch start: {e}"));

    let base = format!("http://127.0.0.1:{port}");
    let client = reqwest::Client::new();
    client
        .post(format!("{base}/indexes/books/documents"))
        .json(&serde_json::json!([{ "id": 1, "title": "Champions of PHP" }]))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    // Indexing is asynchronous; a typo still finds the document.
    let mut hits = 0;
    for _ in 0..50 {
        let body: serde_json::Value = client
            .get(format!("{base}/indexes/books/search?q=champoins"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        hits = body["hits"].as_array().map_or(0, Vec::len);
        if hits > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    assert_eq!(hits, 1, "the indexed document must be found");
    println!("Meilisearch: indexed and found with a typo");

    tokio::task::spawn_blocking(|| runtime::stop("meilisearch"))
        .await
        .unwrap()
        .unwrap();
    assert!(port_is_free(port));
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
