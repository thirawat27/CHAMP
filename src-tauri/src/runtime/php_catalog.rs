//! Latest PHP releases from the official machine-readable feeds.
//!
//! - `php.net/releases/states.php`: which branches are still supported.
//! - `windows.php.net/.../releases.json`: the exact Windows zip name (including
//!   rebuild suffixes such as `8.5.12-1`) and its SHA-256 for each branch.
//! - The static-php-cli bulk listing: the newest Linux/macOS FPM build that
//!   actually exists, which can trail php.net by a few patch releases.
//!
//! Using the real file names avoids guessing URLs, which broke whenever a
//! Windows build was republished or a static build was not published yet.

use crate::runtime::packages::{Checksums, Urls, VersionInfo};
use serde::Deserialize;
use serde_json::Value;

const STATES_URL: &str = "https://www.php.net/releases/states.php";
const WINDOWS_RELEASES_URL: &str = "https://windows.php.net/downloads/releases/releases.json";
const WINDOWS_RELEASES_BASE: &str = "https://windows.php.net/downloads/releases/";
const STATIC_PHP_LIST_URL: &str = "https://dl.static-php.dev/static-php-cli/bulk/?format=json";
const STATIC_PHP_BASE: &str = "https://dl.static-php.dev/static-php-cli/bulk/";

/// (static-php os, static-php arch, catalog platform key)
const STATIC_TARGETS: [(&str, &str, &str); 4] = [
    ("linux", "x86_64", "linuxX64"),
    ("linux", "aarch64", "linuxArm64"),
    ("macos", "x86_64", "macOSX64"),
    ("macos", "aarch64", "macOSArm64"),
];

#[derive(Debug, Deserialize)]
struct StaticListingEntry {
    name: String,
}

/// Numeric version segments, ignoring a rebuild suffix ("8.5.12-1" -> [8, 5, 12]).
pub fn version_key(version: &str) -> Vec<u32> {
    version
        .split('-')
        .next()
        .unwrap_or(version)
        .split('.')
        .map(|segment| segment.parse().unwrap_or(0))
        .collect()
}

/// True when `candidate` is a newer release than `current`.
pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    version_key(candidate) > version_key(current)
}

fn base_version(version: &str) -> String {
    version.split('-').next().unwrap_or(version).to_string()
}

/// Branches ("8.5", "8.4", ...) that php.net still supports, newest first.
fn supported_branches(states: &Value) -> Vec<String> {
    let mut branches: Vec<String> = states
        .as_object()
        .into_iter()
        .flat_map(|majors| majors.values())
        .filter_map(Value::as_object)
        .flat_map(|branches| branches.iter())
        .filter(|(_, info)| matches!(info["state"].as_str(), Some("stable" | "security")))
        .map(|(branch, _)| branch.clone())
        .collect();
    branches.sort_by_key(|branch| std::cmp::Reverse(version_key(branch)));
    branches
}

/// (version, zip file name, sha256) of the non-thread-safe x64 Windows build.
fn windows_build(windows: &Value, branch: &str) -> Option<(String, String, Option<String>)> {
    let release = windows.get(branch)?;
    let version = release["version"].as_str()?.to_string();
    let (_, build) = release.as_object()?.iter().find(|(key, value)| {
        key.starts_with("nts-") && key.ends_with("-x64") && value["zip"]["path"].is_string()
    })?;
    Some((
        version,
        build["zip"]["path"].as_str()?.to_string(),
        build["zip"]["sha256"].as_str().map(str::to_lowercase),
    ))
}

/// Newest static-php FPM build for `branch` on an os/arch, as (version, file name).
fn static_build(files: &[String], branch: &str, os: &str, arch: &str) -> Option<(String, String)> {
    let prefix = format!("php-{branch}.");
    let suffix = format!("-fpm-{os}-{arch}.tar.gz");
    files
        .iter()
        .filter_map(|name| {
            let patch = name.strip_prefix(&prefix)?.strip_suffix(&suffix)?;
            patch
                .chars()
                .all(|ch| ch.is_ascii_digit())
                .then(|| (format!("{branch}.{patch}"), name.clone()))
        })
        .max_by_key(|(version, _)| version_key(version))
}

