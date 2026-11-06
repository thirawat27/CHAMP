//! Refreshing the runtime catalog from upstream release metadata.

use super::{
    embedded_default_runtime_config, get_config, replace_runtime_config,
    runtime_config_to_packages, Checksums, PackagesConfig, RuntimeConfig, Urls, VersionInfo,
    VersionInfoSingleUrl,
};
use serde::Deserialize;
use std::fs;
use std::time::Duration;

/// Older supported releases a catalog refresh keeps offering next to the newest one.
pub(super) const FALLBACK_MYSQL_LTS: &str = "8.4.13";

pub(super) const FALLBACK_POSTGRESQL_PREVIOUS: [&str; 2] = ["17.10", "16.14"];

/// Catalog requests are small; a stalled one must not hang the caller, such as
/// `download_php_version`'s best-effort refresh or the first-run wizard.
pub(super) fn catalog_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("CHAMP runtime catalog refresher")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .https_only(true)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

#[derive(Debug, Deserialize)]
pub(super) struct GitHubLatestRelease {
    tag_name: String,
}

/// Refresh the runtime catalog from upstream release metadata.
///
/// Network failures are non-fatal; the current local catalog is returned so the
/// UI can continue to work offline.
pub async fn refresh_runtime_catalog() -> Result<PackagesConfig, String> {
    let mut config = get_config().unwrap_or_else(embedded_default_runtime_config);
    let client = catalog_http_client()?;
    let mut changed = false;

    if let Some(version) = fetch_github_latest(&client, "caddyserver", "caddy")
        .await
        .map(|tag| tag.strip_prefix('v').map_or(tag.clone(), str::to_string))
    {
        changed |= replace_versions(&mut config.binaries.caddy.versions, caddy_version(&version));
    }

    if let Some(latest) = crate::runtime::php_catalog::fetch_latest_php(&client).await {
        changed |= crate::runtime::php_catalog::merge_php_versions(
            &mut config.binaries.php.versions,
            latest,
        );
    }

    if let Some(version) = fetch_version_from_text(
        &client,
        "https://dev.mysql.com/downloads/mysql/?os=3&platform=",
        "MySQL Community Server ",
    )
    .await
    {
        changed |= replace_version_list(
            &mut config.binaries.mysql.versions,
            vec![
                mysql_version(&version),
                selected_version(mysql_version(FALLBACK_MYSQL_LTS), false),
            ],
        );
    }

    if let Some(version) = fetch_version_from_text(
        &client,
        "https://www.enterprisedb.com/downloads/postgres-postgresql-downloads",
        "PostgreSQL Version",
    )
    .await
    {
        changed |= replace_version_list(
            &mut config.binaries.postgresql.versions,
            vec![
                postgresql_version(&version),
                selected_version(postgresql_version(FALLBACK_POSTGRESQL_PREVIOUS[0]), false),
                selected_version(postgresql_version(FALLBACK_POSTGRESQL_PREVIOUS[1]), false),
            ],
        );
    }

    // Anchor on the download link: the bare product name also matches the page
    // title, and the next number after it was SVG path data ("6.5").
    if let Some(version) = fetch_version_from_text(
        &client,
        "https://www.phpmyadmin.net/downloads/",
        PHPMYADMIN_VERSION_MARKER,
    )
    .await
    {
        changed |= replace_database_tool(
            &mut config.binaries.phpmyadmin.versions,
            "phpmyadmin",
            phpmyadmin_version(&version),
        );
    }

    // The version page is HTML with CSS in front of the number, so the marker
    // must anchor on the download link; an empty marker picked "1.25" out of
    // `font: 13px/1.25` and produced a dead download URL.
    if let Some(version) = fetch_version_from_text(
        &client,
        "https://www.adminer.org/version/?current=0&lang=en",
        ADMINER_VERSION_MARKER,
    )
    .await
    {
        changed |= replace_database_tool(
            &mut config.binaries.phpmyadmin.versions,
            "adminer",
            adminer_version(&version),
        );
    }

    if changed {
        persist_runtime_config_override(&config)?;
        replace_runtime_config(Some(config.clone()));
    }

    Ok(runtime_config_to_packages(&config))
}

