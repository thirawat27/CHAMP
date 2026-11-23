//! Landing page served at `/` while the projects folder has no site of its own.
//!
//! Directory browsing stays off (S-03), so without this page an empty projects
//! folder answers the very first request with a bare "404 Not Found" that
//! explains nothing. The page is regenerated on every Caddy start, so it always
//! shows the current folder, the installed database tool, and the sites found.

use crate::runtime::locator::RuntimePaths;
use std::fs;
use std::path::{Path, PathBuf};

const INDEX_FILES: [&str; 3] = ["index.html", "index.htm", "index.php"];

const WELCOME_CSS: &str = r#"
:root {
  --bg: #eef1f5; --card: #ffffff; --sunken: #e7ebf1; --border: #dce2ea;
  --text: #141b26; --muted: #5e6b7d; --primary: #9a7536; --ok: #17895a;
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #0e131a; --card: #141a23; --sunken: #0b1016; --border: #243041;
    --text: #e7ecf2; --muted: #8d9aad; --primary: #d2b07a; --ok: #5bd39b;
    color-scheme: dark;
  }
}
* { box-sizing: border-box; }
body {
  margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 28px 20px;
  background: var(--bg); color: var(--text); line-height: 1.6;
  font-family: "IBM Plex Sans Thai", "Noto Sans Thai", system-ui, -apple-system, "Segoe UI", sans-serif;
  -webkit-font-smoothing: antialiased;
}
main { width: min(660px, 100%); display: grid; gap: 16px; }
h1 { margin: 0; font-size: 26px; letter-spacing: 0.04em; }
h2 { margin: 0 0 10px; font-size: 13px; font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); }
p { margin: 0; }
.head { display: flex; align-items: center; gap: 12px; }
.led { width: 10px; height: 10px; border-radius: 50%; background: var(--ok); box-shadow: 0 0 0 4px color-mix(in srgb, var(--ok) 20%, transparent); }
.sub { color: var(--muted); font-size: 14px; }
.card { padding: 18px 20px; border: 1px solid var(--border); border-radius: 14px; background: var(--card); }
.sites { display: grid; gap: 8px; }
.sites a {
  display: flex; align-items: center; justify-content: space-between; gap: 12px;
  padding: 11px 14px; border: 1px solid var(--border); border-radius: 10px;
  background: var(--sunken); color: var(--text); text-decoration: none; font-weight: 600;
}
.sites a:hover { border-color: var(--primary); }
.sites a span { color: var(--muted); font-weight: 400; font-size: 13px; }
code, .path {
  font-family: ui-monospace, "Cascadia Code", Consolas, monospace; font-size: 13px;
  word-break: break-all;
}
.path { display: block; margin-top: 10px; padding: 11px 13px; border-radius: 9px; background: var(--sunken); }
.hint { margin-top: 10px; color: var(--muted); font-size: 13.5px; }
.links { display: flex; flex-wrap: wrap; gap: 8px; }
.links a {
  padding: 9px 14px; border: 1px solid var(--border); border-radius: 9px;
  background: var(--card); color: var(--text); text-decoration: none; font-size: 13.5px; font-weight: 600;
}
.links a:hover { border-color: var(--primary); color: var(--primary); }
.foot { color: var(--muted); font-size: 12.5px; text-align: center; }
.th { display: none; }
html[lang="th"] .th { display: revert; }
html[lang="th"] .en { display: none; }
"#;

// Thai is shown only when the browser asks for it; English is the default so
// the page still reads correctly with scripting disabled.
const WELCOME_JS: &str = r#"
if (navigator.language && navigator.language.toLowerCase().startsWith("th")) {
  document.documentElement.lang = "th";
}
"#;

/// True when the folder holds a file Caddy would serve as its index.
pub(super) fn has_index_file(dir: &Path) -> bool {
    INDEX_FILES.iter().any(|name| dir.join(name).is_file())
}

