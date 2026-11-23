//! Project folders and their `<folder>.localhost` domains.
//!
//! Browsers resolve every `*.localhost` name to the loopback address on their
//! own (RFC 6761), so a project gets its own origin at
//! `http://<folder>.localhost:<web port>/` without editing the hosts file and
//! without administrator rights. Caddy maps the name back to the folder at
//! request time, so a new folder works without restarting the web server.
//!
//! `inspect` reads a folder's composer.json and marker files to name its
//! framework, web root, and PHP requirement, so env profiles and agents can
//! give framework-specific advice for any PHP project, not only Laravel.

use std::fs;
use std::path::{Path, PathBuf};

/// Folders whose own `public/index.php` or `public/index.html` exists are
/// served from `public/` (Laravel, Symfony, Slim, ...).
pub const PUBLIC_DIR: &str = "public";

/// The domain label for a project folder, when the folder name is a plain DNS
/// label (letters, digits, and inner hyphens, at most 63 characters).
///
/// Linux file systems are case-sensitive and browsers lowercase the host, so
/// there a folder with capital letters gets no domain.
pub fn domain_label(folder: &str) -> Option<String> {
    let bytes = folder.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= 63
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
        && bytes[0] != b'-'
        && bytes[bytes.len() - 1] != b'-';
    if !valid || (cfg!(target_os = "linux") && folder.bytes().any(|b| b.is_ascii_uppercase())) {
        return None;
    }
    Some(folder.to_ascii_lowercase())
}

/// `http://<folder>.localhost:<port>/`, when the folder can have a domain.
pub fn domain_url(folder: &str, web_port: u16) -> Option<String> {
    domain_label(folder).map(|label| format!("http://{label}.localhost:{web_port}/"))
}

/// composer.json files larger than this are not parsed.
const MAX_COMPOSER_JSON: u64 = 1024 * 1024;

/// What CHAMP can tell about a project folder without running anything.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: PathBuf,
    /// `laravel`, `symfony`, `wordpress`, `codeigniter`, `cakephp`, `slim`,
    /// `yii`, `laminas`, `drupal`; `None` for plain PHP or unknown.
    pub framework: Option<&'static str>,
    pub framework_name: Option<&'static str>,
    /// `public` when the site is served from `public/`, otherwise empty.
    pub web_root: &'static str,
    /// composer.json `require.php`, for example `^8.2`.
    pub php_requirement: Option<String>,
    pub has_composer: bool,
    /// Composer dependencies are installed (`vendor/autoload.php`).
    pub has_vendor: bool,
    pub has_env: bool,
    pub has_env_example: bool,
}

/// Composer packages that identify a framework, checked in order.
const COMPOSER_FRAMEWORKS: [(&str, &str, &str); 12] = [
    ("laravel/framework", "laravel", "Laravel"),
    ("symfony/framework-bundle", "symfony", "Symfony"),
    ("codeigniter4/framework", "codeigniter", "CodeIgniter 4"),
    ("cakephp/cakephp", "cakephp", "CakePHP"),
    ("slim/slim", "slim", "Slim"),
    ("yiisoft/yii2", "yii", "Yii"),
    ("yiisoft/yii-http", "yii", "Yii"),
    ("laminas/laminas-mvc", "laminas", "Laminas"),
    ("drupal/core", "drupal", "Drupal"),
    ("drupal/core-recommended", "drupal", "Drupal"),
    ("roots/wordpress", "wordpress", "WordPress"),
    ("johnpbloch/wordpress", "wordpress", "WordPress"),
];

fn framework_from_composer(composer: &serde_json::Value) -> Option<(&'static str, &'static str)> {
    let has = |package: &str| {
        ["require", "require-dev"]
            .iter()
            .any(|section| composer[section].get(package).is_some())
    };
    COMPOSER_FRAMEWORKS
        .iter()
        .find(|(package, _, _)| has(package))
        .map(|(_, id, name)| (*id, *name))
}

/// The project folder `name` inside `projects_dir`, refusing anything that is
/// not a plain, visible, direct child (no separators, no `..`).
pub fn project_dir(projects_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let plain = !name.is_empty()
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && name.len() <= 255;
    let path = projects_dir.join(name);
    if !plain || !path.is_dir() {
        return Err(format!(
            "No project folder named `{name}`. Use list_projects for the names."
        ));
    }
    Ok(path)
}