/// Build one catalog entry per supported branch, newest first. The entry's
/// `version` is the build available for `platform_key`, and the newest branch
/// that has a build for this platform is marked as the default (`selected`).
pub fn build_php_versions(
    states: &Value,
    windows: Option<&Value>,
    static_files: &[String],
    platform_key: &str,
) -> Vec<VersionInfo> {
    let mut versions: Vec<VersionInfo> = supported_branches(states)
        .into_iter()
        .filter_map(|branch| {
            let mut urls = Urls {
                windows_x64: None,
                windows_arm64: None,
                linux_x64: None,
                linux_arm64: None,
                macos_x64: None,
                macos_arm64: None,
            };
            let mut checksums = Checksums::default();
            let mut platform_versions: Vec<(&str, String)> = Vec::new();

            if let Some((version, file, sha256)) =
                windows.and_then(|feed| windows_build(feed, &branch))
            {
                urls.windows_x64 = Some(format!("{WINDOWS_RELEASES_BASE}{file}"));
                checksums.windows_x64 = sha256;
                platform_versions.push(("windowsX64", base_version(&version)));
            }
            for (os, arch, key) in STATIC_TARGETS {
                if let Some((version, file)) = static_build(static_files, &branch, os, arch) {
                    let url = Some(format!("{STATIC_PHP_BASE}{file}"));
                    match key {
                        "linuxX64" => urls.linux_x64 = url,
                        "linuxArm64" => urls.linux_arm64 = url,
                        "macOSX64" => urls.macos_x64 = url,
                        _ => urls.macos_arm64 = url,
                    }
                    platform_versions.push((key, version));
                }
            }

            let version = platform_versions
                .iter()
                .find(|(key, _)| *key == platform_key)
                .or_else(|| platform_versions.first())
                .map(|(_, version)| version.clone())?;

            Some(VersionInfo {
                id: format!("php-{branch}"),
                version,
                selected: false,
                display_name: format!("PHP {branch}"),
                eol: false,
                lts: false,
                checksums,
                urls,
            })
        })
        .collect();

    let has_platform_build = |info: &VersionInfo| match platform_key {
        "windowsX64" => info.urls.windows_x64.is_some(),
        "linuxX64" => info.urls.linux_x64.is_some(),
        "linuxArm64" => info.urls.linux_arm64.is_some(),
        "macOSX64" => info.urls.macos_x64.is_some(),
        "macOSArm64" => info.urls.macos_arm64.is_some(),
        _ => false,
    };
    if let Some(default) = versions.iter_mut().find(|info| has_platform_build(info)) {
        default.selected = true;
    }
    versions
}

/// Merge the latest supported branches into the catalog. Branches php.net no
/// longer supports stay available for legacy projects but are marked EOL.
/// Returns whether anything changed.
pub fn merge_php_versions(existing: &mut Vec<VersionInfo>, latest: Vec<VersionInfo>) -> bool {
    if latest.is_empty() {
        return false;
    }
    let before = serde_json::to_value(&*existing).ok();
    let latest_ids: Vec<String> = latest.iter().map(|info| info.id.clone()).collect();

    existing.retain(|info| !latest_ids.contains(&info.id));
    for info in existing.iter_mut() {
        info.selected = false;
        info.eol = true;
    }
    existing.extend(latest);
    existing.sort_by_key(|info| std::cmp::Reverse(version_key(info.id.trim_start_matches("php-"))));

    serde_json::to_value(&*existing).ok() != before
}

async fn get_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> Option<T> {
    client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<T>()
        .await
        .ok()
}

/// Fetch the latest supported PHP branches; `None` when php.net is unreachable.
pub async fn fetch_latest_php(client: &reqwest::Client) -> Option<Vec<VersionInfo>> {
    let (states, windows, listing) = tokio::join!(
        get_json::<Value>(client, STATES_URL),
        get_json::<Value>(client, WINDOWS_RELEASES_URL),
        get_json::<Vec<StaticListingEntry>>(client, STATIC_PHP_LIST_URL),
    );
    let static_files: Vec<String> = listing
        .unwrap_or_default()
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    let platform = crate::runtime::downloader::Platform::current().url_key();
    let versions = build_php_versions(&states?, windows.as_ref(), &static_files, &platform);
    (!versions.is_empty()).then_some(versions)
}

