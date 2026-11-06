use super::catalog::*;
use super::*;

#[test]
fn empty_version_lists_are_not_a_usable_catalog() {
    let mut config = embedded_default_runtime_config();
    assert!(config_has_every_package(&config));

    config.binaries.php.versions.clear();
    assert!(!config_has_every_package(&config));
}

#[test]
fn path_segments_reject_anything_that_could_leave_a_folder() {
    for safe in ["php-8.5", "8.5.12-1", "custom_build"] {
        assert!(is_safe_path_segment(safe), "{safe}");
    }
    for unsafe_value in [
        "", ".", "..", "../..", "a/b", "a\\b", "C:", "php-8.5.", "a b",
    ] {
        assert!(!is_safe_path_segment(unsafe_value), "{unsafe_value}");
    }
}

#[test]
fn selection_with_traversal_in_php_id_is_refused() {
    let mut selection = PackageSelection::default();
    assert!(selection.validate_path_ids().is_ok());

    selection.php = "../../..".to_string();
    assert!(selection.validate_path_ids().is_err());
}

fn pinned(version: VersionInfo) -> VersionInfo {
    VersionInfo {
        checksums: Checksums {
            windows_x64: Some("abc".to_string()),
            ..Checksums::default()
        },
        ..version
    }
}

#[test]
fn refresh_keeps_the_checksum_of_an_unchanged_release() {
    let mut versions = vec![pinned(mysql_version("9.7.4")), mysql_version("8.4.1")];

    let changed = replace_version_list(
        &mut versions,
        vec![mysql_version("9.7.4"), mysql_version("8.4.13")],
    );

    assert!(changed);
    assert_eq!(versions[0].checksums.windows_x64.as_deref(), Some("abc"));
}

#[test]
fn refresh_does_not_reuse_a_checksum_for_a_new_release() {
    let mut versions = vec![pinned(mysql_version("9.7.4"))];

    replace_version_list(&mut versions, vec![mysql_version("9.7.5")]);

    assert_eq!(versions[0].checksums.windows_x64, None);
}

#[test]
fn database_tool_refresh_keeps_the_checksum_of_an_unchanged_release() {
    let mut pinned_tool = phpmyadmin_version("5.2.3");
    pinned_tool.checksum = Some("abc".to_string());
    let mut versions = vec![pinned_tool, phpmyadmin_version("5.1.0")];

    replace_database_tool(&mut versions, "phpmyadmin", phpmyadmin_version("5.2.3"));

    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].checksum.as_deref(), Some("abc"));
}

#[test]
fn mysql_archive_url_maps_downloads_to_archives() {
    assert_eq!(
        mysql_archive_url("https://cdn.mysql.com/Downloads/MySQL-9.7/mysql-9.7.1-winx64.zip")
            .as_deref(),
        Some("https://cdn.mysql.com/archives/mysql-9.7/mysql-9.7.1-winx64.zip")
    );
    assert_eq!(
        mysql_archive_url("https://cdn.mysql.com/archives/mysql-9.7/mysql-9.7.1-winx64.zip"),
        None
    );
}

#[test]
fn version_after_marker_accepts_plausible() {
    assert_eq!(
        version_after_marker("downloads of PHP 8.4.3 now", "downloads of PHP "),
        Some("8.4.3".to_string())
    );
}

#[test]
fn version_after_marker_rejects_implausible() {
    // A lone "0" after the marker (no dot) must not be accepted.
    assert_eq!(version_after_marker("release 0 build", "release "), None);

    // A bare year "2024" (no dot) must not be accepted.
    assert_eq!(
        version_after_marker("copyright 2024 vendor", "copyright "),
        None
    );

    // Marker present but followed by non-version garbage.
    assert_eq!(
        version_after_marker("version: none here", "version: "),
        None
    );
}

#[test]
fn adminer_version_ignores_digits_in_page_css() {
    let page = "<style>body { font: 13px/1.25 Verdana; }</style>\
                    <a href=\"../#download\" target=\"_blank\">6.2.0</a>";
    assert_eq!(
        version_after_marker(page, ADMINER_VERSION_MARKER),
        Some("6.2.0".to_string())
    );
    // Page wording changed: no marker means no version, never a CSS number.
    assert_eq!(
        version_after_marker("<style>font: 13px/1.25</style>", ADMINER_VERSION_MARKER),
        None
    );
}