fn escape_html(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&#39;".to_string(),
            _ => ch.to_string(),
        })
        .collect()
}

/// Percent-encode one URL path segment (folder names may contain spaces or Thai).
fn encode_path_segment(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{:02X}", byte),
        })
        .collect()
}

/// True when the folder serves a page at its root or, like Laravel, from `public/`.
fn serves_a_page(dir: &Path) -> bool {
    has_index_file(dir) || has_index_file(&dir.join(crate::projects::PUBLIC_DIR))
}

/// Project folders that already serve a page, sorted case-insensitively.
pub(super) fn list_sites(projects_dir: &Path) -> Vec<String> {
    let mut sites: Vec<String> = match fs::read_dir(projects_dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|entry| entry.path().is_dir() && serves_a_page(&entry.path()))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| !name.starts_with('.'))
            .collect(),
        Err(_) => Vec::new(),
    };
    sites.sort_by_key(|name| name.to_lowercase());
    sites
}

/// Where a listed site opens: its own `<folder>.localhost` origin when the
/// name allows one, else the path under `localhost` (inside `public/` for a
/// folder that only has an index there).
fn site_link(projects_dir: &Path, name: &str, web_port: u16, domains: bool) -> (String, String) {
    if let Some(url) = crate::projects::enabled_domain_url(name, web_port, domains) {
        let shown = url
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();
        return (url, shown);
    }
    let public = if has_index_file(&projects_dir.join(name)) {
        String::new()
    } else {
        format!("{}/", crate::projects::PUBLIC_DIR)
    };
    let href = format!("/{}/{}", encode_path_segment(name), public);
    (href, format!("/{}/{}", name, public))
}

fn render(
    projects_dir: &Path,
    sites: &[String],
    database_tool_id: &str,
    web_port: u16,
    domains: bool,
) -> String {
    let (tool_name, tool_path) = if crate::runtime::packages::is_adminer_id(database_tool_id) {
        ("Adminer", "/adminer/")
    } else {
        ("phpMyAdmin", "/phpmyadmin/")
    };

    let body = if sites.is_empty() {
        format!(
            r#"<section class="card">
      <h2>Projects</h2>
      <p class="en">This folder has no website yet. Create a folder in it with an <code>index.html</code> or <code>index.php</code> file, then open <code>localhost/&lt;folder&gt;/</code>.</p>
      <p class="th">โฟลเดอร์นี้ยังไม่มีเว็บไซต์ สร้างโฟลเดอร์ใหม่ข้างในแล้วใส่ไฟล์ <code>index.html</code> หรือ <code>index.php</code> จากนั้นเปิด <code>localhost/&lt;ชื่อโฟลเดอร์&gt;/</code></p>
      <code class="path">{path}</code>
      <p class="hint en">A file named <code>index.html</code> placed directly in this folder is served here instead of this page.</p>
      <p class="hint th">ถ้าวางไฟล์ <code>index.html</code> ไว้ในโฟลเดอร์นี้โดยตรง หน้านี้จะถูกแทนที่ด้วยไฟล์นั้น</p>
    </section>"#,
            path = escape_html(&projects_dir.to_string_lossy())
        )
    } else {
        let links: String = sites
            .iter()
            .map(|name| {
                let (href, shown) = site_link(projects_dir, name, web_port, domains);
                format!(
                    "\n        <a href=\"{href}\">{label}<span>{shown}</span></a>",
                    href = escape_html(&href),
                    label = escape_html(name),
                    shown = escape_html(&shown)
                )
            })
            .collect();
        format!(
            r#"<section class="card">
      <h2>Projects</h2>
      <div class="sites">{links}
      </div>
      <code class="path">{path}</code>
      {hint}
    </section>"#,
            links = links,
            hint = list_hint(web_port, domains),
            path = escape_html(&projects_dir.to_string_lossy())
        )
    };

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>CHAMP</title>
<style>{css}</style>
</head>
<body>
  <main>
    <header>
      <div class="head"><span class="led"></span><h1>CHAMP</h1></div>
      <p class="sub en">Your local web server is running.</p>
      <p class="sub th">เว็บเซิร์ฟเวอร์ในเครื่องทำงานอยู่</p>
    </header>
    {body}
    <section class="card">
      <h2>Tools</h2>
      <div class="links">
        <a href="{tool_path}">{tool_name}</a>
      </div>
      <p class="hint en">Connect from PHP with host <code>127.0.0.1</code>, not <code>localhost</code>.</p>
      <p class="hint th">เชื่อมต่อฐานข้อมูลจาก PHP ด้วย host <code>127.0.0.1</code> ไม่ใช่ <code>localhost</code></p>
    </section>
    <p class="foot en">Served by CHAMP. Press F1 in the app for the full guide.</p>
    <p class="foot th">หน้านี้มาจาก CHAMP กด F1 ในแอปเพื่อเปิดคู่มือฉบับเต็ม</p>
  </main>
<script>{js}</script>
</body>
</html>
"#,
        css = WELCOME_CSS,
        js = WELCOME_JS,
        body = body,
        tool_path = tool_path,
        tool_name = tool_name,
    )
}