/// Refresh only the PHP section of the catalog (three small JSON requests), so
/// installing or updating PHP always uses the newest patch release.
pub async fn refresh_php_catalog() -> Result<(), String> {
    let client = catalog_http_client()?;
    let latest = crate::runtime::php_catalog::fetch_latest_php(&client)
        .await
        .ok_or("Could not reach php.net")?;

    let mut config = get_config().unwrap_or_else(embedded_default_runtime_config);
    if crate::runtime::php_catalog::merge_php_versions(&mut config.binaries.php.versions, latest) {
        persist_runtime_config_override(&config)?;
        replace_runtime_config(Some(config));
    }
    Ok(())
}

pub(super) async fn fetch_github_latest(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
) -> Option<String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<GitHubLatestRelease>()
        .await
        .ok()
        .map(|release| release.tag_name)
}

pub(super) async fn fetch_version_from_text(
    client: &reqwest::Client,
    url: &str,
    marker: &str,
) -> Option<String> {
    let text = client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()?;
    version_after_marker(&text, marker)
}

pub(super) fn version_after_marker(text: &str, marker: &str) -> Option<String> {
    let haystack = if marker.is_empty() {
        text
    } else {
        let index = text.find(marker)? + marker.len();
        &text[index..]
    };

    let marker_found = marker.is_empty() || text.contains(marker);

    for (start, ch) in haystack.char_indices() {
        if !ch.is_ascii_digit() {
            continue;
        }

        let candidate: String = haystack[start..]
            .chars()
            .take_while(|value| value.is_ascii_digit() || *value == '.')
            .collect();
        let candidate = candidate.trim_end_matches('.').to_string();

        // M-10: only accept a candidate that looks like a real version. If the
        // upstream page wording changed and the first digit run after the marker
        // is not a plausible version (e.g. a year "2024" with no dot, or "0"),
        // keep scanning rather than composing a wrong number into download URLs.
        if is_plausible_version(&candidate) {
            return Some(candidate);
        }
    }

    // M-10: a marker was found but nothing after it parsed as a plausible
    // version. Surface this in logs so a broken scrape is visible instead of
    // silently keeping (or worse, picking a wrong) value. Returning None makes
    // the caller keep its existing local catalog entry, the safe outcome.
    if marker_found {
        eprintln!(
            "runtime catalog refresh: marker {:?} found but no plausible version followed; keeping local catalog entry",
            marker
        );
    }

    None
}

/// M-10: reject implausible version strings scraped from upstream HTML.
///
/// A plausible version has at least one `.`, every dot-separated segment is a
/// non-empty run of ASCII digits, the major segment parses to a `u16` in
/// `1..=99` (rejecting "0" and absurd values), and the whole thing is short.
pub(super) fn is_plausible_version(candidate: &str) -> bool {
    if candidate.len() > 14 {
        return false;
    }
    if !candidate.contains('.') {
        return false;
    }

    let mut segments = candidate.split('.');

    let major = match segments.next() {
        Some(segment) => segment,
        None => return false,
    };
    if major.is_empty() || !major.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    match major.parse::<u16>() {
        Ok(value) if (1..=99).contains(&value) => {}
        _ => return false,
    }

    // Remaining segments (there is at least one because of the '.' check) must
    // each be a non-empty run of ASCII digits.
    segments.all(|segment| !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit()))
}