#[test]
fn phpmyadmin_version_ignores_svg_numbers() {
    let page = "<title>Downloads · phpMyAdmin</title><path d=\"M11 6.5a.5.5\"/>\
                    <a href=\"https://files.phpmyadmin.net/phpMyAdmin/5.2.3/phpMyAdmin-5.2.3.zip\">";
    assert_eq!(
        version_after_marker(page, PHPMYADMIN_VERSION_MARKER),
        Some("5.2.3".to_string())
    );
}

#[test]
fn replace_database_tool_keeps_a_single_current_release() {
    let mut versions = vec![
        phpmyadmin_version("5.2.3"),
        adminer_version("5.4.2"),
        adminer_version("1.25"),
    ];

    assert!(replace_database_tool(
        &mut versions,
        "adminer",
        adminer_version("6.2.0")
    ));
    let adminers: Vec<_> = versions
        .iter()
        .filter(|v| v.id.starts_with("adminer"))
        .collect();
    assert_eq!(adminers.len(), 1);
    assert_eq!(adminers[0].version, "6.2.0");
    assert!(versions.iter().any(|v| v.id == "phpmyadmin-5.2"));

    // Same release again is a no-op.
    assert!(!replace_database_tool(
        &mut versions,
        "adminer",
        adminer_version("6.2.0")
    ));
}

#[test]
fn engine_and_manager_are_reconciled() {
    let legacy = |tool: &str| PackageSelection {
        php: "php-8.5".into(),
        mysql: "mysql-9.7".into(),
        postgresql: "postgresql-18".into(),
        phpmyadmin: tool.into(),
        database_engine: String::new(),
    };
    // Old settings: Adminer meant PostgreSQL, phpMyAdmin meant MySQL.
    assert!(legacy("adminer-6.2").normalized().is_postgresql());
    assert!(!legacy("phpmyadmin-5.2").normalized().is_postgresql());

    // Adminer also works with MySQL when MySQL is chosen explicitly.
    let mut mysql_adminer = legacy("adminer-6.2");
    mysql_adminer.database_engine = "mysql".into();
    assert!(!mysql_adminer.normalized().is_postgresql());

    // phpMyAdmin cannot serve PostgreSQL, so the manager moves to Adminer.
    let mut pg_phpmyadmin = legacy("phpmyadmin-5.2");
    pg_phpmyadmin.database_engine = "postgresql".into();
    assert!(pg_phpmyadmin.normalized().tool_is_adminer());
}

#[test]
fn stale_database_tool_id_resolves_to_current_release() {
    let tool = |id: &str| PhpMyAdminPackage {
        id: id.to_string(),
        version: String::new(),
        display_name: String::new(),
        url: String::new(),
        eol: false,
        lts: false,
        recommended: false,
    };
    let catalog = [tool("phpmyadmin-5.2"), tool("adminer-6.2")];

    assert_eq!(resolve_tool_id_in("adminer-5.4", &catalog), "adminer-6.2");
    assert_eq!(resolve_tool_id_in("adminer-6.2", &catalog), "adminer-6.2");
    assert_eq!(
        resolve_tool_id_in("phpmyadmin-4.9", &catalog),
        "phpmyadmin-5.2"
    );
    // No catalog match: the id is left alone.
    assert_eq!(resolve_tool_id_in("adminer-5.4", &[]), "adminer-5.4");
}

#[test]
fn is_plausible_version_basic_cases() {
    // Valid versions.
    assert!(is_plausible_version("8.4.3"));
    assert!(is_plausible_version("17.10"));
    assert!(is_plausible_version("1.0"));

    // No dot.
    assert!(!is_plausible_version("2024"));
    assert!(!is_plausible_version("8"));

    // Zero / out-of-range major.
    assert!(!is_plausible_version("0.1"));
    assert!(!is_plausible_version("100.1"));

    // Empty segment.
    assert!(!is_plausible_version("8..3"));
    assert!(!is_plausible_version("8."));

    // Too long.
    assert!(!is_plausible_version("1.2345678901234"));

    // Non-digit segment.
    assert!(!is_plausible_version("8.4a"));
}