/// Inspect one project folder.
pub fn inspect(path: &Path) -> ProjectInfo {
    let composer = fs::metadata(path.join("composer.json"))
        .ok()
        .filter(|meta| meta.len() <= MAX_COMPOSER_JSON)
        .and_then(|_| fs::read_to_string(path.join("composer.json")).ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let wordpress_files = path.join("wp-config.php").is_file()
        || path.join("wp-config-sample.php").is_file()
        || path.join("wp-load.php").is_file();
    let framework = composer
        .as_ref()
        .and_then(framework_from_composer)
        .or_else(|| wordpress_files.then_some(("wordpress", "WordPress")));
    let public = path.join(PUBLIC_DIR);
    let serves_public = public.join("index.php").is_file() || public.join("index.html").is_file();
    ProjectInfo {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        path: path.to_path_buf(),
        framework: framework.map(|(id, _)| id),
        framework_name: framework.map(|(_, name)| name),
        web_root: if serves_public { PUBLIC_DIR } else { "" },
        php_requirement: composer
            .as_ref()
            .and_then(|composer| composer["require"]["php"].as_str())
            .map(str::to_string),
        has_composer: composer.is_some(),
        has_vendor: path.join("vendor").join("autoload.php").is_file(),
        has_env: path.join(".env").is_file(),
        has_env_example: path.join(".env.example").is_file(),
    }
}

/// Inspect the project folder `name` in CHAMP's projects folder.
pub fn detect(name: &str) -> Result<ProjectInfo, String> {
    let paths = crate::runtime::locator::get_app_data_paths()?;
    Ok(inspect(&project_dir(&paths.projects_dir, name)?))
}

/// Whether PHP `version` (`8.3.14`) meets a Composer constraint (`^8.2`,
/// `>=8.1 <8.4`, `~8.3.0`, `8.2.*`, `^8.1 || ^8.2`). `None` when the
/// constraint uses syntax this check does not understand.
pub fn php_satisfies(constraint: &str, version: &str) -> Option<bool> {
    let version = parse_version(version)?;
    let mut any = false;
    for group in constraint.split("||").flat_map(|part| part.split('|')) {
        let mut all = true;
        let terms: Vec<&str> = group
            .split([' ', ','])
            .map(str::trim)
            .filter(|term| !term.is_empty())
            .collect();
        if terms.is_empty() {
            return None;
        }
        for term in terms {
            all &= term_matches(term, version)?;
        }
        any |= all;
    }
    Some(any)
}

type Version = (u64, u64, u64);

fn parse_version(text: &str) -> Option<Version> {
    // Windows builds carry rebuild suffixes such as `8.5.12-1`.
    let core = text
        .trim()
        .trim_start_matches(['v', 'V'])
        .split(['-', '+'])
        .next()?;
    let mut parts = core.split('.').map(|part| part.parse::<u64>());
    let major = parts.next()?.ok()?;
    let minor = parts.next().unwrap_or(Ok(0)).ok()?;
    let patch = parts.next().unwrap_or(Ok(0)).ok()?;
    Some((major, minor, patch))
}

fn term_matches(term: &str, version: Version) -> Option<bool> {
    let count = |text: &str| text.split('.').count();
    if let Some(rest) = term.strip_prefix('^') {
        let low = parse_version(rest)?;
        let high = if low.0 > 0 {
            (low.0 + 1, 0, 0)
        } else {
            (0, low.1 + 1, 0)
        };
        return Some(version >= low && version < high);
    }
    if let Some(rest) = term.strip_prefix('~') {
        let low = parse_version(rest)?;
        let high = if count(rest) >= 3 {
            (low.0, low.1 + 1, 0)
        } else {
            (low.0 + 1, 0, 0)
        };
        return Some(version >= low && version < high);
    }
    for (op, test) in [
        (">=", (|v, c| v >= c) as fn(Version, Version) -> bool),
        ("<=", |v, c| v <= c),
        ("!=", |v, c| v != c),
        (">", |v, c| v > c),
        ("<", |v, c| v < c),
        ("==", |v, c| v == c),
        ("=", |v, c| v == c),
    ] {
        if let Some(rest) = term.strip_prefix(op) {
            return Some(test(version, parse_version(rest)?));
        }
    }
    if term == "*" {
        return Some(true);
    }
    if let Some(prefix) = term.strip_suffix(".*") {
        let low = parse_version(prefix)?;
        return Some(match count(prefix) {
            1 => version.0 == low.0,
            _ => version.0 == low.0 && version.1 == low.1,
        });
    }
    // A bare version: exact on the parts it names.
    let wanted = parse_version(term)?;
    Some(match count(term) {
        1 => version.0 == wanted.0,
        2 => (version.0, version.1) == (wanted.0, wanted.1),
        _ => version == wanted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frameworks_are_detected_from_composer_and_wordpress_files() {
        let dir = tempfile::tempdir().unwrap();
        let laravel = dir.path().join("shop");
        fs::create_dir_all(laravel.join("public")).unwrap();
        fs::write(laravel.join("public/index.php"), "<?php").unwrap();
        fs::write(
            laravel.join("composer.json"),
            r#"{"require":{"php":"^8.2","laravel/framework":"^12.0"}}"#,
        )
        .unwrap();
        fs::write(laravel.join(".env.example"), "").unwrap();
        let info = inspect(&laravel);
        assert_eq!(info.framework, Some("laravel"));
        assert_eq!(info.web_root, "public");
        assert_eq!(info.php_requirement.as_deref(), Some("^8.2"));
        assert!(info.has_composer && info.has_env_example && !info.has_env && !info.has_vendor);

        let wp = dir.path().join("blog");
        fs::create_dir_all(&wp).unwrap();
        fs::write(wp.join("wp-load.php"), "<?php").unwrap();
        assert_eq!(inspect(&wp).framework, Some("wordpress"));
        assert_eq!(inspect(&wp).web_root, "");

        let plain = dir.path().join("plain");
        fs::create_dir_all(&plain).unwrap();
        fs::write(plain.join("composer.json"), "not json").unwrap();
        let info = inspect(&plain);
        assert_eq!(info.framework, None);
        assert!(!info.has_composer);
    }

    #[test]
    fn project_dir_refuses_paths_and_hidden_folders() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("shop")).unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        assert!(project_dir(dir.path(), "shop").is_ok());
        for bad in ["", ".git", "..", "../x", "a/b", "a\\b", "C:", "missing"] {
            assert!(project_dir(dir.path(), bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn php_constraints_follow_composer() {
        for (constraint, version, expected) in [
            ("^8.2", "8.3.14", Some(true)),
            ("^8.2", "8.1.30", Some(false)),
            ("^8.2", "9.0.0", Some(false)),
            (">=8.1 <8.4", "8.3.0", Some(true)),
            (">=8.1,<8.4", "8.4.1", Some(false)),
            ("~8.3.0", "8.3.9", Some(true)),
            ("~8.3.0", "8.4.0", Some(false)),
            ("~8.1", "8.4.0", Some(true)),
            ("8.2.*", "8.2.5", Some(true)),
            ("8.2.*", "8.3.0", Some(false)),
            ("^7.4 || ^8.0", "8.5.12-1", Some(true)),
            ("^7.4|^8.0", "7.3.0", Some(false)),
            ("*", "8.0.0", Some(true)),
            ("dev-main", "8.3.0", None),
        ] {
            assert_eq!(
                php_satisfies(constraint, version),
                expected,
                "{constraint} vs {version}"
            );
        }
    }

    #[test]
    fn plain_names_get_a_domain() {
        assert_eq!(
            domain_url("shop", 8080).as_deref(),
            Some("http://shop.localhost:8080/")
        );
        assert_eq!(domain_label("my-app2").as_deref(), Some("my-app2"));
        assert_eq!(
            domain_label(&"a".repeat(63)).as_deref(),
            Some(&*"a".repeat(63))
        );
    }

    #[test]
    fn names_that_are_not_dns_labels_get_none() {
        for name in [
            "",
            "my shop",
            "my_shop",
            "-shop",
            "shop-",
            "a.b",
            "ร้านค้า",
            "../etc",
            &"a".repeat(64),
        ] {
            assert_eq!(domain_label(name), None, "{name:?}");
        }
    }

    #[test]
    fn capital_letters_follow_the_file_system() {
        let label = domain_label("MyShop");
        if cfg!(target_os = "linux") {
            assert_eq!(label, None);
        } else {
            assert_eq!(label.as_deref(), Some("myshop"));
        }
    }
}