pub(super) fn persist_runtime_config_override(config: &RuntimeConfig) -> Result<(), String> {
    let app_paths = crate::runtime::locator::get_app_data_paths()
        .map_err(|e| format!("Failed to get app data paths: {}", e))?;
    fs::create_dir_all(&app_paths.config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;
    let target = app_paths.config_dir.join("runtime-config.json");
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize runtime config: {}", e))?;
    crate::fs_util::atomic_write(&target, format!("{content}\n"))
        .map_err(|e| format!("Failed to write {}: {}", target.display(), e))
}

pub(super) fn replace_versions(versions: &mut Vec<VersionInfo>, next: VersionInfo) -> bool {
    if versions.len() == 1 && versions[0].id == next.id && versions[0].version == next.version {
        versions[0].selected = true;
        return false;
    }
    let mut next = vec![next];
    keep_known_checksums(versions, &mut next);
    *versions = next;
    true
}

/// Upstream pages give version numbers but no digests, so an entry rebuilt from a
/// scrape has no checksum. For an unchanged release, the one already pinned still
/// applies, and dropping it would silently disable verification.
pub(super) fn keep_known_checksums(current: &[VersionInfo], next: &mut [VersionInfo]) {
    for entry in next {
        if let Some(known) = current
            .iter()
            .find(|known| known.id == entry.id && known.version == entry.version)
        {
            entry.checksums = known.checksums.clone();
        }
    }
}

pub(super) fn replace_version_list(
    versions: &mut Vec<VersionInfo>,
    next: Vec<VersionInfo>,
) -> bool {
    let changed = versions.len() != next.len()
        || versions.iter().zip(&next).any(|(current, updated)| {
            current.id != updated.id
                || current.version != updated.version
                || current.selected != updated.selected
                || current.display_name != updated.display_name
                || current.eol != updated.eol
                || current.lts != updated.lts
        });

    if changed {
        let mut next = next;
        keep_known_checksums(versions, &mut next);
        *versions = next;
    }

    changed
}

pub(super) fn selected_version(mut version: VersionInfo, selected: bool) -> VersionInfo {
    version.selected = selected;
    version
}

pub(super) fn version_major_minor(version: &str) -> String {
    let mut parts = version.split('.');
    match (parts.next(), parts.next()) {
        (Some(major), Some(minor)) => format!("{major}.{minor}"),
        _ => version.to_string(),
    }
}

pub(super) fn caddy_version(version: &str) -> VersionInfo {
    VersionInfo {
        id: format!("caddy-{}", version_major_minor(version)),
        version: version.to_string(),
        selected: true,
        display_name: format!("Caddy {version}"),
        eol: false,
        lts: true,
        checksums: Checksums::default(),
        urls: Urls {
            windows_x64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_windows_amd64.zip")),
            windows_arm64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_windows_arm64.zip")),
            linux_x64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_linux_amd64.tar.gz")),
            linux_arm64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_linux_arm64.tar.gz")),
            macos_x64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_mac_amd64.tar.gz")),
            macos_arm64: Some(format!("https://github.com/caddyserver/caddy/releases/download/v{version}/caddy_{version}_mac_arm64.tar.gz")),
        },
    }
}

pub(super) fn mysql_version(version: &str) -> VersionInfo {
    let family = version_major_minor(version);
    VersionInfo {
        id: format!("mysql-{family}"),
        version: version.to_string(),
        selected: true,
        display_name: format!("MySQL {version}"),
        eol: false,
        lts: false,
        checksums: Checksums::default(),
        urls: Urls {
            windows_x64: Some(format!("https://cdn.mysql.com/Downloads/MySQL-{family}/mysql-{version}-winx64.zip")),
            windows_arm64: None,
            linux_x64: Some(format!("https://cdn.mysql.com/Downloads/MySQL-{family}/mysql-{version}-linux-glibc2.28-x86_64.tar.xz")),
            linux_arm64: Some(format!("https://cdn.mysql.com/Downloads/MySQL-{family}/mysql-{version}-linux-glibc2.28-aarch64.tar.xz")),
            macos_x64: Some(format!("https://cdn.mysql.com/Downloads/MySQL-{family}/mysql-{version}-macos15-x86_64.tar.gz")),
            macos_arm64: Some(format!("https://cdn.mysql.com/Downloads/MySQL-{family}/mysql-{version}-macos15-arm64.tar.gz")),
        },
    }
}

/// cdn.mysql.com keeps only the current patch release under `/Downloads/` and
/// moves superseded builds to `/archives/mysql-<family>/`.
pub fn mysql_archive_url(url: &str) -> Option<String> {
    let (family, file) = url
        .strip_prefix("https://cdn.mysql.com/Downloads/MySQL-")?
        .split_once('/')?;
    Some(format!(
        "https://cdn.mysql.com/archives/mysql-{family}/{file}"
    ))
}

pub(super) fn postgresql_version(version: &str) -> VersionInfo {
    let major = version.split('.').next().unwrap_or(version);
    VersionInfo {
        id: format!("postgresql-{major}"),
        version: version.to_string(),
        selected: true,
        display_name: format!("PostgreSQL {version}"),
        eol: false,
        lts: false,
        checksums: Checksums::default(),
        urls: Urls {
            windows_x64: Some(format!("https://get.enterprisedb.com/postgresql/postgresql-{version}-1-windows-x64-binaries.zip")),
            windows_arm64: None,
            linux_x64: Some(format!("https://downloads.percona.com/downloads/postgresql-distribution-{major}/{version}/binary/tarball/percona-postgresql-{version}-ssl3-linux-x86_64.tar.gz")),
            linux_arm64: Some(format!("https://downloads.percona.com/downloads/postgresql-distribution-{major}/{version}/binary/tarball/percona-postgresql-{version}-ssl3-linux-aarch64.tar.gz")),
            macos_x64: Some(format!("https://get.enterprisedb.com/postgresql/postgresql-{version}-1-osx-binaries.zip")),
            macos_arm64: Some(format!("https://get.enterprisedb.com/postgresql/postgresql-{version}-1-osx-binaries.zip")),
        },
    }
}

pub(super) fn phpmyadmin_version(version: &str) -> VersionInfoSingleUrl {
    VersionInfoSingleUrl {
        id: format!("phpmyadmin-{}", version_major_minor(version)),
        version: version.to_string(),
        selected: true,
        display_name: format!("phpMyAdmin {version}"),
        eol: false,
        lts: false,
        checksum: None,
        url: format!("https://files.phpmyadmin.net/phpMyAdmin/{version}/phpMyAdmin-{version}-all-languages.zip"),
    }
}

/// Text right before the version number in phpMyAdmin's download links.
pub(super) const PHPMYADMIN_VERSION_MARKER: &str = "files.phpmyadmin.net/phpMyAdmin/";

/// Text right before the version number on adminer.org's version page.
pub(super) const ADMINER_VERSION_MARKER: &str = "#download\" target=\"_blank\">";

/// Keep exactly one release of a web database manager (`adminer` or
/// `phpmyadmin`): it only ever offers the latest download, so older entries
/// (and ones an earlier bad refresh wrote) point at dead URLs.
pub(super) fn replace_database_tool(
    versions: &mut Vec<VersionInfoSingleUrl>,
    family: &str,
    next: VersionInfoSingleUrl,
) -> bool {
    let unchanged = {
        let mut current = versions.iter().filter(|v| v.id.starts_with(family));
        matches!(
            (current.next(), current.next()),
            (Some(only), None) if only.id == next.id && only.version == next.version && only.url == next.url
        )
    };
    if unchanged {
        return false;
    }

    let was_selected = versions
        .iter()
        .any(|version| version.id.starts_with(family) && version.selected);
    // An unchanged release keeps its pinned checksum; see `keep_known_checksums`.
    let known_checksum = versions
        .iter()
        .find(|version| version.id == next.id && version.version == next.version)
        .and_then(|version| version.checksum.clone());
    versions.retain(|version| !version.id.starts_with(family));
    versions.push(VersionInfoSingleUrl {
        selected: was_selected,
        checksum: next.checksum.or(known_checksum),
        ..next
    });
    true
}

pub(super) fn adminer_version(version: &str) -> VersionInfoSingleUrl {
    VersionInfoSingleUrl {
        id: format!("adminer-{}", version_major_minor(version)),
        version: version.to_string(),
        selected: false,
        display_name: format!("Adminer {version}"),
        eol: false,
        lts: false,
        checksum: None,
        url: format!("https://www.adminer.org/static/download/{version}/adminer-{version}.php"),
    }
}