/// The note under the project list.
fn list_hint(web_port: u16, domains: bool) -> String {
    if domains {
        format!(
            r#"<p class="hint en">Every folder also opens at <code>http://&lt;folder&gt;.localhost:{port}</code>, served from <code>public/</code> when it has one. New folders appear here within a few seconds while CHAMP is open.</p>
      <p class="hint th">ทุกโฟลเดอร์เปิดได้ที่ <code>http://&lt;ชื่อโฟลเดอร์&gt;.localhost:{port}</code> ด้วย (ใช้ <code>public/</code> เป็นรากเว็บถ้ามี) โฟลเดอร์ใหม่จะแสดงในรายการนี้ภายในไม่กี่วินาทีขณะเปิด CHAMP</p>"#,
            port = web_port
        )
    } else {
        r#"<p class="hint en">New folders appear here within a few seconds while CHAMP is open. Turn on Project domains in CHAMP Settings → General to also open each folder at its own <code>&lt;folder&gt;.localhost</code> address.</p>
      <p class="hint th">โฟลเดอร์ใหม่จะแสดงในรายการนี้ภายในไม่กี่วินาทีขณะเปิด CHAMP เปิด โดเมนโปรเจกต์ ใน Settings → ทั่วไป ถ้าต้องการเปิดแต่ละโฟลเดอร์ที่ <code>&lt;ชื่อโฟลเดอร์&gt;.localhost</code></p>"#
            .to_string()
    }
}