/// windows.php.net moves superseded builds to `/archives/`.
pub fn windows_archive_url(url: &str) -> Option<String> {
    url.strip_prefix(WINDOWS_RELEASES_BASE)
        .filter(|file| !file.starts_with("archives/"))
        .map(|file| format!("{WINDOWS_RELEASES_BASE}archives/{file}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn states() -> Value {
        json!({
            "8": {
                "8.5": { "state": "stable" },
                "8.4": { "state": "stable" },
                "8.2": { "state": "security" },
                "8.1": { "state": "eol" }
            },
            "7": { "7.4": { "state": "eol" } }
        })
    }

    fn windows() -> Value {
        json!({
            "8.5": {
                "version": "8.5.12-1",
                "ts-vs17-x64": { "zip": { "path": "php-8.5.12-1-Win32-vs17-x64.zip" } },
                "nts-vs17-x64": { "zip": { "path": "php-8.5.12-1-nts-Win32-vs17-x64.zip", "sha256": "ABC" } }
            },
            "8.4": {
                "version": "8.4.27",
                "nts-vs17-x64": { "zip": { "path": "php-8.4.27-nts-Win32-vs17-x64.zip", "sha256": "def" } }
            }
        })
    }

    fn static_files() -> Vec<String> {
        [
            "php-8.5.8-fpm-linux-x86_64.tar.gz",
            "php-8.5.12-cli-linux-x86_64.tar.gz",
            "php-8.5.10-fpm-linux-x86_64.tar.gz",
            "php-8.5.9-fpm-linux-x86_64.tar.gz",
            "php-8.2.29-fpm-linux-x86_64.tar.gz",
        ]
        .iter()
        .map(|name| name.to_string())
        .collect()
    }

    /// Live check against the real feeds: `cargo test php_catalog -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_feeds_produce_a_latest_php() {
        let client = reqwest::Client::builder()
            .user_agent("CHAMP test")
            .build()
            .unwrap();
        let versions = fetch_latest_php(&client).await.expect("feeds reachable");
        for info in &versions {
            println!(
                "{} {} selected={} win={:?} sha={:?} linux={:?}",
                info.id,
                info.version,
                info.selected,
                info.urls.windows_x64,
                info.checksums.windows_x64,
                info.urls.linux_x64
            );
        }
        assert!(versions.iter().any(|info| info.selected));
    }

    #[test]
    fn version_keys_compare_numerically() {
        assert!(is_newer_version("8.5.12", "8.5.9"));
        assert!(is_newer_version("8.5.12-1", "8.5.11"));
        assert!(!is_newer_version("8.5.12-1", "8.5.12"));
        assert!(is_newer_version("8.10.0", "8.9.9"));
    }

    #[test]
    fn windows_entries_use_real_file_names_and_checksums() {
        let versions =
            build_php_versions(&states(), Some(&windows()), &static_files(), "windowsX64");
        let ids: Vec<&str> = versions.iter().map(|info| info.id.as_str()).collect();
        assert_eq!(ids, ["php-8.5", "php-8.4", "php-8.2"]);

        let latest = &versions[0];
        assert!(latest.selected);
        assert_eq!(latest.version, "8.5.12");
        assert_eq!(
            latest.urls.windows_x64.as_deref(),
            Some("https://windows.php.net/downloads/releases/php-8.5.12-1-nts-Win32-vs17-x64.zip")
        );
        assert_eq!(latest.checksums.windows_x64.as_deref(), Some("abc"));
        assert!(versions[1..].iter().all(|info| !info.selected));
    }

    #[test]
    fn linux_uses_newest_static_build_that_exists() {
        let versions = build_php_versions(&states(), Some(&windows()), &static_files(), "linuxX64");
        let latest = &versions[0];
        assert_eq!(latest.version, "8.5.10");
        assert_eq!(
            latest.urls.linux_x64.as_deref(),
            Some(
                "https://dl.static-php.dev/static-php-cli/bulk/php-8.5.10-fpm-linux-x86_64.tar.gz"
            )
        );
        assert!(latest.selected);
    }

    #[test]
    fn default_skips_branches_without_a_build_for_this_platform() {
        let files = vec!["php-8.2.29-fpm-macos-aarch64.tar.gz".to_string()];
        let versions = build_php_versions(&states(), Some(&windows()), &files, "macOSArm64");
        let selected: Vec<&str> = versions
            .iter()
            .filter(|info| info.selected)
            .map(|info| info.id.as_str())
            .collect();
        assert_eq!(selected, ["php-8.2"]);
    }

    #[test]
    fn merge_marks_unsupported_branches_eol_and_sorts() {
        let latest = build_php_versions(&states(), Some(&windows()), &static_files(), "windowsX64");
        let mut existing = build_php_versions(
            &json!({ "8": { "8.1": { "state": "stable" }, "8.4": { "state": "stable" } } }),
            Some(&json!({
                "8.1": { "version": "8.1.30", "nts-vs16-x64": { "zip": { "path": "php-8.1.30-nts-Win32-vs16-x64.zip" } } },
                "8.4": { "version": "8.4.20", "nts-vs17-x64": { "zip": { "path": "php-8.4.20-nts-Win32-vs17-x64.zip" } } }
            })),
            &[],
            "windowsX64",
        );

        assert!(merge_php_versions(&mut existing, latest));
        let summary: Vec<(&str, &str, bool, bool)> = existing
            .iter()
            .map(|info| {
                (
                    info.id.as_str(),
                    info.version.as_str(),
                    info.selected,
                    info.eol,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("php-8.5", "8.5.12", true, false),
                ("php-8.4", "8.4.27", false, false),
                ("php-8.2", "8.2.29", false, false),
                ("php-8.1", "8.1.30", false, true),
            ]
        );
    }

    #[test]
    fn archive_url_is_derived_once() {
        assert_eq!(
            windows_archive_url("https://windows.php.net/downloads/releases/php-8.4.1-nts-Win32-vs17-x64.zip")
                .as_deref(),
            Some("https://windows.php.net/downloads/releases/archives/php-8.4.1-nts-Win32-vs17-x64.zip")
        );
        assert!(
            windows_archive_url("https://windows.php.net/downloads/releases/archives/x.zip")
                .is_none()
        );
        assert!(windows_archive_url("https://example.com/php.zip").is_none());
    }
}