/// Write the landing page when its content changed, and return the folder
/// Caddy serves it from.
///
/// It is a snapshot of the projects folder, so it is written when Caddy
/// starts and again by `ProcessManager::refresh_welcome_page` on every status
/// poll; before 2.1.1 only the start wrote it, and a folder added or made
/// reachable later (after an update, a moved project, a link to another
/// drive) stayed missing until the next restart.
pub(super) fn write_welcome_page(
    paths: &RuntimePaths,
    database_tool_id: &str,
    web_port: u16,
    domains: bool,
) -> Result<PathBuf, String> {
    let dir = paths.config_dir.join("welcome");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create welcome directory: {}", e))?;

    let sites = list_sites(&paths.projects_dir);
    let html = render(
        &paths.projects_dir,
        &sites,
        database_tool_id,
        web_port,
        domains,
    );
    let file = dir.join("index.html");
    if fs::read_to_string(&file).ok().as_deref() != Some(html.as_str()) {
        crate::fs_util::atomic_write(&file, html)
            .map_err(|e| format!("Failed to write welcome page: {}", e))?;
    }

    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_paths(temp: &Path) -> RuntimePaths {
        RuntimePaths {
            caddy: temp.join("caddy"),
            php_cgi: temp.join("php-cgi"),
            php_ini: temp.join("php.ini"),
            mysql: temp.join("mysqld"),
            postgresql: temp.join("postgres"),
            adminer: temp.join("adminer"),
            mysql_data_dir: temp.join("mysql"),
            postgresql_data_dir: temp.join("postgresql"),
            logs_dir: temp.join("logs"),
            config_dir: temp.join("config"),
            projects_dir: temp.join("projects"),
        }
    }

    /// The 2.1.0 bug: the page was written once at Caddy start, so a folder
    /// added (or reachable) later never appeared until a restart.
    #[test]
    fn a_rewrite_picks_up_new_folders_and_skips_unchanged_pages() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = runtime_paths(temp.path());
        fs::create_dir_all(&paths.projects_dir).unwrap();
        let page = |paths: &RuntimePaths| {
            let dir = write_welcome_page(paths, "phpmyadmin-5.2", 8080, true).unwrap();
            fs::read_to_string(dir.join("index.html")).unwrap()
        };
        assert!(!page(&paths).contains("E-Commerce"));

        let shop = paths.projects_dir.join("E-Commerce");
        fs::create_dir_all(&shop).unwrap();
        fs::write(shop.join("index.php"), "<?php").unwrap();
        let html = page(&paths);
        assert!(html.contains("e-commerce.localhost:8080"), "{html}");

        // Unchanged content is not rewritten (it is checked on every poll).
        let file = paths.config_dir.join("welcome").join("index.html");
        let before = fs::metadata(&file).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        page(&paths);
        assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn with_domains_off_sites_link_to_their_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let projects = temp.path();
        fs::create_dir_all(projects.join("shop")).unwrap();
        fs::write(projects.join("shop").join("index.php"), "x").unwrap();
        let html = render(
            projects,
            &["shop".to_string()],
            "phpmyadmin-5.2",
            8080,
            false,
        );
        assert!(html.contains("href=\"/shop/\""), "{html}");
        assert!(!html.contains("shop.localhost"));
        assert!(html.contains("Project domains"));
    }

    #[test]
    fn lists_only_folders_that_serve_a_page() {
        let temp = tempfile::tempdir().expect("temp dir");
        let projects = temp.path();
        for (name, index) in [
            ("shop", Some("index.php")),
            ("blog", Some("index.html")),
            ("notes", None),
            (".hidden", Some("index.html")),
        ] {
            let dir = projects.join(name);
            fs::create_dir_all(&dir).unwrap();
            if let Some(index) = index {
                fs::write(dir.join(index), "x").unwrap();
            }
        }
        fs::write(projects.join("readme.txt"), "x").unwrap();

        assert_eq!(list_sites(projects), vec!["blog", "shop"]);

        // A Laravel-style folder serves its page from public/.
        let api = projects.join("api").join("public");
        fs::create_dir_all(&api).unwrap();
        fs::write(api.join("index.php"), "x").unwrap();
        assert_eq!(list_sites(projects), vec!["api", "blog", "shop"]);
    }

    #[test]
    fn sites_link_to_their_own_domain() {
        let temp = tempfile::tempdir().expect("temp dir");
        let projects = temp.path();
        fs::create_dir_all(projects.join("my_api").join("public")).unwrap();
        let sites = vec!["shop".to_string(), "my_api".to_string()];
        let html = render(projects, &sites, "adminer-5.4", 8080, true);

        assert!(html.contains("href=\"http://shop.localhost:8080/\""));
        assert!(html.contains("<span>shop.localhost:8080</span>"));
        // No domain for an underscore name; its page is inside public/.
        assert!(html.contains("href=\"/my_api/public/\""));
        assert!(html.contains(".localhost:8080</code>"));
    }

    #[test]
    fn empty_folder_explains_what_to_do() {
        let temp = tempfile::tempdir().expect("temp dir");
        let html = render(temp.path(), &[], "phpmyadmin-5.2", 8080, true);

        assert!(html.contains("index.php"));
        assert!(html.contains("/phpmyadmin/"));
        assert!(html.contains("โฟลเดอร์นี้ยังไม่มีเว็บไซต์"));
        assert!(!html.contains("class=\"sites\""));
    }

    #[test]
    fn sites_are_listed_with_escaped_and_encoded_names() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sites = vec!["my shop".to_string(), "a<b".to_string()];
        for name in &sites {
            fs::create_dir_all(temp.path().join(name)).unwrap_or(()); // "a<b" is invalid on Windows
        }
        let html = render(temp.path(), &sites, "adminer-5.4", 8080, true);

        // Names that are not DNS labels keep the path link.
        assert!(html.contains("href=\"/my%20shop/"));
        assert!(html.contains("href=\"/a%3Cb/"));
        assert!(html.contains("a&lt;b"));
        assert!(!html.contains("a<b<"));
        assert!(html.contains("/adminer/"));
        assert!(html.contains("Adminer"));
    }

    #[test]
    fn writes_the_page_next_to_the_other_generated_config() {
        let temp = tempfile::tempdir().expect("temp dir");
        let base = temp.path();
        let paths = RuntimePaths {
            caddy: base.join("caddy"),
            php_cgi: base.join("php-cgi"),
            php_ini: base.join("php.ini"),
            mysql: base.join("mysqld"),
            postgresql: base.join("postgres"),
            adminer: base.join("adminer"),
            mysql_data_dir: base.join("mysql"),
            postgresql_data_dir: base.join("postgresql"),
            logs_dir: base.join("logs"),
            config_dir: base.join("config"),
            projects_dir: base.join("projects"),
        };
        fs::create_dir_all(&paths.projects_dir).unwrap();

        let dir = write_welcome_page(&paths, "phpmyadmin-5.2", 8080, true).expect("welcome page");

        assert_eq!(dir, paths.config_dir.join("welcome"));
        let html = fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("CHAMP"));
    }

    /// Writes the full generated Caddyfile and landing page for a manual
    /// `caddy validate` / `caddy run` check:
    /// `CHAMP_CADDY_CHECK_DIR=<dir> cargo test caddy_check -- --ignored`
    #[test]
    #[ignore]
    fn caddy_check_writes_full_config() {
        let Some(base) = std::env::var_os("CHAMP_CADDY_CHECK_DIR").map(PathBuf::from) else {
            return;
        };
        let paths = RuntimePaths {
            caddy: base.join("caddy"),
            php_cgi: base.join("php-cgi"),
            php_ini: base.join("php.ini"),
            mysql: base.join("mysqld"),
            postgresql: base.join("postgres"),
            adminer: base.join("adminer"),
            mysql_data_dir: base.join("mysql"),
            postgresql_data_dir: base.join("postgresql"),
            logs_dir: base.join("logs"),
            config_dir: base.join("config"),
            projects_dir: base.join("projects"),
        };
        for dir in [
            &paths.projects_dir,
            &paths.logs_dir,
            &paths.config_dir,
            &paths.adminer,
        ] {
            fs::create_dir_all(dir).unwrap();
        }
        let shop = paths.projects_dir.join("shop");
        fs::create_dir_all(&shop).unwrap();
        fs::write(shop.join("index.html"), "<h1>SHOP SITE</h1>").unwrap();

        let welcome = write_welcome_page(&paths, "phpmyadmin-5.2", 18092, true).unwrap();
        super::super::caddyfile::generate_caddyfile(
            &paths.config_dir.join("Caddyfile"),
            &paths,
            18092,
            19092,
            &welcome,
            true,
        )
        .unwrap();
    }

    #[test]
    fn index_detection_only_accepts_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        assert!(!has_index_file(temp.path()));

        fs::create_dir(temp.path().join("index.html")).unwrap();
        assert!(!has_index_file(temp.path()), "a directory is not an index");

        fs::write(temp.path().join("index.php"), "x").unwrap();
        assert!(has_index_file(temp.path()));
    }
}
